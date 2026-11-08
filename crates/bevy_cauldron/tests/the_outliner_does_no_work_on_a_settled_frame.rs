//! **The outliner stays inside the UI's budget: a settled frame does no work that grows with the map** (UI-93, #259).
//!
//! A tree of two thousand pieces is built once when it opens, again when a piece comes or goes or the search changes, and not at
//! all while nothing moves; and the widget that shows it is a screenful of entities however long the list is (gooey's virtualized
//! `tree`). The first version rebuilt a fingerprint of every piece on every frame, which at ten thousand pieces and sixty frames
//! a second is a second of string building a person would never see the reason for. A wall-clock bound is not what holds this
//! (the gate runs under load, and a timing assertion there fails for reasons that are not the code: #322); the count of builds
//! is the number the budget is written against, and it is exact.

use bevy::prelude::*;
use bevy_cauldron::editor::place_for_test;
use bevy_cauldron::outliner::{Outliner, OutlinerItem, OutlinerSearch, OutlinerTree};
use bevy_gooey::{Table, Tree, ValueChange};

use crate::fixtures;

/// How many pieces the reference map holds. Big enough that an O(n) cost on every frame is not subtle.
const PIECES: usize = 2_000;
/// How many entities the tree widget and its rows may be, however many pieces there are: a header-less table's pool.
const WIDGET_ENTITIES_MAX: usize = 60;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn rig() -> App {
    let mut fixture = fixtures::Fixture::new("outliner_budget").side(400.0).descriptor("site/wall", "alpha");
    for i in 0..PIECES {
        let (x, z) = ((i % 50) as f32 * 2.0 + 1.0, (i / 50) as f32 * 2.0 + 1.0);
        fixture = fixture.place("site/wall", (x, z));
    }
    let root = fixture.build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("the View menu carries the outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    app
}

fn rebuilds(app: &mut App) -> u32 {
    let world = app.world_mut();
    world.query::<&Outliner>().single(world).expect("the outliner is open").rebuilds()
}

fn nodes(app: &mut App) -> usize {
    let world = app.world_mut();
    let table = world.query_filtered::<&Table, With<OutlinerTree>>().single(world).expect("one tree");
    table.source::<Tree>().map_or(0, Tree::node_count)
}

#[test]
fn a_settled_map_and_search_cost_no_rebuild_and_a_change_costs_one() {
    let mut app = rig();
    assert_eq!(nodes(&mut app), PIECES + 1, "the pieces under their one group");
    let built = rebuilds(&mut app);
    assert_eq!(built, 1, "the tree was built once when the section opened");

    updates(&mut app, 120);
    assert_eq!(rebuilds(&mut app), built, "a hundred and twenty settled frames rebuild nothing");

    place_for_test(app.world_mut(), "site/wall", (399.0, 399.0));
    updates(&mut app, 4);
    assert_eq!(rebuilds(&mut app), built + 1, "a piece added is one rebuild");
    assert_eq!(nodes(&mut app), PIECES + 2);

    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerSearch>>().single(world).expect("one search box")
    };
    app.world_mut().commands().trigger(ValueChange { source: search, value: "wall".to_owned(), is_final: false });
    updates(&mut app, 4);
    assert_eq!(rebuilds(&mut app), built + 2, "a search is one rebuild");
    updates(&mut app, 60);
    assert_eq!(rebuilds(&mut app), built + 2, "and holding it costs none");
}

#[test]
fn the_tree_widget_is_a_screenful_of_entities_however_long_the_list() {
    let mut app = rig();
    let tree = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerTree>>().single(world).expect("one tree")
    };
    let mut count = 0;
    let mut stack = vec![tree];
    while let Some(at) = stack.pop() {
        count += 1;
        if let Some(kids) = app.world().get::<Children>(at) {
            stack.extend(kids.iter());
        }
    }
    assert!(count <= WIDGET_ENTITIES_MAX, "{PIECES} pieces are drawn by {count} entities, not one per piece");
}
