//! **Does the editor survive a resize of its surface?**
//!
//! The crash the ui_audit records is a *race*, and a race that only shows on one machine's window
//! is a race nobody can test. So this probe forces the race: it resizes the surface the way
//! `fit_surface_to_window` does when the window's physical size differs from the surface's, and
//! then reports what the camera believes and what the image is.
//!
//! The two numbers are the whole crash:
//!
//! * the image's size — the colour attachment the pass will be given;
//! * the camera's `target_info.physical_size` — the size the **depth** attachment is built from.
//!
//! When they differ, wgpu refuses the pass and the default error handler exits the application.

use bevy::prelude::{PluginGroup, default};

/// The surface image's current physical size.
fn image_size(app: &mut bevy::prelude::App) -> Option<(u32, u32)> {
    let surface = app
        .world()
        .resource::<bevy_cauldron::surface::Surface>()
        .image
        .clone();
    let images = app
        .world()
        .resource::<bevy::asset::Assets<bevy::image::Image>>();
    images.get(&surface).map(|i| {
        (
            i.texture_descriptor.size.width,
            i.texture_descriptor.size.height,
        )
    })
}

/// Every camera's idea of its target's size.
fn camera_sizes(app: &mut bevy::prelude::App) -> Vec<(isize, bool, (u32, u32))> {
    let mut q = app
        .world_mut()
        .query::<(&bevy::camera::Camera, Option<&bevy::camera::Camera3d>)>();
    q.iter(app.world())
        .map(|(cam, cam3d)| {
            (
                cam.order,
                cam3d.is_some(),
                cam.computed
                    .target_info
                    .as_ref()
                    .map(|t| (t.physical_size.x, t.physical_size.y))
                    .unwrap_or((0, 0)),
            )
        })
        .collect()
}

fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run -p bevy_cauldron --example probe_surface_resize -- <project-root>");
        eprintln!();
        eprintln!("  <project-root>  a directory containing `assets/`, which the probe loads");
        std::process::exit(2);
    };
    let root = std::fs::canonicalize(&root).unwrap_or_else(|_| std::path::PathBuf::from(&root));

    // **The startup path, with a window** — the one the ui_audit says crashes. The window is a
    // *component* on an entity, not a real one: `winit` refuses a second `EventLoop` in a
    // process, and the size is the only thing the surface reads.
    let mut app = bevy::prelude::App::new();
    app.add_plugins(
        bevy::prelude::DefaultPlugins
            .set(bevy::asset::AssetPlugin {
                file_path: root.to_string_lossy().into_owned() + "/assets",
                ..default()
            })
            .set(bevy::window::WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            .set(bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                    bevy::render::settings::WgpuSettings {
                        backends: None,
                        ..default()
                    },
                )),
                ..default()
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::audio::AudioPlugin>(),
    );
    app.world_mut().spawn((
        bevy::window::PrimaryWindow,
        bevy::window::Window {
            resolution: bevy::window::WindowResolution::new(3396, 1356),
            ..default()
        },
    ));
    #[cfg(feature = "debugger")]
    app.init_resource::<bevy_debugger_bevy::DebugCursor>();
    bevy_cauldron::harness::add_editor_plugins(&mut app);

    let report = |app: &mut bevy::prelude::App, label: &str| {
        let image = image_size(app).expect("a surface image");
        let cameras = camera_sizes(app);
        // **Only the cameras that share the surface image matter.** The window camera (order 0)
        // renders to the *window*, not the image, so its target size is the window's — a different
        // question, and not the crash.
        let shared: Vec<_> = cameras.iter().filter(|c| c.0 != 0).collect();
        let mismatched = shared.iter().any(|c| c.2 != image);
        println!(
            "{label}: image {image:?}, the cameras that share it {shared:?} -> {}",
            if mismatched { "MISMATCH" } else { "agree" }
        );
    };

    // **The startup frames.** A windowed app resizes its surface on its first frame that has a
    // window — and that resize is the one the camera learns about a frame late.
    report(&mut app, "at the first frame");
    app.update();
    report(&mut app, "after one frame");
    app.update();
    report(&mut app, "after two frames");

    // **And the resize the crash actually needs: the window changing size after the cameras
    // exist.** The audit's two numbers — 3396 and 3440 — are the window's physical size before
    // and after the compositor finishes sizing it, and that is the whole story: the editor's
    // first frame has a window that is nearly the right size, and a frame later it is *exactly*
    // the right size. The second resize is the one that meets a camera that already exists.
    {
        let mut sizes = Vec::new();
        {
            let mut windows = app.world_mut().query::<&mut bevy::window::Window>();
            for mut w in windows.iter_mut(app.world_mut()) {
                w.resolution = bevy::window::WindowResolution::new(3440, 1440);
                sizes.push((
                    w.resolution.physical_width(),
                    w.resolution.physical_height(),
                ));
            }
        }
        println!("the window is now {sizes:?}");
        report(
            &mut app,
            "immediately after the window changes size, before any frame",
        );
        app.update();
        report(&mut app, "after one frame");
        for _ in 0..3 {
            app.update();
        }
        report(&mut app, "after four frames");
    }
}
