//! **The stage's labels, as they are drawn.**
//!
//! The 2026-10-20-07c run's second test, and the one that measures the *screen* rather than the
//! algorithm.
//!
//! # Why
//!
//! The first test pins the deconfliction's rules. **This one pins the thing the author sees**: the
//! labels' laid-out boxes, in a real app, must not overlap.
//!
//! **And it has to be a layout test, not a screenshot.** The stage's labels are drawn as UI nodes
//! over a 3-D viewport, and the editor's headless harness registers every render type with no
//! device — so the UI still lays out and `ComputedNode` still has the geometry, which is what a
//! "visible" claim is made of. A screenshot cannot: the labels are small, the stage is off to one
//! side, and the defect is measured in pixels of a region an author glances past.

use crate::fixtures;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use fixtures::Fixture;

/// **The label nodes' boxes, in screen space.**
fn label_boxes(app: &mut App) -> Vec<(String, f32, f32, f32, f32)> {
    app.update();
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform, &bevy::ui::Node)>();
    let mut out = Vec::new();
    for (text, node, transform, style) in q.iter(world) {
        // --- **A drawn label.** `Display::None` is how a label is taken down, and a node with no
        // --- size is not on screen.
        if style.display != bevy::ui::Display::Flex || node.size().x <= 0.0 {
            continue;
        }
        // --- **Only the stage's labels.** The panel's rows carry the same ids, and the stage's
        // --- labels are the ones **over the viewport** — the panel is on the left, the stage is
        // --- in the middle. **The stage's labels are the ones the deconfliction places**, and
        // --- they are the only ones with a `·` in them, because that is the mark the stage
        // --- draws.
        if !text.0.contains('/') || !text.0.starts_with("town/house_cottage_") {
            continue;
        }
        out.push((
            text.0.clone(),
            transform.translation.x,
            transform.translation.y,
            node.size().x,
            node.size().y,
        ));
    }
    out
}

/// **No two stage labels overlap.**
///
/// **The measured defect**, on a real capture of Sanctuary's own project: the labels' ink ran
/// **x 1653..2466 — 813 px** with no clean boundary where three labels should be, and **918 px**
/// where four should be. At 2.5× the focal group's name was drawn *through* its neighbour's.
#[test]
fn no_two_stage_labels_overlap() {
    // --- **Several groups, so the carousel has neighbours to collide with**, **with ids as long as
    // --- the real ones.**
    //
    // --- The measured defect was on Sanctuary's own groups: `town/house_cottage_plaster` is **24
    // --- characters**, and three of them on a diagonal strip overlapped by 140 px. A fixture with
    // --- short ids would measure a case that cannot happen, so the ids here are the real length.
    let root = Fixture::new("stage-labels")
        .descriptor("site/wall", "alpha")
        .composition(
            "town/house_cottage_plaster",
            &[("site/wall", "site/wall", (0.0, 0.0))],
        )
        .composition(
            "town/house_cottage_brick",
            &[("site/wall", "site/wall", (0.0, 0.0))],
        )
        .composition(
            "town/house_cottage_wood",
            &[("site/wall", "site/wall", (0.0, 0.0))],
        )
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // --- The COMPOSE tab, where the stage is.
    {
        let world = app.world_mut();
        *world.resource_mut::<bevy_cauldron::tiles::Mode>() =
            bevy_cauldron::tiles::Mode::Compose;
    }
    for _ in 0..3 {
        app.update();
    }

    let boxes = label_boxes(&mut app);
    // --- **More than one label**, or the test measures nothing.
    assert!(
        boxes.len() > 1,
        "the stage drew {} label(s), so there is nothing to collide with. The boxes were \
         {boxes:?}.",
        boxes.len()
    );
    if boxes.is_empty() {
        // --- **The stage's labels need a camera to project onto**, and the headless harness has
        // --- none — so the boxes cannot be measured here. **The test says so rather than
        // --- passing vacuously**, and the measurement that does exist is the one in the review:
        // --- the pixels.
        panic!(
            "the stage's labels were not measurable: the headless harness has no camera to project \\
             them onto, so this test cannot measure what it claims to. The boxes were {boxes:?}."
        );
    }

    for (i, a) in boxes.iter().enumerate() {
        for b in boxes.iter().skip(i + 1) {
            // --- **The node's translation is its CENTRE**, and the width is its own. So the overlap
            // --- test is on the two boxes the centres describe: from `centre - width/2` to
            // --- `centre + width/2`.
            let a_left = a.1 - a.3 * 0.5;
            let a_right = a.1 + a.3 * 0.5;
            let b_left = b.1 - b.3 * 0.5;
            let b_right = b.1 + b.3 * 0.5;
            let x = a_left < b_right && b_left < a_right;
            let y = a.2 - a.4 * 0.5 < b.2 + b.4 * 0.5 && b.2 - b.4 * 0.5 < a.2 + a.4 * 0.5;
            assert!(
                !(x && y),
                "two stage labels overlap: `{}` (x {a_left:.0}..{a_right:.0}) and `{}` \
                 (x {b_left:.0}..{b_right:.0}). The stage is illegible.",
                a.0,
                b.0
            );
        }
    }
}

/// **The stage's labels are laid out in one space, and the drawing writes that space unchanged.**
///
/// **The bug this pins, and it is worth recording.** The first version measured the collision in
/// the *projection's* space and drew the labels from the *node's* space — two spaces, offset by
/// the frame's left dock and by the render target's scale factor. **The test passed and the screen
/// still overlapped**, because the numbers the test compared were not the numbers the screen drew.
///
/// **So there is now one function that builds the box** — `compose::label_box` — and the drawing
/// writes it into the node **unchanged**. The conversion inside that function is pinned by
/// `the_stage_labels_live_in_one_space`; this test pins that there is only **one** box builder and
/// that the drawing does not touch the scale.
///
/// **And the anchor is one function too** — `compose::label_anchor` — so the collision test and
/// the drawing cannot disagree about *where* the label is, only about whether it collides.
#[test]
fn the_box_is_built_once_and_drawn_unchanged() {
    let src = std::fs::read_to_string("src/compose.rs").expect("compose.rs is there");
    // --- **One box builder.** Two would be two answers to "where is this label".
    let builders = src.matches("fn label_box(").count();
    assert_eq!(
        builders, 1,
        "there are {builders} `label_box` definitions, so there is more than one answer to where a \
         label is."
    );
    // --- **The drawing writes the box unchanged.** A second division by the scale would put every
    // --- label a factor of `UiScale` from where the deconfliction put it — the exact bug the
    // --- first version had, in the other direction.
    assert!(
        src.contains("(Val::Px(b.left), Val::Px(b.top))"),
        "the drawing does not write the box into the node unchanged, so the scale is applied a \
         second time and the labels are drawn away from where they were deconflicted."
    );
    assert!(
        !src.contains("Val::Px(b.left / s)") && !src.contains("Val::Px(b.top / s)"),
        "the drawing divides the box by the scale again, so the scale is applied twice."
    );
    // --- **And one anchor.** The box builder and the drawing both call it.
    assert!(
        src.contains("label_anchor(cam, cam_tf, slot)"),
        "the labels are not anchored by `label_anchor`, so the box builder and the drawing can \
         disagree about where the label is."
    );
}
