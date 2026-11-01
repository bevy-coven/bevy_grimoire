//! **`water_run` settles a spring into a pond, and keeping it is one undo** (#100).

use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::water::WaterField;
use bevy_grimoire_mapper::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_grimoire_mapper::project::Project>().clone();
    let dir = project.grimoire_dir.clone();
    let open = app.world().resource::<bevy_grimoire_mapper::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, dir)
}

/// A 24 x 24 m valley: a tilted slope that drains to the -z edge, with a walled bowl at z 14..20
/// (rim 3 m above its floor) that the spring's water has to fill before it can leave.
fn valley() -> Heightfield {
    let mut g = Heightfield::flat((0.0, 0.0), 1.0, 24, 24, 0.0);
    for iz in 0..24usize {
        for ix in 0..24usize {
            let slope = 0.2 * iz as f32;
            let in_bowl = (8..16).contains(&ix) && (14..20).contains(&iz);
            let rim = in_bowl && (ix == 8 || ix == 15 || iz == 14 || iz == 19);
            g.heights[iz * 24 + ix] = if in_bowl && !rim { slope - 1.0 } else if rim { slope + 2.0 } else { slope };
        }
    }
    g
}

fn done(r: Result<Applied, ops::Refusal>) -> String {
    match r.unwrap() {
        Applied::Done { operation_id, .. } => operation_id,
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
}

fn setup(tag: &str) -> (Ctx, std::path::PathBuf) {
    let (mut ctx, dir) = project(tag);
    std::fs::create_dir_all(dir.join("terrain")).unwrap();
    std::fs::write(dir.join("terrain/m.ron"), ron::to_string(&valley()).unwrap()).unwrap();
    // A spring inside the bowl, so the pond has to hold it.
    done(ops::dispatch(&mut ctx, "water_place", &json!({"map": "m", "place": {"Spring": {"x": 11.5, "z": 16.5, "rate_m3_s": 2.0, "radius_m": 1.0}}})));
    (ctx, dir)
}

#[test]
fn a_spring_in_a_bowl_settles_into_a_pond_that_reloads() {
    let (mut ctx, dir) = setup("pond");
    let file = dir.join("water/m.field.ron");
    assert!(!file.exists(), "placing never runs the simulation");
    let planned = ops::preview(&ctx, &bevy_grimoire_mapper::ops_water::WaterRun, &json!({"map": "m", "pour_s": 20.0})).unwrap();
    assert!(!file.exists(), "a preview keeps nothing: {planned}");
    done(ops::dispatch(&mut ctx, "water_run", &json!({"map": "m", "pour_s": 20.0})));
    let field: WaterField = ron::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(field.settled, "the water stopped moving after {} steps", field.steps);
    // 2 m3/s for 20 s = 40 m3; the bowl (6x4 inside the rim, floor 1 m below the slope) holds it.
    assert!(field.volume_m3() > 38.0, "the bowl kept the poured water: {}", field.volume_m3());
    let (pond, dry) = (field.depth_at(11.5, 16.5), field.depth_at(2.5, 5.0));
    assert!(pond > 0.3, "a pond stands where the spring is: {pond} m");
    assert!(dry < 1e-3, "the slope outside is dry: {dry} m");
    // Still water: one level across the pond, not a heap under the spring.
    let level = |x: f32, z: f32| field.depth_at(x, z) + valley().height_at(x, z);
    assert!((level(11.5, 16.5) - level(13.5, 17.5)).abs() < 0.05, "the pond is level");
}

#[test]
fn running_and_keeping_is_one_undo() {
    let (mut ctx, dir) = setup("undo");
    let file = dir.join("water/m.field.ron");
    let first = done(ops::dispatch(&mut ctx, "water_run", &json!({"map": "m", "pour_s": 5.0})));
    let before = std::fs::read_to_string(&file).unwrap();
    let second = done(ops::dispatch(&mut ctx, "water_run", &json!({"map": "m", "pour_s": 10.0})));
    assert_ne!(std::fs::read_to_string(&file).unwrap(), before);
    ops::undo(&mut ctx, Some(&second)).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "one undo restores the earlier kept water");
    ops::undo(&mut ctx, Some(&first)).unwrap();
    assert!(!file.exists(), "and one more leaves none");
}

#[test]
fn a_run_with_no_water_or_no_ground_is_refused() {
    let (mut ctx, dir) = project("water_run_settles_a_pond_refuse");
    let r = ops::dispatch(&mut ctx, "water_run", &json!({"map": "m"}));
    assert!(matches!(r, Err(_) | Ok(Applied::Refused(_))));
    done(ops::dispatch(&mut ctx, "water_place", &json!({"map": "m", "place": {"Spring": {"x": 1.0, "z": 1.0, "rate_m3_s": 1.0, "radius_m": 1.0}}})));
    let r = ops::dispatch(&mut ctx, "water_run", &json!({"map": "m"}));
    assert!(matches!(r, Err(_) | Ok(Applied::Refused(_))), "no terrain to run on");
    assert!(!dir.join("water/m.field.ron").exists());
}
