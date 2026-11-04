//! **The left pane is as wide as the panel inside it** (#282, #283).
//!
//! The shell's gooey split gave the left pane `SplitFraction(0.22)` of the body — 204 logical px at
//! 1280 — while the panel parented into it asks for `CONTROLS_W` 380 (456 on screen at the editor's
//! 1.2 scale). The pane clips what overflows it and the viewport is drawn over the rest, so the right
//! half of every control in the inspector was cut off or simply not there: `ENVIRONMENT`'s `18:30` showed
//! half a button and `23:00` none, and a click at `23:00`'s own reported centre (250, 206) landed on the
//! **viewport**, because the viewport starts at x = 233.
//!
//! **Measured on big, 2026-11-03**, editor `--features bevy_elements` under Xvfb at 1280 x 720: the
//! pointer's `HoverMap` entry for a click at `23:00`'s centre was the viewport node (size 724 x 582,
//! centre (595, 398)), not the button. The first draft of this fix blamed `Pickable::IGNORE` on the
//! pane; `bevy_ui`'s picking backend (`picking_backend.rs`) does not stop at an ignoring *ancestor*,
//! and the build with the pane made hoverable left the click dead, so that theory is wrong.
//!
//! The assertions are about geometry, which is what was wrong: the panel's right edge is inside the
//! pane's, and a button in the panel is not under the viewport.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::chrome::{Frame, PanelWidth, Side};
use bevy_cauldron::harness;

use crate::fixtures;

/// A node's rectangle in layout pixels.
fn rect_of(app: &App, e: Entity) -> Rect {
    let node = app.world().get::<ComputedNode>(e).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(e).expect("placed");
    Rect::from_center_size(at.translation, node.size())
}

fn editor(name: &str, width: u32) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(width, 720),
            ..default()
        },
        PrimaryWindow,
    ));
    for _ in 0..12 {
        app.update();
    }
    app
}

/// **The visible left panel fits in the pane the shell gave it.**
#[test]
fn the_left_pane_is_at_least_as_wide_as_the_panel_in_it() {
    let mut app = editor("left_pane_fits_panel", 1280);
    let left = app.world().resource::<Frame>().left;
    let pane = app.world().get::<ChildOf>(left).expect("the left dock is adopted into a pane").parent();
    let pane_rect = rect_of(&app, pane);

    let panels: Vec<(Entity, Rect)> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &PanelWidth, &Node, &ComputedNode)>();
        q.iter(world)
            .filter(|(_, w, n, c)| w.side == Side::Left && n.display != Display::None && c.size().x > 0.0)
            .map(|(e, ..)| e)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|e| (e, rect_of(&app, e)))
            .collect()
    };
    assert!(!panels.is_empty(), "the editor has a visible left panel");
    for (panel, rect) in panels {
        assert!(
            rect.max.x <= pane_rect.max.x + 1.0,
            "panel {panel:?} runs to x = {} but its pane ends at x = {}: everything past the pane's \
             edge is clipped or under the viewport, so a click on a control there lands on the world \
             (`23:00` at its own centre armed nothing)",
            rect.max.x,
            pane_rect.max.x
        );
    }
}

/// **The viewport still keeps the room B-014 promised**, now that the pane is wider.
#[test]
fn a_narrow_window_still_leaves_the_viewport_its_floor() {
    let app = editor("left_pane_narrow_window", 900);
    let viewport = app.world().resource::<Frame>().viewport;
    let width = app.world().get::<ComputedNode>(viewport).expect("laid out").size().x;
    assert!(
        width >= bevy_gooey::DOCK_PANE_MIN_PX,
        "at 900 px the viewport is {width} px, under the {} px pane floor",
        bevy_gooey::DOCK_PANE_MIN_PX
    );
}
