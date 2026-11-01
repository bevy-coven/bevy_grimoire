//! **A map is renamed through the registry, so an agent and the N key do the same thing.**
//!
//! `N` used to change the open map's name in memory, and the file only followed on the next save.
//! It is now the registered operation `map_rename`: the name is validated, the file moves at once,
//! a taken name is refused before anything is written, and the rename is undone by the same log
//! the author's undo reads. These go through the registered BRP methods, the way the MCP server does.

use bevy::input::keyboard::{Key, KeyboardInput};
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
        serde_json::json!({"operation": "map_rename", "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: Option<&str>) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn held_name(app: &App) -> String {
    app.world().resource::<OpenMap>().map.name.clone()
}

fn held_path(app: &App) -> std::path::PathBuf {
    app.world().resource::<OpenMap>().map_path.clone()
}

fn maps_dir(app: &App) -> std::path::PathBuf {
    held_path(app)
        .parent()
        .expect("a map file has a directory")
        .to_path_buf()
}

#[test]
fn an_agent_renames_a_map_and_the_file_moves_with_it() {
    let mut app = editor("map_renames");
    let before = held_path(&app);
    assert!(before.exists(), "the fixture map is on disk");

    let answer = apply(&mut app, serde_json::json!({"name": "Town Two"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    assert_eq!(
        held_name(&app),
        "town_two",
        "the name is forced to snake_case"
    );
    let after = held_path(&app);
    assert_eq!(after, maps_dir(&app).join("town_two.map.ron"));
    assert!(after.exists(), "the new file is written at once");
    assert!(!before.exists(), "the old file is removed at once");

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(held_name(&app), "m", "undo puts the name back");
    assert_eq!(held_path(&app), before, "undo puts the path back");
    assert!(before.exists(), "undo puts the file back");
    assert!(!after.exists(), "undo removes the renamed file");
}

#[test]
fn a_name_another_map_has_is_refused_and_nothing_moves() {
    let mut app = editor("map_collides");
    let before = held_path(&app);
    let taken = maps_dir(&app).join("taken.map.ron");
    std::fs::copy(&before, &taken).expect("a second map to collide with");
    let taken_text = std::fs::read_to_string(&taken).expect("the other map reads back");

    let answer = apply(&mut app, serde_json::json!({"name": "taken"}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(held_name(&app), "m", "a refused rename keeps the name");
    assert!(
        before.exists(),
        "a refused rename leaves the file where it was"
    );
    assert_eq!(
        std::fs::read_to_string(&taken).expect("the other map is still there"),
        taken_text,
        "a refused rename does not touch the map it collided with"
    );
}

#[test]
fn an_empty_name_is_refused_and_nothing_moves() {
    let mut app = editor("map_empty");
    let before = held_path(&app);
    let answer = apply(&mut app, serde_json::json!({"name": "   "}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(held_name(&app), "m");
    assert!(before.exists());
}

#[test]
fn an_unknown_field_is_refused() {
    let mut app = editor("map_field");
    let answer = apply(&mut app, serde_json::json!({"name": "x", "nmae": "y"}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(held_name(&app), "m", "a refused call changes nothing");
}

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];

/// **One key press, as the window delivers it**: pressed, then released.
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

/// **The N key, as a person presses it**: the name field opens, each letter arrives as a key
/// press, and the caller decides how the field closes.
fn open_name_and_type(app: &mut App, name: &str) {
    tap(app, Key::Character("n".into()), KeyCode::KeyN);
    for c in name.chars() {
        let code = LETTERS[(c as u8 - b'a') as usize];
        tap(app, Key::Character(c.to_string().into()), code);
    }
}

/// **N, a name, Enter**: the file moves at once, the way an agent's `map_rename` moves it.
#[test]
fn n_then_a_name_then_enter_moves_the_file_through_the_registry() {
    let mut app = editor("n_enter");
    let before = held_path(&app);
    open_name_and_type(&mut app, "lakeside");
    tap(&mut app, Key::Enter, KeyCode::Enter);
    assert_eq!(
        held_name(&app),
        "lakeside",
        "Enter must commit the typed name"
    );
    let after = held_path(&app);
    assert_eq!(after, maps_dir(&app).join("lakeside.map.ron"));
    assert!(
        after.exists(),
        "the new file must be on disk, not waiting for a save"
    );
    assert!(
        !before.exists(),
        "the old file must be removed by the rename"
    );
}

/// **N, a name, Escape**: nothing is renamed and nothing is written.
#[test]
fn n_then_escape_renames_nothing() {
    let mut app = editor("n_escape");
    let before = held_path(&app);
    open_name_and_type(&mut app, "lakeside");
    tap(&mut app, Key::Escape, KeyCode::Escape);
    assert_eq!(held_name(&app), "m", "Escape must leave the name alone");
    assert_eq!(held_path(&app), before);
    assert!(before.exists(), "the old file must still be there");
    assert!(
        !maps_dir(&app).join("lakeside.map.ron").exists(),
        "Escape must write nothing"
    );
}

/// **`grimoire/project_root` names the map the author renamed, while it is open.** The rename changes
/// the open map and the file, and the project's own name is only brought up to date on a crossing,
/// so an agent asking which map is open was told the old name.
#[test]
fn project_root_names_the_renamed_map_while_it_is_open() {
    let mut app = editor("project_root_renamed");
    let answer = apply(&mut app, serde_json::json!({"name": "Town Two"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");

    let root = call(&mut app, "grimoire/project_root", serde_json::json!({}));
    assert_eq!(
        root["map"], "town_two",
        "project_root says the map is `{}`, but the author renamed it to `town_two`",
        root["map"]
    );
}
