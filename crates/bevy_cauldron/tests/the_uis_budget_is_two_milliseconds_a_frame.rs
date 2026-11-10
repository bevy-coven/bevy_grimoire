//! **The UI's update budget: what the panels cost on a frame where nothing happened** (UI-93, #259).
//!
//! `docs/ui/vision.md` P6 takes Nielsen Norman Group's 0.1 s — *"under 0.1 s needs no indicator and feels instant"* — as the
//! ceiling for input to visual. This measures the editor's share of it: the systems that read content, on a frame where
//! nothing changed, with 5,000 history rows and 10,000 outliner nodes standing.
//!
//! **A whole frame's time is not the editor's cost**, and the number here is not one. The harness, the terminal, the
//! thumbnails and this machine's other work all land in a frame, and the empty editor's own frame on `bmb` is ~4.3 ms before
//! any panel has content (measured 2026-11-09, this worktree) — a budget written against that fails for reasons that are not
//! the panels. So the two systems whose work grows with what a panel shows time themselves into `ui_budget::UiFrame`, and the
//! test reads that: the panels' own cost, against [`MAX_MS_PER_FRAME`] (2 ms of the 100 ms budget).
//!
//! **The content is the issue's own**: 10,000 placements on the map and 5,000 entries on the undo stack, put there the way
//! an author's session does. Both panels show a window rather than their contents — the history list is nine rows and gooey's
//! tree is virtualized — so what these numbers prove is that the panels do not pay for the depth as it grows.

use bevy::prelude::*;
use bevy_cauldron::editor::{self, EditorState, HistorySlot};
use bevy_cauldron::outliner::{Outliner, OutlinerItem, OutlinerTree};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::ui_budget::{self, UiFrame, MAX_MS_PER_FRAME};
use bevy_gooey::{Table, Tree, ValueChange};

use crate::fixtures;

/// The reference fixture's history depth: the issue's 5,000 rows.
const HISTORY_ROWS: usize = 5_000;
/// The reference fixture's outliner size: the issue's 10,000 nodes.
const OUTLINER_NODES: usize = 10_000;
/// How many settled frames the measurement averages over. Enough that one unlucky frame — a page fault, another test on
/// this machine — cannot decide the answer.
const SETTLED_FRAMES: usize = 20;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **The reference fixture**: a map of `OUTLINER_NODES` pieces, an outliner showing all of them, and a history stack of
/// `HISTORY_ROWS` entries.
fn rig(tag: &str) -> App {
    let mut fixture = fixtures::Fixture::new(tag).side(400.0).descriptor("site/wall", "alpha");
    for i in 0..OUTLINER_NODES {
        fixture = fixture.place_as(&format!("wall@{i}"), "site/wall", ((i % 200) as f32 * 2.0, (i / 200) as f32 * 2.0));
    }
    let root = fixture.build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    // The outliner section, as View ▸ Show outliner puts it there.
    let row = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &OutlinerItem)>();
        q.iter(world).map(|(e, _)| e).next().expect("View offers Show outliner")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 8);
    build_the_history(&mut app);
    updates(&mut app, 4);
    app
}

/// **5,000 real undo entries**, pushed through the one method every edit site records with, so the history panel is looking at
/// the depth an author's session would have rather than at a number written into a struct.
fn build_the_history(app: &mut App) {
    {
        let mut state = app.world_mut().resource_mut::<EditorState>();
        for _ in 0..HISTORY_ROWS {
            editor::plant_history_entry_for_test(&mut state);
        }
    }
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), HISTORY_ROWS, "the history is the depth the budget is written against");
}

/// **The worst settled frame's panel cost**, in milliseconds: the largest `last_ms` over `SETTLED_FRAMES` frames with nothing
/// moving. The worst rather than the mean, because a budget a person feels is about the frame that stutters.
fn worst_settled_ms(app: &mut App) -> f64 {
    updates(app, 2);
    let mut worst: f64 = 0.0;
    for _ in 0..SETTLED_FRAMES {
        app.update();
        let frame = *app.world().resource::<UiFrame>();
        assert!(frame.last_systems >= 1, "at least one content system timed itself in the frame that finished: {frame:?}");
        worst = worst.max(frame.last_ms);
    }
    worst
}

/// **The number the issue asks for**: on 5,000 history rows and 10,000 outliner nodes, a frame where nothing changed spends
/// less than the UI's share of the 0.1 s budget in the systems that read that content.
#[test]
fn a_settled_frame_on_the_reference_fixture_stays_under_the_ui_budget() {
    let mut app = rig(&format!("ui_budget_settled_{}", std::process::id()));
    let worst = worst_settled_ms(&mut app);
    let frame = *app.world().resource::<UiFrame>();
    assert!(
        worst < MAX_MS_PER_FRAME,
        "the panels cost {worst:.3} ms on the worst settled frame, of {HISTORY_ROWS} history rows and {OUTLINER_NODES} \
         outliner nodes — over the {MAX_MS_PER_FRAME} ms the UI may spend of a 100 ms response ({frame:?}). Something on this \
         frame is rebuilding a list it did not have to; docs/ui/vision.md P6 takes Nielsen Norman Group's 0.1 s."
    );
    assert!(frame.within_budget(), "the resource's own gate agrees with the number the test just read: {frame:?}");
}

/// **And the panels are showing what they were given**, so the number above measures real content rather than two empty
/// widgets.
#[test]
fn the_reference_fixture_has_the_content_the_budget_is_written_against() {
    let mut app = rig(&format!("ui_budget_content_{}", std::process::id()));
    let nodes = {
        let world = app.world_mut();
        let table = world.query_filtered::<&Table, With<OutlinerTree>>().single(world).expect("one tree");
        table.source::<Tree>().map_or(0, Tree::node_count)
    };
    assert_eq!(nodes, OUTLINER_NODES + 1, "every piece on the map, under its one group");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), HISTORY_ROWS, "and the history stack is at depth");
    assert_eq!(app.world().resource::<OpenMap>().map.placements.len(), OUTLINER_NODES, "the map holds the pieces the outliner lists");
    // The window each panel draws is a screenful, not one row per entry.
    let slots = app.world_mut().query_filtered::<(), With<HistorySlot>>().iter(app.world()).count();
    assert!(slots > 0 && slots < 20, "the history panel holds a window's worth of rows, not one row per entry: {slots}");
}

/// **A change costs one rebuild, and the frames after it cost none.** This is the half of the budget a person feels: the frame
/// they act on may do the work, and the frames after it may not.
#[test]
fn one_outliner_change_rebuilds_once_and_the_frames_after_it_do_not() {
    let mut app = rig(&format!("ui_budget_change_{}", std::process::id()));
    let rebuilds = |app: &mut App| app.world_mut().query_filtered::<&Outliner, ()>().iter(app.world()).next().map(|o| o.rebuilds()).unwrap_or(0);
    let before = rebuilds(&mut app);
    // A piece arrives, which is the change the outliner exists to follow.
    editor::place_for_test(app.world_mut(), "site/wall", (500.0, 500.0));
    updates(&mut app, 4);
    let after = rebuilds(&mut app);
    assert_eq!(after, before + 1, "one change is one rebuild of the tree ({before} -> {after})");
    updates(&mut app, 30);
    assert_eq!(rebuilds(&mut app), after, "thirty settled frames rebuild nothing ({after} -> {})", rebuilds(&mut app));
}

/// **The gate can fail.** The same predicate that passes the editor refuses a frame over the number, so the assertion above
/// is a proof rather than a report.
#[test]
fn the_budget_gate_refuses_a_frame_over_the_number() {
    let over = UiFrame { last_ms: MAX_MS_PER_FRAME + 1.0, last_systems: 1, ..default() };
    assert!(!over.within_budget(), "a frame over the number is refused: {over:?}");
    let under = UiFrame { last_ms: MAX_MS_PER_FRAME - 1.0, last_systems: 1, ..default() };
    assert!(under.within_budget(), "and one under it passes: {under:?}");
    assert_eq!(MAX_MS_PER_FRAME, 2.0, "2 ms of Nielsen Norman Group's 0.1 s, which is the number the issue names");
    // The clock adds a system's cost and counts it, which is what makes `last_ms` the panels' own work.
    let mut frame = UiFrame::default();
    let started = ui_budget::begin();
    ui_budget::end(started, &mut frame);
    assert_eq!(frame.systems, 1);
    assert!(frame.ms >= 0.0);
}

/// **The clock sees a frame's real work, not just a tick.** A rebuild of the reference fixture's tree — a change a person made,
/// which the budget allows — is measured by the same instrument, so the number the test above reads is the panels' cost and the
/// gate would fire if that work happened on a frame where nothing moved. This is the measurement that makes the assertion
/// above a gate rather than a formality.
#[test]
fn the_clock_is_the_panels_own_work_not_a_tick() {
    let mut app = rig(&format!("ui_budget_clock_{}", std::process::id()));
    let settled = worst_settled_ms(&mut app);
    // A piece arrives: the outliner rebuilds ten thousand nodes, which is exactly the work the budget forbids on a settled
    // frame.
    editor::place_for_test(app.world_mut(), "site/wall", (500.0, 500.0));
    let mut rebuilt: f64 = 0.0;
    for _ in 0..4 {
        app.update();
        rebuilt = rebuilt.max(app.world().resource::<UiFrame>().last_ms);
    }
    assert!(rebuilt > settled, "a frame that rebuilds ten thousand nodes costs more than a settled one: {rebuilt:.3} vs {settled:.3} ms");
    assert!(
        rebuilt > MAX_MS_PER_FRAME,
        "and that rebuild is over the {MAX_MS_PER_FRAME} ms budget ({rebuilt:.3} ms), which is what the gate above refuses a \
         settled frame for: the same work would happen on every frame if the panel rebuilt without a change"
    );
}
