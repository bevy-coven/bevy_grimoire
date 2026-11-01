//! **The refusal is on screen, and not only in a status field.**
//!
//! The 2026-10-20-07b run's third test, and the one that answers the question the other two only
//! imply: **does the refusal reach a node the author can see?**
//!
//! # Why this test exists
//!
//! The first two tests ask whether the refusal is a [`bevy_cauldron::chrome::Status::problem`]
//! and whether it survives. **Neither asks whether the author can see it** — and that is the
//! whole of the defect: the refusal was in the status, in the *right* slot, and **the panel's
//! receipt row was empty in a capture**.
//!
//! So this one asks the question the capture asked: **is there a visible node carrying the
//! refusal's words?**
//!
//! **And it is a layout test, not a status test.** The headless harness registers every render
//! type with no device, so the UI still lays out and `ComputedNode` still has the geometry — which
//! is what a "visible" claim is made of.

use crate::fixtures;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::harness;
use fixtures::Fixture;

/// **Every text on screen, and where it is.**
fn visible_texts(app: &mut App) -> Vec<(String, bool)> {
    app.update();
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform, &bevy::ui::Node)>();
    let mut out = Vec::new();
    for (text, node, transform, style) in q.iter(world) {
        // **A node that is drawn.** Bevy's `Node` carries `display`; a hidden one is `Display::None`.
        let shown = style.display == bevy::ui::Display::Flex
            && node.size().x > 0.0
            && node.size().y > 0.0
            && transform.translation.x.is_finite();
        out.push((text.0.clone(), shown));
    }
    out
}

/// **A refused fill is on screen, in words the author can read.**
#[test]
fn a_refused_fill_is_on_screen() {
    let root = Fixture::new("fill-refusal-on-screen")
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

    // --- What the status says.
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

    // --- And whether it is on screen.
    let texts = visible_texts(&mut app);
    let on_screen: Vec<&String> = texts
        .iter()
        .filter(|(_, shown)| *shown)
        .map(|(t, _)| t)
        .filter(|t| t.contains("cannot be box filled") || t.contains("mounts to a"))
        .collect();
    assert!(
        !on_screen.is_empty(),
        "the refusal was raised but **nothing on screen says it**. The refusal was:\\n{refusal}\\n\\
         and the visible texts were: {texts:?}\\n\\
         A refusal the author cannot see is a refusal the author does not have."
    );
}
