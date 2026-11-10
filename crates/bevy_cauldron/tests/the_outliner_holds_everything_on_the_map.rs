//! **The outliner is a tree of everything on the map, not only its pieces, and a piece row's menu pins it or
//! hides it** (UI-51, #237).
//!
//! The issue's What is "a tree of the map's contents: placements by layer and group, compositions, people,
//! triggers, notes. Includes search and visibility/lock toggles." The first slice built the placements, grouped
//! by kind, with the search and the selection. This file is the rest:
//!
//! - **Compositions, people, triggers and notes are groups of their own**, each holding the rows of that kind,
//!   so a map with a stamp, a person, a trigger and a note has all four in the tree beside its pieces — and a
//!   search reaches them too (`Mara` narrows to the people's group and that person).
//! - **A piece row's menu carries Pin and Hide.** Pin is the piece's `placement_set_pin` through
//!   `editor::pin_or_unpin` — the same function the `O` key's prompt goes through, so a pin **asks for its
//!   reason** in the field the editor already has, and the reason is the person's own. Hide takes the piece out
//!   of the viewport for this session ([`Eyes`]) and writes nothing: the map format has no "hidden" field.
//! - **A row that is not a piece has no menu at all.** The tree's rows are a pool, so the menu is taken off a
//!   row rebound to a group or to something that is not a placement.
//!
//! Every one of these goes through the real widget path: the row's `ContextMenu`, the panel's `ContextTarget`
//! and the item's `Activate` — the same three things a right-click delivers. Nothing here calls the outliner's
//! own state directly, so deleting the menu would fail these tests.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::editor::{EditorState, Placement};
use bevy_cauldron::outliner::{
    COMPOSITIONS, Eyes, NOTES, PEOPLE, TRIGGERS, Outliner, OutlinerItem, OutlinerSearch, OutlinerTree, group_key,
    placement_key,
};
use bevy_cauldron::project::OpenMap;
use bevy_gooey::{ContextMenu, ContextTarget, MenuItem, MenuPanel, Table, TableRow, Tree, ValueChange};

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
/// **The notes file is written before the editor opens** (its name follows the map's), so the view reads it on
/// its first look; **the two files the operations own** (`people/`, `scripts/`) go in the directory the project
/// itself names, because a test that guessed `assets/grimoire/` would be right only until a project bound its
/// library somewhere else. The project is marked changed after they are written, which is what a person editing
/// a file on disk does, so the tree looks again.
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

/// **A tree row's entity**, by the key it shows: the widget's own answer, because the rows are a pool.
fn row_entity(app: &mut App, key: u64) -> Entity {
    let world = app.world_mut();
    let mut rows = world.query::<(Entity, &TableRow)>();
    rows.iter(world).find(|(_, row)| row.key == key).map(|(entity, _)| entity).expect("the row is drawn")
}

/// **Choose `label` from the menu a right-click on the row `key` opens**, through the real path: the row names
/// the menu (`ContextMenu`), the panel is given the row it was opened for (`ContextTarget`), and the item takes
/// the `Activate` the pointer or Enter sends.
fn choose(app: &mut App, key: u64, label: &str) {
    let row = row_entity(app, key);
    assert!(
        app.world().entity(row).contains::<ContextMenu>(),
        "the row {key:x} names the row menu a right-click opens"
    );
    // **The outliner's own menu**, named by the row: the editor has a menu panel per thing that offers one.
    let panel = {
        let world = app.world_mut();
        let named = world.entity(row).get::<ContextMenu>().expect("the row names a menu").0;
        assert!(world.entity(named).contains::<MenuPanel>(), "the row's menu is a menu panel");
        named
    };
    app.world_mut().entity_mut(panel).insert(ContextTarget(row));
    let item = {
        let world = app.world_mut();
        let mut texts = world.query::<(&Text, &ChildOf)>();
        let mut items = world.query_filtered::<(), With<MenuItem>>();
        let found: Vec<Entity> = texts
            .iter(world)
            .filter(|(t, p)| t.0 == label && items.get(world, p.parent()).is_ok())
            .map(|(_, p)| p.parent())
            .collect();
        assert_eq!(found.len(), 1, "one menu item says `{label}`: {found:?}");
        found[0]
    };
    app.world_mut().commands().trigger(Activate { entity: item });
    updates(app, 4);
}

/// **The tree holds a group for every kind of thing on the map**, and a piece is still a row of its own.
#[test]
fn the_tree_holds_compositions_people_triggers_and_notes_beside_the_pieces() {
    // A person and a trigger are files the project reads on a change, so the fixture writes them before the
    // section opens; the stamp and the note are in the map the editor opened.
    let mut app = rig("outliner_everything");
    let keys: Vec<u64> = GROUP_KINDS.iter().map(|k| group_key(k)).chain([group_key("prop")]).collect();
    let held = labels(&mut app, &keys);
    assert_eq!(
        held.len(),
        5,
        "a group for the pieces, the compositions, the people, the triggers and the notes: {held:?}"
    );
    let order: Vec<&str> = held
        .iter()
        .filter_map(|(k, _)| GROUP_KINDS.iter().find(|kind| group_key(kind) == *k).copied())
        .collect();
    assert_eq!(order, GROUP_KINDS, "the map's own groups are drawn in the issue's order: {held:?}");

    let piece = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let rows = labels(&mut app, &[placement_key(&piece)]);
    assert_eq!(rows.len(), 1, "the map's piece is still a row of its own: {rows:?}");
    assert!(rows[0].1.contains(&piece), "named by its id: {rows:?}");
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

/// **A row's Pin is the piece's pin, and it asks for its reason**, as the `O` key's prompt does: the toggle puts
/// the piece in the editor's pin field, and the reason the person types is what the pin carries.
#[test]
fn a_rows_pin_asks_for_its_reason_and_pins_the_piece_through_the_pin_operation() {
    let mut app = rig("outliner_lock");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    // The person and trigger files were written before the section opened, so the tree is one build.
    choose(&mut app, placement_key(&id), "Pin");

    // **A pin waits for its reason**: nothing is pinned yet, and nothing is on the undo stack.
    let map = app.world().resource::<OpenMap>().map.clone();
    assert!(!map.placements.iter().any(|p| p.id == id && p.owned), "a pin waits for its reason");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 0, "and nothing was written");

    // The reason, typed the way the person types it at the field; Enter commits it.
    bevy_cauldron::editor::type_the_pin_reason_for_test(app.world_mut(), "holds the doorway");
    updates(&mut app, 4);
    let map = app.world().resource::<OpenMap>().map.clone();
    let placed = map.placements.iter().find(|p| p.id == id).expect("the piece is where it was");
    assert!(placed.owned, "the reason committed the pin");
    assert_eq!(placed.owned_because.as_deref(), Some("holds the doorway"), "and the reason is the person's own");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 1, "one operation, so one Cmd+Z");

    // Pin again unpins at once: an unpin has no reason to ask for.
    choose(&mut app, placement_key(&id), "Pin");
    let map = app.world().resource::<OpenMap>().map.clone();
    assert!(!map.placements.iter().any(|p| p.id == id && p.owned), "unpinning is immediate");
}

/// **A row that is not a piece names no menu at all**: the people, the triggers and the notes are not
/// placements, so their rows offer neither Pin nor Hide — and so does a group row.
#[test]
fn a_row_that_is_not_a_piece_has_no_menu() {
    let mut app = rig("outliner_toggles_none");
    let piece = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let person = outliner(&mut app).rows_under(group_key(PEOPLE))[0];
    let note = outliner(&mut app).rows_under(group_key(NOTES))[0];
    let named = |app: &mut App, key: u64| {
        let entity = row_entity(app, key);
        app.world().entity(entity).contains::<ContextMenu>()
    };
    assert!(named(&mut app, placement_key(&piece)), "a piece's row names the menu");
    assert!(!named(&mut app, group_key(PEOPLE)), "the people's group does not: nothing to pin, nothing to draw");
    assert!(!named(&mut app, person), "nor a person's row");
    assert!(!named(&mut app, note), "nor a note's");
}

/// **A row's Hide takes the piece out of the viewport for this session**, and the map is not written by it.
#[test]
fn a_rows_hide_takes_the_piece_out_of_the_session_and_writes_nothing() {
    let mut app = rig("outliner_eye");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let dirty_before = app.world().resource::<OpenMap>().dirty();
    choose(&mut app, placement_key(&id), "Hide");

    assert_eq!(app.world().resource::<OpenMap>().dirty(), dirty_before, "the map is not written by an eye");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 0, "and there is nothing to undo");
    let hidden = app.world().resource::<Eyes>().hidden();
    assert_eq!(hidden, vec![id.clone()], "the eye hid that piece and no other: {hidden:?}");

    choose(&mut app, placement_key(&id), "Hide");
    assert!(!app.world().resource::<Eyes>().is_hidden(&id), "and the eye again shows it");
}

/// **A hidden piece is not drawn in the viewport**, and hiding it again brings it back.
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
    choose(&mut app, placement_key(&id), "Hide");
    updates(&mut app, 2);
    assert!(!drawn(&mut app), "the eye takes it out of the viewport");
    choose(&mut app, placement_key(&id), "Hide");
    updates(&mut app, 2);
    assert!(drawn(&mut app), "and puts it back");
}

/// **The eye is reset at a door**: another map's rows are not the ids it holds, so opening one shows everything.
#[test]
fn the_eye_does_not_outlive_the_map() {
    let mut app = rig("outliner_eye_door");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    choose(&mut app, placement_key(&id), "Hide");
    assert!(app.world().resource::<Eyes>().is_hidden(&id), "the piece is behind the eye");
    bevy_cauldron::screen::reset_door_state_for_test(app.world_mut());
    assert!(app.world().resource::<Eyes>().is_empty(), "and a door puts every eye back");
}
