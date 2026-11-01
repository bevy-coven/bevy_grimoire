//! **The stage's labels live in one space, and that space is the one they are drawn in.**
//!
//! The 2026-10-20-07c run's test, and the one that pins the *space* rather than the algorithm.
//!
//! # Why this test, and why it is a unit test
//!
//! **The defect was a disagreement between two spaces, and it could not be seen by a test that
//! compared the spaces to each other.** The first version measured the collision in the
//! *projection's* space and drew the label in the *node's* space; the labels were tested against
//! each other in one space and drawn in another, so **the test passed and the screen overlapped**.
//!
//! **So the test pins the conversion, not the comparison.** [`label_box`] is the one function that
//! turns "where the group is on the screen" and "how wide the label is" into a box, and
//! **the drawing writes that box into the node unchanged** — so if the conversion is right, the
//! test and the screen cannot disagree.
//!
//! # The three spaces
//!
//! Measured on the editor at `UiScale(2.4)`, on a 2560×1406 window, on Sanctuary's `town` kit —
//! the focal group's 26-character id:
//!
//! | | space | the measured value |
//! |---|---|---|
//! | `world_to_viewport` | **physical** pixels | `(1765.0, 878.6)` |
//! | `ComputedNode.size` | **physical** pixels | `412 × 32` |
//! | `Val::Px(x)` | **logical** pixels | — |
//!
//! **The scale is applied exactly once, on the way in.**

use bevy::prelude::*;

/// **The scale the editor runs at.** `EDITOR_UI_SCALE` is 1.2 and the window's own factor is 2.0,
/// so the render target's scale factor is 2.4 — see `bevy_ui`'s `update.rs`, which multiplies
/// `camera.target_scaling_factor()` by `ui_scale.0`.
const SCALE: f32 = 2.4;

/// **The measured projection and the measured node size**, from the capture of 2026-10-21.
const ANCHOR: Vec2 = Vec2::new(1765.0, 878.59454);
const NODE_SIZE: Vec2 = Vec2::new(412.0, 32.0);

/// **The box is in logical pixels, and the scale is applied once.**
///
/// **The measured failure this pins.** The scale was applied **twice** — once when the box was
/// built and again when it was written into the node — which put every label a factor of 2.4 from
/// where the deconfliction had put it, so the labels were tested in one place and drawn in another.
/// **And it was applied once too few** in the version before that, which put them 20 % off.
#[test]
fn the_box_is_in_the_space_the_node_is_drawn_in() {
    let b = bevy_cauldron::compose::label_box(ANCHOR, NODE_SIZE, SCALE)
        .expect("a laid-out node has a box");

    // --- **The width is the node's own size, in logical pixels.** The node's 412 physical px over
    // --- 26 characters is 6.60 logical px per glyph, against `BODY_CHAR_W`'s 6.6 — so the
    // --- constant and the node agree, and the box carries the node's number.
    assert_eq!(
        b.width,
        NODE_SIZE.x / SCALE,
        "the box's width is not the node's own width"
    );
    assert_eq!(
        b.height,
        NODE_SIZE.y / SCALE,
        "the box's height is not the node's own height"
    );

    // --- **The left is the anchor, minus half the width, in logical pixels.** The box is centred
    // --- on the group.
    let expect_left = ANCHOR.x / SCALE - b.width * 0.5;
    assert!(
        (b.left - expect_left).abs() < 0.01,
        "the box's left is {}, and it should be {expect_left}: the anchor's x over the scale, less \
         half the width.",
        b.left
    );

    // --- **And the scale is applied EXACTLY once.** This is the whole test: the box is drawn by
    // --- writing it into a `Val::Px`, and Bevy multiplies that by the scale on the way to
    // --- physical. **So the box's left, multiplied by the scale, must be the anchor's x less the
    // --- node's half-width** — the group's own centre, in physical pixels.
    let drawn_physical = b.left * SCALE;
    let expect_physical = ANCHOR.x - NODE_SIZE.x * 0.5;
    assert!(
        (drawn_physical - expect_physical).abs() < 0.01,
        "the label would be drawn at {drawn_physical} physical px, and the group's centre is at \
         {expect_physical}. The scale was applied the wrong number of times."
    );
}

/// **The box's top is the anchor's y, plus the drop** — the label sits *under* its miniature.
#[test]
fn the_box_sits_below_its_group() {
    let b = bevy_cauldron::compose::label_box(ANCHOR, NODE_SIZE, SCALE)
        .expect("a laid-out node has a box");
    // --- **The drop is the same constant the drawing uses**, so the two cannot drift.
    let expect_top = ANCHOR.y / SCALE + bevy_cauldron::compose::LABEL_DROP;
    assert!(
        (b.top - expect_top).abs() < 0.01,
        "the box's top is {}, and it should be {expect_top}: the anchor's y over the scale, plus \
         the drop.",
        b.top
    );
}

/// **A node that is not laid out yet has no box** — and a label with no box has nothing to collide
/// with, so it is not drawn.
#[test]
fn a_node_with_no_size_has_no_box() {
    assert!(
        bevy_cauldron::compose::label_box(ANCHOR, Vec2::ZERO, SCALE).is_none(),
        "a zero-sized node has a box, so a label that has not been laid out yet would be drawn \
         somewhere."
    );
}

/// **A scale of zero does not put every label at infinity.**
///
/// **A host misconfiguration rather than a state to render around**, so the guard is in the code
/// and the test says so — a `NaN` position is a label drawn at the origin of the universe.
#[test]
fn a_zero_scale_does_not_divide_by_zero() {
    let b = bevy_cauldron::compose::label_box(ANCHOR, NODE_SIZE, 0.0)
        .expect("a laid-out node has a box");
    assert!(
        b.left.is_finite() && b.top.is_finite() && b.width.is_finite(),
        "the box is not finite: {b:?}"
    );
}
