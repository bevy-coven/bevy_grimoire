//! **A composed proposal is drawn piece by piece** — E-061.
//!
//! # What was wrong
//!
//! B-046 made `G` draw every proposed piece as a translucent ghost, so the author sees the layout
//! before `Enter` keeps it. A **composed** proposal (`Shift+G`, `layout_propose {source: composed}`)
//! lays stamps rather than placements, and that arm was left as a comment: *"stamps are not drawn
//! piece by piece … drawing its members here would duplicate that expansion's placement rules."* The
//! author was given a cell outline and a count for a layout they could not see — the same defect one
//! source over.
//!
//! # What is asserted
//!
//! The fix draws the members through `composition::expand` and `stamped_heights` — the expansion and
//! the seating `layout_accept`'s keep lands through — so the ghosts are the rows `Enter` will lay:
//!
//! 1. one ghost per expansion row, and none before the proposal;
//! 2. the count is the expanded member count, not the stamp count;
//! 3. `Esc` takes them away with the proposal.

use crate::fixtures;

use bevy::prelude::*;
use bevy_cauldron::editor::{Ghost, Proposal};
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::project::OpenMap;

/// Two one-cell bounded compositions, each of **two** members — so a stamp's expansion is a number
/// this test can tell apart from the stamp count.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .descriptor("crate", "alpha")
        .bounded_composition(
            "tile_floor",
            (1.0, 1.0, 1.0),
            &[("base", "floor", (0.0, 0.0)), ("onit", "crate", (0.0, 0.0))],
        )
        .bounded_composition(
            "tile_crate",
            (1.0, 1.0, 1.0),
            &[("base", "floor", (0.0, 0.0)), ("onit", "crate", (0.0, 0.0))],
        )
        .place("floor", (0.5, 0.5))
        .place("crate", (0.5, 0.5))
        .build("m")
}

/// Press one key, once, through the editor's own live-key path.
fn press_once(app: &mut App, key: KeyCode) {
    press_with(app, &[], key);
}

/// Press `mods` and `key` together — `just_pressed` refuses a binding whose modifier state does not
/// match exactly, so the composed source needs the platform modifier held.
fn press_with(app: &mut App, mods: &[KeyCode], key: KeyCode) {
    let mods = mods.to_vec();
    app.add_systems(
        Update,
        IntoScheduleConfigs::before(
            move |mut keys: ResMut<ButtonInput<KeyCode>>, mut done: Local<bool>| {
                if !*done {
                    keys.release_all();
                    for m in &mods {
                        keys.press(*m);
                    }
                    keys.press(key);
                    *done = true;
                }
            },
            bevy_cauldron::keys::Phase::Act,
        ),
    );
    app.update();
}

fn ghosts(app: &mut App) -> usize {
    let mut q = app.world_mut().query_filtered::<Entity, With<Ghost>>();
    q.iter(app.world()).count()
}

/// **The composed proposal's members are on screen before `Enter`.**
#[test]
fn a_composed_proposal_draws_every_expanded_member() {
    let root = project("e061-composed-drawn");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(ghosts(&mut app), 0, "nothing is proposed yet");

    press_with(&mut app, &bevy_cauldron::keys::MOD_KEYS[..1], binding(Action::GenerateComposed).key);
    for _ in 0..3 {
        app.update();
    }

    let proposal = app
        .world()
        .resource::<Proposal>()
        .0
        .as_ref()
        .cloned()
        .expect("the fixture's map must be generative as compositions");
    assert!(
        proposal.composes,
        "this test is about a composed proposal, and the key did not make one"
    );
    assert!(
        !proposal.stamps.is_empty(),
        "a composed proposal that stamps nothing proves nothing"
    );

    // The rows `Enter` will lay, computed the way the keep computes them.
    let open = app.world().resource::<OpenMap>();
    let project = app.world().resource::<bevy_cauldron::project::Project>();
    let mut landed = open.map.clone();
    // **Copied out before the queries below**, because `ghosts` needs `&mut App` and the map is
    // read again after it.
    let (rows_before, stamps_before) = (open.map.placements.len(), open.map.stamps.len());
    landed.stamps.retain(|s| s.owned);
    landed.stamps.extend(proposal.stamps.iter().cloned());
    let expanded = bevy_grimoire_core::composition::expand(
        &landed,
        &landed.stamps,
        &project.compositions.compositions,
        &project.library,
    )
    .unwrap_or_else(|e| panic!("the proposal must expand: {e}"));
    let expanded_rows = expanded.placements.len();
    assert!(
        expanded_rows > proposal.stamps.len(),
        "the fixture's compositions must carry more than one member each, or drawing {} stamps and \
         {} rows is the same picture",
        proposal.stamps.len(),
        expanded_rows
    );

    let drawn = ghosts(&mut app);
    assert_eq!(
        drawn, expanded_rows,
        "every member the proposal would stamp must be drawn as a ghost — {expanded_rows} rows \
         expand, {drawn} drawn"
    );

    // And nothing was committed: the map still holds its own two rows and no stamps.
    assert_eq!(rows_before, 2, "a proposal must not change the map");
    assert_eq!(stamps_before, 0, "a proposal must not stamp anything");
}

/// **`Esc` takes the composed ghosts away with the proposal.**
#[test]
fn discarding_a_composed_proposal_leaves_no_ghosts_behind() {
    let root = project("e061-composed-discard");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    press_with(&mut app, &bevy_cauldron::keys::MOD_KEYS[..1], binding(Action::GenerateComposed).key);
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
    assert_eq!(
        ghosts(&mut app),
        0,
        "and the ghosts must go with it, or a discarded layout leaves pieces on screen"
    );
}
