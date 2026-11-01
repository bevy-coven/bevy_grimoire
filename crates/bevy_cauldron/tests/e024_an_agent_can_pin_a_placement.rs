//! **An agent can pin a placement, and take the pin back, through the same log the author's undo uses.**
//!
//! The regression tests for E-024: pin/unpin was a human-only action (`OwnToggle`, on the
//! unmigrated list). It is now the registered operation `placement_set_pin`. These go through the
//! registered BRP methods, the way the MCP server does, and assert on the map the editor holds,
//! the file on disk, and the log.

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
        serde_json::json!({"operation": "placement_set_pin", "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: Option<&str>) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn an_agent_pins_a_placement_and_the_file_says_so() {
    let mut app = editor("agent_pins");
    let id = held(&app).placements[0].id.clone();
    assert!(!held(&app).placements[0].owned, "the fixture starts unpinned");

    let answer = apply(
        &mut app,
        serde_json::json!({"id": id, "pinned": true, "because": "load-bearing wall"}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"].as_str().expect("an apply returns its id").to_owned();

    let p = &held(&app).placements[0];
    assert!(p.owned, "the editor holds the pin");
    assert_eq!(p.owned_because.as_deref(), Some("load-bearing wall"));
    let d = &on_disk(&app).placements[0];
    assert!(d.owned, "the file holds the pin");
    assert_eq!(d.owned_because.as_deref(), Some("load-bearing wall"));

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert!(!held(&app).placements[0].owned, "undo takes the pin back in the editor");
    assert!(!on_disk(&app).placements[0].owned, "undo takes the pin back on disk");
}

#[test]
fn a_pin_without_a_reason_is_refused_and_nothing_changes() {
    let mut app = editor("agent_pin_no_reason");
    let id = held(&app).placements[0].id.clone();

    let answer = apply(&mut app, serde_json::json!({"id": id, "pinned": true}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert!(!held(&app).placements[0].owned, "a refused pin changes nothing");
    assert!(!on_disk(&app).placements[0].owned, "a refused pin writes nothing");
}

#[test]
fn an_unknown_placement_is_refused() {
    let mut app = editor("agent_pin_unknown");
    let answer = apply(
        &mut app,
        serde_json::json!({"id": "no-such-piece", "pinned": true, "because": "x"}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
}

/// **Typing `o` into a name field does not pin.**
///
/// `O` is `Action::OwnToggle` on the Map, and the Map's name field (`grouping`) takes the keyboard
/// while it is open. With the field open the letter must land in the field: no placement changes,
/// nothing enters the undo stack, and the pin verb never answers. The receipt says so — a pin verb
/// with no target writes a note, and that note must not appear.
#[test]
fn typing_o_into_a_name_field_does_not_pin() {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy_cauldron::editor::EditorState;

    let mut app = editor("typing-guard");
    *app.world_mut().resource_mut::<bevy_cauldron::tiles::Mode>() =
        bevy_cauldron::tiles::Mode::Map;
    app.update();
    app.world_mut().resource_mut::<EditorState>().grouping = Some(String::new());
    app.update();

    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyO,
            logical_key: Key::Character("o".into()),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.update();

    let state = app.world().resource::<EditorState>();
    assert_eq!(
        state.grouping.as_deref(),
        Some("o"),
        "with the name field open the letter lands in the field"
    );
    assert!(
        !state.status.note_text().contains("pin"),
        "the pin verb must not answer while the field holds the keyboard; the receipt said `{}`",
        state.status.note_text()
    );
    assert_eq!(
        state.undo_depth(),
        0,
        "a letter typed into a name field is not an undoable step"
    );
    assert!(
        held(&app).placements.iter().all(|p| !p.owned),
        "no placement may be pinned by a letter typed into a name"
    );
}
