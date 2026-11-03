//! **A pointer over the frame's own furniture is left where the UI picker can see it** (UI-30, #224's
//! second acceptance item).
//!
//! `retarget_pointer` puts **every** pointer on the surface *image*, because the interface renders
//! into an image and `bevy_ui`'s backend only keeps a pointer whose `target` equals its camera's
//! `RenderTarget`. That is right for a pointer over the viewport — it is how a row lights and a chart
//! answers — and wrong for one over a dock: the UI picker never considers an image-targeted pointer
//! for window-space nodes, so the **divider cannot be pressed anywhere**, and with it no dock can ever
//! be resized by a person or by `tools/demo/record.py`.
//!
//! Measured 2026-11-03 on the fixture project: the pointer sat at the divider's own centre
//! (218.5, 398.0) and its target was `Image(..)`, so the press reached nothing. Pass 273 spent three
//! attempts on the drag believing the fault was in `inject_clicks`; this is the actual cause.
//!
//! **What this pins:** with the pointer over a dock, `PointerLocation.target` is the **window**, which
//! is what the UI picker keeps. Over the viewport it stays the image, which is what the map's own
//! picking needs. The frame's regions are named (`Viewport`, `Left dock`, `Right dock`, `Status bar`),
//! so the two are told apart by the one thing that already knows: the rect under the pointer.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::harness;

use crate::fixtures;

#[test]
fn a_pointer_over_a_dock_targets_the_window_and_over_the_viewport_targets_the_image() {
    let root = fixtures::Fixture::new("pointer_over_a_dock")
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
    let viewport_at = {
        let f = app.world().resource::<bevy_cauldron::chrome::Frame>();
        app.world()
            .get::<bevy::ui::UiGlobalTransform>(f.viewport)
            .copied()
            .expect("the viewport is laid out")
            .translation
    };

    let target_at = |app: &mut App, at: Vec2| {
        app.world_mut().resource_mut::<bevy_debugger_bevy::DebugCursor>().0 = Some(at);
        for _ in 0..3 {
            app.update();
        }
        app.world_mut()
            .query::<&bevy::picking::pointer::PointerLocation>()
            .iter(app.world())
            .find_map(|l| l.location().map(|l| l.target.clone()))
    };

    // **Over the divider**: the window, so the UI picker keeps it and the press lands.
    let on_divider = target_at(&mut app, divider_at);
    assert!(
        matches!(
            on_divider,
            Some(bevy::camera::NormalizedRenderTarget::Window(_))
        ),
        "a pointer over the split's divider targets {on_divider:?}. The UI picker only keeps a \
         pointer whose target equals its camera's, so an image-targeted one never reaches the \
         divider and **no dock can be resized** — by a person or by `tools/demo/record.py`"
    );

    // **Over the viewport**: still the image, which is where the map's own picking reads it.
    let on_viewport = target_at(&mut app, viewport_at);
    assert!(
        matches!(
            on_viewport,
            Some(bevy::camera::NormalizedRenderTarget::Image(_))
        ),
        "a pointer over the viewport must stay on the surface image, or the map stops taking \
         clicks and the wheel stops zooming: it targeted {on_viewport:?}"
    );
}
