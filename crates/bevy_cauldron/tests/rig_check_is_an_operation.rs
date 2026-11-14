//! **The Anim tab's project-wide check is an operation, so `C` and an agent's call are one** (E-002, #129 slice 32).
//!
//! `CheckAllRigs` leaves the allowlist of unmigrated actions. `C` dispatches `rig_check_all`; an agent calls the same operation
//! through `grimoire/operation_apply`. Either way the bench shows the project-wide summary and the watcher is asked to measure
//! what has no report yet, and the status line says how many.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::anim_tab::{BenchState, View};
use bevy_cauldron::anim_watch::BenchReports;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use serde_json::{Value, json};

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// An editor on the Anim tab, over the shipped rig manifest.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("rig_check_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    std::fs::copy(workspace_asset("grimoire/rigs.yaml"), root.join("assets/grimoire/rigs.yaml")).expect("rigs.yaml");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Anim).unwrap_or_else(|e| panic!("{e}"));
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

fn faces_the_summary(app: &App) -> bool {
    app.world().resource::<BenchState>().view == View::All
}

fn put_on_one_rig(app: &mut App) {
    let said = apply(app, "rig_walk", json!({"to": 0}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(!faces_the_summary(app), "walking to a rig is looking at one");
}

fn rig_count(app: &App) -> usize {
    app.world().resource::<BenchState>().names().len()
}

/// **`C` and an agent show the summary through one operation**, naming the rigs with no report; a rig that is measured is not
/// asked for again.
#[test]
fn c_and_an_agent_check_every_rig_through_one_operation() {
    let mut app = editor("check");
    let rigs = rig_count(&app);
    assert!(rigs > 0, "the shipped manifest has rigs");
    put_on_one_rig(&mut app);

    tap(&mut app, KeyCode::KeyC);
    assert!(faces_the_summary(&app), "C shows the project-wide summary");
    let lines = logged(&app, "rig_check_all");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    put_on_one_rig(&mut app);
    let measured_before = app.world().resource::<BenchReports>().by_rig.len();
    let said = apply(&mut app, "rig_check_all", json!({}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!(faces_the_summary(&app), "an agent shows the editor's own summary");
    // The answer's sentence is the operation's own: `all rigs measured`, or how many it queued.
    let want = {
        let left = rigs - measured_before;
        if left == 0 { "all rigs measured".to_owned() } else { format!("measuring {left} rig(s)...") }
    };
    assert_eq!(said["summary"], want.as_str(), "it queues exactly the rigs that have no report: {said}");
    let status = app.world().resource::<BenchState>().status.line().to_string();
    assert_eq!(status, want, "the status line is the operation's own sentence");
    assert_eq!(logged(&app, "rig_check_all").len(), 2, "the agent's call is logged too");

    // Given the watcher time, every rig has a report and a second check has nothing to ask for.
    for _ in 0..400 {
        app.update();
    }
    put_on_one_rig(&mut app);
    let said = apply(&mut app, "rig_check_all", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    let want = {
        let left = rigs - app.world().resource::<BenchReports>().by_rig.len();
        if left == 0 { "all rigs measured".to_owned() } else { format!("measuring {left} rig(s)...") }
    };
    assert_eq!(said["summary"], want.as_str(), "what is queued is what has no report: {said}");

    let refused = apply(&mut app, "rig_check_all", json!({"rig": "x"}));
    assert_eq!(refused["outcome"], "refused", "it takes no fields: {refused}");
}

/// **The action names the operation it dispatches**, which is what takes it off the allowlist.
#[test]
fn the_check_action_names_the_operation_it_dispatches() {
    assert_eq!(Action::CheckAllRigs.operation(), Some("rig_check_all"));
}
