//! **A refused redo leaves the redo stack as it was, and says why (B-026, the redo half).**
//!
//! Undo and redo are one function (`apply`), and a refusal keeps its entry on the stack it came from.
//! The undo half is tested in `a_refused_undo_keeps_its_entry.rs`; this is the other stack. A stale
//! entry on the redo stack used to be lost the same way on the first Ctrl+Y, and a refused redo must
//! not leak onto the undo stack either.

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, plant_stale_redo_for_test, redo_for_test};
use bevy_cauldron::harness;

use crate::fixtures;

use fixtures::Fixture;

fn booted() -> App {
    let root = Fixture::new("refusedredo")
        .descriptor("tile_a", "alpha")
        .place("tile_a", (0.5, 0.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app
}

/// **A refused redo of a stale entry keeps the entry on the redo stack and says why.**
#[test]
fn a_refused_redo_keeps_its_entry_and_says_why() {
    let mut app = booted();
    plant_stale_redo_for_test(app.world_mut(), "ghost");
    assert_eq!(
        app.world().resource::<EditorState>().redo_depth(),
        1,
        "the planted entry is on the redo stack"
    );

    redo_for_test(app.world_mut());
    app.update();

    let state = app.world().resource::<EditorState>();
    assert_eq!(
        state.redo_depth(),
        1,
        "a refused redo popped its entry and pushed nothing back, so the author lost the stale entry"
    );
    assert_eq!(
        state.undo_depth(),
        0,
        "a refused redo moved its entry onto the undo stack"
    );
    assert!(
        state.status.problem_text().contains("ghost"),
        "a refused redo must say why, where the author looks. problem: {:?}",
        state.status.problem_text()
    );
}
