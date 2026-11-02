//! **The status band says how many pieces are in hand** (UI-34, #228).
//!
//! The band's left end answers the one question a person asks before a stamp: how many am I
//! carrying. Before this, the only answer was the ghost itself — counting boxes on the map, which
//! the acceptance calls out as the thing a status bar exists to replace.
//!
//! Two claims, and each can fail:
//!
//! 1. **A set in hand shows its count**, through the same `hold_set_for_test` the turn-key test
//!    uses, so this cannot pass on a hand-written string.
//! 2. **Nothing in hand shows nothing.** Not `"0 selected"`: a row that says the same thing on
//!    every frame is one a reader stops seeing, and the band already has a labeler line that says
//!    nothing until there is a model to name.
//!
//! The count is `pieces + stamps` — a stamp is a whole tile in hand, not one piece of it, and a
//! person carrying three tiles reads `3`, not the number of pieces those tiles hold.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    (root, app)
}

/// What the band currently says, or `None` when the readout has no text to say it with.
fn said(app: &mut App) -> Option<String> {
    let mut q = app
        .world_mut()
        .query_filtered::<&Text, With<bevy_cauldron::chrome::SelectionCount>>();
    q.iter(app.world()).next().map(|t| t.0.clone())
}

/// A set of `n` pieces at the anchor — the same shape the turn-key test builds.
fn set_of(n: usize) -> bevy_cauldron::editor::CloneSet {
    bevy_cauldron::editor::CloneSet {
        pieces: (0..n)
            .map(|i| bevy_cauldron::editor::ClonePiece {
                descriptor: "floor".to_owned(),
                offset: (i as f32 * 0.5, 0.0),
                yaw: 0.0,
                tip: (0, 0),
                lift: 0.0,
                note: None,
                owned: false,
                owned_because: None,
                patch: None,
                on: bevy_cauldron::editor::CloneHost::Layer,
            })
            .collect(),
        stamps: Vec::new(),
        centre_off: (0.0, 0.0),
        half: (0.5, 0.5),
        yaw: 0.0,
    }
}

/// **Nothing in hand says nothing**, so the band does not carry a permanent `0`.
#[test]
fn the_status_band_says_nothing_until_a_set_is_in_hand() {
    let (_root, mut app) = editor("status-empty");
    assert_eq!(
        said(&mut app).as_deref(),
        Some(""),
        "the band must carry the readout and have nothing to say with it; `Some(\"0 selected\")` \
         would be a row that reads the same on every frame"
    );
}

/// **Three pieces in hand read `3 selected`.**
#[test]
fn three_pieces_in_hand_read_as_three_selected() {
    let (_root, mut app) = editor("status-three");
    // **The Clone tool first**, or the editor puts the set down before the band can read it:
    // `clone_drag` clears `held` whenever `state.tool` is not `Tool::Clone`, which is the rule that
    // stops a set surviving a switch of tool. A test that skipped this would be asserting on a state
    // the editor refuses to hold.
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().tool =
        bevy_cauldron::editor::Tool::Clone;
    {
        let mut drag = app
            .world_mut()
            .resource_mut::<bevy_cauldron::editor::CloneDrag>();
        bevy_cauldron::editor::hold_set_for_test(set_of(3), &mut drag);
    }
    for _ in 0..3 {
        app.update();
    }
    // **What the band *sees*, before what it says.** If the resource has the set and the readout is
    // still empty, the fault is the system's; if the resource is empty, the fault is the test's.
    let held = app
        .world()
        .get_resource::<bevy_cauldron::editor::CloneDrag>()
        .expect("the editor has a CloneDrag")
        .held_pieces();
    assert_eq!(
        said(&mut app).as_deref(),
        Some("3 selected"),
        "the band should read the set in hand; the resource says {held:?}"
    );
}
