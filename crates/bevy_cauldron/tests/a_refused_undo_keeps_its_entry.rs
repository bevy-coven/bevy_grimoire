//! **A refused undo leaves the stack as it was, and says why (B-026).**
//!
//! `undo` used to pop the entry before it applied it, and a refused apply pushed nothing back. So a
//! stale entry, one whose piece or stamp the map no longer holds, was lost on the first Cmd+Z: the
//! author's next undo reached for the entry beneath it, and the refused one was simply gone. This
//! plants a stale stamp move and drives the real `undo`.

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, plant_stale_move_for_test, undo_for_test};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

use fixtures::Fixture;

fn booted() -> App {
    let root = Fixture::new("refusedundo")
        .descriptor("tile_a", "alpha")
        .place("tile_a", (0.5, 0.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app
}

/// **A refused undo of a stale entry keeps the entry on the stack and says why.**
#[test]
fn a_refused_undo_keeps_its_entry_and_says_why() {
    let mut app = booted();
    plant_stale_move_for_test(app.world_mut(), "ghost");

    let next_before = {
        let state = app.world().resource::<EditorState>();
        let open = app.world().resource::<OpenMap>();
        state.undo_next(&open.map)
    };
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 1, "the planted entry is on the stack");

    undo_for_test(app.world_mut());
    app.update();

    let state = app.world().resource::<EditorState>();
    let open = app.world().resource::<OpenMap>();
    assert_eq!(
        state.undo_depth(),
        1,
        "a refused undo popped its entry and pushed nothing back, so the author lost the stale \
         entry and the one beneath it is next. The stack must be as it was."
    );
    assert_eq!(
        state.undo_next(&open.map),
        next_before,
        "the entry at the top of the undo stack changed under a refused undo"
    );
    assert!(
        state.status.problem_text().contains("ghost"),
        "a refused undo must say why, where the author looks. problem: {:?}",
        state.status.problem_text()
    );
}
