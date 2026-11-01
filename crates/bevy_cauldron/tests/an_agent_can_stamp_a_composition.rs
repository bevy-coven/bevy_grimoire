//! **An agent stamps a composition onto the map, and takes it back** (E-072, `stamp_add`).
//!
//! Through the registered BRP methods: define a group from two placements, stamp it, see the stamp on
//! the map and in its file; undo removes it, redo puts it back at the same place; a composition the
//! project lacks and a name already taken are refused and write nothing.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("crate", "alpha", 1.0, 1.0)
        .sized_descriptor("barrel", "alpha", 1.0, 1.0)
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

fn stamps(app: &App) -> Vec<(String, (f32, f32))> {
    app.world().resource::<OpenMap>().map.stamps.iter().map(|s| (s.id.clone(), s.at)).collect()
}

fn with_group(name: &str) -> App {
    let mut app = editor(name);
    let mut ids = vec![];
    for (piece, at) in [("crate", [1.5, 1.5]), ("barrel", [2.5, 1.5])] {
        let a = apply(&mut app, "placement_add", serde_json::json!({"piece": piece, "at": at}));
        ids.push(a["log_entry"]["after"]["id"].as_str().expect("a row").to_owned());
    }
    let d = apply(&mut app, "composition_define", serde_json::json!({"id": "corner", "from": ids}));
    assert_eq!(d["outcome"], "applied", "{d}");
    app
}

#[test]
fn a_stamp_lands_on_the_map_and_undo_and_redo_move_it_off_and_on() {
    let mut app = with_group("stamp_add_roundtrip");
    let a = apply(&mut app, "stamp_add", serde_json::json!({"of": "corner", "at": [6.5, 6.5], "yaw": 1.0}));
    assert_eq!(a["outcome"], "applied", "{a}");
    assert_eq!(stamps(&app), vec![("corner@3".to_owned(), (6.5, 6.5))], "minted past the two placements");
    let file = std::fs::read_to_string(
        app.world().resource::<OpenMap>().map_path.clone(),
    )
    .expect("the map file");
    assert!(file.contains("corner@3"), "saved: {file}");

    let id = a["operation_id"].as_str().expect("an id").to_owned();
    let back = undo(&mut app, &id);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert!(stamps(&app).is_empty(), "undo takes the stamp off");
    let undone = back["operation_id"].as_str().expect("the undo has an id").to_owned();
    let again = undo(&mut app, &undone);
    assert_eq!(again["outcome"], "applied", "{again}");
    assert_eq!(stamps(&app).len(), 1, "undoing the undo puts it back");
}

#[test]
fn an_unknown_composition_or_a_taken_name_is_refused_and_writes_nothing() {
    let mut app = with_group("stamp_add_refusals");
    let r = apply(&mut app, "stamp_add", serde_json::json!({"of": "nowhere", "at": [6.5, 6.5]}));
    assert_eq!(r["outcome"], "refused", "{r}");
    assert!(r.to_string().contains("nowhere"), "names the composition: {r}");
    assert!(stamps(&app).is_empty());

    let ok = apply(&mut app, "stamp_add", serde_json::json!({"of": "corner", "at": [6.5, 6.5], "id": "mine"}));
    assert_eq!(ok["outcome"], "applied", "{ok}");
    let dup = apply(&mut app, "stamp_add", serde_json::json!({"of": "corner", "at": [9.5, 9.5], "id": "mine"}));
    assert_eq!(dup["outcome"], "refused", "{dup}");
    assert_eq!(stamps(&app).len(), 1);

    let forged = apply(&mut app, "stamp_add", serde_json::json!({"remove": "mine"}));
    assert_eq!(forged["outcome"], "refused", "an undo-only form is not a way to remove a stamp: {forged}");
}
