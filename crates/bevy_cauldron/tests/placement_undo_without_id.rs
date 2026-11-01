//! **`operation_undo` with no id takes back a placement change** (issue #88): the documented "newest
//! change to the open map" used to refuse one, because `placement_set_yaw` logged no map.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn turn(app: &mut App, yaw: f32) -> serde_json::Value {
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_yaw",
            "parameters": {"id": id, "yaw": yaw}, "dry_run": false}),
    )
}

fn undo(app: &mut App) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"dry_run": false}),
    )
}

fn yaw_held(app: &App) -> f32 {
    app.world().resource::<OpenMap>().map.placements[0].yaw
}

fn yaw_on_disk(app: &App) -> f32 {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap();
    bevy_grimoire_core::map::Map::parse(&text).unwrap().placements[0].yaw
}

fn oplog(app: &App) -> std::path::PathBuf {
    let root = app.world().resource::<OpenMap>().map_path.clone();
    let mut dir = root.as_path();
    while !dir.join("assets/grimoire/oplog.jsonl").exists() {
        dir = dir.parent().expect("the fixture has an oplog");
    }
    dir.join("assets/grimoire/oplog.jsonl")
}

#[test]
fn an_undo_with_no_id_takes_back_a_turn() {
    let mut app = editor("undo_noid_turn");
    assert_eq!(turn(&mut app, 110.0)["outcome"], "applied");
    let answer = undo(&mut app);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(yaw_held(&app), 0.0);
    assert_eq!(yaw_on_disk(&app), 0.0);
}

#[test]
fn a_turn_logged_before_placements_recorded_their_map_is_still_found() {
    let mut app = editor("undo_noid_old_log");
    assert_eq!(turn(&mut app, 110.0)["outcome"], "applied");
    let path = oplog(&app);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#""map":"m","#), "the line records its map: {text}");
    std::fs::write(&path, text.replace(r#""map":"m","#, "")).unwrap();
    let answer = undo(&mut app);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(yaw_held(&app), 0.0);
}

#[test]
fn a_turn_on_another_map_is_not_taken_back_by_the_open_map() {
    let mut app = editor("undo_noid_other_map");
    assert_eq!(turn(&mut app, 110.0)["outcome"], "applied");
    let path = oplog(&app);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace(r#""map":"m""#, r#""map":"m2""#)).unwrap();
    let answer = undo(&mut app);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(yaw_held(&app), 110.0);
}
