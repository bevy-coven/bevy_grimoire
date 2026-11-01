//! **B-030: undoing a change made before a rename names the rename**, not the old map name. The
//! oplog ties a change to its map by name, so the refusal is right, but "Open `m` to take it back"
//! sends the author to a map that no longer exists.

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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

#[test]
fn the_refusal_names_the_rename_to_undo_first() {
    let mut app = editor("undo_across_rename");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    let turn = apply(&mut app, "placement_set_yaw", serde_json::json!({"id": id, "yaw": 110.0}));
    assert_eq!(turn["outcome"], "applied", "{turn}");
    let turn_id = turn["operation_id"].as_str().unwrap().to_owned();
    let rename = apply(&mut app, "map_rename", serde_json::json!({"name": "town"}));
    assert_eq!(rename["outcome"], "applied", "{rename}");
    let rename_id = rename["operation_id"].as_str().unwrap().to_owned();

    let refused = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": turn_id, "dry_run": false}),
    );
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let text = refused.to_string();
    assert!(text.contains(&rename_id), "names the rename `{rename_id}`: {text}");
    assert!(!text.contains("Open `m`"), "does not send the author to the old name: {text}");
}
