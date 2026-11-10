//! **The UI budget's second half: the frame after a change, gooey's own drawing, and the path from a keystroke to the
//! rows** (UI-93, #259).
//!
//! `the_uis_budget_is_two_milliseconds_a_frame.rs` holds a *settled* frame to [`MAX_MS_PER_FRAME`] and only reports the
//! frame a change lands on. The issue asks for more, and this file is that more, on the same reference fixture (5,000
//! history rows, 10,000 outliner nodes):
//!
//! - **the frame after a change is held to a measured number** — a piece arrives on a ten-thousand-piece map, the
//!   outliner and the history rebuild, and the systems that followed the content cost [`MAX_CHANGE_FRAME_MS`] at most, on
//!   the worst of several changes. The 2 ms of a settled frame is not reachable on a frame that rebuilds 10,000 outliner
//!   rows and 5,000 history lines (the floor measured on `bmb` is 3.8 ms, see below), so the number asserted is the floor
//!   with headroom, and gooey's own share of that frame is held to the 2 ms;
//! - **gooey's own table and tree drawing is in the clock**, not only the editor's two systems, because the virtualized
//!   lists are what a long session spends its frames on;
//! - **a typed search reaches the rows inside the 0.1 s** Nielsen Norman Group names, measured from the keystroke to the
//!   frame the narrowed tree is drawn in.
//!
//! **What the 0.1 s covers here, and what it does not.** A headless test has no display, so the picture's present and the
//! monitor's scan-out are not in the number; everything the editor itself does between the input and the frame that would
//! be presented is. It is the editor's share of the response time, named as that.

use std::time::Instant;

use bevy::prelude::*;
use bevy_cauldron::editor::{self, EditorState};
use bevy_cauldron::outliner::{OutlinerItem, OutlinerSearch, OutlinerTree};
use bevy_cauldron::ui_budget::{UiFrame, MAX_MS_PER_FRAME};
use bevy_gooey::{Table, Tree, ValueChange};

use crate::fixtures;

/// The reference fixture's history depth: the issue's 5,000 rows.
const HISTORY_ROWS: usize = 5_000;
/// The reference fixture's outliner size: the issue's 10,000 nodes.
const OUTLINER_NODES: usize = 10_000;
/// How many changes the frame-after-a-change measurement takes the worst of, so one unlucky frame — a page fault, another
/// test on this machine — cannot decide the answer.
const CHANGES: usize = 5;
/// **The most the panels may cost on the frame a change lands on, milliseconds.** The measured floor on `bmb` (2026-11-10,
/// the optimized test profile) is 3.8 ms: 2.7 ms the outliner's rebuild (a fingerprint 0.35 ms, `nodes_for` 1.5 ms, gooey's
/// `set_nodes` 0.7 ms), 1.0 ms the history's 5,000 labels, 0.1 ms gooey's own drawing. It was 5.4 ms before this pass took
/// three descriptor lookups, a haystack, a label and a kind copy per unfiltered piece out of the outliner (4.3 ms there).
/// 6 ms is the floor with the headroom a slower machine needs; a rebuild that went quadratic in the content fails it by far.
const MAX_CHANGE_FRAME_MS: f64 = 6.0;
/// **Nielsen Norman Group's 0.1 s**, milliseconds: under it a response needs no indicator and feels instant.
const INPUT_TO_VISUAL_MS: f64 = 100.0;
/// The most frames a keystroke may take to reach the tree: the change, the rebuild, and one to draw it.
const MOST_FRAMES_TO_REACH_THE_TREE: usize = 4;

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
        let mut q = world.query::<(Entity, &OutlinerItem)>();
        q.iter(world).map(|(e, _)| e).next().expect("View offers Show outliner")
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

fn tree_nodes(app: &mut App) -> usize {
    let world = app.world_mut();
    let table = world.query_filtered::<&Table, With<OutlinerTree>>().single(world).expect("one tree");
    table.source::<Tree>().map_or(0, Tree::node_count)
}

/// **What the panels cost on the frame a piece arrives on**: the total and gooey's share of it, milliseconds.
fn frame_after_a_change(app: &mut App, at: (f32, f32)) -> (f64, f64) {
    editor::place_for_test(app.world_mut(), "site/wall", at);
    let mut worst = (0.0_f64, 0.0_f64);
    let mut rebuilt = false;
    for _ in 0..4 {
        app.update();
        let frame = *app.world().resource::<UiFrame>();
        if frame.last_rebuilds > 0 {
            rebuilt = true;
            worst = (worst.0.max(frame.last_ms), worst.1.max(frame.last_gooey_ms));
        }
    }
    assert!(rebuilt, "a piece added is a rebuild of the tree, and the frame says so");
    worst
}

/// **The frame a change lands on is held to the number it costs, and gooey's share of it to the 2 ms.** The frame
/// rebuilds 10,000 outliner rows and 5,000 history lines, which a settled frame never does; see [`MAX_CHANGE_FRAME_MS`]
/// for what it costs and why it is not the settled 2 ms. gooey's own tree and table drawing on that frame — what the
/// virtualized lists spend — is inside [`MAX_MS_PER_FRAME`] on its own.
#[test]
fn the_frame_after_a_change_stays_inside_its_measured_budget_at_ten_thousand_nodes() {
    let mut app = rig("ui_budget_change_frame");
    let mut worst = (0.0_f64, 0.0_f64);
    for n in 0..CHANGES {
        let at = (500.0 + n as f32 * 2.0, 500.0);
        let (ms, gooey) = frame_after_a_change(&mut app, at);
        worst = (worst.0.max(ms), worst.1.max(gooey));
        updates(&mut app, 3);
    }
    assert!(
        worst.0 < MAX_CHANGE_FRAME_MS,
        "the panels cost {:.3} ms on the frame a change landed on, of {HISTORY_ROWS} history rows and {OUTLINER_NODES} \
         outliner nodes — over the {MAX_CHANGE_FRAME_MS} ms this frame is measured at",
        worst.0
    );
    assert!(
        worst.1 < MAX_MS_PER_FRAME,
        "gooey's own table and tree drawing cost {:.3} ms on the frame a change landed on — over the {MAX_MS_PER_FRAME} ms \
         the UI may spend of a 100 ms response",
        worst.1
    );
}

/// **gooey's own table and tree drawing is in the clock.** The editor's two timed systems say what the editor's code
/// costs; the virtualized lists are drawn by gooey's `update_trees` and `update_tables`, which a long session spends its
/// frames in. A clock that left them out would pass a UI whose lists were slow.
#[test]
fn gooeys_own_table_and_tree_drawing_is_in_the_frame_clock() {
    let mut app = rig("ui_budget_gooey_clock");
    updates(&mut app, 3);
    let frame = *app.world().resource::<UiFrame>();
    assert!(frame.last_gooey_ms > 0.0, "gooey's table and tree systems were timed in the frame that finished: {frame:?}");
    assert!(
        frame.last_ms >= frame.last_gooey_ms,
        "the frame's total includes gooey's share ({} ms of {} ms): {frame:?}",
        frame.last_gooey_ms,
        frame.last_ms
    );
}

/// **A typed search reaches the tree inside a tenth of a second.** The keystroke is the search box's change; the clock
/// runs from before it is sent until the frame the narrowed tree stands in has finished, over ten thousand pieces.
#[test]
fn a_typed_search_reaches_the_tree_inside_a_tenth_of_a_second() {
    let mut app = rig("ui_budget_input_path");
    let before = tree_nodes(&mut app);
    assert_eq!(before, OUTLINER_NODES + 1, "the tree holds every piece under its one group");
    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerSearch>>().single(world).expect("one search box")
    };
    let started = Instant::now();
    app.world_mut().commands().trigger(ValueChange { source: search, value: "wall@12".to_owned(), is_final: false });
    let mut frames = 0;
    while tree_nodes(&mut app) == before && frames < MOST_FRAMES_TO_REACH_THE_TREE {
        app.update();
        frames += 1;
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let narrowed = tree_nodes(&mut app);
    assert!(narrowed < before, "the search narrowed the tree within {MOST_FRAMES_TO_REACH_THE_TREE} frames ({before} -> {narrowed})");
    assert!(
        elapsed_ms < INPUT_TO_VISUAL_MS,
        "a keystroke took {elapsed_ms:.1} ms and {frames} frame(s) to reach the tree, over the {INPUT_TO_VISUAL_MS} ms of \
         Nielsen Norman Group's 0.1 s (the editor's share: no present, no display)"
    );
}
