//! **`terrain_stroke` sculpts the map's saved heightfield, and is one undo** (#99).

use bevy_grimoire_core::heightfield::Heightfield;
use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let file = project.grimoire_dir.join("terrain").join("m.ron");
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn raise(ctx: &mut Ctx, extra: serde_json::Value) -> Result<Applied, ops::Refusal> {
    let mut p = json!({"map": "m", "stroke": {"brush": "Raise", "radius_m": 2.0, "strength": 1.5, "points": [[5.0, 5.0]]}});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    ops::dispatch(ctx, "terrain_stroke", &p)
}

fn load(file: &std::path::Path) -> Heightfield {
    ron::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

#[test]
fn a_stroke_changes_only_the_cells_under_it_and_one_undo_puts_them_back() {
    let (mut ctx, file) = project("stroke");
    let create = json!({"create": {"origin": [0.0, 0.0], "cell_m": 1.0, "width": 11, "depth": 11, "height_m": 0.0}});
    let Applied::Done { operation_id, .. } = raise(&mut ctx, create).unwrap() else { panic!("the stroke was refused") };
    let after = load(&file);
    let flat = Heightfield::flat((0.0, 0.0), 1.0, 11, 11, 0.0);
    let raised: Vec<usize> = (0..after.heights.len()).filter(|&i| after.heights[i] != flat.heights[i]).collect();
    assert!(!raised.is_empty() && raised.len() < after.heights.len(), "some cells, not all: {raised:?}");
    assert_eq!(after.height_at(5.0, 5.0), 1.5);
    assert_eq!(after.height_at(0.0, 0.0), 0.0, "far cells are untouched");
    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    assert_eq!(load(&file), flat, "one undo restores every cell");
}

#[test]
fn a_map_with_no_terrain_refuses_and_writes_nothing() {
    let (mut ctx, file) = project("none");
    let refused = match raise(&mut ctx, json!({})) {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("a stroke on a map with no terrain was applied"),
    };
    assert!(refused.contains("no terrain"), "{refused}");
    assert!(!file.exists());
}
