//! **`terrain_preset` bakes a `bevy_elements` preset into the map's saved terrain, in one undo** (#99).
#![cfg(feature = "bevy_elements")]

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

fn load(file: &std::path::Path) -> Heightfield {
    ron::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn bake(ctx: &mut Ctx, preset: &str, seed: u64) -> Result<Applied, ops::Refusal> {
    ops::dispatch(ctx, "terrain_preset", &json!({"map": "m", "preset": preset, "seed": seed, "cell_m": 8.0, "size": [32, 24]}))
}

#[test]
fn a_preset_becomes_the_saved_terrain_per_seed_and_one_undo_removes_it() {
    let (mut ctx, file) = project("preset");
    let Applied::Done { operation_id, .. } = bake(&mut ctx, "hills", 7).unwrap() else { panic!("refused") };
    let a = load(&file);
    assert_eq!((a.width, a.depth), (32, 24));
    let (lo, hi) = a.heights.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    assert!(hi - lo > 5.0, "hills have relief, got {}", hi - lo);
    bake(&mut ctx, "hills", 7).unwrap();
    assert_eq!(load(&file), a, "same seed, same ground");
    bake(&mut ctx, "hills", 8).unwrap();
    assert_ne!(load(&file).heights, a.heights, "another seed, other ground");
    ops::undo(&mut ctx, None).unwrap();
    ops::undo(&mut ctx, None).unwrap();
    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    assert!(!file.exists(), "undoing the first bake leaves the map with no terrain");
}

#[test]
fn an_unknown_preset_is_refused_and_writes_nothing() {
    let (mut ctx, file) = project("unknown");
    let refused = match bake(&mut ctx, "volcano", 0) {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("accepted"),
    };
    assert!(refused.contains("hills"), "{refused}");
    assert!(!file.exists());
}
