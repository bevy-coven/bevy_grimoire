//! **The strip's door cue reads as a switch, not as another tab** — `feedback-1793928006124`, oracle `spec`.
//!
//! The note, *"The "Map / Kit" toggles get confused with the Meshes, Tiles, Compose tabs"*, came with a frame
//! whose strip reads `1 MESHES · 2 TILES · 3 COMPOSE · Map`: the cue to the other door is one bare word on the
//! tab row's baseline, and the pick's rect is exactly that word. The strip is correct (the sibling test in
//! `capture_feedback_1793928006124.rs` passes on the base commit); what is wrong is that the cue says only
//! the destination's name, which is also a tab's grammar. The spec: the words the cue draws say that it
//! switches, and still name the other workspace in a node of its own.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::OtherWorkspaceMark;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("barrel", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// Every word the cue draws, in order, joined by a space.
fn cue_words(app: &mut App) -> String {
    let world = app.world_mut();
    let mark = world.query_filtered::<Entity, With<OtherWorkspaceMark>>().single(world).expect("one door cue");
    let kids: Vec<Entity> = world.get::<Children>(mark).map(|c| c.iter().collect()).unwrap_or_default();
    kids.into_iter()
        .filter_map(|k| world.get::<Text>(k).map(|t| t.0.clone()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_door_cue_says_it_switches_on_both_doors() {
    let mut app = editor("capture_1793928006124_switch");
    assert_eq!(cue_words(&mut app), "Switch to Kit", "on the Map door the cue is an action naming the Kit door");

    bevy_cauldron::screen::cross(app.world_mut(), Workspace::Kit).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(cue_words(&mut app), "Switch to Map", "on the Kit door the word beside the tabs is not a bare `Map`");
}
