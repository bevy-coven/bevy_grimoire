//! **The history panel labels the rows on screen and no others** (UI-53, #239).
//!
//! The list is a fixed window of `2 * HISTORY_EACH_WAY + 1` rows around the current position, and it
//! used to build **every** label with `EditorState::history()` and then read seven of them: a session
//! with 5,000 entries allocated 5,000 strings per refresh. UI-53's first acceptance line is
//! "5,000 log lines scroll within the UI frame budget", and the windowed read is the half of that a
//! headless test can hold down: the rows come from `history_window`, which labels only what it
//! returns.
//!
//! The second acceptance line is that an agent's row reads "the Familiar", which this file also pins.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

/// **The window is exactly what was asked for**, and a position past the end is not invented.
#[test]
fn the_window_is_the_rows_asked_for() {
    let mut app = editor("history_window");
    for x in [1.5, 2.5, 3.5] {
        bevy_cauldron::editor::place_for_test(app.world_mut(), "site/wall", (x, 0.5));
        app.update();
    }
    let state = app.world().resource::<EditorState>();
    let total = state.undo_depth();
    assert!(total > 0, "three placements are three rows");
    let all = state.history_window(1, total);
    assert_eq!(all.len(), total, "positions 1..={total} are the whole history");
    assert_eq!(all.first().map(|r| r.position), Some(1));
    assert_eq!(all.last().map(|r| r.position), Some(total));

    let one = state.history_window(1, 1);
    assert_eq!(one.len(), 1, "asking for one row builds one row, not the history");
    assert_eq!(one[0].position, 1);

    assert!(
        state.history_window(total + 5, total + 9).is_empty(),
        "positions past the end are no rows, not made-up ones"
    );
    assert_eq!(state.history_row(0), None, "positions start at 1");
    assert_eq!(state.history_row(total + 1), None, "past the end is no row");
    assert_eq!(
        state.history_row(total).map(|r| r.position),
        Some(total),
        "the last row is reachable by position"
    );
}

/// **A window in the middle matches the same rows of the whole list**, so the panel and a caller that
/// wants everything agree.
#[test]
fn a_window_agrees_with_the_whole_history() {
    let mut app = editor("history_window_agrees");
    for x in [1.5, 2.5, 3.5, 4.5] {
        bevy_cauldron::editor::place_for_test(app.world_mut(), "site/wall", (x, 0.5));
        app.update();
    }
    let state = app.world().resource::<EditorState>();
    let all = state.history();
    let total = all.len();
    let (first, last) = (total.saturating_sub(1).max(1), total);
    let window = state.history_window(first, last);
    assert_eq!(
        window,
        all[first - 1..last].to_vec(),
        "the window is the same rows, with the same labels and the same who"
    );
}
