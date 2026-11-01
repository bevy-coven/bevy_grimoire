//! **A generator's layout is drawn before it is kept** — B-046.
//!
//! # What was wrong
//!
//! `G` proposes a whole layout and then asks a yes/no question: *"289 piece(s) replacing 289 …
//! Enter keeps it, Esc throws it away"*. **The pieces were never drawn.** `draw_proposal` outlined
//! each proposed cell in a 1-px line on the ground plane, and that is all: measured on `town_block`
//! (289 placements, the map the bug was filed against), pressing `G` changed **3,779 of 1,014,944
//! viewport pixels — 0.37%** — and every one of them was a hairline under the magenta field. The
//! author was asked to keep or discard a layout they could not see.
//!
//! That is the brief's own rule, broken by the very operation it names: *"Operations that are
//! irreversible or wide in scope are previewed and need explicit confirmation before they commit."*
//!
//! # What is asserted
//!
//! The preview is the **pieces**, drawn through the editor's existing ghost mechanism (`Ghost` +
//! `fade_ghost`, the same one the armed brush uses), so what the author sees before `Enter` is the
//! same shapes and the same seatings they will get after it. Two tests:
//!
//! 1. A proposal spawns one ghosted entity per proposed piece — not merely an outline.
//! 2. `Esc` takes them all away, so a discarded layout leaves no ghosts behind.

use crate::fixtures;

use bevy::prelude::*;
use bevy_cauldron::editor::{Ghost, Proposal};
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::project::OpenMap;
use fixtures::Fixture;

/// A 6 x 6 map of a 1 m `crate` grid: enough cells that a learned grammar has something to fill and
/// small enough that the solve is instant.
fn project(name: &str) -> std::path::PathBuf {
    let mut f = Fixture::new(name).descriptor("crate", "alpha").side(6.0);
    for (i, x) in [0.5_f32, 1.5, 2.5, 3.5, 4.5].iter().enumerate() {
        for z in [0.5_f32, 1.5, 2.5] {
            f = f.place_as(&format!("seed{i}_{}", (z * 10.0) as i32), "crate", (*x, z));
        }
    }
    f.build("m")
}

/// Press one key, once, through the editor's own live-key path.
fn press_once(app: &mut App, key: KeyCode) {
    app.add_systems(
        Update,
        IntoScheduleConfigs::before(
            move |mut keys: ResMut<ButtonInput<KeyCode>>, mut done: Local<bool>| {
                if !*done {
                    keys.release_all();
                    keys.press(key);
                    *done = true;
                }
            },
            bevy_cauldron::keys::Phase::Act,
        ),
    );
    app.update();
}

/// Where each ghosted root stands, in the order the query returns them.
fn ghost_positions(app: &mut App) -> Vec<Vec3> {
    let mut q = app
        .world_mut()
        .query_filtered::<&Transform, With<Ghost>>();
    q.iter(app.world()).map(|t| t.translation).collect()
}

/// How many ghosted entities the world holds.
fn ghosts(app: &mut App) -> usize {
    let mut q = app.world_mut().query_filtered::<Entity, With<Ghost>>();
    q.iter(app.world()).count()
}

/// **The proposal is drawn.** One ghost per proposed piece, so the author is looking at the layout
/// rather than at a sentence about it.
#[test]
fn a_proposal_draws_the_pieces_before_enter_keeps_them() {
    let root = project("b046-proposal-is-drawn");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(ghosts(&mut app), 0, "nothing is proposed yet");

    press_once(&mut app, binding(Action::Generate).key);
    for _ in 0..3 {
        app.update();
    }

    let proposed = app
        .world()
        .resource::<Proposal>()
        .0
        .as_ref()
        .map(|p| p.placements.len() + p.stamps.len())
        .unwrap_or(0);
    assert!(
        proposed > 0,
        "the fixture's map must be generatable, or this test proves nothing"
    );
    assert_eq!(
        ghosts(&mut app),
        proposed,
        "every proposed piece must be on screen as a ghost before Enter — {proposed} \
         proposed, {} drawn",
        ghosts(&mut app)
    );

    // And nothing was committed: the map still holds exactly its five seed rows.
    let rows = app.world().resource::<OpenMap>().map.placements.len();
    assert_eq!(rows, 15, "a proposal must not change the map");
}

/// **`Esc` throws the layout away and the ghosts with it.**
#[test]
fn discarding_a_proposal_leaves_no_ghosts_behind() {
    let root = project("b046-discard-clears");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    press_once(&mut app, binding(Action::Generate).key);
    for _ in 0..3 {
        app.update();
    }
    assert!(ghosts(&mut app) > 0, "the proposal must be drawn first");

    press_once(&mut app, binding(Action::Cancel).key);
    for _ in 0..3 {
        app.update();
    }
    assert!(
        app.world().resource::<Proposal>().0.is_none(),
        "Esc must take the proposal away"
    );
    assert_eq!(ghosts(&mut app), 0, "and its ghosts with it");
}

/// **A proposal standing while the map moves is redrawn where the map now is.**
///
/// Found by review. The ghosts capture the map's origin once, at spawn; an agent's
/// `map_set_settings` (or a `Cmd+Z` of one, or a file edit the watcher adopts) moves every real piece
/// and every `draw_proposal` outline while they stay behind — and `Enter` then lays the pieces
/// somewhere the preview never showed. The ghost's own translation is the evidence.
#[test]
fn moving_the_map_under_a_proposal_moves_its_ghosts() {
    let root = project("b046-origin-moves");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    press_once(&mut app, binding(Action::Generate).key);
    for _ in 0..3 {
        app.update();
    }

    let before: Vec<Vec3> = ghost_positions(&mut app);
    assert!(!before.is_empty(), "the proposal must be drawn first");

    // The map moves 10 m along +X, the way an agent's `map_set_settings` does.
    app.world_mut().resource_mut::<OpenMap>().map.origin.0 += 10.0;
    for _ in 0..3 {
        app.update();
    }

    let after = ghost_positions(&mut app);
    assert_eq!(
        after.len(),
        before.len(),
        "the same pieces must still be previewed"
    );
    for (b, a) in before.iter().zip(&after) {
        assert!(
            (a.x - b.x - 10.0).abs() < 1e-3,
            "a ghost must follow the map it stands on: was at x {}, now x {}",
            b.x,
            a.x
        );
    }
}
