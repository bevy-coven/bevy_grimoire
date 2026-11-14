//! **The Meshes tab's rescan is an operation, so `R` and an agent's call are one** (E-002, #129 slice 33).
//!
//! `Rescan` leaves the allowlist of unmigrated actions. `R` dispatches `kit_rescan`; an agent calls the same operation through
//! `grimoire/operation_apply`. Either way the pack's directory is read again and a mesh that arrived since the editor opened
//! becomes a candidate.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Meshes tab, over a fixture kit with one piece in its library.
fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("kit_rescan_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    (app, root)
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

fn tap(app: &mut App, key: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
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

fn candidates(app: &App) -> usize {
    app.world().resource::<ImportState>().candidates.len()
}

/// **A mesh arrives while the editor is open**: a copy of the fixture's own, in a pack of its own.
fn a_mesh_arrives(root: &std::path::Path, pack: &str) {
    let from = root.join("assets/alpha/floor.glb");
    let to = root.join("assets").join(pack);
    std::fs::create_dir_all(&to).unwrap_or_else(|e| panic!("{to:?}: {e}"));
    std::fs::copy(&from, to.join("arrived.glb")).unwrap_or_else(|e| panic!("{from:?}: {e}"));
}

/// **`R` and an agent read the directory again through one operation**: nothing sees a new mesh until one asks.
#[test]
fn r_and_an_agent_rescan_the_kits_mesh_directory_through_one_operation() {
    let (mut app, root) = editor("rescan");
    let before = candidates(&app);

    a_mesh_arrives(&root, "first_pack");
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(candidates(&app), before, "a mesh that arrives is not seen until something rescans");

    tap(&mut app, KeyCode::KeyR);
    assert_eq!(candidates(&app), before + 1, "R reads the directory again and finds the new mesh");
    let lines = logged(&app, "kit_rescan");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    a_mesh_arrives(&root, "second_pack");
    let said = apply(&mut app, "kit_rescan", json!({}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(candidates(&app), before + 2, "an agent's rescan reads the editor's own candidate list");
    assert_eq!(logged(&app, "kit_rescan").len(), 2, "the agent's call is logged too");

    let refused = apply(&mut app, "kit_rescan", json!({"pack": "x"}));
    assert_eq!(refused["outcome"], "refused", "it takes no fields: {refused}");
}

/// **A rescan whose log line cannot be written scans nothing** (E-002, #129 slice 33 review): the operation is refused after its
/// commit, so the answer the agent reads is "refused" and the editor's own state must not move either.
#[test]
fn a_rescan_whose_log_line_cannot_be_written_scans_nothing() {
    let (mut app, root) = editor("unlogged");
    let before = candidates(&app);
    a_mesh_arrives(&root, "unlogged_pack");
    let log = bevy_cauldron::ops::oplog_path_at(&root);
    let _ = std::fs::remove_file(&log);
    std::fs::create_dir_all(&log).expect("a directory where the log goes");
    let said = apply(&mut app, "kit_rescan", json!({}));
    assert_eq!(said["outcome"], "refused", "a change that cannot be recorded is refused: {said}");
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(candidates(&app), before, "and a refused call does not read the directory");
}

/// **The action names the operation it dispatches**, which is what takes it off the allowlist.
#[test]
fn the_rescan_action_names_the_operation_it_dispatches() {
    assert_eq!(Action::Rescan.operation(), Some("kit_rescan"));
}
