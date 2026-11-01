//! **A stale entry that refuses twice is dropped, and the entry beneath it is reached (gap 3).**
//!
//! B-026 keeps a refused entry on the stack, so the author can see why and try again. An entry that
//! refuses on every Cmd+Z, though, sat above every older change for good: the author could never
//! take anything back past it. This plants a valid placement, then a stale entry above it, and drives
//! the real `undo`.

use bevy::prelude::*;
use bevy_cauldron::editor::{
    EditorState, place_for_test, plant_stale_move_for_test, undo_for_test,
};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

use fixtures::Fixture;

fn booted() -> App {
    let root = Fixture::new("staledrop")
        .descriptor("tile_a", "alpha")
        .place("tile_a", (0.5, 0.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app
}

fn pieces(app: &App) -> usize {
    app.world().resource::<OpenMap>().map.placements.len()
}

#[test]
fn a_stale_entry_that_refuses_twice_is_dropped_and_the_entry_beneath_is_reached() {
    let mut app = booted();
    place_for_test(app.world_mut(), "tile_a", (2.5, 0.5));
    app.update();
    assert_eq!(pieces(&app), 2, "the fixture's piece and the placed one");
    plant_stale_move_for_test(app.world_mut(), "ghost");
    assert_eq!(
        app.world().resource::<EditorState>().undo_depth(),
        2,
        "the placement and the stale entry are both on the stack"
    );

    undo_for_test(app.world_mut());
    app.update();
    assert_eq!(
        app.world().resource::<EditorState>().undo_depth(),
        2,
        "the first refusal keeps the stale entry (B-026)"
    );

    undo_for_test(app.world_mut());
    app.update();
    {
        let state = app.world().resource::<EditorState>();
        assert_eq!(
            state.undo_depth(),
            1,
            "a stale entry that refused twice stayed on the stack above the placement"
        );
        assert!(
            state.status.problem_text().contains("dropped"),
            "dropping an entry must be said where the author looks: {:?}",
            state.status.problem_text()
        );
    }

    undo_for_test(app.world_mut());
    app.update();
    assert_eq!(
        pieces(&app),
        1,
        "the undo beneath the stale entry did not take back the placement"
    );
}
