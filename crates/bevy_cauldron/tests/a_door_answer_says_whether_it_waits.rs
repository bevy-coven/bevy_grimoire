//! **A door's answer over BRP says what happened (cycle 8, item 1).**
//!
//! # What the bug was
//!
//! `grimoire/operation_apply` answered `"applied"` for `door_open` whenever the operation's plan and
//! commit succeeded. But the editor can still refuse the request after that answer: a door already
//! waiting on the leaving question (`request_door`) refuses, and a dirty map makes the request wait
//! on the person. The agent was told the door was open when it was not, or that it had moved when
//! the person had still to answer a question.
//!
//! # What is being proved
//!
//! 1. **A dirty map asked to leave is answered as waiting for the person**, with the reason, and the
//!    request is stored so the editor asks the leaving question.
//! 2. **A door asked for while the leaving question is up is refused**, not answered as applied, and
//!    the request is not stored.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::tiles::Door;
use serde_json::json;

use crate::fixtures;

/// **The editor, with the bridge open.** Same plugin graph as `an_agent_change_is_on_the_receipt`.
fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

/// **Call one of the editor's BRP commands through the registered method**, as the MCP server does.
fn call(
    app: &mut App,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

/// **Ask for a door, for real** (`dry_run` is `true` by default on this method).
fn open_door(app: &mut App, door: &str) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        json!({"operation": "door_open", "parameters": {"door": door}, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("the operation answers: {e}"))
}

#[test]
fn a_dirty_map_asked_to_leave_is_answered_as_waiting_for_the_person() {
    let mut app = editor("door_bri_waits");
    app.world_mut().resource_mut::<OpenMap>().map.note = Some("an edit that is not saved".to_owned());
    let answer = open_door(&mut app, "kit");
    assert_eq!(answer["outcome"], json!("waiting"), "{answer}");
    assert_eq!(
        answer["waiting_on"],
        json!("the map has unsaved changes"),
        "the answer says what the person is being asked about: {answer}"
    );
    assert_eq!(
        app.world().resource::<EditorState>().door_request.as_ref().map(|r| r.door),
        Some(Door::Kit),
        "the request is stored, so the editor asks the leaving question"
    );
}

#[test]
fn a_door_asked_for_while_the_leaving_question_is_up_is_refused() {
    let mut app = editor("door_bri_leaving");
    app.world_mut().resource_mut::<EditorState>().leaving = true;
    let answer = open_door(&mut app, "kit");
    assert_eq!(answer["outcome"], json!("refused"), "{answer}");
    assert!(
        answer["refusal"].as_str().unwrap_or_default().contains("\"Leave this map?\""),
        "the refusal names the question the person must answer first: {answer}"
    );
    assert!(
        app.world().resource::<EditorState>().door_request.is_none(),
        "a refused request is not stored"
    );
}
