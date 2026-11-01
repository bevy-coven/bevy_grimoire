//! **A run is added, moved and removed through the registry, and each is taken back by its id** (#115).
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

fn add(app: &mut App) -> serde_json::Value {
    apply(
        app,
        "run_add",
        serde_json::json!({"id": "fence_a", "run": {"descriptor": "fence", "from": [0.0, 0.0], "to": [8.0, 0.0]}}),
    )
}

#[test]
fn moving_an_end_changes_the_rows_and_undo_restores_the_file_byte_for_byte() {
    let mut app = editor("run_ops_set");
    let before_add = file(&app);
    let added = id_of(&add(&mut app));
    let with_run = file(&app);
    let n = rows(&app);
    assert!(n >= 4, "an 8 m run of 2 m posts lays several rows, got {n}");
    assert_ne!(with_run, before_add, "run_add saved the map");
    assert!(with_run.contains("fence_a"), "the file holds the run, not only the rows");

    let set = id_of(&apply(&mut app, "run_set", serde_json::json!({"id": "fence_a", "to": [16.0, 0.0]})));
    assert!(rows(&app) > n, "moving `to` further lays more rows");
    assert_eq!(app.world().resource::<OpenMap>().map.runs.len(), 1, "still one run, not its rows");

    let back = undo(&mut app, &set);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(file(&app), with_run, "undo restores the file byte for byte");
    assert_eq!(rows(&app), n);

    let gone = undo(&mut app, &added);
    assert_eq!(gone["outcome"], "applied", "{gone}");
    assert!(app.world().resource::<OpenMap>().map.runs.is_empty(), "undo of the add takes the run off");
}

#[test]
fn a_removal_is_put_back_where_it_stood() {
    let mut app = editor("run_ops_remove");
    id_of(&add(&mut app));
    let with_run = file(&app);

    let removed = id_of(&apply(&mut app, "run_remove", serde_json::json!({"id": "fence_a"})));
    assert!(app.world().resource::<OpenMap>().map.runs.is_empty());
    assert!(!file(&app).contains("fence_a"), "the file no longer holds the run");

    let back = undo(&mut app, &removed);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(file(&app), with_run, "the run is back, byte for byte");
}

#[test]
fn a_run_that_does_not_lay_is_refused_and_the_file_is_untouched() {
    let mut app = editor("run_ops_refuse");
    let before = file(&app);
    let answer = apply(
        &mut app,
        "run_add",
        serde_json::json!({"id": "bad", "run": {"descriptor": "no/such", "from": [0.0, 0.0], "to": [8.0, 0.0]}}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert!(answer.to_string().contains("bad"), "the refusal names the run: {answer}");
    assert_eq!(file(&app), before);

    id_of(&add(&mut app));
    let twice = add(&mut app);
    assert_eq!(twice["outcome"], "refused", "a taken id is refused: {twice}");
    let nothing = apply(&mut app, "run_set", serde_json::json!({"id": "fence_a"}));
    assert_eq!(nothing["outcome"], "refused", "{nothing}");
}
