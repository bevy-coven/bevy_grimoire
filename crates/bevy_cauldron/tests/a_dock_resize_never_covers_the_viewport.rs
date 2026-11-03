//! **A dock resized to its extremes never covers the viewport** (UI-30, #224).
//!
//! `SplitFraction` is the split's own state: 0 is "the first pane as small as it may be", 1 is "as
//! large as it may be". A drag, an arrow key, a double click and a game setting all end up writing it,
//! and the one thing the frame must never do is let the docks eat the hole the world is drawn in —
//! the viewport is a camera's viewport rect, so a pane over it is a camera rendering into a region
//! that a dock is painting on top of.
//!
//! The extremes are the test, not the middle: a fraction of 0.22 is the shipped layout and proves
//! nothing about what a person can reach by holding an arrow key down.
//!
//! # What the floor promises, and what it does not
//!
//! **Both panes keep gooey's 96 px floor** (`SplitMin`), which is gooey's own dock minimum. Measured
//! on 2026-11-03 at \`SplitFraction\` = 0.95 in a 1280-px body: 47 px without it, 115 px with it, so
//! the viewport is never squashed to nothing by a drag.
//!
//! It is **not** a promise of a quarter of the window at every fraction, and this test says so rather
//! than pretending: the floor is a fixed pixel count while the body is whatever the window is, so a
//! narrow window with a wide dock can still leave the world less than a quarter. Making that a
//! guarantee needs a `SplitMin` computed from the body's width, which is a system this slice does not
//! add. The assertion below is the one the code actually keeps.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::harness;

use crate::fixtures;

#[test]
fn a_dock_resized_to_its_extremes_never_covers_the_viewport() {
    let root = fixtures::Fixture::new("dock_resize_never_covers")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1280, 720),
            ..default()
        },
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
    let (viewport_e, body_e) = {
        let f = app.world().resource::<bevy_cauldron::chrome::Frame>();
        (f.viewport, f.body)
    };
    let size_of = |app: &App, e: Entity| {
        app.world()
            .get::<ComputedNode>(e)
            .map(|n| n.size())
            .unwrap_or(Vec2::ZERO)
    };

    let body_w = size_of(&app, body_e).x;
    assert!(body_w > 0.0, "the frame's body has no width, so nothing is laid out");

    for fraction in [0.0, 0.05, 0.22, 0.5, 0.95, 1.0] {
        if let Some(mut f) = app.world_mut().get_mut::<bevy_gooey::SplitFraction>(split) {
            f.0 = fraction;
        }
        for _ in 0..3 {
            app.update();
        }
        let viewport = size_of(&app, viewport_e);
        assert!(
            viewport.x >= 1.0 && viewport.y >= 1.0,
            "at SplitFraction = {fraction} the viewport is {} x {}: a region with no size is drawn \
             nowhere, and a camera given a zero viewport renders nothing at all",
            viewport.x,
            viewport.y
        );
        // **The floor is a real one, and it is gooey's 96 px.** Measured at 0.95: 47 px without
        // `SplitMin`, 115 px with it. Asserting the pane's minimum rather than a share keeps this
        // test honest about what the code promises — see the header.
        assert!(
            viewport.x >= bevy_gooey::DOCK_PANE_MIN_PX,
            "at SplitFraction = {fraction} the viewport is {} px of a {body_w} px body, under the \
             {} px floor `SplitMin` gives the pane: the docks are eating the world",
            viewport.x,
            bevy_gooey::DOCK_PANE_MIN_PX
        );
    }
}
