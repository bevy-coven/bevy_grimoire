//! **A scatter stroke plants trees only under the stroke, on the terrain, as one undo step** (#102).
//! Needs the `bevy_flora` feature: `cargo test -p bevy_cauldron --features bevy_flora --test editor forest_stroke::`.
//! Without it the operation refuses and says so.

#[cfg(feature = "bevy_flora")]
use bevy_cauldron::ops::Applied;
use bevy_cauldron::ops::{self, Ctx};
#[cfg(feature = "bevy_flora")]
use bevy_cauldron::ops_forest::under_stroke;
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

fn stroke() -> serde_json::Value {
    json!({"map": "m", "plant": {"points": [[0.0, 0.0], [20.0, 0.0]], "radius_m": 4.0, "spacing_m": 2.0, "seed": 3,
        "species": [{"name": "oak", "weight": 3.0}, {"name": "birch", "weight": 1.0}]}})
}

#[cfg(feature = "bevy_flora")]
#[test]
fn trees_stand_only_under_the_stroke_on_the_ground_and_one_undo_removes_them() {
    let (mut ctx, project) = open("forest_stroke");
    ops::dispatch(
        &mut ctx,
        "terrain_stroke",
        &json!({"map": "m", "create": {"origin": [-10.0, -10.0], "cell_m": 1.0, "width": 50, "depth": 30, "height_m": 5.0},
                "stroke": {"brush": "Raise", "radius_m": 6.0, "strength": 2.0, "points": [[10.0, 0.0]]}}),
    )
    .unwrap();
    let Applied::Done { operation_id, .. } = ops::dispatch(&mut ctx, "forest_scatter", &stroke()).unwrap() else { panic!("refused") };
    let forest = forest_of(&project, "m");
    assert!(forest.trees.len() > 20, "a 24 m x 8 m stroke at 2 m spacing plants a forest, got {}", forest.trees.len());
    let ground: bevy_grimoire_core::heightfield::Heightfield =
        ron::from_str(&std::fs::read_to_string(project.grimoire_dir.join("terrain").join("m.ron")).unwrap()).unwrap();
    let line = [(0.0, 0.0), (20.0, 0.0)];
    for t in &forest.trees {
        assert!(under_stroke(t.at, &line, 4.0), "tree {} at {:?} is outside the stroke", t.id, t.at);
        assert_eq!(t.ground_m, ground.height_at(t.at.0, t.at.1), "tree {} does not stand on the terrain", t.id);
    }
    assert!(forest.trees.iter().any(|t| t.ground_m > 5.5), "the raised hill lifts the trees on it");
    assert!(forest.trees.iter().any(|t| t.species == "birch") && forest.trees.iter().any(|t| t.species == "oak"));
    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    assert!(forest_of(&project, "m").trees.is_empty(), "one undo removes the whole stroke");
}

#[cfg(feature = "bevy_flora")]
#[test]
fn the_same_seed_plants_the_same_trees_and_a_second_stroke_adds_without_crowding() {
    let (mut a, pa) = open("forest_seed_a");
    let (mut b, pb) = open("forest_seed_b");
    ops::dispatch(&mut a, "forest_scatter", &stroke()).unwrap();
    ops::dispatch(&mut b, "forest_scatter", &stroke()).unwrap();
    assert_eq!(forest_of(&pa, "m"), forest_of(&pb, "m"));
    let n = forest_of(&pa, "m").trees.len();
    ops::dispatch(&mut a, "forest_scatter", &stroke()).unwrap();
    assert_eq!(forest_of(&pa, "m").trees.len(), n, "the same stroke twice plants nothing new");
}

#[cfg(not(feature = "bevy_flora"))]
#[test]
fn a_build_without_flora_refuses_and_names_the_feature() {
    let (mut ctx, project) = open("forest_refused");
    let e = ops::dispatch(&mut ctx, "forest_scatter", &stroke()).expect_err("refused");
    assert!(e.0.contains("--features bevy_flora"), "{}", e.0);
    assert!(forest_of(&project, "m").trees.is_empty());
}

#[test]
fn a_stroke_without_a_species_is_refused() {
    let (mut ctx, _) = open("forest_no_species");
    let mut p = stroke();
    p["plant"]["species"] = json!([]);
    assert!(ops::dispatch(&mut ctx, "forest_scatter", &p).is_err());
}
