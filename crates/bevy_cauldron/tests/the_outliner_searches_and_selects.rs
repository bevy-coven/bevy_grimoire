//! **The outliner lists the map's pieces by what they are, a search narrows it, and a row selects its piece** (UI-51, #237).
//!
//! Three pieces on a map: two lamps (kind `light-source`) and a chair (kind `prop`). Searching `lamp` leaves the two lamps
//! under their group and nothing else; a row clicked selects its piece as the primary; several rows select several; a
//! group row selects nothing; and a piece selected in the viewport is the selected row.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_cauldron::editor::{TargetLock, lock_on_placement};
use bevy_cauldron::outliner::{OutlinerItem, OutlinerSearch, OutlinerTree, group_key, placement_key};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::selection::Selection;
use bevy_gooey::{Checked, Table, TableSelection, Tree, ValueChange};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// The editor on a map of two lamps and a chair. Returns the app and the three ids, lamps first.
fn rig(tag: &str) -> (App, [String; 3]) {
    let root: PathBuf = fixtures::Fixture::new(tag)
        .kind_tokens(&["light-source"])
        .descriptor("lamp_tall", "alpha")
        .descriptor("lamp_short", "alpha")
        .descriptor("chair", "alpha")
        .place("lamp_tall", (1.0, 1.0))
        .place("lamp_short", (2.0, 1.0))
        .place("chair", (3.0, 1.0))
        .build("m");
    // The fixture gives every descriptor the kind `prop`; the lamps are lights.
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.yaml");
    fixtures::edit_library(&library, |lib| {
        for lamp in ["lamp_tall", "lamp_short"] {
            let row = lib.descriptors.iter_mut().find(|d| d.id == lamp).unwrap_or_else(|| panic!("no `{lamp}` in the library"));
            row.kind = vec!["light-source".to_owned()];
        }
    });

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    show_the_outliner(&mut app, true);
    let ids: Vec<String> = app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).collect();
    let [a, b, c]: [String; 3] = ids.try_into().expect("three pieces on the map");
    (app, [a, b, c])
}

/// The person chose View > Show outliner (or chose it again).
fn show_the_outliner(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("the View menu carries one Show outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

fn tree_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<OutlinerTree>>().single(world).expect("one outliner tree")
}

/// Everything the tree holds, as `(key, label)`, found by asking it for each key the test knows.
fn rows(app: &mut App, keys: &[u64]) -> Vec<(u64, String)> {
    let entity = tree_entity(app);
    let table = app.world().get::<Table>(entity).expect("the tree is a table");
    let tree = table.source::<Tree>().expect("a tree");
    keys.iter().filter(|k| tree.contains(**k)).map(|k| (*k, tree.label(*k).unwrap_or_default().to_owned())).collect()
}

fn node_count(app: &mut App) -> usize {
    let entity = tree_entity(app);
    app.world().get::<Table>(entity).and_then(|t| t.source::<Tree>()).map_or(0, Tree::node_count)
}

fn search_for(app: &mut App, words: &str) {
    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerSearch>>().single(world).expect("one search box")
    };
    app.world_mut().commands().trigger(ValueChange { source: search, value: words.to_owned(), is_final: false });
    updates(app, 3);
}

fn click_rows(app: &mut App, keys: &[u64]) {
    let tree = tree_entity(app);
    let value = TableSelection(keys.iter().copied().collect());
    app.world_mut().commands().trigger(ValueChange { source: tree, value, is_final: true });
    updates(app, 3);
}

#[test]
fn the_tree_groups_the_pieces_by_what_they_are() {
    let (mut app, [lamp_a, lamp_b, chair]) = rig("outliner_groups");
    assert_eq!(node_count(&mut app), 5, "two groups and three pieces");
    let keys = [group_key("light-source"), group_key("prop"), placement_key(&lamp_a), placement_key(&lamp_b), placement_key(&chair)];
    let held = rows(&mut app, &keys);
    assert_eq!(held.len(), 5, "each key names a row: {held:?}");
    assert!(held.iter().find(|(k, _)| *k == placement_key(&chair)).is_some_and(|(_, label)| label.contains("chair")), "a piece's row names its descriptor: {held:?}");
}

#[test]
fn searching_lamp_leaves_the_lamps_and_the_group_that_holds_them() {
    let (mut app, [lamp_a, lamp_b, chair]) = rig("outliner_search");
    search_for(&mut app, "lamp");
    let keys = [group_key("light-source"), group_key("prop"), placement_key(&lamp_a), placement_key(&lamp_b), placement_key(&chair)];
    let held: Vec<u64> = rows(&mut app, &keys).into_iter().map(|(k, _)| k).collect();
    assert_eq!(held, vec![keys[0], keys[2], keys[3]], "the light-source group and both lamps, and not the chair or its group");
    assert_eq!(node_count(&mut app), 3);

    search_for(&mut app, "lamp tall");
    assert_eq!(node_count(&mut app), 2, "every word has to match: one lamp under its group");

    search_for(&mut app, "zzz");
    assert_eq!(node_count(&mut app), 0, "nothing matches, so no group is drawn either");

    search_for(&mut app, "");
    assert_eq!(node_count(&mut app), 5, "a cleared search is the whole map again");
}

#[test]
fn a_row_selects_its_piece_and_a_group_selects_nothing() {
    let (mut app, [lamp_a, lamp_b, chair]) = rig("outliner_select");
    click_rows(&mut app, &[placement_key(&chair)]);
    assert_eq!(app.world().resource::<Selection>().ids(), std::slice::from_ref(&chair), "the row's piece is the selection");
    assert_eq!(app.world().resource::<TargetLock>().placement_id(), Some(chair.as_str()), "and the primary, which the POSE block and the viewport read");

    click_rows(&mut app, &[placement_key(&lamp_b), placement_key(&lamp_a)]);
    let selected = app.world().resource::<Selection>().ids().to_vec();
    assert_eq!(selected.len(), 2, "two rows select two pieces: {selected:?}");
    assert_eq!(selected[0], lamp_a, "the primary is the first in the map's order, whichever row was clicked first");
    assert_eq!(app.world().resource::<TargetLock>().placement_id(), Some(lamp_a.as_str()));

    click_rows(&mut app, &[group_key("prop")]);
    assert_eq!(app.world().resource::<Selection>().ids().to_vec(), selected, "a group row names no piece, so the selection stays");
}

#[test]
fn a_piece_selected_in_the_viewport_is_the_selected_row() {
    let (mut app, [lamp_a, _lamp_b, chair]) = rig("outliner_follow");
    assert!(lock_on_placement(app.world_mut(), &chair));
    app.world_mut().resource_mut::<Selection>().set(vec![chair.clone()]);
    updates(&mut app, 4);
    let tree = tree_entity(&mut app);
    let selected = app.world().get::<TableSelection>(tree).expect("the tree shows a selection").clone();
    assert_eq!(selected, TableSelection::of(placement_key(&chair)), "the chair's row is the selected one");

    // A search that leaves the piece out of the tree leaves no row selected rather than a ghost.
    search_for(&mut app, "lamp");
    let selected = app.world().get::<TableSelection>(tree).cloned().unwrap_or_default();
    assert!(!selected.contains(placement_key(&chair)), "the chair is not in the searched tree: {selected:?}");
    assert!(!selected.contains(placement_key(&lamp_a)));
}

#[test]
fn the_outliner_is_not_there_until_the_view_menu_asks_and_goes_when_it_is_asked_again() {
    let root = fixtures::Fixture::new("outliner_menu").descriptor("site/wall", "alpha").place("site/wall", (1.0, 1.0)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    let count = |app: &mut App| {
        let world = app.world_mut();
        (world.query_filtered::<(), With<OutlinerTree>>().iter(world).count(), world.query_filtered::<(), With<OutlinerSearch>>().iter(world).count())
    };
    let ticked = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<Has<Checked>, With<OutlinerItem>>().single(world).expect("one row")
    };
    assert_eq!(count(&mut app), (0, 0), "nothing is there until it is asked for: no tree and no search box among the screens that count them");
    assert!(!ticked(&mut app));

    show_the_outliner(&mut app, true);
    assert_eq!(count(&mut app), (1, 1), "asked for, there is one tree and one search box");
    assert!(ticked(&mut app), "and the row is ticked");
    show_the_outliner(&mut app, true);
    assert_eq!(count(&mut app), (1, 1), "asking twice does not make two");

    show_the_outliner(&mut app, false);
    assert_eq!(count(&mut app), (0, 0), "and asked away it goes");
    assert!(!ticked(&mut app));
}
