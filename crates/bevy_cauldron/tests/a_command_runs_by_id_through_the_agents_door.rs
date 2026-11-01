//! **A command runs by id for an agent exactly as its key runs for a person** (UI-90, #256).
//!
//! `grimoire/run_command { id }` is the agent's door onto the one command registry
//! (`commands.rs`): the id names an `Action`, the editor presses that action's chord, and the
//! same dispatcher, with the same gates, answers. So the agent cannot do what the key would refuse,
//! and a verb needs no second front end. `grimoire/state.ui` says where the person is looking, so an
//! agent knows which commands are live before it asks.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::commands;
use bevy_cauldron::keys::Live;
use bevy_cauldron::project::OpenMap;
use serde_json::json;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .mounted_descriptor("market_stall_awning_with_lanterns", "alpha", "worktop")
        .surface_descriptor("worktop_table", "alpha", "worktop")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn map(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

/// **Lay a run of fence the way the editor's own tests do**, so there is one thing to take back.
fn lay_a_run(app: &mut App) {
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
}

/// **Cmd+Z as a person presses it.**
fn cmd_z(app: &mut App) {
    let m = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(m);
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyZ,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(m);
    app.update();
}

/// **The call, then the frames the key needs to land and let go.**
fn run(app: &mut App, id: &str) -> serde_json::Value {
    let answer = call(app, "grimoire/run_command", json!({"id": id}));
    for _ in 0..4 {
        app.update();
    }
    answer
}

/// **`edit.undo` over the door undoes exactly as Cmd+Z does**, and the state says the person is in
/// the workbench. The twin app takes the key; the two maps must be the same.
#[test]
fn an_agent_runs_edit_undo_exactly_as_cmd_z_does() {
    let mut by_key = editor("cmdkey");
    let mut by_id = editor("cmdid");
    let start = map(&by_id);
    for app in [&mut by_key, &mut by_id] {
        lay_a_run(app);
    }
    let laid = map(&by_id);
    assert_ne!(laid.placements.len(), start.placements.len(), "the run laid nothing; nothing to undo");

    cmd_z(&mut by_key);
    let answer = run(&mut by_id, "edit.undo");

    assert_eq!(answer["outcome"], json!("ran"), "answer: {answer}");
    assert_eq!(answer["id"], json!("edit.undo"));
    assert_eq!(
        map(&by_id).placements.len(),
        map(&by_key).placements.len(),
        "`edit.undo` must leave the map as Cmd+Z leaves it"
    );
    assert_eq!(map(&by_id).placements.len(), start.placements.len(), "the run is taken back");

    let state = call(&mut by_id, "grimoire/state", json!({}));
    assert_eq!(state["ui"]["screen"], json!("workbench"), "state: {state}");
    assert_eq!(state["editor"]["redo_depth"], json!(1), "the undone run is on the redo stack");

    let again = run(&mut by_id, "edit.redo");
    assert_eq!(again["outcome"], json!("ran"), "answer: {again}");
    assert_eq!(map(&by_id).placements.len(), laid.placements.len(), "`edit.redo` puts the run back");
}

/// **A command the key would refuse is refused, with the registry's own reason, and nothing runs.**
#[test]
fn a_command_that_is_not_enabled_is_refused_with_its_reason() {
    let mut app = editor("cmdoff");
    lay_a_run(&mut app);
    let live = *app.world().resource::<Live>();
    let (command, why) = commands::all()
        .into_iter()
        .find_map(|c| c.enabled(live).err().map(|why| (c, why)))
        .expect("some command is disabled in the map's idle state");
    let before = map(&app);

    let answer = run(&mut app, command.id);

    assert_eq!(answer["outcome"], json!("refused"), "answer: {answer}");
    assert_eq!(answer["why"], json!(why), "the reason is the registry's: {answer}");
    assert_eq!(map(&app), before, "a refused command changes nothing");
}

/// **An id nobody registered is refused and the answer helps**: it names the ids that contain the text.
#[test]
fn an_unknown_id_is_refused_and_names_the_nearest_ids() {
    let mut app = editor("cmdunknown");
    let answer = run(&mut app, "edit.undoo");
    assert_eq!(answer["outcome"], json!("refused"), "answer: {answer}");
    let near: Vec<&str> = answer["did_you_mean"]
        .as_array()
        .unwrap_or_else(|| panic!("no did_you_mean: {answer}"))
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(near.contains(&"edit.undo"), "`edit.undo` is the nearest id: {near:?}");
}

/// **`grimoire/state.ui` says where the person is looking**: the screen, the workspace, the panels it
/// has and the region that has the keys.
#[test]
fn the_state_says_the_screen_the_workspace_the_panels_and_who_has_the_keys() {
    let mut app = editor("cmdstate");
    let state = call(&mut app, "grimoire/state", json!({}));
    let ui = &state["ui"];
    assert_eq!(ui["screen"], json!("workbench"), "ui: {ui}");
    assert_eq!(ui["workspace"], json!("MAP"), "ui: {ui}");
    assert_eq!(ui["panels"], json!(["MAP"]), "the Map door shows one panel: {ui}");
    assert_eq!(ui["focus"], json!("viewport"), "nothing holds the keys but the viewport: {ui}");
}
