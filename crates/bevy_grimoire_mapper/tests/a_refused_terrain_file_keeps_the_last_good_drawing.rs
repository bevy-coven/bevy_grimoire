//! **A terrain, water or volume file that is refused keeps the last good drawing, and the editor
//! says which file and field** (#200, H3).
//!
//! The editor draws the open map's ground, water and carved volume from three files and looks at them
//! twice a second. A bad edit (a hand edit, a truncated write, an agent's mistake) used to be
//! dropped silently for water and volume and drew no ground for terrain. Now each edit is one verdict
//! per file, in `grimoire_state.reloads`: `applied`, or `refused` with the field and the reason, and
//! a refusal leaves what was drawn as it was, so the view never shows half an edit.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::project::Project;
use bevy_grimoire_mapper::terrain_view::TerrainView;
use serde_json::{Value, json};

use crate::fixtures;

const GROUND: &str = "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, heights: [0.0, 0.0, 0.0, 0.0])";
const WATER: &str = "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.5, 0.0, 0.0, 0.0], settled: true, steps: 3)";
const WATER_DEEPER: &str = "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.9, 0.0, 0.0, 0.0], settled: true, steps: 4)";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn write(app: &App, dir: &str, file: &str, text: &str) {
    let dir = app.world().resource::<Project>().grimoire_dir.join(dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(dir.join(file), text).unwrap_or_else(|e| panic!("{dir:?}/{file}: {e}"));
    // Distinct modification times, so the view sees every edit.
    std::thread::sleep(Duration::from_millis(30));
}

/// Frames for `secs` seconds of wall time: the view looks twice a second.
fn pump(app: &mut App, secs: f64) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 6.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("never saw: {what}; reloads {}", reloads(app));
}

fn reloads(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["reloads"].clone()
}

fn entities(app: &mut App, view: TerrainView) -> Vec<Entity> {
    let world = app.world_mut();
    let mut found: Vec<Entity> = world.query::<(Entity, &TerrainView)>().iter(world).filter(|(_, v)| **v == view).map(|(e, _)| e).collect();
    found.sort();
    found
}

fn verdict(app: &mut App, kind: &str) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == kind).cloned()).unwrap_or(Value::Null)
}

fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

/// A map with its ground and one pool drawn, every file applied.
fn drawn(tag: &str) -> (App, Entity) {
    let mut app = editor(tag);
    write(&app, "terrain", "m.ron", GROUND);
    write(&app, "water", "m.field.ron", WATER);
    until(&mut app, "the pool drawn", |a| entities(a, TerrainView::Water).len() == 1);
    assert_eq!(entities(&mut app, TerrainView::Ground).len(), 1);
    assert_eq!(verdict(&mut app, "water_field")["outcome"], json!("applied"));
    assert_eq!(verdict(&mut app, "terrain")["outcome"], json!("applied"));
    let pool = entities(&mut app, TerrainView::Water)[0];
    (app, pool)
}

#[test]
fn a_water_field_that_does_not_fill_its_shape_is_refused_and_the_pool_stays() {
    let (mut app, pool) = drawn("reload_water_short");
    write(&app, "water", "m.field.ron", "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.5, 0.0, 0.0], settled: true, steps: 3)");
    until(&mut app, "the refusal", |a| verdict(a, "water_field")["outcome"] == json!("refused"));

    let v = verdict(&mut app, "water_field");
    assert!(v["file"].as_str().unwrap().ends_with("water/m.field.ron"), "{v}");
    assert!(v["reason"].as_str().unwrap().contains("3 depths for 2x2"), "{v}");
    assert_eq!(entities(&mut app, TerrainView::Water), vec![pool], "the pool drawn before stays, untouched");
    assert_eq!(entities(&mut app, TerrainView::Ground).len(), 1, "the ground is still drawn");
    assert_eq!(verdict(&mut app, "terrain")["outcome"], json!("applied"), "the other files are fine by themselves");
    assert_eq!(reloads(&mut app)["ok"], json!(false));
    let said = problems(&app);
    assert!(said.iter().any(|p| p.contains("water/m.field.ron") && p.contains("3 depths for 2x2")), "the status line names it: {said:?}");
}

#[test]
fn a_field_of_the_wrong_type_is_named() {
    let (mut app, pool) = drawn("reload_water_wrong_type");
    write(&app, "water", "m.field.ron", "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.5, 0.0, 0.0, 0.0], settled: 7, steps: 3)");
    until(&mut app, "the refusal", |a| verdict(a, "water_field")["outcome"] == json!("refused"));
    assert_eq!(verdict(&mut app, "water_field")["field"], json!("settled"));
    assert_eq!(entities(&mut app, TerrainView::Water), vec![pool]);
}

#[test]
fn a_volume_edit_that_cannot_be_meshed_is_refused_and_the_ground_stays() {
    let (mut app, pool) = drawn("reload_volume_zero");
    let ground = entities(&mut app, TerrainView::Ground);
    use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
    let flat = Edit { shape: Shape::Sphere { center: [1.0, 0.0, 1.0], radius_m: 0.0 }, mode: Mode::Carve, smooth_m: 0.0 };
    write(&app, "volume", "m.ron", &ron::to_string(&Volume { edits: vec![flat], ..Volume::default() }).unwrap());
    until(&mut app, "the refusal", |a| verdict(a, "volume")["outcome"] == json!("refused"));
    let v = verdict(&mut app, "volume");
    assert!(v["reason"].as_str().unwrap().contains("edit 0") && v["reason"].as_str().unwrap().contains("above zero"), "{v}");
    assert_eq!(entities(&mut app, TerrainView::Ground), ground);
    assert_eq!(entities(&mut app, TerrainView::Water), vec![pool]);
}

#[test]
fn one_edit_is_one_verdict_however_many_frames_pass_and_a_fix_applies() {
    let (mut app, pool) = drawn("reload_water_once");
    let before = reloads(&mut app)["seq"].as_u64().unwrap();
    write(&app, "water", "m.field.ron", "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.5], settled: true, steps: 3)");
    until(&mut app, "the refusal", |a| verdict(a, "water_field")["outcome"] == json!("refused"));
    let refused_at = reloads(&mut app)["seq"].as_u64().unwrap();
    assert_eq!(refused_at, before + 1, "one edit, one decision");
    let said = app.world().resource::<EditorState>().status.raised();

    pump(&mut app, 1.6);
    assert_eq!(reloads(&mut app)["seq"].as_u64().unwrap(), refused_at, "no new decision while nothing was edited");
    assert_eq!(app.world().resource::<EditorState>().status.raised(), said, "the refusal is said once, not every half second");

    write(&app, "water", "m.field.ron", WATER_DEEPER);
    until(&mut app, "the fix applied", |a| verdict(a, "water_field")["outcome"] == json!("applied"));
    assert_eq!(reloads(&mut app)["seq"].as_u64().unwrap(), refused_at + 1);
    assert_eq!(reloads(&mut app)["ok"], json!(true));
    let now = entities(&mut app, TerrainView::Water);
    assert_eq!(now.len(), 1);
    assert_ne!(now[0], pool, "the fixed file is drawn: the pool was rebuilt from it");
}

#[test]
fn a_refused_file_that_is_then_removed_is_no_longer_refused() {
    // An undo takes a bad file away; the view follows it, and the state does not keep the old refusal.
    let (mut app, _) = drawn("reload_water_removed");
    write(&app, "water", "m.field.ron", "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.5], settled: true, steps: 3)");
    until(&mut app, "the refusal", |a| verdict(a, "water_field")["outcome"] == json!("refused"));
    let file = app.world().resource::<Project>().grimoire_dir.join("water").join("m.field.ron");
    std::fs::remove_file(&file).unwrap_or_else(|e| panic!("{file:?}: {e}"));
    until(&mut app, "the refusal gone", |a| verdict(a, "water_field") == Value::Null);
    assert_eq!(reloads(&mut app)["ok"], json!(true), "{}", reloads(&mut app));
    assert!(entities(&mut app, TerrainView::Water).is_empty(), "no file, no water");
    assert_eq!(entities(&mut app, TerrainView::Ground).len(), 1);
}
