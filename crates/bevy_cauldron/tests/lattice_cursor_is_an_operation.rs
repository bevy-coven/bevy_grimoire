//! **The lattice grid's cursor is moved by an operation, so a key and an agent's call are one** (E-002, #129 slice 27).
//!
//! Six verbs leave the allowlist of unmigrated actions: `CellLeft`, `CellRight`, `CellForward` and `CellBack` (`F`, `H`,
//! `T` and `G`, which step the selected cell along a layer) and `LayerDown` and `LayerUp` (`[` and `]`, which step the
//! layer the grid shows). They dispatch `lattice_cursor_step`; an agent calls the same operation through
//! `grimoire/operation_apply`, and the editor's own `CellEdit` holds the answer either way. A cursor is what a person is
//! looking at, so a step writes no file and takes no undo step.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{CellEdit, Mode};
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Meshes tab over a piece two cells wide and deep (`site/hall`) and a tall one (`site/tower`).
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("lattice_cursor_{tag}_{}", std::process::id()))
        .sized_descriptor("site/hall", "alpha", 2.0, 2.0)
        .tall_descriptor("site/tower", "alpha", 2.0)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
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

/// **A key as a person presses it**, down and up around two frames.
fn tap(app: &mut App, key: KeyCode) {
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
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

/// The grid's divisions for the selected piece, as the editor derives them.
fn divisions(app: &App, id: &str) -> (u32, u32, u32) {
    let project = app.world().resource::<Project>();
    let piece = project.library.get(id).expect("the fixture's piece");
    project.divisions_of(piece).expect("a lattice the editor can hold")
}

fn at(app: &App) -> Option<(u32, u32, u32)> {
    app.world().resource::<CellEdit>().at
}

fn layer(app: &App) -> u32 {
    app.world().resource::<CellEdit>().layer
}

/// **With no piece selected there is no grid, so the call is refused**, and nothing moves.
#[test]
fn an_agent_cannot_step_a_cursor_on_a_grid_that_is_not_there() {
    let mut app = editor("none");
    let refused = apply(&mut app, "lattice_cursor_step", json!({"by": "right"}));
    assert_eq!(refused["outcome"], "refused", "no piece is selected, so there is no grid: {refused}");
    assert_eq!(at(&app), None);
}

/// **The four cell keys and an agent step one cursor, and the walk clamps at the grid's edges.**
#[test]
fn the_cell_keys_and_an_agent_step_one_cursor_and_clamp_at_the_edges() {
    let mut app = editor("cells");
    let picked = apply(&mut app, "piece_select", json!({"id": "site/hall"}));
    assert_eq!(picked["outcome"], "applied", "{picked}");
    let (dx, _, dz) = divisions(&app, "site/hall");
    assert!(dx >= 2 && dz >= 2, "the fixture's piece is at least two cells each way: {dx}x{dz}");
    assert_eq!(at(&app), None, "no cell is selected until a step");

    tap(&mut app, KeyCode::KeyH);
    assert_eq!(at(&app), Some((1, 0, 0)), "right steps the first cell along x");
    let lines = logged(&app, "lattice_cursor_step");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "right", "{:?}", lines[0]);

    let said = apply(&mut app, "lattice_cursor_step", json!({"by": "forward"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(at(&app), Some((1, 0, 1)), "an agent's step moves the editor's own cursor along z");
    tap(&mut app, KeyCode::KeyT);
    assert_eq!(at(&app), Some((1, 0, (2u32).min(dz - 1))), "forward steps along z and stops at the edge");
    for _ in 0..dz {
        apply(&mut app, "lattice_cursor_step", json!({"by": "forward"}));
    }
    assert_eq!(at(&app).map(|c| c.2), Some(dz - 1), "the cursor stops at the far edge");
    for _ in 0..dz {
        tap(&mut app, KeyCode::KeyG);
    }
    assert_eq!(at(&app).map(|c| c.2), Some(0), "back steps to the near edge and no further");
    for _ in 0..=dx {
        apply(&mut app, "lattice_cursor_step", json!({"by": "right"}));
    }
    assert_eq!(at(&app).map(|c| c.0), Some(dx - 1));
    for _ in 0..=dx {
        tap(&mut app, KeyCode::KeyF);
    }
    assert_eq!(at(&app).map(|c| c.0), Some(0), "left stops at the first column");
    let refused = apply(&mut app, "lattice_cursor_step", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
}

/// **The layer keys and an agent step the layer the grid shows, and the cursor rides the layer.**
#[test]
fn the_layer_keys_and_an_agent_step_one_layer() {
    let mut app = editor("layers");
    apply(&mut app, "piece_select", json!({"id": "site/tower"}));
    let (_, dy, _) = divisions(&app, "site/tower");
    assert!(dy >= 2, "the fixture's piece is at least two layers tall: {dy}");
    assert_eq!(layer(&app), 0);
    tap(&mut app, KeyCode::BracketRight);
    assert_eq!(layer(&app), 1, "] shows the next layer");
    assert_eq!(at(&app), Some((0, 1, 0)), "and the cursor rides the layer");
    apply(&mut app, "lattice_cursor_step", json!({"by": "down"}));
    assert_eq!(layer(&app), 0, "an agent steps back down");
    for _ in 0..=dy {
        apply(&mut app, "lattice_cursor_step", json!({"by": "up"}));
    }
    assert_eq!(layer(&app), dy - 1, "the top layer is as far as it goes");
    for _ in 0..=dy {
        tap(&mut app, KeyCode::BracketLeft);
    }
    assert_eq!(layer(&app), 0, "and so is the bottom one");
}

/// **The six actions name the operation they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_six_actions_name_the_operation_they_dispatch() {
    for action in [Action::CellLeft, Action::CellRight, Action::CellForward, Action::CellBack, Action::LayerDown, Action::LayerUp] {
        assert_eq!(action.operation(), Some("lattice_cursor_step"), "{action:?}");
    }
}

/// **A step whose log line cannot be written is taken back, and the cursor stays where it was**: the key applies the
/// answer only when the operation was applied, as every other key does.
#[test]
fn a_step_whose_log_line_cannot_be_written_leaves_the_cursor_where_it_was() {
    let mut app = editor("unwritable");
    let picked = apply(&mut app, "piece_select", json!({"id": "site/hall"}));
    assert_eq!(picked["outcome"], "applied", "{picked}");
    tap(&mut app, KeyCode::KeyH);
    assert_eq!(at(&app), Some((1, 0, 0)), "a step on a log that can be written moves the cursor");

    // A directory where the log's file goes: the append fails, so the dispatch is refused after its commit.
    let log = bevy_cauldron::ops::oplog_path_at(&app.world().resource::<Project>().root);
    let _ = std::fs::remove_file(&log);
    std::fs::create_dir_all(&log).expect("a directory in the log's place");
    tap(&mut app, KeyCode::KeyH);
    assert_eq!(at(&app), Some((1, 0, 0)), "a step whose log line cannot be written is refused, and moves nothing");
}
