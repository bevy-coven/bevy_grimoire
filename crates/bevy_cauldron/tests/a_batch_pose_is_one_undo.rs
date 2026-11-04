//! **Several pieces posed at once are one operation, one log line and one undo** (UI-50, #236).
//!
//! #236's first acceptance item: "Two pieces with different turns show '—'; typing 90 turns both through
//! one log line." The mixed-value display is the inspector's; the *operation* is this file's subject,
//! because "one log line" cannot be built on a loop of single-piece calls — that would be two lines and
//! two undos.
//!
//! `placement_set_pose` takes `ids` as well as `id`. The single-`id` form is unchanged, byte for byte,
//! including the shape of what it records (the undo's own check compares that shape against the map, so
//! changing it refuses every existing caller's undo — which is exactly what a first version of this
//! change did, and what `placement_set_pose_is_an_operation` caught).

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("table", "alpha")
        .descriptor("crate", "alpha")
        .descriptor("barrel", "alpha")
        .place_as("table@0", "table", (0.0, 0.0))
        .place_as("crate@0", "crate", (2.0, 0.0))
        .place_as("barrel@0", "barrel", (3.0, 3.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The registered methods an agent's request arrives through (the same pair the other operation
    // tests add): without them `RemoteMethods` does not exist and nothing is callable.
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get(method)
        .cloned()
    else {
        panic!("`{method}` is not registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("grimoire/operation_apply")
        .cloned()
    else {
        panic!("grimoire/operation_apply is not registered");
    };
    app.world_mut()
        .run_system_with(
            id,
            Some(serde_json::json!({
                "operation": operation,
                "parameters": parameters,
                "dry_run": false,
            })),
        )
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

fn yaw_of(app: &mut App, id: &str) -> f32 {
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
    open.map.placements.iter().find(|p| p.id == id).map(|p| p.yaw).unwrap_or(f32::NAN)
}

/// **Two pieces, one facing, one line.**
#[test]
fn two_pieces_turn_with_one_log_line() {
    let mut app = editor("batch_pose_two");
    let before = yaw_of(&mut app, "table@0");
    assert_ne!(before, 90.0, "the fixture starts the table unposed");
    let answer = apply(
        &mut app,
        "placement_set_pose",
        serde_json::json!({"ids": ["table@0", "crate@0"], "yaw": 90.0, "tip": [0, 0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(yaw_of(&mut app, "table@0"), 90.0);
    assert_eq!(yaw_of(&mut app, "crate@0"), 90.0, "both pieces turned");

    let after = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": answer["operation_id"], "dry_run": false}),
    );
    assert_eq!(after["outcome"], "applied", "one undo takes the batch back: {after}");
    assert_ne!(yaw_of(&mut app, "table@0"), 90.0, "the table is unposed again");
    assert_ne!(yaw_of(&mut app, "crate@0"), 90.0, "and so is the crate");
}

/// **A batch that names an unknown piece poses nothing at all**, so a typo cannot half-move a room.
#[test]
fn a_batch_with_one_unknown_id_poses_nothing() {
    let mut app = editor("batch_pose_unknown");
    let before = yaw_of(&mut app, "table@0");
    let answer = apply(
        &mut app,
        "placement_set_pose",
        serde_json::json!({"ids": ["table@0", "nothing_here"], "yaw": 90.0, "tip": [0, 0]}),
    );
    assert_ne!(answer["outcome"], "applied", "the request is refused: {answer}");
    assert_eq!(yaw_of(&mut app, "table@0"), before, "the table was not turned");
}

/// **`id` and `ids` together is a refusal**, not a guess about which wins.
#[test]
fn naming_one_thing_twice_is_refused() {
    let mut app = editor("batch_pose_both");
    let answer = apply(
        &mut app,
        "placement_set_pose",
        serde_json::json!({"id": "table@0", "ids": ["crate@0"], "yaw": 90.0, "tip": [0, 0]}),
    );
    assert_ne!(answer["outcome"], "applied", "naming one thing two ways is refused: {answer}");
    assert_ne!(yaw_of(&mut app, "table@0"), 90.0);
}

/// **A batch whose pieces face different ways is refused**, because one undo could not put them all
/// back with one call: an inverse is replayed as the operation's own parameters.
#[test]
fn a_batch_of_differently_faced_pieces_is_refused() {
    let mut app = editor("batch_pose_mixed");
    // Turn one of them first, so the pair no longer shares an old facing.
    let first = apply(
        &mut app,
        "placement_set_pose",
        serde_json::json!({"id": "crate@0", "yaw": 45.0, "tip": [0, 0]}),
    );
    assert_eq!(first["outcome"], "applied", "{first}");
    let table_before = yaw_of(&mut app, "table@0");
    let answer = apply(
        &mut app,
        "placement_set_pose",
        serde_json::json!({"ids": ["table@0", "crate@0"], "yaw": 90.0, "tip": [0, 0]}),
    );
    assert_ne!(answer["outcome"], "applied", "the batch is refused: {answer}");
    assert_eq!(yaw_of(&mut app, "table@0"), table_before, "nothing was posed");
    assert_eq!(yaw_of(&mut app, "crate@0"), 45.0, "and the crate kept its own facing");
}
