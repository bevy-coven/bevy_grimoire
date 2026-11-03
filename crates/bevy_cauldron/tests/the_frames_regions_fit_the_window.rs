//! **The frame's regions must fit the window they are laid out in** (UI-30, #224).
//!
//! A docked frame is only docked if its three regions share the row instead of each asking for its own
//! content width. Measured on 2026-11-03 before this assertion existed, in a 1280-px window:
//!
//! ```text
//! left      456 px
//! viewport  724 px
//! right     317 px
//!            ---- 1497 px in 1280, 217 px of overflow
//! ```
//!
//! The split carried `width: Val::Auto`, so its flex basis was its content rather than the room left
//! over, and the viewport — the region that is supposed to be *what is left* — took a width of its own.
//! The fix is a zero basis with `flex_grow: 1`, the same shape every other centre in this frame uses.
//!
//! This is a layout assertion rather than a capture on purpose: it is stronger than a still for "does
//! it fit", and a still of an overflowing frame looks normal because the overflow is off-screen.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, Val};
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::harness;

use crate::fixtures;

#[test]
fn the_frames_regions_fit_the_window() {
    const WIDTH: f32 = 1280.0;
    let root = fixtures::Fixture::new("the_frames_regions_fit_the_window")
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH as u32, 720),
            ..default()
        },
        PrimaryWindow,
    ));
    for _ in 0..8 {
        app.update();
    }

    let (body_e, left_e, viewport_e, right_e) = {
        let f = app.world().resource::<bevy_cauldron::chrome::Frame>();
        (f.body, f.left, f.viewport, f.right)
    };
    let size_of = |e: Entity| {
        app.world()
            .get::<ComputedNode>(e)
            .map(|n| n.size())
            .unwrap_or(Vec2::ZERO)
    };
    let (left, viewport, right) = (
        size_of(left_e),
        size_of(viewport_e),
        size_of(right_e),
    );
    let body = size_of(body_e);

    assert!(body.x > 0.0, "the frame's body has no width, so nothing is laid out");
    // The three regions are children of the body and must share the one row.
    for (name, s) in [
        ("left", left),
        ("viewport", viewport),
        ("right", right),
    ] {
        assert!(
            s.x >= 1.0 && s.y >= 1.0,
            "the {name} region is {}x{}: a region with no size is drawn nowhere",
            s.x,
            s.y
        );
    }
    let total = left.x + viewport.x + right.x;
    assert!(
        total <= body.x + 1.0,
        "the frame's regions are {total} px wide in a {} px body ({} px over): \
         the regions are not sharing the row",
        body.x,
        total - body.x
    );
    // The viewport is "what is left", so it must be the widest of the three and hold a usable
    // share of the window rather than the sliver a content basis would give it.
    assert!(
        viewport.x >= body.x * 0.5,
        "the viewport is {} px of a {} px body: under half the window is left for the world",
        viewport.x,
        body.x
    );

    // And the rect handed to the camera is inside the image, which is where a viewport that
    // overflows the window dies with "Attachments have differing sizes" (`fit_viewport_to_frame`).
    let tf = app.world().get::<UiGlobalTransform>(viewport_e).copied();
    if let Some(tf) = tf {
        let min = tf.translation - viewport / 2.0;
        assert!(
            min.x >= -1.0 && min.y >= -1.0,
            "the viewport starts at ({:.0}, {:.0}): a negative origin is a rect outside the image",
            min.x,
            min.y
        );
    }
    let _ = Val::Px(0.0);
}
