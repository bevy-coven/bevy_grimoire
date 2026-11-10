//! **The window fits the monitor it opens on, and goes back where it was only if that place is still on a
//! monitor** (#292). The decision is pure (`window_frame::choose`); this drives the systems that gather the
//! monitors and apply it, with a monitor and a window spawned by hand, because a test has no display.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, PrimaryWindow, WindowPosition};
use bevy_cauldron::recent::{Recent, RecentProjects, WindowFrame};
use bevy_cauldron::window_frame::{LastFrame, WindowFramePlugin};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_window_frame_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// An app with one 1280x720 monitor and a primary window, whose recent list holds `saved`.
fn app_with(tag: &str, saved: Option<WindowFrame>) -> (App, PathBuf) {
    let file = scratch(tag).join("recent.yaml");
    Recent { window: saved, ..Recent::default() }.save(&file).unwrap_or_else(|e| panic!("{e}"));
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).insert_resource(RecentProjects(file.clone())).add_plugins(WindowFramePlugin);
    app.world_mut().spawn((
        Monitor {
            name: None,
            physical_height: 720,
            physical_width: 1280,
            physical_position: IVec2::ZERO,
            refresh_rate_millihertz: None,
            scale_factor: 1.0,
            video_modes: Vec::new(),
        },
        PrimaryMonitor,
    ));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    (app, file)
}

fn window_frame(app: &mut App) -> (Vec2, WindowPosition) {
    let mut q = app.world_mut().query_filtered::<&Window, With<PrimaryWindow>>();
    let w = q.single(app.world()).unwrap_or_else(|e| panic!("{e}"));
    (Vec2::new(w.width(), w.height()), w.position)
}

#[test]
fn with_nothing_saved_the_window_is_fitted_and_centred_on_the_monitor() {
    let (mut app, _) = app_with("fit", None);
    app.update();
    // 85% of 1280x720 is 1088x612, and the height is floored at 640.
    assert_eq!(window_frame(&mut app), (Vec2::new(1088.0, 640.0), WindowPosition::At(IVec2::new(96, 40))));
}

#[test]
fn a_saved_frame_that_is_on_the_monitor_is_used() {
    let saved = WindowFrame { x: 50, y: 30, w: 1100, h: 650 };
    let (mut app, _) = app_with("kept", Some(saved));
    app.update();
    assert_eq!(window_frame(&mut app), (Vec2::new(1100.0, 650.0), WindowPosition::At(IVec2::new(50, 30))));
}

#[test]
fn a_saved_frame_that_is_outside_every_monitor_is_ignored() {
    let saved = WindowFrame { x: 5000, y: 30, w: 1100, h: 650 };
    let (mut app, _) = app_with("gone", Some(saved));
    app.update();
    assert_eq!(window_frame(&mut app), (Vec2::new(1088.0, 640.0), WindowPosition::At(IVec2::new(96, 40))), "a monitor that was unplugged is not somewhere to put a window");
}

#[test]
fn the_frame_the_window_had_is_written_when_the_application_exits() {
    let (mut app, file) = app_with("exit", None);
    app.update();
    let dragged = WindowFrame { x: 200, y: 100, w: 1100, h: 660 };
    {
        let mut q = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>();
        let mut window = q.single_mut(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
        window.resolution.set(1100.0, 660.0);
        window.position = WindowPosition::At(IVec2::new(200, 100));
    }
    app.update();
    assert_eq!(app.world().resource::<LastFrame>().0, Some(dragged));
    app.world_mut().write_message(AppExit::Success);
    app.update();
    assert_eq!(Recent::load(&file).unwrap_or_else(|e| panic!("{e}")).window, Some(dragged));
}
