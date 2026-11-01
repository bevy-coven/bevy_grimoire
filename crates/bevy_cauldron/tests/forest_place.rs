//! **A tree placed by hand has an age and a seed, stands on the terrain, and the same seed grows the same tree** (#102).
//! `cargo test -p bevy_cauldron --features bevy_flora --test editor forest_place::`

#![cfg(feature = "bevy_flora")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::ops_forest::forest_of;
use bevy_cauldron::project::Project;
use serde_json::json;

use crate::fixtures;

fn open(tag: &str) -> (Ctx, Project) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project.clone()), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, project)
}

fn place(species: &str, x: f32, seed: i32) -> serde_json::Value {
    json!({"map": "m", "place": {"species": species, "at": [x, 0.0], "age_years": 40.0, "seed": seed}})
}

#[test]
fn a_placed_tree_keeps_its_age_and_seed_and_stands_on_the_ground_and_one_undo_removes_it() {
    let (mut ctx, project) = open("forest_place");
    ops::dispatch(
        &mut ctx,
        "terrain_stroke",
        &json!({"map": "m", "create": {"origin": [-10.0, -10.0], "cell_m": 1.0, "width": 50, "depth": 30, "height_m": 5.0},
                "stroke": {"brush": "Raise", "radius_m": 6.0, "strength": 2.0, "points": [[10.0, 0.0]]}}),
    )
    .unwrap();
    let Applied::Done { operation_id, .. } = ops::dispatch(&mut ctx, "forest_scatter", &place("oak", 10.0, 7)).unwrap() else { panic!("refused") };
    let forest = forest_of(&project, "m");
    assert_eq!(forest.trees.len(), 1);
    let t = &forest.trees[0];
    assert_eq!((t.species.as_str(), t.at, t.age_years, t.seed), ("oak", (10.0, 0.0), Some(40.0), Some(7)));
    assert!(t.ground_m > 5.5, "the tree stands on the raised hill, not at 0: {}", t.ground_m);
    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    assert!(forest_of(&project, "m").trees.is_empty());
}

#[test]
fn the_same_species_and_seed_grow_the_same_tree_and_another_seed_a_different_one() {
    let (mut a, pa) = open("forest_place_a");
    let (mut b, pb) = open("forest_place_b");
    for (ctx, x) in [(&mut a, 1.0), (&mut b, 9.0)] {
        ops::dispatch(ctx, "forest_scatter", &place("oak", x, 5)).unwrap();
        ops::dispatch(ctx, "forest_scatter", &place("oak", x + 3.0, 6)).unwrap();
    }
    let (fa, fb) = (forest_of(&pa, "m").trees, forest_of(&pb, "m").trees);
    let look = |t: &bevy_cauldron::ops_forest::Tree| (t.yaw, t.scale, t.variant);
    assert_eq!(look(&fa[0]), look(&fb[0]), "same seed, same tree wherever it stands");
    assert_ne!(look(&fa[0]), look(&fa[1]), "another seed grows another tree");
    assert_eq!(fa[1].id, 1);
}

#[test]
fn a_stroke_and_a_placed_tree_together_are_refused() {
    let (mut ctx, project) = open("forest_place_both");
    let both = json!({"map": "m", "place": {"species": "oak", "at": [0.0, 0.0]},
        "plant": {"points": [[0.0, 0.0]], "radius_m": 3.0, "spacing_m": 1.0, "species": [{"name": "oak", "weight": 1.0}]}});
    assert!(ops::dispatch(&mut ctx, "forest_scatter", &both).is_err());
    assert!(forest_of(&project, "m").trees.is_empty());
}
