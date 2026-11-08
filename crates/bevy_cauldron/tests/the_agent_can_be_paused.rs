//! **A person can pause the agent's writes, and the agent is told why** (UI-60, #246).
//!
//! An agent operation is attributed (the history row says the Familiar) and takes back with one undo; View > Pause agent writes
//! makes the next write refuse with a problem, changes nothing and logs nothing, lets a preview through, and refuses the
//! agent's undo too; turning it off lets the agent write again.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::familiar::{AgentPause, PauseItem};
use bevy_cauldron::harness;
use bevy_gooey::{Checked, ValueChange};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("table", "alpha").place_as("table@0", "table", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The registered methods an agent's request arrives through.
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("`{method}` is not registered");
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"));
    for _ in 0..3 {
        app.update();
    }
    answer
}

fn turn_to(app: &mut App, yaw: f32, dry_run: bool) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_yaw", "parameters": {"id": "table@0", "yaw": yaw}, "dry_run": dry_run}),
    )
}

fn yaw_of(app: &App) -> f32 {
    app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements.iter().find(|p| p.id == "table@0").map(|p| p.yaw).expect("the table")
}

/// The person chose View > Pause agent writes (or chose it again).
fn pause(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PauseItem>>().single(world).expect("the View menu carries one Pause agent writes row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    for _ in 0..4 {
        app.update();
    }
}

fn newest_row_is_the_agents(app: &App) -> bool {
    let state = app.world().resource::<EditorState>();
    let depth = state.undo_depth();
    state.history_window(depth, depth).first().is_some_and(|row| row.agent)
}

#[test]
fn an_agent_operation_is_attributed_and_takes_back_and_a_pause_refuses_the_next_write() {
    let mut app = editor("agent_pause");
    assert_eq!(app.world().resource::<AgentPause>(), &AgentPause(false), "the editor starts unpaused");

    let answer = turn_to(&mut app, 90.0, false);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(yaw_of(&app), 90.0);
    assert!(newest_row_is_the_agents(&app), "the history row is the agent's, which the panel shows as the Familiar");

    let depth = app.world().resource::<EditorState>().undo_depth();
    pause(&mut app, true);
    assert!(app.world().resource::<AgentPause>().0, "the row switches it on");
    let ticked = {
        let world = app.world_mut();
        world.query_filtered::<Has<Checked>, With<PauseItem>>().single(world).expect("one row")
    };
    assert!(ticked, "and is ticked");

    let refused = turn_to(&mut app, 180.0, false);
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert!(refused["refusal"].as_str().is_some_and(|why| why.contains("paused") && why.contains("placement_set_yaw")), "{refused}");
    assert_eq!(yaw_of(&app), 90.0, "nothing changed");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), depth, "and nothing was recorded");
    let said = app.world().resource::<EditorState>().status.line().to_owned();
    assert!(said.contains("paused"), "the status line says why, as a problem: `{said}`");

    let preview = turn_to(&mut app, 180.0, true);
    assert_ne!(preview["outcome"], "refused", "a preview is not a write: {preview}");
    assert_eq!(yaw_of(&app), 90.0, "and changes nothing");

    pause(&mut app, false);
    let again = turn_to(&mut app, 180.0, false);
    assert_eq!(again["outcome"], "applied", "turned off, the agent writes again: {again}");
    assert_eq!(yaw_of(&app), 180.0);
}

#[test]
fn a_pause_refuses_the_agents_undo_too() {
    let mut app = editor("agent_pause_undo");
    let done = turn_to(&mut app, 90.0, false);
    assert_eq!(done["outcome"], "applied", "{done}");
    pause(&mut app, true);
    let refused = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": done["operation_id"], "dry_run": false}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert_eq!(yaw_of(&app), 90.0, "the change stands");
    pause(&mut app, false);
    let taken = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": done["operation_id"], "dry_run": false}));
    assert_eq!(taken["outcome"], "applied", "{taken}");
    assert_ne!(yaw_of(&app), 90.0, "unpaused, the agent's own undo takes it back");
}
