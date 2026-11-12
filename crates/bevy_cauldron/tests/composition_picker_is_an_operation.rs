//! **The composition picker's cursor is an operation, so its keys and an agent's call are one** (E-002, #129 slice 22).
//!
//! Three verbs leave the allowlist of unmigrated actions: `PickComposition` (`P`, which gives the arrows to the picker
//! and takes them back) and `CompositionPrev` and `CompositionNext` (Shift+up and Shift+down, which walk it). They
//! dispatch `composition_picker_toggle` and `composition_picker_walk`; an agent calls the same two through
//! `grimoire/operation_apply`, and the editor's own `Picker` holds the answer either way.
//!
//! **The walk clamps at the ends**, where it once wrapped: the rule now lives in the operation, once, and the key and the
//! agent cannot disagree about it.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::compose::picker::Picker;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("composition_picker_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .composition("arch", &[("post_a", "site/wall", (0.0, 0.0)), ("post_b", "site/wall", (2.0, 0.0))])
        .composition("door", &[("post_a", "site/wall", (0.0, 0.0)), ("post_b", "site/wall", (1.0, 0.0))])
        .composition("gate", &[("post_a", "site/wall", (0.0, 0.0)), ("post_b", "site/wall", (3.0, 0.0))])
        .place("site/wall", (0.0, 0.0))
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

/// **A key as a person presses it**, with `held` down around it.
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

fn picker(app: &App) -> Picker {
    app.world().resource::<Picker>().clone()
}

/// **`P` gives the arrows to the picker through the operation**, in the log as the person's own line, and an agent's
/// call to the same operation gives them back.
#[test]
fn p_toggles_the_picker_through_the_operation_and_an_agent_toggles_it_back() {
    let mut app = editor("toggle");
    assert!(!picker(&app).active, "a session opens with the arrows on the palette");
    tap(&mut app, &[], KeyCode::KeyP);
    assert!(picker(&app).active, "P gives the arrows to the picker");
    let lines = logged(&app, "composition_picker_toggle");
    assert_eq!(lines.len(), 1, "the key is one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    let said = apply(&mut app, "composition_picker_toggle", json!({}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!(!picker(&app).active, "and the same operation hands the arrows back");
}

/// **Shift+down walks the picker through the operation, and the walk clamps at the ends**; an agent walks it back.
#[test]
fn the_walk_keys_and_an_agent_move_one_cursor_and_clamp_at_the_ends() {
    let mut app = editor("walk");
    tap(&mut app, &[], KeyCode::KeyP);
    assert_eq!(picker(&app).selected, 0);
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowDown);
    assert_eq!(picker(&app).selected, 1, "Shift+down walks to the next composition");
    let lines = logged(&app, "composition_picker_walk");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].1["params"], json!({"by": "next"}), "{:?}", lines[0]);

    let said = apply(&mut app, "composition_picker_walk", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(picker(&app).selected, 2, "an agent's walk moves the editor's own cursor");
    apply(&mut app, "composition_picker_walk", json!({"by": "next"}));
    assert_eq!(picker(&app).selected, 2, "and it stops at the end of the list rather than wrapping");
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowUp);
    assert_eq!(picker(&app).selected, 1, "Shift+up walks back");
    apply(&mut app, "composition_picker_walk", json!({"by": "prev"}));
    apply(&mut app, "composition_picker_walk", json!({"by": "prev"}));
    assert_eq!(picker(&app).selected, 0, "and stops at the start");
    let refused = apply(&mut app, "composition_picker_walk", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
}

/// **The three actions name their operations**, which is what takes them off the allowlist.
#[test]
fn the_three_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::PickComposition.operation(), Some("composition_picker_toggle"));
    assert_eq!(Action::CompositionPrev.operation(), Some("composition_picker_walk"));
    assert_eq!(Action::CompositionNext.operation(), Some("composition_picker_walk"));
}
