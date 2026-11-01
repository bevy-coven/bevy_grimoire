//! **The planted forest is drawn with the crate's own trees, and follows its files** (#102).
//! `cargo test -p bevy_cauldron --features bevy_flora --test editor forest_view::`

#![cfg(feature = "bevy_flora")]

use bevy::prelude::*;
use bevy_cauldron::forest_view::ForestView;
use bevy_cauldron::ops::{self, Ctx};
use bevy_cauldron::ops_forest::forest_of;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_flora::{PooledTree, ProceduralTree, TreeAge};
use serde_json::json;

use crate::fixtures;

fn open(tag: &str) -> (App, Ctx, Project) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    (app, Ctx { project: Some(project.clone()), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, project)
}

/// Step the app until `done` holds; the view looks at its files twice a second.
fn until(app: &mut App, what: &str, done: impl Fn(&mut World) -> bool) {
    for _ in 0..60 {
        app.update();
        if done(app.world_mut()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("never happened: {what}");
}

fn drawn(world: &mut World) -> usize {
    world.query::<&ForestView>().iter(world).count()
}

fn placed_radius(world: &mut World) -> Option<f32> {
    world.query_filtered::<&ProceduralTree, With<ForestView>>().iter(world).next().map(|t| t.0.trunk.radius)
}

#[test]
fn every_tree_in_the_file_is_drawn_where_it_stands_and_follows_the_files() {
    let (mut app, mut ctx, project) = open("forest_view");
    ops::dispatch(&mut ctx, "tree_edit", &json!({"name": "tall", "preset": "birch", "set": {"trunk.radius": 0.3}})).unwrap();
    ops::dispatch(
        &mut ctx,
        "forest_scatter",
        &json!({"map": "m", "plant": {"points": [[0.0, 0.0], [12.0, 0.0]], "radius_m": 4.0, "spacing_m": 3.0, "seed": 3, "species": [{"name": "oak", "weight": 1.0}]}}),
    )
    .unwrap();
    ops::dispatch(&mut ctx, "forest_scatter", &json!({"map": "m", "place": {"species": "trees/tall.tree.ron", "at": [20.0, 5.0], "age_years": 12.0, "seed": 7}})).unwrap();
    let forest = forest_of(&project, "m");
    let scattered = forest.trees.iter().filter(|t| t.seed.is_none()).count();
    assert!(scattered >= 2, "the stroke planted a few oaks: {forest:?}");
    until(&mut app, "the forest is drawn", |w| drawn(w) == forest.trees.len());

    let origin = app.world().resource::<OpenMap>().map.origin;
    let drop = bevy_cauldron::editor::ground_drop(&project);
    let world = app.world_mut();
    let mut seen = 0;
    for (view, at, pooled, grown, age) in world.query::<(&ForestView, &Transform, Option<&PooledTree>, Option<&ProceduralTree>, Option<&TreeAge>)>().iter(world) {
        let tree = forest.trees.iter().find(|t| t.id == view.id).expect("a drawn tree is a tree in the file");
        let want = Vec3::new(origin.0 + tree.at.0, origin.1 + tree.ground_m - drop, origin.2 + tree.at.1);
        assert!(at.translation.distance(want) < 1e-4, "tree {} stands at {} not {want}", tree.id, at.translation);
        assert!((at.scale - Vec3::splat(tree.scale)).length() < 1e-5, "tree {} is scaled by the file", tree.id);
        let yaw = Quat::from_rotation_y(tree.yaw);
        assert!(at.rotation.dot(yaw).abs() > 1.0 - 1e-6, "tree {} is turned by its yaw {}: {:?} not {yaw:?}", tree.id, tree.yaw, at.rotation);
        match tree.seed {
            None => {
                let pooled = pooled.expect("a scattered tree is drawn from the crate's pool");
                assert_eq!(pooled.variant, tree.variant);
                assert!(grown.is_none());
            }
            Some(seed) => {
                let grown = grown.expect("a placed tree is grown by the crate");
                assert_eq!(grown.0.seed, seed, "the tree's own seed grows it, so it is the game's tree");
                assert_eq!(grown.0.trunk.radius, 0.3, "its species is the project's .tree.ron");
                assert_eq!(age.map(|a| a.0), tree.age_years);
                assert!(pooled.is_none());
            }
        }
        seen += 1;
    }
    assert_eq!(seen, forest.trees.len());

    // A tree edit regrows the tree that uses the file.
    ops::dispatch(&mut ctx, "tree_edit", &json!({"name": "tall", "set": {"trunk.radius": 0.9}})).unwrap();
    until(&mut app, "the placed tree regrows from the edited file", |w| placed_radius(w) == Some(0.9));
    assert_eq!(drawn(app.world_mut()), forest.trees.len());

    // An undo of the placed tree takes it out of the viewport too.
    ops::undo(&mut ctx, None).unwrap();
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(forest_of(&project, "m").trees.len(), scattered, "the second undo took the placed tree back");
    until(&mut app, "the undone tree is gone", |w| drawn(w) == scattered && placed_radius(w).is_none());

    // A forest file broken by hand keeps the trees drawn rather than emptying the viewport.
    std::fs::write(project.grimoire_dir.join("forest/m.ron"), "(trees: [(id: 0,").unwrap();
    for _ in 0..20 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(drawn(app.world_mut()), scattered, "a file that does not parse leaves the drawing as it was");
}
