//! **A pointer over the frame's furniture points at the same render target its UI camera does**
//! (UI-30, #224's second acceptance item).
//!
//! `retarget_pointer` puts every pointer on the surface **image**, and that is correct: the editor's
//! interface camera (`surface.rs`, `IsDefaultUiCamera`) renders to that image too, and `bevy_ui`'s
//! picking backend keeps a pointer only where **`render_target == pointer_location.target`**
//! (`bevy_ui-0.20.0/src/picking_backend.rs:129`).
//!
//! **This test exists because pass 280 changed that to the window and was wrong.** Its reasoning was
//! plausible — a UI picker sounding like a window-space thing — and it broke the very case it meant to
//! fix: with the pointer on `Window(NormalizedWindowRef(..))` and the divider's UI camera on
//! `Image(..)`, `Hovered` stayed false (measured, `divider Hovered? false`), so the drag still did
//! nothing. Reverted here; the invariant it should have asserted is the one below.
//!
//! What the divider still needs is a different question, recorded on the issue: its
//! `ComputedUiTargetCamera` **does** resolve (`camera: Some(..)`), so the picker has a camera to
//! consider and something else is stopping the hit.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::harness;

use crate::fixtures;

#[test]
fn a_pointer_over_a_dock_points_at_the_render_target_its_ui_camera_uses() {
    let root = fixtures::Fixture::new("pointer_target_matches_ui_camera")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(1280, 720), ..default() },
        PrimaryWindow,
    ));
    for _ in 0..8 {
        app.update();
    }

    let split = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_gooey::Split>>()
        .iter(app.world())
        .next()
        .expect("the shell has one split");
    let divider = {
        let children = app.world().get::<Children>(split).expect("the split has children");
        children
            .iter()
            .find(|c| {
                matches!(
                    app.world().get::<bevy_gooey::Role>(*c),
                    Some(bevy_gooey::Role::SplitDivider)
                )
            })
            .expect("the split has a divider")
    };
    let divider_at = app
        .world()
        .get::<bevy::ui::UiGlobalTransform>(divider)
        .copied()
        .expect("the divider is laid out")
        .translation;

    // **The camera the UI picker will consider for this node**, and the target it renders to.
    let (ui_camera, ui_target) = {
        let camera_entity = app
            .world()
            .get::<bevy::ui::ComputedUiTargetCamera>(divider)
            .and_then(|c| c.get())
            .expect("the divider resolves a UI target camera, or the picker has nothing to consider");
        let target = app
            .world()
            .get::<bevy::camera::RenderTarget>(camera_entity)
            .cloned()
            .expect("a UI camera renders somewhere");
        (camera_entity, target)
    };
    let _ = ui_camera;

    app.world_mut().resource_mut::<bevy_debugger_bevy::DebugCursor>().0 = Some(divider_at);
    for _ in 0..4 {
        app.update();
    }

    let pointer_target = app
        .world_mut()
        .query::<&bevy::picking::pointer::PointerLocation>()
        .iter(app.world())
        .find_map(|l| l.location().map(|l| l.target.clone()))
        .expect("the pointer has a location");

    // **The one thing `bevy_ui`'s backend requires.** Comparing the *normalized* forms, because that is
    // what the backend compares: the pointer carries `NormalizedRenderTarget` and the camera carries
    // `RenderTarget`.
    let normalized_camera = bevy::window::WindowRef::Entity(
        app.world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .next()
            .expect("a primary window"),
    )
    .normalize(None);
    let normalized_camera = app
        .world()
        .get::<bevy::camera::RenderTarget>(ui_camera)
        .and_then(|t| match t {
            bevy::camera::RenderTarget::Window(_) => {
                Some(bevy::camera::NormalizedRenderTarget::Window(normalized_camera.unwrap()))
            }
            bevy::camera::RenderTarget::Image(i) => {
                Some(bevy::camera::NormalizedRenderTarget::Image(i.clone()))
            }
            _ => None,
        })
        .expect("the UI camera's target normalizes");

    assert_eq!(
        pointer_target,
        normalized_camera,
        "the pointer points at {pointer_target:?} and the divider's UI camera renders to \
         {ui_target:?} (normalized {normalized_camera:?}). `bevy_ui`'s backend keeps a pointer only \
         where `render_target == pointer_location.target`, so these must be equal for the divider to \
         ever be hovered or pressed — pass 280 made them differ and broke the drag it meant to fix"
    );
}
