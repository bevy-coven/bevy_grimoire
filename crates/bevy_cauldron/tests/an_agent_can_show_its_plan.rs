//! **An agent shows its plan, and the person can read it before it acts** (E-069, `plan_set`).
//!
//! Through the registered BRP methods: set a plan (it lands in the `.plan.ron` beside the map and in
//! `grimoire_state`), replace it, clear it, and undo each step; a plan with an empty item is refused and
//! writes nothing, and the preview writes nothing.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("crate", "alpha").build("m");
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

fn run(app: &mut App, operation: &str, parameters: serde_json::Value, dry_run: bool) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": dry_run}),
    )
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    run(app, operation, parameters, false)
}

fn undo(app: &mut App, id: &str) -> serde_json::Value {
    call(app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}))
}

fn plan_file(app: &App) -> Option<String> {
    let map_path = app.world().resource::<OpenMap>().map_path.clone();
    std::fs::read_to_string(bevy_grimoire_core::plan::path_for_map(&map_path)).ok()
}

fn shown(app: &mut App) -> Vec<(String, String)> {
    let state = call(app, "grimoire/state", serde_json::json!({}));
    state["plan"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("grimoire_state has a plan section: {state}"))
        .iter()
        .map(|i| (i["text"].as_str().unwrap().to_owned(), i["status"].as_str().unwrap().to_owned()))
        .collect()
}

fn pair(text: &str, status: &str) -> (String, String) {
    (text.to_owned(), status.to_owned())
}

#[test]
fn a_plan_is_kept_beside_the_map_and_shown_in_the_state() {
    let mut app = editor("plan_set_shown");
    assert!(shown(&mut app).is_empty(), "no plan yet");
    assert!(plan_file(&app).is_none());

    let a = apply(
        &mut app,
        "plan_set",
        serde_json::json!({"items": [
            {"text": "lay the road", "status": "done"},
            {"text": "place the lamps", "status": "doing"},
            {"text": "add a fence"},
        ]}),
    );
    assert_eq!(a["outcome"], "applied", "{a}");
    assert_eq!(
        shown(&mut app),
        vec![pair("lay the road", "done"), pair("place the lamps", "doing"), pair("add a fence", "pending")]
    );
    let file = plan_file(&app).expect("the plan file is beside the map");
    assert!(file.contains("place the lamps"), "saved: {file}");
}

#[test]
fn a_plan_replaces_the_last_and_each_step_undoes_and_redoes() {
    let mut app = editor("plan_set_undo");
    let one = apply(&mut app, "plan_set", serde_json::json!({"items": [{"text": "first"}]}));
    assert_eq!(one["outcome"], "applied", "{one}");
    let two = apply(
        &mut app,
        "plan_set",
        serde_json::json!({"items": [{"text": "first", "status": "done"}, {"text": "second", "status": "doing"}]}),
    );
    assert_eq!(two["outcome"], "applied", "{two}");
    assert_eq!(shown(&mut app), vec![pair("first", "done"), pair("second", "doing")]);

    let two_id = two["operation_id"].as_str().expect("an id").to_owned();
    let back = undo(&mut app, &two_id);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(shown(&mut app), vec![pair("first", "pending")], "undo restores the previous plan");

    let undone = back["operation_id"].as_str().expect("the undo has an id").to_owned();
    let again = undo(&mut app, &undone);
    assert_eq!(again["outcome"], "applied", "{again}");
    assert_eq!(shown(&mut app), vec![pair("first", "done"), pair("second", "doing")], "redo puts it back");
}

#[test]
fn an_empty_list_clears_the_plan_and_undo_brings_it_back() {
    let mut app = editor("plan_set_clear");
    apply(&mut app, "plan_set", serde_json::json!({"items": [{"text": "only step", "status": "doing"}]}));
    let cleared = apply(&mut app, "plan_set", serde_json::json!({"items": []}));
    assert_eq!(cleared["outcome"], "applied", "{cleared}");
    assert!(shown(&mut app).is_empty());

    let id = cleared["operation_id"].as_str().expect("an id").to_owned();
    let back = undo(&mut app, &id);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(shown(&mut app), vec![pair("only step", "doing")]);
}

#[test]
fn a_preview_and_a_refusal_write_nothing() {
    let mut app = editor("plan_set_refusals");
    let preview = run(&mut app, "plan_set", serde_json::json!({"items": [{"text": "a thing"}]}), true);
    assert_ne!(preview["outcome"], "refused", "{preview}");
    assert!(plan_file(&app).is_none(), "a preview writes no file");
    assert!(shown(&mut app).is_empty());

    let blank = apply(&mut app, "plan_set", serde_json::json!({"items": [{"text": "ok"}, {"text": "  "}]}));
    assert_eq!(blank["outcome"], "refused", "{blank}");
    assert!(plan_file(&app).is_none());

    let odd = apply(&mut app, "plan_set", serde_json::json!({"items": [{"text": "ok", "status": "blocked"}]}));
    assert_eq!(odd["outcome"], "refused", "{odd}");
    let extra = apply(&mut app, "plan_set", serde_json::json!({"items": [], "note": "x"}));
    assert_eq!(extra["outcome"], "refused", "{extra}");
    assert!(shown(&mut app).is_empty());
}
