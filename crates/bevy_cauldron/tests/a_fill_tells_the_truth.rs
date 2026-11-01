//! **The fill's refusals reach the author, and the cap names a route that works.**
//!
//! The regression tests for the 2026-10-20-07b run, driven through the **real editor systems** —
//! `fill_for_test` is the release path of the place tool, without the mouse.
//!
//! # What was wrong
//!
//! **The editor's `Status` has two slots, and its own doc says which belongs where:**
//!
//! - `Status::note` — *"what just happened, and went as asked"*, for *"a refusal the author is not
//!   owed an alarm about — clicking bare ground with the remove tool armed"*. **An answer.**
//! - `Status::problem` — *"the editor could not do what was asked, **or did only part of it**"*.
//!   **A refusal.**
//!
//! **The fill's three refusals were in the receipt slot**, so a fill that could not be done said
//! nothing where the author looks — measured on a capture, the INSPECTOR's receipt row was empty
//! while the ghost's refusal for the same piece was plainly visible.
//!
//! **And the cap's message promised a route that does not exist:** *"fill again to continue"*, and
//! the second fill **cannot** continue, because the flood will not start on a cell that already
//! holds a piece.

use crate::fixtures;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use fixtures::Fixture;

/// **A project with a piece that cannot be filled with** — one that must sit on a host.
///
/// The fixture's ordinary piece is `OnFloor`, and a fill lays that happily: the first draft of this
/// test used it and measured *"filled 16"* — **a receipt, not a refusal**. A piece with
/// `mount: OnSurface("worktop")` **rests on a host**, and the core refuses to lay it on open floor:
/// *"a fill covers open floor and none of these cells offers one"*.
fn project(name: &str) -> std::path::PathBuf {
    Fixture::new(name)
        // **A piece that offers the surface** — the host the mounted piece would sit on. The
        // library refuses a piece that rests on a class nothing offers, and it is right: nothing
        // would ever place it.
        .surface_descriptor("site/desk", "alpha", "worktop")
        .mounted_descriptor("site/wall", "alpha", "worktop")
        .build("m")
}

/// **A refused fill is a `problem`, and the author can see it.**
///
/// The `Status` type's own contract is the rule: *a refusal handed back by `bevy_grimoire_core` is a
/// `problem`*. The receipt slot is a dim row, erased by the next receipt and by a tab switch, so a
/// refusal in it is a refusal the author never sees.
#[test]
fn a_refused_fill_is_a_problem_and_the_author_can_see_it() {
    let root = project("fill-refusal-is-a-problem");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // --- **A fill that cannot be done.** A piece that mounts on a surface cannot be laid on
    // --- open floor, so the core refuses and the editor has to say so.
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

    let world = app.world_mut();
    let state = world.resource::<EditorState>();
    assert!(
        state.status.has_problem(),
        "a fill that could not be done said nothing the author can see. The `Status` contract: \\
         *the editor could not do what was asked* is a `problem`, which reaches the toast and the \\
         journal. The status is {:?}",
        state.status
    );
    assert!(
        state.status.note_text().is_empty(),
        "and the receipt slot should be empty — a refusal is not a receipt, and a receipt is what \
         the next receipt overwrites. The status is {:?}",
        state.status
    );
}

/// **A refusal survives a receipt and a tab switch.**
///
/// **This is the property that makes the channel matter.** A `note` is one `String` that the next
/// receipt overwrites, and `Status::clear_note` erases it on a tab switch — and that method exists
/// because a *stale receipt* was the whole of a bug. **A refusal is not a stale receipt.**
#[test]
fn a_refusal_survives_a_receipt_and_a_tab_switch() {
    let mut app = harness::build_headless(&project("fill-refusal-survives"), "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

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

    let world = app.world_mut();
    let state = world.resource::<EditorState>();
    let refusal = state.status.problem_text().to_owned();
    assert!(
        !refusal.is_empty(),
        "the fill was refused and the refusal was raised"
    );

    // --- A receipt, written after the refusal.
    {
        let mut state = world.resource_mut::<EditorState>();
        state.status.note("filled 12 with wall");
    }
    // --- And a tab switch, which is `clear_note`.
    {
        let mut state = world.resource_mut::<EditorState>();
        state.status.clear_note();
    }
    let state = world.resource::<EditorState>();
    assert_eq!(
        state.status.problem_text(),
        refusal,
        "a receipt and a tab switch took the refusal away. `clear_note` clears receipts; a refusal \\
         is not a receipt, and the author has to still be able to learn that the fill did not happen."
    );
}

/// **The cap's message names a route that works.**
///
/// The research's criterion 1: *any message that tells the author to take an action must be
/// followed, in a test, by performing that action and asserting the stated result.* The failing form
/// of it is the test in `fill.rs`: `the_caps_promise_cannot_be_kept_by_a_second_fill`.
#[test]
fn the_caps_message_names_a_route_that_works() {
    let route = bevy_cauldron::fill::CAP_ROUTE;
    assert!(
        route.contains("another empty cell"),
        "the cap's message must name the route that works — a fill from a cell the first fill did \\
         not reach — because the route that does NOT work is filling the same cell again. It said \\
         `{route}`"
    );
    assert!(
        !route.to_lowercase().contains("fill again to continue"),
        "the cap's message must not promise 'fill again to continue', because the second fill in \\
         the same cell is refused: the flood will not start on a cell that already holds a piece. \\
         It said `{route}`"
    );
}

/// **The two fills say the same kind of thing.**
///
/// The box fill said *"stopped at the 4096 cell cap"* and the flood said *"fill again to continue"*.
/// **A message that differs between two ways of doing the same thing is a message one of them got
/// wrong** — and this run's measurement is that the flood's was wrong.
#[test]
fn the_two_fills_say_the_same_kind_of_thing() {
    // **Both files, because the flood's message moved.** Since S-001 the flood's cap sentence is
    // written by `ops::fill` — one operation with two front ends — so counting only `editor.rs`
    // would report one use and fail on the consolidation rather than on a disagreement. What this
    // guards is unchanged: *both* fills name the same route. The box fill's is still in `editor.rs`.
    let src: String = ["src/editor.rs", "src/ops.rs"]
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}")))
        .collect();
    let route_uses = src.matches("fill::CAP_ROUTE").count();
    assert!(
        route_uses >= 2,
        "the two fills' cap messages do not both name `fill::CAP_ROUTE` — they use it {route_uses} \
         times. A message that differs between two ways of doing the same thing is a message one \
         of them got wrong."
    );
}
