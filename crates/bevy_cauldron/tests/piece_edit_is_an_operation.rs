//! **Sending a piece to the Meshes tab is an operation, so the define key and an agent's call are one** (E-002, #129 slice 37).
//!
//! `EditTile` leaves the allowlist of unmigrated actions. The define key on the Map (`Cmd+Delete`) resolves which piece (the one under
//! the cursor, or the one armed in PLACE) and dispatches `piece_edit` with its id; an agent calls the same operation through
//! `grimoire/operation_apply`, naming the piece. Either way the Meshes tab opens on the piece and one line is logged.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Map over a kit with two library pieces.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("piece_edit_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .descriptor("wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn apply(app: &mut App, operation: &str, parameters: Value) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered");
    };
    app.world_mut()
        .run_system_with(id, Some(json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

fn tap(app: &mut App, held: &[KeyCode], key: KeyCode) {
    for h in held {
        send(app, *h, ButtonState::Pressed);
    }
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
    for h in held {
        send(app, *h, ButtonState::Released);
    }
    app.update();
    app.update();
}

fn logged(app: &App, operation: &str) -> Vec<(String, Value)> {
    let root = app.world().resource::<Project>().root.clone();
    let text = std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(&root)).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == operation)
        .map(|v| (v["source"].as_str().unwrap_or_default().to_owned(), v))
        .collect()
}

fn selected(app: &App) -> Option<String> {
    app.world().resource::<ImportState>().selected_library_id.clone()
}

/// **The define key and an agent open the Meshes tab on a piece through one operation**: the tab, the selection and one logged line.
#[test]
fn the_define_key_and_an_agent_send_a_piece_to_the_meshes_tab_through_one_operation() {
    let mut app = editor("edit");
    assert!(*app.world().resource::<Mode>() == Mode::Map, "the editor opens on the Map");

    // The piece armed in PLACE is the key's subject when nothing is under the cursor.
    let said = apply(&mut app, "brush_arm", json!({"descriptor": "floor"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    app.update();
    tap(&mut app, &[bevy_cauldron::keys::MOD_KEYS[0]], bevy_cauldron::keys::REMOVE_KEY);
    assert!(*app.world().resource::<Mode>() == Mode::Meshes, "the define key opens the Meshes tab");
    assert_eq!(selected(&app).as_deref(), Some("floor"), "on the piece armed in PLACE");
    let lines = logged(&app, "piece_edit");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["id"], "floor", "{:?}", lines[0]);

    // An agent names the piece, and the editor's own tab and selection are what move.
    app.world_mut().insert_resource(Mode::Map);
    app.world_mut().resource_mut::<ImportState>().selected_library_id = None;
    app.update();
    let said = apply(&mut app, "piece_edit", json!({"id": "wall"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!(*app.world().resource::<Mode>() == Mode::Meshes, "an agent opens the editor's own Meshes tab");
    assert_eq!(selected(&app).as_deref(), Some("wall"));
    let lines = logged(&app, "piece_edit");
    assert_eq!(lines.len(), 2, "the agent's call is logged too");
    assert_eq!(lines[1].0, "agent", "{:?}", lines[1]);

    // A piece the library does not have is refused by name and moves nothing; so is a call that names none.
    app.world_mut().insert_resource(Mode::Map);
    let refused = apply(&mut app, "piece_edit", json!({"id": "no_such_piece"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert!(*app.world().resource::<Mode>() == Mode::Map, "a refused call does not switch tabs");
    let refused = apply(&mut app, "piece_edit", json!({}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
}

/// **The action names the operation it dispatches**, which is what takes it off the allowlist.
#[test]
fn the_define_action_names_the_operation_it_dispatches() {
    assert_eq!(Action::EditTile.operation(), Some("piece_edit"));
}
