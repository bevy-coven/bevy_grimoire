//! **The editor's surface and its camera must agree on the target's size.**
//!
//! # The crash this ratchets
//!
//! `bevy_cauldron . <map>` — the command that opens the editor straight onto a map — died
//! inside two seconds, reproduced 4/4, with:
//!
//! ```text
//! Attachments have differing sizes: the depth attachment's texture view has extent
//! (3396, 1356, 1) but is followed by the color attachment at index 0's texture view which has
//! (3440, 1440, 1)
//! Quitting the application due to Validation RenderError
//! ```
//!
//! The mechanism, read out of Bevy 0.20's source:
//!
//! * the world camera is **3-D**, and so is the only camera with a **depth attachment**;
//! * `prepare_core_3d_depth_textures` sizes that depth from
//!   `camera.computed.target_info.physical_size` — the image's size **as it was when the camera
//!   last refreshed**;
//! * a camera refreshes on the image's `AssetEvent::Modified`, and that event is flushed in
//!   `PostUpdate` **after** `camera_system` has already read the message stream — so a resize is
//!   always **one frame late** at the camera;
//! * the frame in between assembles the pass: a depth texture sized for the old extent and a
//!   colour attachment for the new one. wgpu refuses a pass whose attachments differ, and the
//!   default error handler treats that as fatal.
//!
//! **The invariant:** the image the camera renders into and the size the camera believes it is
//! are the same number. A frame in which they are not is the crash.
//!
//! # What is asserted
//!
//! 1. With a window, the surface is **created at the window's physical size** — so the startup
//!    resize never happens, and there is no event to be late.
//! 2. After a resize, the camera's `target_info` and the image's size **agree**.
//! 3. A headless run has no window and keeps its placeholder, and `resize_surface` still works.

use bevy::prelude::*;

use bevy_cauldron::harness;

use crate::fixtures;

/// The surface image's current physical size.
fn surface_size(app: &App) -> Option<(u32, u32)> {
    let surface = app
        .world()
        .resource::<bevy_cauldron::surface::Surface>();
    let images = app.world().resource::<Assets<Image>>();
    images.get(&surface.image).map(|i| {
        (
            i.texture_descriptor.size.width,
            i.texture_descriptor.size.height,
        )
    })
}

/// The cameras that **share the surface image**, and the size each believes it is.
///
/// The window camera is deliberately not in this list: it renders to the *window*, not the image,
/// so its target size is the window's — a different question, and not the crash.
fn shared_camera_sizes(app: &mut App) -> Vec<(isize, (u32, u32))> {
    let mut q = app.world_mut().query::<&Camera>();
    q.iter(app.world())
        .filter(|cam| cam.order != 0)
        .map(|cam| {
            (
                cam.order,
                cam.computed
                    .target_info
                    .as_ref()
                    .map(|t| (t.physical_size.x, t.physical_size.y))
                    .unwrap_or((0, 0)),
            )
        })
        .collect()
}

/// The window's physical size.
fn window_size(app: &mut App) -> Option<(u32, u32)> {
    let mut windows = app.world_mut().query::<&Window>();
    windows.iter(app.world()).next().map(|w| {
        (
            w.resolution.physical_width(),
            w.resolution.physical_height(),
        )
    })
}

/// **The surface is created at the window's size, and a resize never leaves the cameras behind.**
///
/// This is the crash fix, asserted rather than described.
///
/// **Two things, and both are needed:**
///
/// 1. **The surface is created at the window's size.** The window is spawned by
///    `WindowPlugin::build` — which `DefaultPlugins` runs first — so the size is known before the
///    image is made, and the image never has to be resized at startup.
/// 2. **A resize announces itself in the frame the camera reads it.** The world camera is 3-D, and
///    so is the only camera with a **depth attachment**, which `prepare_core_3d_depth_textures`
///    sizes from `camera.computed.target_info.physical_size` — the image's size *as it was when
///    the camera last refreshed*. A camera refreshes on the image's `AssetEvent::Modified`, and
///    that event is flushed in `PostUpdate` **after** `camera_system` has already read the message
///    stream — so a resize is one frame late at the camera, and that frame assembles the pass with
///    a depth texture for the old extent and a colour attachment for the new one. wgpu refuses a
///    pass whose attachments differ, and the default handler exits the application.
///
///    So the editor writes the event itself, in `PreUpdate` — before `camera_system` reads the
///    stream in `PostUpdate`.
///
/// **A window *entity*, not a real window.** `winit` refuses a second `EventLoop` in a process
/// ("`EventLoop` must be created on the main thread") and a test harness runs on a worker, so a
/// real window cannot be opened here — and it need not be: the surface reads a `Window`
/// *component*, and that is what this spawns. The sizes are the ones the crash named: 3396×1356
/// before the compositor finishes, 3440×1440 after.
#[test]
fn the_surface_is_created_at_the_windows_size() {
    let root = fixtures::Fixture::new("surface_window").build("town");
    let root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let mut app = App::new();
    // **The harness's plugin set, plus a window.** `WindowPlugin` with `primary_window: None` and
    // `WinitPlugin` disabled is how the headless runs avoid taking the thread; a `Window` *entity*
    // is spawned by hand instead, so the surface has a window to ask without a real one.
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: root.to_string_lossy().into_owned() + "/assets",
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            // Every render type, no device — the harness's own arrangement, and for the same
            // reason: the surface's *size* is a question about an `Image` asset, and asking it
            // needs no GPU.
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
    // **A window entity with a size**, spawned before the editor's plugins — which is the order
    // `main.rs` has: `DefaultPlugins` first, the editor's after.
    app.world_mut().spawn((
        bevy::window::PrimaryWindow,
        Window {
            // **The audit's first number** — the window's physical size before the compositor
            // finishes sizing it. The second number is below, and the change between them is
            // the crash.
            resolution: bevy::window::WindowResolution::new(3396, 1356),
            ..default()
        },
    ));
    // The debugger's own resource, which `view::sense_pointer` reads — the harness initialises it
    // for the same reason, and the reason it is there is the `debugger` feature being default-on.
    #[cfg(feature = "debugger")]
    app.init_resource::<bevy_debugger_bevy::DebugCursor>();
    bevy_cauldron::harness::add_editor_plugins(&mut app);
    for _ in 0..3 {
        app.update();
    }

    // **The startup claim.** The surface is the window's physical size, so the image never has to
    // be resized at startup — and a resize is one frame late at the camera, and that frame is the
    // crash.
    let window = window_size(&mut app).expect("a window");
    let surface = surface_size(&app).expect("a surface image");
    assert_eq!(
        surface, window,
        "the surface must be created at the window's physical size — a resize after the fact is \
         one frame late at the camera, and that frame is the crash"
    );

    // **And the crash's own shape: the window changes size, and the cameras must not be left
    // behind.**
    //
    // The audit's two numbers — 3396×1356 and 3440×1440 — are the window's physical size
    // before and after the compositor finishes sizing it. That second size arrives a frame *after*
    // the cameras exist, which is exactly when the mismatch is fatal.
    {
        let mut windows = app.world_mut().query::<&mut Window>();
        for mut w in windows.iter_mut(app.world_mut()) {
            w.resolution = bevy::window::WindowResolution::new(3440, 1440);
        }
    }
    app.update();

    // **The image and the cameras that share it must agree, on the very first frame after the
    // change.** The window camera is not in this list: it renders to the *window*, not the image,
    // so its target size is a different question.
    let image = surface_size(&app).expect("a surface image");
    let shared = shared_camera_sizes(&mut app);
    assert!(
        !shared.is_empty(),
        "the editor's cameras must be on the surface, or this test measures nothing"
    );
    for (order, size) in &shared {
        assert_eq!(
            *size, image,
            "the camera at order {order} still believes the target is {size:?} while the image is \
             {image:?}. A pass assembled from those two is refused by wgpu, and the default error \
             handler exits the application — this is the crash."
        );
    }
}

/// **A headless run has no window, keeps its placeholder, and still resizes.**
///
/// The harness is how the headless ratchets measure, and `resize_surface` is how a test asks for
/// another shape. Neither may change.
#[test]
fn a_headless_run_keeps_its_placeholder_and_resizes() {
    let root = fixtures::Fixture::new("headless_placeholder").build("town");
    let mut app = harness::build_headless(&root, "town", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    assert_eq!(
        surface_size(&app),
        Some((1536, 864)),
        "a headless run has no window to ask, so the placeholder stands"
    );

    harness::resize_surface(&mut app, 800, 600).expect("the surface resizes");
    assert_eq!(
        surface_size(&app),
        Some((800, 600)),
        "and a test can still ask for another shape"
    );
}
