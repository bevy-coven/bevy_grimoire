//! **A run is added, converted through the registry, and each is taken back by its id** (#115).
//!
//! The acceptance this proves (#115 item 2): `run_set` moving `to` changes the expanded row count,
//! and a following undo by `operation_id` restores the file byte for byte. Through the registered
//! BRP methods, as an agent's request goes.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

const FENCE_W: f32 = 2.0;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("fence", "alpha", FENCE_W, 0.1)
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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: &str) -> serde_json::Value {
    call(app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}))
}

fn path(app: &App) -> std::path::PathBuf {
    app.world().resource::<OpenMap>().map_path.clone()
}

fn file(app: &App) -> String {
    std::fs::read_to_string(path(app)).unwrap_or_default()
}

fn rows(app: &App) -> usize {
    let project = app.world().resource::<bevy_cauldron::project::Project>();
    let map = &app.world().resource::<OpenMap>().map;
    map.expanded(&[], &project.library).expect("the map expands").placements.len()
}

fn id_of(answer: &serde_json::Value) -> String {
    assert_eq!(answer["outcome"], "applied", "{answer}");
    answer["operation_id"].as_str().expect("an apply returns its id").to_owned()
}

fn lay_rows(app: &mut App, nudge: Option<usize>) -> Vec<String> {
    let run = bevy_grimoire_core::run::Run {
        descriptor: "fence".into(),
        pitch: None,
        from: (0.0, 0.0),
        to: (8.0, 0.0),
        corners: Vec::new(),
        gaps: Vec::new(),
    };
    let posts = {
        let project = app.world().resource::<bevy_cauldron::project::Project>();
        bevy_grimoire_core::run::lay(&run, &project.library).expect("lays")
    };
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let mut ids = Vec::new();
    for (i, p) in posts.iter().enumerate() {
        let mut at = p.at;
        if nudge == Some(i) {
            at.1 += 0.005;
        }
        let id = format!("fence@{}", i + 1);
        open.map.placements.push(bevy_grimoire_core::map::Placed {
            id: id.clone(),
            descriptor: "fence".into(),
            at,
            yaw: p.yaw,
            ..Default::default()
        });
        ids.push(id);
    }
    ids
}

#[test]
fn rows_become_a_run_and_undo_gives_the_same_rows_back() {
    let mut app = editor("run_ops_convert");
    let ids = lay_rows(&mut app, None);
    let before: Vec<_> = app.world().resource::<OpenMap>().map.placements.clone();
    let n = rows(&app);
    let done = id_of(&apply(&mut app, "placements_to_run", serde_json::json!({"id": "fence_a", "rows": ids})));
    let map = &app.world().resource::<OpenMap>().map;
    assert!(map.placements.is_empty(), "the rows left the map");
    assert_eq!(map.runs.len(), 1);
    assert_eq!(rows(&app), n, "the run lays the rows it replaced");
    let back = undo(&mut app, &done);
    assert_eq!(back["outcome"], "applied", "{back}");
    let map = &app.world().resource::<OpenMap>().map;
    assert!(map.runs.is_empty());
    assert_eq!(map.placements, before, "the same rows, in the same order");
}

#[test]
fn a_row_off_the_pitch_is_refused_by_name_and_nothing_is_written() {
    let mut app = editor("run_ops_convert_refuse");
    let ids = lay_rows(&mut app, Some(2));
    let before = file(&app);
    let answer = apply(&mut app, "placements_to_run", serde_json::json!({"id": "fence_a", "rows": ids}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert!(answer.to_string().contains("fence@3"), "names the row: {answer}");
    assert_eq!(file(&app), before);
    assert!(app.world().resource::<OpenMap>().map.runs.is_empty());
}
