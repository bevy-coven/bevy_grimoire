//! **`water_place` saves springs, sinks and lakes beside the map, one undo each** (#100).

use bevy_grimoire_core::water::WaterPlan;
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
    let file = project.grimoire_dir.join("water").join("m.ron");
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn load(file: &std::path::Path) -> WaterPlan {
    ron::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn done(r: Result<Applied, ops::Refusal>) -> String {
    match r.unwrap() {
        Applied::Done { operation_id, .. } => operation_id,
        _ => panic!("refused"),
    }
}

#[test]
fn placed_water_reloads_and_each_placement_is_one_undo() {
    let (mut ctx, file) = project("water");
    let spring = json!({"map": "m", "place": {"Spring": {"x": 3.0, "z": 4.0, "rate_m3_s": 0.5, "radius_m": 1.5}}});
    let lake = json!({"map": "m", "place": {"Lake": {"x": 9.0, "z": 9.0, "radius_m": 4.0, "level_m": 1.25}}});
    done(ops::dispatch(&mut ctx, "water_place", &spring));
    let second = done(ops::dispatch(&mut ctx, "water_place", &lake));
    let plan = load(&file);
    assert_eq!((plan.springs.len(), plan.lakes.len()), (1, 1));
    assert_eq!(plan.springs[0].rate_m3_s, 0.5);
    assert_eq!(plan.lakes[0].level_m, 1.25);
    ops::undo(&mut ctx, Some(&second)).unwrap();
    let plan = load(&file);
    assert_eq!((plan.springs.len(), plan.lakes.len()), (1, 0), "one undo takes back one placement");
}

#[test]
fn clearing_is_one_undo_and_a_bad_radius_writes_nothing() {
    let (mut ctx, file) = project("clear");
    done(ops::dispatch(&mut ctx, "water_place", &json!({"map": "m", "place": {"Sink": {"x": 0.0, "z": 0.0, "rate_m3_s": 1.0, "radius_m": 2.0}}})));
    let clear = done(ops::dispatch(&mut ctx, "water_place", &json!({"map": "m", "clear": true})));
    assert!(!file.exists());
    ops::undo(&mut ctx, Some(&clear)).unwrap();
    assert_eq!(load(&file).sinks.len(), 1);
    let bad = json!({"map": "m", "place": {"Spring": {"x": 0.0, "z": 0.0, "rate_m3_s": 1.0, "radius_m": 0.0}}});
    assert!(matches!(ops::dispatch(&mut ctx, "water_place", &bad), Err(_) | Ok(Applied::Refused(_))));
    assert_eq!(load(&file).springs.len(), 0);
}
