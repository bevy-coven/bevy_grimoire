//! **The strip's "the other door" is a control, not a caption** (#288).
//!
//! The strip beside the tabs says which door is the other one ("Kit" on the Map door) and
//! `=` crosses to it, but a click on the words did nothing: a label that names a place and cannot be
//! clicked is the one that gets tried. It runs what the key runs.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Door;
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("barrel", "alpha")
        .place_as("barrel@0", "barrel", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Click the name of the other door and you are there; click the name again and you are back.**
#[test]
fn a_click_on_the_other_doors_name_crosses_to_it() {
    let mut app = editor("other_door_click");
    assert_eq!(*app.world().resource::<Door>(), Door::Map, "the editor opens on the Map door");

    let kit = find_by_label(&mut app, "Kit").expect("the Map door's strip names the Kit door");
    click(&mut app, kit);
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Door>(), Door::Kit, "a click on \"the kit door\" opens the Kit door");

    // "Map" is also a menu's title, so the strip's own name is found by what it is.
    let map = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_cauldron::tiles::OtherDoorLabel>>()
        .single(app.world())
        .expect("the Kit door's strip names the Map door");
    click(&mut app, map);
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Door>(), Door::Map, "and the other way");
}
