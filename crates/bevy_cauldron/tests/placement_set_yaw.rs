//! **An agent can turn a placement, and take the turn back, through the same log the author's undo uses.**
//!
//! The regression tests for the operation behind `TurnPieceLeft` and `TurnPieceRight`: turning a
//! piece was a human-only edit on the unmigrated list. It is now the registered operation
//! `placement_set_yaw`. These go through the registered BRP methods, the way the MCP server does,
//! and assert on the map the editor holds, the file on disk, and the log.

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

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_yaw", "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: Option<&str>) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn an_agent_turns_a_placement_and_the_file_says_so() {
    let mut app = editor("agent_turns");
    let id = held(&app).placements[0].id.clone();
    assert_eq!(
        held(&app).placements[0].yaw,
        0.0,
        "the fixture starts unturned"
    );

    let answer = apply(&mut app, serde_json::json!({"id": id, "yaw": 90.0}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    assert_eq!(
        held(&app).placements[0].yaw,
        90.0,
        "the editor holds the turn"
    );
    assert_eq!(
        on_disk(&app).placements[0].yaw,
        90.0,
        "the file holds the turn"
    );

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(
        held(&app).placements[0].yaw,
        0.0,
        "undo turns it back in the editor"
    );
    assert_eq!(
        on_disk(&app).placements[0].yaw,
        0.0,
        "undo turns it back on disk"
    );
}

#[test]
fn a_facing_is_wrapped_into_a_full_turn() {
    let mut app = editor("agent_turns_wrap");
    let id = held(&app).placements[0].id.clone();

    let answer = apply(&mut app, serde_json::json!({"id": id, "yaw": -15.0}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(
        held(&app).placements[0].yaw,
        345.0,
        "-15 is 345, the way the keys wrap"
    );
    assert_eq!(on_disk(&app).placements[0].yaw, 345.0);
}

#[test]
fn an_unknown_placement_is_refused_and_nothing_changes() {
    let mut app = editor("agent_turns_unknown");
    let answer = apply(
        &mut app,
        serde_json::json!({"id": "no-such-piece", "yaw": 90.0}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(
        held(&app).placements[0].yaw,
        0.0,
        "a refused turn changes nothing"
    );
    assert_eq!(
        on_disk(&app).placements[0].yaw,
        0.0,
        "a refused turn writes nothing"
    );
}

#[test]
fn an_unknown_field_is_refused() {
    let mut app = editor("agent_turns_field");
    let id = held(&app).placements[0].id.clone();
    let answer = apply(
        &mut app,
        serde_json::json!({"id": id, "yaw": 90.0, "pitch": 10.0}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(
        held(&app).placements[0].yaw,
        0.0,
        "a refused turn changes nothing"
    );
}

#[test]
fn an_unrelated_hand_edit_does_not_block_the_undo_of_a_turn() {
    let mut app = editor("agent_turns_hand_edit");
    let id = held(&app).placements[0].id.clone();
    let answer = apply(&mut app, serde_json::json!({"id": id, "yaw": 90.0}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    // **A piece added by hand, outside the log, after the turn.** It is not the turned piece, so
    // it must not block the turn's undo.
    let mut added = held(&app).placements[0].clone();
    added.id = "hand-added".to_owned();
    app.world_mut()
        .resource_mut::<OpenMap>()
        .map
        .placements
        .push(added);

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(
        held(&app).placements[0].yaw,
        0.0,
        "undo turns the turned piece back"
    );
}
