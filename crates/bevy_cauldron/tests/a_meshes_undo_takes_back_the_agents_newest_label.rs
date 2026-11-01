//! **One Cmd+Z on the Meshes door takes back the newest label change, an agent's included.**
//!
//! Cycle 9 gap 1. A person lights a chip on one piece (a snapshot on the Meshes history), then an
//! agent runs `label_set` on another piece. Before the fix that agent write went onto the Map door's
//! stack, so the Meshes Cmd+Z restored the whole-library snapshot taken before the chip, and the
//! agent's label vanished from memory and disk with nothing logged. Now the agent's write is a step
//! on the Meshes history too, so the newest change is the one that goes back first.
//!
//! These drive the human front end: the chip is toggled through the same path a chip click takes,
//! the agent's write arrives as a registered BRP call, and Cmd+Z is a real key chord on the Meshes tab.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures;

fn editor_at(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .look_tokens(&["worn"])
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
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

fn agent_labels(app: &mut App, id: &str, tokens: &[&str]) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "label_set",
            "parameters": {"id": id, "axis": "look", "tokens": tokens},
            "dry_run": false,
        }),
    )
}

/// **A person's chip click, as the chip's caller makes it:** the history entry is taken before the
/// write, and recorded only when the write lands.
fn person_lights_chip(app: &mut App, id: &str, tokens: &[String]) {
    app.world_mut()
        .resource_scope(|world, mut import: Mut<ImportState>| {
            world.resource_scope(|_world, mut project: Mut<Project>| {
                let before = import.snapshot(&project);
                if bevy_cauldron::editor::set_label(&mut import, &mut project, id, "look", tokens) {
                    import.record(before);
                }
            });
        });
    app.update();
}

fn look_of(app: &App, id: &str) -> Vec<String> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not in the live measured library"))
        .look
        .clone()
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **Cmd+Z**, the platform modifier held while Z is pressed.
fn cmd_z(app: &mut App) {
    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(held);
    tap(
        app,
        Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        KeyCode::KeyZ,
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(held);
    app.update();
}

#[test]
fn a_meshes_undo_after_an_agent_label_takes_back_the_agents_label_not_the_persons_chip() {
    let mut app = editor_at("meshes-undo-agent-label");

    // Both pieces start as "plain". The person lights "worn" on the wall; the Meshes history holds that step.
    person_lights_chip(&mut app, "site/wall", &["plain".to_string(), "worn".to_string()]);
    assert_eq!(look_of(&app, "site/wall"), vec!["plain".to_string(), "worn".to_string()], "the chip lit");

    // Then an agent labels the gate. This is the newest change.
    let answer = agent_labels(&mut app, "site/gate", &["worn"]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(look_of(&app, "site/gate"), vec!["worn".to_string()], "the agent's write landed");

    // One Cmd+Z on the Meshes door takes back the newest change: the agent's label on the gate.
    cmd_z(&mut app);

    assert_eq!(
        look_of(&app, "site/gate"),
        vec!["plain".to_string()],
        "the agent's newest label is what Cmd+Z takes back"
    );
    assert_eq!(
        look_of(&app, "site/wall"),
        vec!["plain".to_string(), "worn".to_string()],
        "the person's older chip is still lit: Cmd+Z did not reach past the agent's write"
    );
}
