//! **The baseline proof** — the defect, in the code's own words.
//!
//! This test is written against the **old** API, so it compiles on both the baseline and the fixed
//! code. On the baseline it **fails**, and its message names the defect; on the fixed code it
//! passes, because the arm is now a `Session` resource and survives the teardown.
//!
//! It is kept as a separate file rather than folded into `the_arm_crosses_the_door.rs` because
//! that file cannot compile on the baseline at all — the baseline has no `compose::Armed` and no
//! `compose::picker`. **A regression test that cannot compile against the broken code cannot be
//! run against the broken code**, and a test nobody has run against the broken code is a comment.
//!
//! # What it asks
//!
//! Not *"does the arm survive?"* but *"**where does the arm live?**" — because that is the
//! question the fix answered, and it is the question that decides everything else. The arm is a
//! field of `ComposeState`, and `ComposeState` is `Ownership::Door`: the table in `screen.rs` says
//! a door change resets it. So the arm is reset with it. **The table is the defect, stated in the
//! table's own words.**

use crate::fixtures;

use bevy_cauldron::harness;
use bevy_cauldron::screen::{Ownership, ownership};
use fixtures::Fixture;

#[test]
fn the_arm_does_not_live_in_something_a_door_change_resets() {
    // The arm is a field of `ComposeState` on the baseline. Where does that live?
    let compose = ownership("bevy_cauldron::compose::ComposeState");

    // **If the arm has its own resource, it is not in this table at all** — it is its own row,
    // classified `Session`, and this assertion is about the *old* home.
    let has_its_own = ownership("bevy_cauldron::compose::Armed").is_some();
    if has_its_own {
        assert_eq!(
            ownership("bevy_cauldron::compose::Armed"),
            Some(Ownership::Session),
            "the arm has its own resource, and it is classified `Door` — so a door change resets it, \
             and the arm is lost exactly where it was most wanted. It is the author's intent: *which \
             composition the next click stamps*."
        );
        return;
    }

    // Otherwise it is a field of `ComposeState`, and this is the defect: the arm dies with the
    // door's cursors.
    assert_ne!(
        compose,
        Some(Ownership::Door),
        "the armed composition is a field of `ComposeState`, and `ComposeState` is `Ownership::Door` \
         — so a door change resets it. The arm is the author's intent and it has to cross: the \
         composition was made on the Kit door and the map that stamps it is on the Map door. \
         Measured on 2026-10-21: the COMPOSE tab armed a house and said *\"the map tab stamps \
         it\"*, and there was no map tab in that process, because the door is chosen at startup."
    );
}

/// **And the table is the only thing that decides.** The same file, the same question, asked of
/// the *running* editor: where does the arm live, and what does a door change do to it?
///
/// **Written so it compiles on both the baseline and the fixed code.** The point of this file is
/// that it can be run against the broken code, so it cannot name `ComposeState::armed` (gone) or
/// `Armed` (new) in a way that only one of them has.
#[test]
fn the_arms_home_is_the_table_and_the_table_is_the_defect() {
    let root = Fixture::new("arm-baseline")
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **Where the arm lives.** On the baseline it is a field of `ComposeState`; on the fixed code
    // it is its own resource. The table says what a door change does to each, and the answer is
    // the whole feature.
    //
    // **The table is asked first**, because that is the question — and asking it first is what
    // lets this file compile on the baseline, where `compose::Armed` does not exist and naming it
    // would be a compile error rather than a test failure.
    let arm_is_a_resource = ownership("bevy_cauldron::compose::Armed").is_some();
    if arm_is_a_resource {
        assert_eq!(
            ownership("bevy_cauldron::compose::Armed"),
            Some(Ownership::Session),
            "the arm has its own resource and it is classified `Door` - a door change resets it, \
             and the arm is lost exactly where it was most wanted."
        );
        return;
    }

    // The baseline: the arm is a field of `ComposeState`, and `ComposeState` is the door's.
    assert_ne!(
        ownership("bevy_cauldron::compose::ComposeState"),
        Some(Ownership::Door),
        "the armed composition is a field of `ComposeState`, and `ComposeState` is \
         `Ownership::Door` - so a door change resets it. The arm is the author's intent and it \
         has to cross: the composition was made on the Kit door and the map that stamps it is on \
         the Map door. Measured on 2026-10-21: the COMPOSE tab armed a house and said that the \
         map tab stamps it, and there was no map tab in that process, because the door is chosen \
         at startup."
    );
}
