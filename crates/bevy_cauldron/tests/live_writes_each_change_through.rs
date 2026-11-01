//! **`--live` writes every change through to disk, and without it an unsaved change stays unsaved.**
//!
//! A running game watches the project's files, so an edit reaches it only when it is on disk. The
//! registered operations already save as they commit; the keys that are not yet operations (lift,
//! tip, move, straighten, the editor's own undo) change the map in memory and leave it unsaved. The
//! write-through runs in `PostUpdate`, after the frame's edits, through the same `OpenMap::save` the
//! save key uses, so those edits reach the game too.

use bevy::prelude::*;
use bevy_grimoire_core::map::Map;
use bevy_cauldron::harness;
use bevy_cauldron::live::LiveWrite;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

const ID: &str = "crate_0";

fn editor(name: &str, live: bool) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place_as(ID, "site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    if live {
        app.insert_resource(LiveWrite);
    }
    for _ in 0..8 {
        app.update();
    }
    app
}

/// Lift the placement in place, as the lift keys do (`editor::lift_under_cursor`): the map changes
/// in memory and is left unsaved.
fn lift(app: &mut App, metres: f32) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let Some(placed) = open.map.placements.iter_mut().find(|p| p.id == ID) else {
        panic!("`{ID}` is not in the open map");
    };
    placed.lift = metres;
    assert!(open.dirty(), "an in-place edit leaves the map unsaved");
}

fn lift_on_disk(app: &App) -> f32 {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Map::parse(&text)
        .unwrap_or_else(|e| panic!("{e}"))
        .placements
        .iter()
        .find(|p| p.id == ID)
        .map(|p| p.lift)
        .unwrap_or_else(|| panic!("`{ID}` is not on disk"))
}

#[test]
fn live_writes_each_change_through() {
    let mut app = editor("live_on", true);
    assert_eq!(lift_on_disk(&app), 0.0);
    lift(&mut app, 0.25);
    app.update();
    assert_eq!(lift_on_disk(&app), 0.25, "the lift did not reach the file");
    assert!(
        !app.world().resource::<OpenMap>().dirty(),
        "the map was written, so it is not unsaved"
    );
}

#[test]
fn without_live_a_change_stays_in_memory_until_saved() {
    let mut app = editor("live_off", false);
    lift(&mut app, 0.25);
    app.update();
    assert_eq!(lift_on_disk(&app), 0.0, "the file changed without --live");
    assert!(app.world().resource::<OpenMap>().dirty());
}
