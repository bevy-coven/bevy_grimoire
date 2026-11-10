//! **The outliner is a tree of everything on the map, not only its pieces, and its rows carry the two
//! toggles a tree row has** (UI-51, #237).
//!
//! The issue's What is "a tree of the map's contents: placements by layer and group, compositions, people,
//! triggers, notes. Includes search and visibility/lock toggles." The first slice built the placements,
//! grouped by kind, with the search and the selection. This file is the rest:
//!
//! - **Compositions, people, triggers and notes are groups of their own**, each holding the rows of that
//!   kind, so a map with a stamp, a person, a trigger and a note has all four in the tree beside its
//!   pieces — and a search reaches them too (`Mara` narrows to the people's group and that person).
//! - **A row's lock is the piece's pin**, through `placement_set_pin`, so the toggle is the same write the
//!   `P` key makes: one operation, one log line, one Cmd+Z. A row for a thing that is not a placement has
//!   no lock.
//! - **A row's eye is the piece's visibility in this session.** The map format has no "hidden" field and
//!   this slice does not add one (a hidden piece is view state, and a map file that says a wall is
//!   invisible is a map every consumer has to agree with); the eye takes a piece out of the viewport and
//!   forgets it when the editor closes.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, Placement};
use bevy_cauldron::outliner::{
    COMPOSITIONS, NOTES, PEOPLE, TRIGGERS, Outliner, OutlinerItem, OutlinerSearch, OutlinerTree, group_key,
    placement_key, set_eye_for_test, set_lock_for_test,
};
use bevy_cauldron::project::OpenMap;
use bevy_gooey::{Table, Tree, ValueChange};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// The groups the tree opens for what is not a piece, in the order the tree draws them.
const GROUP_KINDS: [&str; 4] = [COMPOSITIONS, PEOPLE, TRIGGERS, NOTES];

/// A project with a piece, a stamp, a person, a trigger and a note.
///
/// **The notes file is written before the editor opens** (its name follows the map's), so the view reads it
/// on its first look; **the two files the operations own** (`people/`, `scripts/`) go in the directory the
/// project itself names, because a test that guessed `assets/grimoire/` would be right only until a project
/// bound its library somewhere else. Writing them after the open is what a person editing a file on disk
/// does, and the tree is asked to rebuild the way a map edit asks it to.
fn rig(tag: &str) -> App {
    let root: PathBuf = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .composition("arch", &[("post_a", "site/wall", (0.0, 0.0)), ("post_b", "site/wall", (2.0, 0.0))])
        .place("site/wall", (1.0, 1.0))
        .build("m");
    let maps = root.join("assets/grimoire/maps");
    add_a_stamp(&maps.join("m.map.ron"));
    write_file(
        &maps.join("m.notes.ron"),
        "(version: 1, notes: [(id: \"lamp_note\", targets: [Cell(1, 1)], text: \"a lamp belongs here\", state: Open)])",
    );
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    let grimoire = app.world().resource::<bevy_cauldron::project::Project>().grimoire_dir.clone();
    write_file(&grimoire.join("people/m.ron"), "(version: 1, people: [(name: \"Mara\", at: (2.0, 2.0))])");
    write_file(
        &grimoire.join("scripts/m.ron"),
        "(version: 1, triggers: [(id: \"greet\", on: \"Activity(\\\"work\\\")\", then: \"()\")])",
    );
    // The section's first build reads all four files, so there is nothing to ask for.
    show_the_outliner(&mut app, true);
    app
}

fn write_file(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

/// **Put one stamp in the map file**, so the map the editor opens carries a composition.
fn add_a_stamp(map: &Path) {
    let text = std::fs::read_to_string(map).unwrap_or_else(|e| panic!("{map:?}: {e}"));
    let stamped = "(id: \"arch_here\", of: \"arch\", at: (3.0, 3.0), yaw: 0.0, overrides: [], of_fingerprint: None, note: None)";
    assert!(text.contains("stamps: []"), "the fixture's map has room for a stamp: {text}");
    write_file(map, &text.replacen("stamps: []", &format!("stamps: [{stamped}]"), 1));
}

fn show_the_outliner(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<OutlinerItem>>()
            .single(world)
            .expect("the View menu carries one Show outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

fn tree_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<OutlinerTree>>().single(world).expect("one outliner tree")
}

fn node_count(app: &mut App) -> usize {
    let entity = tree_entity(app);
    app.world().get::<Table>(entity).and_then(|t| t.source::<Tree>()).map_or(0, Tree::node_count)
}

fn labels(app: &mut App, keys: &[u64]) -> Vec<(u64, String)> {
    let entity = tree_entity(app);
    let table = app.world().get::<Table>(entity).expect("the tree is a table");
    let tree = table.source::<Tree>().expect("a tree");
    keys.iter().filter(|k| tree.contains(**k)).map(|k| (*k, tree.label(*k).unwrap_or_default().to_owned())).collect()
}

fn search_for(app: &mut App, words: &str) {
    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerSearch>>().single(world).expect("one search box")
    };
    app.world_mut().commands().trigger(ValueChange { source: search, value: words.to_owned(), is_final: false });
    updates(app, 3);
}

fn outliner(app: &mut App) -> &Outliner {
    let world = app.world_mut();
    world.query::<&Outliner>().single(world).expect("the outliner is open")
}

/// **The tree holds a group for every kind of thing on the map**, and a piece is still a row of its own.
#[test]
fn the_tree_holds_compositions_people_triggers_and_notes_beside_the_pieces() {
    let mut app = rig("outliner_everything");
    let keys: Vec<u64> = GROUP_KINDS.iter().map(|k| group_key(k)).chain([group_key("prop")]).collect();
    let held = labels(&mut app, &keys);
    assert_eq!(
        held.len(),
        5,
        "a group for the pieces, the compositions, the people, the triggers and the notes: {held:?}"
    );
    let order: Vec<&str> = held.iter().filter_map(|(k, _)| GROUP_KINDS.iter().find(|kind| group_key(kind) == *k).copied()).collect();
    assert_eq!(order, GROUP_KINDS, "the map's own groups are drawn in the issue's order: {held:?}");

    let piece = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let rows = labels(&mut app, &[placement_key(&piece)]);
    assert_eq!(rows.len(), 1, "the map's piece is still a row of its own: {rows:?}");
    assert!(rows[0].1.contains(&piece), "named by its id: {rows:?}");
}

/// **Each group holds the rows of its own kind**, named so a person can tell them apart.
#[test]
fn every_group_holds_the_things_of_its_kind() {
    let mut app = rig("outliner_group_rows");
    let outliner = app.world_mut().query::<&Outliner>().single(app.world()).expect("open");
    let children: Vec<Vec<u64>> = GROUP_KINDS.iter().map(|kind| outliner.rows_under(group_key(kind)).to_vec()).collect();
    assert_eq!(children.iter().map(Vec::len).collect::<Vec<_>>(), vec![1, 1, 1, 1], "one row under each: {children:?}");

    let labels: Vec<String> = children
        .iter()
        .flatten()
        .filter_map(|key| {
            let entity = {
                let world = app.world_mut();
                world.query_filtered::<Entity, With<OutlinerTree>>().single(world).ok()?
            };
            app.world().get::<Table>(entity)?.source::<Tree>()?.label(*key).map(str::to_owned)
        })
        .collect();
    assert!(labels.iter().any(|l| l.contains("arch_here")), "the stamp's row names it: {labels:?}");
    assert!(labels.iter().any(|l| l == "Mara"), "the person's row is their name: {labels:?}");
    assert!(labels.iter().any(|l| l == "greet"), "the trigger's row is its id: {labels:?}");
    assert!(labels.iter().any(|l| l.contains("a lamp belongs here")), "the note's row shows its words: {labels:?}");
}

/// **A search reaches the other groups too**: a word in a person's name leaves the people's group and that
/// person, and takes every group with nothing in it out.
#[test]
fn a_search_narrows_the_other_groups_the_way_it_narrows_the_pieces() {
    let mut app = rig("outliner_search_groups");
    let before = node_count(&mut app);
    assert!(before >= 9, "the whole tree: five groups and four rows under the others, was {before}");

    search_for(&mut app, "Mara");
    let keys: Vec<u64> = GROUP_KINDS.iter().map(|k| group_key(k)).chain([group_key("prop")]).collect();
    let held: Vec<u64> = labels(&mut app, &keys).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        held,
        vec![group_key(PEOPLE)],
        "the people's group and nothing else survives a search for a person: {held:?}"
    );

    search_for(&mut app, "");
    assert_eq!(node_count(&mut app), before, "a cleared search is the whole map again");
}

/// **A row's lock is the piece's pin**, written through the operation the `P` key writes.
#[test]
fn a_rows_lock_pins_its_piece_through_the_pin_operation() {
    let mut app = rig("outliner_lock");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    set_lock_for_test(app.world_mut(), placement_key(&id), true);
    updates(&mut app, 4);

    let map = app.world().resource::<OpenMap>().map.clone();
    let placed = map.placements.iter().find(|p| p.id == id).expect("the piece is where it was");
    assert!(placed.owned, "the row's lock pinned the piece");
    assert!(placed.owned_because.is_some(), "and a pin carries its reason");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 1, "one operation, so one Cmd+Z");

    set_lock_for_test(app.world_mut(), placement_key(&id), false);
    updates(&mut app, 4);
    let map = app.world().resource::<OpenMap>().map.clone();
    assert!(!map.placements.iter().any(|p| p.id == id && p.owned), "unlocking unpins it");
}

/// **A row that is not a piece offers neither toggle**: the people, the triggers and the notes are not
/// placements, so there is nothing to pin and nothing to draw.
#[test]
fn a_row_that_is_not_a_piece_offers_no_lock_and_no_eye() {
    let mut app = rig("outliner_toggles_none");
    let piece = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let outliner = outliner(&mut app);
    let person = outliner.rows_under(group_key(PEOPLE))[0];
    let note = outliner.rows_under(group_key(NOTES))[0];
    assert!(!outliner.toggles_offered(group_key(PEOPLE)), "a group is not a thing to pin");
    assert!(!outliner.toggles_offered(person), "a person is not a placement");
    assert!(!outliner.toggles_offered(note), "and a note is drawn nowhere");
    assert!(outliner.piece_of(placement_key(&piece)).is_some(), "while a piece's row names its piece");
    assert!(outliner.toggles_offered(placement_key(&piece)), "so a piece's row offers both");
}

/// **A row's eye hides its piece in this session**, and the map is not written by it.
#[test]
fn a_rows_eye_hides_the_piece_in_the_session_and_writes_nothing() {
    let mut app = rig("outliner_eye");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let dirty_before = app.world().resource::<OpenMap>().dirty();
    set_eye_for_test(app.world_mut(), placement_key(&id), false);
    updates(&mut app, 4);

    assert_eq!(app.world().resource::<OpenMap>().dirty(), dirty_before, "the map is not written by an eye");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 0, "and there is nothing to undo");
    let hidden = app.world().resource::<bevy_cauldron::outliner::Eyes>().hidden();
    assert_eq!(hidden, vec![id.clone()], "the eye hid that piece and no other: {hidden:?}");

    set_eye_for_test(app.world_mut(), placement_key(&id), true);
    updates(&mut app, 4);
    assert!(
        !app.world().resource::<bevy_cauldron::outliner::Eyes>().is_hidden(&id),
        "and the eye again shows it"
    );
}

/// **A hidden piece is not drawn in the viewport**, and showing it again brings it back.
#[test]
fn a_hidden_piece_is_not_drawn_and_showing_it_again_brings_it_back() {
    let mut app = rig("outliner_eye_draws");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let entity = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Placement)>();
        q.iter(world).find(|(_, p)| p.0 == id).map(|(e, _)| e).expect("the map drew the piece")
    };
    let drawn = |app: &mut App| {
        app.world().get::<Visibility>(entity).copied().unwrap_or(Visibility::Inherited) != Visibility::Hidden
    };
    assert!(drawn(&mut app), "a piece starts visible");
    set_eye_for_test(app.world_mut(), placement_key(&id), false);
    updates(&mut app, 4);
    assert!(!drawn(&mut app), "the eye takes it out of the viewport");
    set_eye_for_test(app.world_mut(), placement_key(&id), true);
    updates(&mut app, 4);
    assert!(drawn(&mut app), "and puts it back");
}
