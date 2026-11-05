//! **The startup says which part of drawing works, in the log** (#306, #307).
//!
//! Measured 2026-11-04 on `mac_air`: a notarized release DMG launched from Finder drew a black window while the
//! editor's offscreen surface held 73 sampled colours, and nothing anywhere said why. `log_file` captures whatever
//! `tracing` says; this plugin says the facts a black window is about, each when it becomes true, under the target
//! `cauldron::startup`:
//!
//! 1. **The window's surface** — its size in physical pixels and its scale, once it has one; a surface still 0x0
//!    after [`WINDOW_CHECK_FRAME`] frames is the commonest cause of a window that is present and black.
//! 2. **The cameras**, at frame [`CAMERAS_AT_FRAME`]: order, active, target and layers, one line each. The editor
//!    draws into an image and one window camera mirrors it (`surface.rs`), so a window camera missing, inactive or
//!    ordered wrong is visible here.
//! 3. **The window self-check** — a screenshot of the window's own swapchain once it has focus, sampled on a
//!    [`SAMPLE_GRID`] lattice. Fewer than [`MIN_WINDOW_COLOURS`] colours is a black window, said as an `error`.
//!    It waits for focus because macOS keeps an unfocused window's surface current only while on screen
//!    (`surface.rs`: "7,188 distinct colours focused, 1 behind something else").
//! 4. **The frames, every [`PULSE_SECONDS`]** — a count that does not rise is a window that never presented,
//!    which is a different fault from a frame that was drawn black.
//!
//! The adapter is not repeated here: `bevy_render` logs its `AdapterInfo` at `info` when the device is made, and
//! the file layer captures it. The plugin writes no file itself, so it is harmless in a test; `harness.rs` does not
//! add it (a headless editor has no window to check).

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, RenderTarget};
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::{PrimaryWindow, Window};

/// Seconds between frame-count lines: often enough to see a freeze, rare enough to read.
pub const PULSE_SECONDS: f32 = 10.0;

/// The frame the cameras are listed at: the editor's door has spawned its surface by then.
pub const CAMERAS_AT_FRAME: u32 = 30;

/// The earliest frame the window is screenshotted at, and the frame a 0x0 surface is reported by.
pub const WINDOW_CHECK_FRAME: u32 = 120;

/// The frame by which a window that never had focus is said to have had none.
pub const NEVER_FOCUSED_FRAME: u32 = 600;

/// The side of the lattice the self-check samples the window on.
pub const SAMPLE_GRID: u32 = 64;

/// A window with fewer distinct sampled colours than this shows nothing: the Projects home samples dozens.
pub const MIN_WINDOW_COLOURS: usize = 8;

/// The `tracing` target every line of this plugin carries, so a reader can grep the startup out of the log.
const TARGET: &str = "cauldron::startup";

/// The facts a black window is made of, each said once, and then a pulse.
pub struct StartupProbePlugin;

impl Plugin for StartupProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (report_surface, report_cameras, check_window, report_frames));
    }
}

/// **How big the window's surface is**, the first frame it has a size; a warning once if it has none by
/// [`WINDOW_CHECK_FRAME`].
fn report_surface(mut said: Local<bool>, frames: Res<FrameCount>, windows: Query<&Window, With<PrimaryWindow>>) {
    if *said {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let size = window.physical_size();
    if size.x == 0 || size.y == 0 {
        if frames.0 >= WINDOW_CHECK_FRAME {
            *said = true;
            warn!(target: TARGET, "window surface is 0x0 after {WINDOW_CHECK_FRAME} frames: nothing can be drawn");
        }
        return;
    }
    *said = true;
    info!(target: TARGET, "window surface {}x{} physical, scale {}, mode {:?}", size.x, size.y, window.scale_factor(), window.mode);
}

/// **Every camera at [`CAMERAS_AT_FRAME`]**: order, active, what it draws into and on which layers.
fn report_cameras(frames: Res<FrameCount>, cameras: Query<(Entity, &Camera, Option<&RenderTarget>, Option<&RenderLayers>)>) {
    if frames.0 != CAMERAS_AT_FRAME {
        return;
    }
    for (entity, camera, target, layers) in &cameras {
        let target = match target {
            Some(RenderTarget::Window(_)) => "Window",
            Some(RenderTarget::Image(_)) => "Image",
            _ => "other",
        };
        // The layer indices alone; a camera with no `RenderLayers` sees layer 0, Bevy's default.
        let layers: Vec<usize> = layers.map_or_else(|| vec![0], |l| l.iter().collect());
        info!(target: TARGET, "camera {entity}: order {}, active {}, target {target}, layers {layers:?}", camera.order, camera.is_active);
    }
}

/// **What the window itself shows**, once per run: a screenshot of its swapchain the first frame after
/// [`WINDOW_CHECK_FRAME`] that it has focus. A window that never takes focus is said to have had none, once,
/// and the check keeps waiting for it.
fn check_window(
    mut done: Local<bool>,
    mut warned: Local<bool>,
    frames: Res<FrameCount>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if *done || frames.0 < WINDOW_CHECK_FRAME {
        return;
    }
    let Ok(window) = windows.single() else { return };
    if !window.focused {
        if !*warned && frames.0 >= NEVER_FOCUSED_FRAME {
            *warned = true;
            warn!(target: TARGET, "window has not had focus: the self-check waits for it (an unfocused macOS window reads as one flat colour)");
        }
        return;
    }
    *done = true;
    commands.spawn(Screenshot::primary_window()).observe(|shot: On<ScreenshotCaptured>| {
        match sampled_colours(&shot.image, SAMPLE_GRID) {
            Some((colours, brightest)) => {
                info!(target: TARGET, "window self-check: {colours} colours, brightest channel {brightest}");
                if colours < MIN_WINDOW_COLOURS {
                    error!(target: TARGET, "the window is black: the editor draws into its surface but nothing reaches the window");
                }
            }
            None => warn!(target: TARGET, "window self-check: unreadable format {:?}", shot.image.texture_descriptor.format),
        }
    });
}

/// **Frames run in the last [`PULSE_SECONDS`]**, with the window's size and focus, so one line answers "is it
/// drawing", "at what size" and "can the check judge it".
fn report_frames(
    time: Res<Time>,
    mut since: Local<f32>,
    mut previous: Local<u32>,
    frames: Res<FrameCount>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    *since += time.delta_secs();
    if *since < PULSE_SECONDS {
        return;
    }
    *since = 0.0;
    let (size, focused) = match windows.single() {
        Ok(w) => {
            let s = w.physical_size();
            (format!("{}x{}", s.x, s.y), w.focused.to_string())
        }
        Err(_) => ("none".to_owned(), "n/a".to_owned()),
    };
    info!(target: TARGET, "{} frames in the last {PULSE_SECONDS} s, surface {size}, focused {focused}", frames.0.wrapping_sub(*previous));
    *previous = frames.0;
}

/// **How many distinct colours an image shows, and its brightest channel**, sampled on a `grid` x `grid` lattice.
/// `None` for a format that cannot be read as RGBA.
pub fn sampled_colours(image: &Image, grid: u32) -> Option<(usize, u8)> {
    let rgba = image.clone().try_into_dynamic().ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 || grid == 0 {
        return Some((0, 0));
    }
    let mut seen = std::collections::HashSet::new();
    let mut brightest = 0u8;
    for gy in 0..grid {
        for gx in 0..grid {
            // Cell centres, so a two-colour split down the middle is sampled on both sides.
            let x = ((u64::from(gx) * 2 + 1) * u64::from(w) / (u64::from(grid) * 2)) as u32;
            let y = ((u64::from(gy) * 2 + 1) * u64::from(h) / (u64::from(grid) * 2)) as u32;
            let p = rgba.get_pixel(x.min(w - 1), y.min(h - 1)).0;
            seen.insert((p[0], p[1], p[2]));
            brightest = brightest.max(p[0]).max(p[1]).max(p[2]);
        }
    }
    Some((seen.len(), brightest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    const SIDE: u32 = 64;

    fn extent() -> Extent3d {
        Extent3d { width: SIDE, height: SIDE, depth_or_array_layers: 1 }
    }

    #[test]
    fn a_black_image_is_one_colour_with_no_light() {
        let image = Image::new_fill(extent(), TextureDimension::D2, &[0, 0, 0, 255], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        assert_eq!(sampled_colours(&image, SAMPLE_GRID), Some((1, 0)));
    }

    #[test]
    fn a_red_and_blue_image_is_two_colours_at_full_brightness() {
        let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
        for _y in 0..SIDE {
            for x in 0..SIDE {
                data.extend_from_slice(if x < SIDE / 2 { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
            }
        }
        let image = Image::new(extent(), TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        assert_eq!(sampled_colours(&image, SAMPLE_GRID), Some((2, 255)));
    }
}
