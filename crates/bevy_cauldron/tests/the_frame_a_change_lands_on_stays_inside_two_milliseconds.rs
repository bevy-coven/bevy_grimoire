//! **A change costs the UI less than 2 ms on every frame it touches, at 10,000 outliner nodes and 5,000 history lines**
//! (UI-93, #259).
//!
//! The issue's acceptance is "the test fails when the UI systems exceed 2 ms per frame on the reference fixture". A settled
//! frame was held to that already (`the_uis_budget_is_two_milliseconds_a_frame.rs`); the frame a change lands on rebuilt both
//! lists whole and cost 3.8 ms on `bmb` and 6.6 ms on `big`, so `the_ui_frame_covers_a_change_and_the_input_path.rs` holds it
//! to the floor with headroom. The lists now take the one row a change names (`Tree::insert`, `Tree::relabel`), and this
//! file holds the frames after a change to the issue's own number, with nothing added for the machine but the best of
//! [`CHANGES`] changes: a floor is a minimum, and a loaded machine adds to a frame and never takes from one.
//!
//! **Every frame from the change to the settled frame is held to it**, not only the frame that did the work: the UI systems
//! are the editor's own two (the outliner's and the history's) and gooey's tree and table drawing, all in
//! [`UiFrame`]'s total. A frame is the worst of the ones the change touched.
//!
//! The reference fixture is the one the other budget files use. Like them this is a wall-clock test, so it is in
//! `.config/nextest.toml`'s list of tests that run alone.

use bevy::prelude::*;
use bevy_cauldron::editor::{self, EditorState};
use bevy_cauldron::outliner::{Outliner, OutlinerItem};
use bevy_cauldron::ui_budget::{MAX_MS_PER_FRAME, UiFrame};
use bevy_gooey::ValueChange;

use crate::fixtures;

/// The reference fixture's history depth: the issue's 5,000 rows.
const HISTORY_ROWS: usize = 5_000;
/// The reference fixture's outliner size: the issue's 10,000 nodes.
const OUTLINER_NODES: usize = 10_000;
/// How many changes the measurement takes the best of, so a frame that met another build, a page fault or the gate on this
/// machine cannot decide the answer.
const CHANGES: usize = 15;
/// How many frames after a change are measured: the change, the frame gooey draws it in, and the ones that must be quiet.
const FRAMES_AFTER_A_CHANGE: usize = 4;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **The reference fixture**: a map of `OUTLINER_NODES` pieces, an outliner showing all of them, and a history of
/// `HISTORY_ROWS` entries.
fn rig(tag: &str) -> App {
    let mut fixture = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).side(400.0).descriptor("site/wall", "alpha");
    for i in 0..OUTLINER_NODES {
        fixture = fixture.place_as(&format!("wall@{i}"), "site/wall", ((i % 200) as f32 * 2.0, (i / 200) as f32 * 2.0));
    }
    let root = fixture.build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("View offers Show outliner")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 8);
    {
        let mut state = app.world_mut().resource_mut::<EditorState>();
        for _ in 0..HISTORY_ROWS {
            editor::plant_history_entry_for_test(&mut state);
        }
    }
    updates(&mut app, 4);
    app
}

/// **What the UI systems cost on each frame from a change to the settled frame, the worst of them**, milliseconds, and the
/// frames in which the editor's lists did their work.
fn worst_frame_after_a_change(app: &mut App, change: impl FnOnce(&mut World)) -> f64 {
    change(app.world_mut());
    let mut worst = 0.0_f64;
    for _ in 0..FRAMES_AFTER_A_CHANGE {
        app.update();
        worst = worst.max(app.world().resource::<UiFrame>().last_ms);
    }
    worst
}

fn patches(app: &mut App) -> u32 {
    let world = app.world_mut();
    world.query::<&Outliner>().single(world).expect("the outliner is open").patches()
}

/// **A piece placed on a ten-thousand-piece map, and a line added to a five-thousand-line history, cost the UI under 2 ms a
/// frame.** The outliner takes the one row, the history takes the one line and the one it relabels, and gooey's own drawing
/// patches the rows in view: none of them reads the 10,000 pieces again to find a place for one.
#[test]
fn a_piece_placed_costs_the_ui_under_two_milliseconds_a_frame_at_ten_thousand_nodes() {
    let mut app = rig("the_frame_a_change_lands_on_place");
    let before = patches(&mut app);
    let mut best = f64::INFINITY;
    for n in 0..CHANGES {
        let at = (500.0 + n as f32 * 2.0, 500.0);
        let worst = worst_frame_after_a_change(&mut app, |world| editor::place_for_test(world, "site/wall", at));
        best = best.min(worst);
        updates(&mut app, 3);
    }
    assert_eq!(patches(&mut app) - before, CHANGES as u32, "every piece placed reached the outliner as a patch, not a whole build");
    assert!(
        best < MAX_MS_PER_FRAME,
        "the UI cost {best:.3} ms on the worst frame after a piece was placed, of {HISTORY_ROWS} history lines and {OUTLINER_NODES} \
         outliner nodes — over the {MAX_MS_PER_FRAME} ms a frame may spend of Nielsen Norman Group's 0.1 s"
    );
}

/// **An undo and a redo cost the UI under 2 ms a frame as well**: the history relabels the two lines the editor moved
/// between, and the outliner takes a piece out and puts it back.
#[test]
fn an_undo_and_a_redo_cost_the_ui_under_two_milliseconds_a_frame_at_ten_thousand_nodes() {
    let mut app = rig("the_frame_a_change_lands_on_undo");
    editor::place_for_test(app.world_mut(), "site/wall", (500.0, 500.0));
    updates(&mut app, 4);
    let mut best_undo = f64::INFINITY;
    let mut best_redo = f64::INFINITY;
    for _ in 0..CHANGES {
        best_undo = best_undo.min(worst_frame_after_a_change(&mut app, editor::undo_for_test));
        updates(&mut app, 3);
        best_redo = best_redo.min(worst_frame_after_a_change(&mut app, editor::redo_for_test));
        updates(&mut app, 3);
    }
    // An undo takes the piece out of the tree, and `Tree::remove` reads the tree's order again: gooey's own cost, which is
    // what this is allowed beyond the history's two relabelled lines. The redo puts it back as an insert.
    assert!(best_redo < MAX_MS_PER_FRAME, "a redo cost the UI {best_redo:.3} ms on its worst frame, over {MAX_MS_PER_FRAME} ms");
    assert!(best_undo < MAX_MS_PER_FRAME, "an undo cost the UI {best_undo:.3} ms on its worst frame, over {MAX_MS_PER_FRAME} ms");
}
