//! **The window fits the screen it opens on, and goes back where it was** (#292).
//!
//! The window was a fixed 1280x720 wherever it opened, so on a 1024x640 screen it ran off the edge. Now:
//!
//! * the window cannot be made smaller than [`MIN_SIZE`] (`main.rs` sets the resize constraint);
//! * on the first frame it has a monitor to measure, it takes [`FIT_FRACTION`] of the primary monitor's logical
//!   size, capped at [`MAX_FIT`], floored at [`MIN_SIZE`], and is centred ([`fit`]);
//! * the frame it had when a project last opened, or the editor last closed, is kept in the recent-projects file
//!   ([`crate::recent::Recent::window`]) and used instead of the fit, **when it still lies within a monitor**
//!   ([`choose`]): a monitor that was unplugged leaves a saved frame nobody can see, and that is ignored;
//! * the window then **takes the size the system gave it** ([`take_the_given_size`]), which is not always the one
//!   asked for: macOS will not open a window taller than the room between the menu bar and the Dock.
//!
//! The decision is two pure functions over [`MonitorArea`]s; the systems only gather monitors and apply the answer.
//! A window in fullscreen (`GRIMOIRE_FULLSCREEN`, for automated checks) is left alone.

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, PrimaryWindow, WindowEvent, WindowMode, WindowPosition, WindowResized};

use crate::recent::{self, RecentProjects, WindowFrame};
use crate::screen::Screen;

/// The smallest window the editor's layout is built for, in logical pixels.
pub const MIN_SIZE: Vec2 = Vec2::new(1024.0, 640.0);

/// The largest a window gets from the monitor fit, in logical pixels: a larger monitor has room to spare, and a
/// person who wants more can drag it.
pub const MAX_FIT: Vec2 = Vec2::new(1600.0, 1000.0);

/// The share of the monitor's logical size the fitted window takes.
pub const FIT_FRACTION: f32 = 0.85;

/// A monitor as the decision sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonitorArea {
    /// Top-left corner on the desktop, in physical pixels.
    pub origin: IVec2,
    /// Size in physical pixels.
    pub size: UVec2,
    /// Physical pixels per logical pixel.
    pub scale: f32,
}

impl MonitorArea {
    fn of(monitor: &Monitor) -> MonitorArea {
        MonitorArea { origin: monitor.physical_position, size: monitor.physical_size(), scale: monitor.scale_factor as f32 }
    }

    /// The monitor's size in logical pixels.
    fn logical(&self) -> Vec2 {
        self.size.as_vec2() / self.scale
    }

    fn holds(&self, point: IVec2) -> bool {
        let end = self.origin + self.size.as_ivec2();
        (self.origin.x..end.x).contains(&point.x) && (self.origin.y..end.y).contains(&point.y)
    }

    /// Whether `frame`, whose size is logical, lies wholly on this monitor.
    fn contains(&self, frame: WindowFrame) -> bool {
        let end = self.origin + self.size.as_ivec2();
        let far = IVec2::new(frame.x, frame.y) + (UVec2::new(frame.w, frame.h).as_vec2() * self.scale).round().as_ivec2();
        self.holds(IVec2::new(frame.x, frame.y)) && far.x <= end.x && far.y <= end.y
    }
}

/// The window a monitor gets when nothing was saved: [`FIT_FRACTION`] of it, capped, floored, centred.
pub fn fit(monitor: &MonitorArea) -> WindowFrame {
    let size = (monitor.logical() * FIT_FRACTION).min(MAX_FIT).max(MIN_SIZE).round();
    let physical = (size * monitor.scale).round().as_ivec2();
    let corner = monitor.origin + (monitor.size.as_ivec2() - physical).max(IVec2::ZERO) / 2;
    WindowFrame { x: corner.x, y: corner.y, w: size.x as u32, h: size.y as u32 }
}

/// The frame the window opens in: the one `saved` when it lies within a monitor and is no smaller than
/// [`MIN_SIZE`], else the fit to `primary`.
pub fn choose(saved: Option<WindowFrame>, monitors: &[MonitorArea], primary: &MonitorArea) -> WindowFrame {
    saved
        .filter(|f| f.w as f32 >= MIN_SIZE.x && f.h as f32 >= MIN_SIZE.y)
        .filter(|f| monitors.iter().any(|m| m.contains(*f)))
        .unwrap_or_else(|| fit(primary))
}

/// Where the window is now, when the system has told it (a window that was never placed has no position).
fn frame_of(window: &Window) -> Option<WindowFrame> {
    let WindowPosition::At(at) = window.position else { return None };
    Some(WindowFrame { x: at.x, y: at.y, w: window.width().round() as u32, h: window.height().round() as u32 })
}

/// The window as it was last seen, kept up to date while it exists: when a window is closed it is already gone by
/// the time the application exits.
#[derive(Resource, Default, Debug)]
pub struct LastFrame(pub Option<WindowFrame>);

pub struct WindowFramePlugin;

impl Plugin for WindowFramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastFrame>()
            .init_resource::<FrameAsked>()
            // Written by `take_the_given_size`. `WindowPlugin` registers both; registering is idempotent, and an app
            // without it (a test's `MinimalPlugins`) still runs the plugin.
            .add_message::<WindowResized>()
            .add_message::<WindowEvent>()
            .add_systems(First, take_the_given_size)
            .add_systems(Update, (fit_the_window, watch_the_window).chain())
            .add_systems(OnEnter(Screen::Editor), save_the_window)
            .add_systems(Last, save_the_window.run_if(on_message::<AppExit>));
    }
}

/// Set when [`fit_the_window`] has asked the system for a frame, until [`take_the_given_size`] has read what it got.
#[derive(Resource, Default, Debug)]
struct FrameAsked(bool);

/// Places the window once, on the first frame a monitor is known: the saved frame if it is still on screen, else the fit.
fn fit_the_window(
    monitors: Query<(&Monitor, Has<PrimaryMonitor>)>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    recents: Option<Res<RecentProjects>>,
    mut asked: ResMut<FrameAsked>,
    mut placed: Local<bool>,
) {
    if *placed {
        return;
    }
    let Ok(mut window) = windows.single_mut() else { return };
    let areas: Vec<(MonitorArea, bool)> = monitors.iter().map(|(m, primary)| (MonitorArea::of(m), primary)).collect();
    let Some((primary, _)) = areas.iter().find(|(_, primary)| *primary).or(areas.first()) else { return };
    *placed = true;
    if window.mode != WindowMode::Windowed {
        return;
    }
    let saved = recents.and_then(|r| match recent::Recent::load(&r.0) {
        Ok(list) => list.window,
        Err(e) => {
            warn!("window position not restored: {e}");
            None
        }
    });
    let all: Vec<MonitorArea> = areas.iter().map(|(m, _)| *m).collect();
    let frame = choose(saved, &all, primary);
    window.resolution.set(frame.w as f32, frame.h as f32);
    window.position = WindowPosition::At(IVec2::new(frame.x, frame.y));
    asked.0 = true;
}

/// **The window is the size the system gave it, not the size it was asked to be.**
///
/// [`fit_the_window`] asks for a frame by writing `Window::resolution`. `bevy_winit` passes the request on in `Last`
/// and waits for the system's `Resized` to say what it got ("In `None` case, the request will be handled by
/// winit::event::WindowEvent::Resized", `bevy_winit-0.20.0/src/system.rs:405`). A system that refuses the request
/// *without changing the window* sends no `Resized`, because nothing was resized, so `Window::resolution` keeps the
/// refused size for the rest of the run.
///
/// macOS does exactly that with a window taller than the room between the menu bar and the Dock, and a frame saved
/// in full screen is one. Measured on mac_air on 2026-11-06, a 1280x828 screen with 735 points free: the saved
/// frame asked for 1280x802, the window had already been held to 1280x703 when it opened, so asking for more
/// changed nothing, and `Window::resolution` said 802 for the whole session. The surface and the pointer are both
/// sized from that number (`crate::surface`, `crate::view::sense_pointer`), so the picture was squeezed into 703
/// points while the cursor was read against 802: every hover and click landed below the button it was aimed at,
/// further below the lower the button.
///
/// So in the frame after the request was applied, the window's own inner size is read from winit and, when it
/// differs, becomes `Window::resolution`, announced with the two messages `bevy_winit` writes for a `Resized`
/// (`react_to_resize`), so the cameras refit as they would for any resize. The request is answered by then
/// wherever a refusal is silent: macOS applies `setContentSize` before returning, and the instrumented launch read
/// the held size in the very next frame. A run with no winit window (headless) has nothing to read; a window winit
/// has not created yet reports its real size through `Resized` when it is.
fn take_the_given_size(
    mut asked: ResMut<FrameAsked>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut resized: MessageWriter<WindowResized>,
    mut window_events: MessageWriter<WindowEvent>,
    _winit_is_on_the_main_thread: NonSendMarker,
) {
    if !std::mem::take(&mut asked.0) {
        return;
    }
    let Ok((entity, mut window)) = windows.single_mut() else { return };
    let Some(given) = bevy::winit::WINIT_WINDOWS.with_borrow(|w| w.get_window(entity).map(|w| w.inner_size())) else {
        return;
    };
    let asked_for = (window.resolution.physical_width(), window.resolution.physical_height());
    if (given.width, given.height) == asked_for {
        return;
    }
    info!(
        target: "cauldron::startup",
        "the window was given {}x{} physical, not the {}x{} asked for: the system held it to the room it has",
        given.width, given.height, asked_for.0, asked_for.1
    );
    window.resolution.set_physical_resolution(given.width, given.height);
    let event = WindowResized { window: entity, width: window.width(), height: window.height() };
    resized.write(event.clone());
    window_events.write(event.into());
}

fn watch_the_window(windows: Query<&Window, (With<PrimaryWindow>, Changed<Window>)>, mut last: ResMut<LastFrame>) {
    if let Ok(window) = windows.single()
        && let Some(frame) = frame_of(window)
        && last.0 != Some(frame)
    {
        last.0 = Some(frame);
    }
}

/// Writes the window's frame to the recent-projects file: when a project opens, and when the editor closes.
fn save_the_window(last: Res<LastFrame>, recents: Option<Res<RecentProjects>>) {
    // A file write of the exit: listed with its bound, so a quit stuck in it is named (`exit_watchdog.rs`, #427).
    let _wait = crate::exit_watchdog::waiting("saving the window's place", std::time::Duration::from_secs(1));
    if let (Some(frame), Some(recents)) = (last.0, recents)
        && let Err(e) = recent::remember_window(&recents.0, frame)
    {
        warn!("window position not remembered: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, w: u32, h: u32, scale: f32) -> MonitorArea {
        MonitorArea { origin: IVec2::new(x, y), size: UVec2::new(w, h), scale }
    }

    #[test]
    fn a_large_monitor_gets_the_capped_size_in_the_middle() {
        let m = monitor(0, 0, 3840, 2160, 1.0);
        assert_eq!(fit(&m), WindowFrame { x: 1120, y: 580, w: 1600, h: 1000 });
    }

    #[test]
    fn a_monitor_that_is_just_big_enough_gets_the_share_of_it() {
        let m = monitor(0, 0, 1920, 1080, 1.0);
        let f = fit(&m);
        assert_eq!((f.w, f.h), (1600, 918));
        assert_eq!((f.x, f.y), (160, 81));
    }

    #[test]
    fn a_small_monitor_is_floored_at_the_smallest_window() {
        let m = monitor(0, 0, 1024, 640, 1.0);
        assert_eq!(fit(&m), WindowFrame { x: 0, y: 0, w: 1024, h: 640 });
    }

    #[test]
    fn a_scaled_monitor_is_measured_in_logical_pixels() {
        // 2880x1800 physical at 2x is 1440x900 logical: 85% is 1224x765, centred in physical pixels.
        let m = monitor(0, 0, 2880, 1800, 2.0);
        assert_eq!(fit(&m), WindowFrame { x: 216, y: 135, w: 1224, h: 765 });
    }

    #[test]
    fn a_saved_frame_on_a_monitor_is_kept_and_one_off_every_monitor_is_not() {
        let main = monitor(0, 0, 1920, 1080, 1.0);
        let all = [main];
        let on = WindowFrame { x: 100, y: 80, w: 1200, h: 800 };
        assert_eq!(choose(Some(on), &all, &main), on);
        let off = WindowFrame { x: 4000, y: 80, w: 1200, h: 800 };
        assert_eq!(choose(Some(off), &all, &main), fit(&main), "a monitor that is gone leaves a frame nobody can see");
        let hanging = WindowFrame { x: 1000, y: 80, w: 1200, h: 800 };
        assert_eq!(choose(Some(hanging), &all, &main), fit(&main), "a frame running off the edge is not on the monitor");
        let tiny = WindowFrame { x: 0, y: 0, w: 300, h: 200 };
        assert_eq!(choose(Some(tiny), &all, &main), fit(&main), "a window smaller than the editor can lay out in is not restored");
        assert_eq!(choose(None, &all, &main), fit(&main));
    }

    #[test]
    fn a_frame_on_the_second_monitor_is_kept_while_it_is_there() {
        let (left, right) = (monitor(0, 0, 1920, 1080, 1.0), monitor(1920, 0, 2560, 1440, 1.0));
        let frame = WindowFrame { x: 2000, y: 100, w: 1500, h: 900 };
        assert_eq!(choose(Some(frame), &[left, right], &left), frame);
        assert_eq!(choose(Some(frame), &[left], &left), fit(&left), "unplugged");
    }
}
