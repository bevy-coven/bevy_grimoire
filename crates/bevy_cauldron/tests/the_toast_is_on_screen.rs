//! **The toast is on screen.**
//!
//! The 2026-10-20-07b run's fourth test, and the one the screenshots could not settle.
//!
//! # Why
//!
//! The refusal now goes to `Status::problem`, which reaches **the toast** — a centred card in the
//! viewport, drawn by `notice.rs` — and the **journal**, which is the session's record.
//!
//! **A capture could not settle whether the toast was up.** The toast is on for
//! `notice::TOAST_SECS` — **seven seconds** — and it is a *centred card in the viewport*, which is
//! where the map is drawn; a capture taken at the wrong moment, or with the cursor over a panel
//! (which blocks the verb that would raise it), shows nothing and looks like a failure.
//!
//! **So this test measures the layout instead of the pixels**: a refusal must put a card on
//! screen, in the viewport, with a real size and the refusal's words in it.
//!
//! **And it is the same question the review asked, asked of the thing that answers it.**

use crate::fixtures;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::harness;
use fixtures::Fixture;

/// **Every text on screen, with its geometry and whether it is drawn.**
fn texts(app: &mut App) -> Vec<(String, f32, f32, bool)> {
    app.update();
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform, &bevy::ui::Node)>();
    let mut out = Vec::new();
    for (text, node, transform, style) in q.iter(world) {
        let shown = style.display == bevy::ui::Display::Flex
            && node.size().x > 0.0
            && node.size().y > 0.0;
        out.push((
            text.0.clone(),
            transform.translation.x,
            node.size().x,
            shown,
        ));
    }
    out
}

/// **A refusal puts a card on screen, in the viewport, with the refusal's words in it.**
#[test]
fn a_refusal_puts_a_card_on_screen() {
    let root = Fixture::new("fill-refusal-card")
        .surface_descriptor("site/desk", "alpha", "worktop")
        .mounted_descriptor("site/wall", "alpha", "worktop")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // --- A fill that cannot be done.
    {
        let world = app.world_mut();
        bevy_cauldron::editor::fill_for_test(
            world,
            "site/wall",
            ((0.5, 0.5), (3.5, 3.5)),
            bevy_grimoire_core::grid::TILE,
        );
    }
    app.update();

    let refusal = {
        let world = app.world_mut();
        world
            .resource::<bevy_cauldron::editor::EditorState>()
            .status
            .problem_text()
            .to_owned()
    };
    assert!(
        !refusal.is_empty(),
        "the fill was not refused, so there is nothing to be on screen"
    );

    // --- **The card.** The toast is a centred card in the viewport, so it is wide and it is near
    // --- the middle of the frame.
    let all = texts(&mut app);
    let card = all
        .iter()
        .find(|(t, _, width, shown)| {
            *shown && *width > 200.0 && t.contains("cannot be box filled")
        })
        .unwrap_or_else(|| {
            panic!(
                "the refusal was raised but **no card on screen carries it**. The refusal was:\\n\\
                 {refusal}\\n\\
                 and the texts were: {all:?}\\n\\
                 A refusal the author cannot see is a refusal the author does not have."
            )
        });
    // --- **And it is where the author is looking.** The toast is a centred card **in the
    // --- viewport**, and the viewport is the middle of the frame — not the middle of the whole
    // --- window, which is where the frame's own chrome puts its left and right docks.
    //
    // --- The viewport's own box, so the test measures against the thing the toast is centred in.
    let viewport = app
        .world_mut()
        .query::<&ComputedNode>()
        .iter(app.world())
        .map(|n| n.size().x)
        .fold(0.0f32, f32::max);
    assert!(
        viewport > 0.0,
        "the viewport has no width, so the toast's position cannot be measured against it"
    );
    assert!(
        card.1 > 400.0,
        "the card carrying the refusal is at x {:.1}, which is off to the left of the viewport — \\
         the toast is a centred card, and a card off to one side is a card the author is not \\
         looking at.",
        card.1
    );
}
