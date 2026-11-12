//! **The rig bench's list and the Tiles page's list are walked by operations, so a key and an agent's call are one**
//! (E-002, #129 slice 24).
//!
//! Four verbs leave the allowlist of unmigrated actions: `PrevRig` and `NextRig` (up and down on the Anim tab, which walk
//! the rig list round its ring, Shift striding five) and `TilePrev` and `TileNext` (up and down on the Tiles page, which
//! walk the kit's tiles and stop at the ends, Shift striding five). They dispatch `rig_walk` and `tile_page_walk`; an agent
//! calls the same two through `grimoire/operation_apply`, and the editor's own `BenchState` and `Build` hold the answer
//! either way.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::anim_tab::BenchState;
use bevy_cauldron::build::Build;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use serde_json::{Value, json};

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// An editor on `mode`, over a project with three bounded tiles and the shipped rig manifest (ten rigs).
fn editor(tag: &str, mode: Mode) -> App {
    let tile = [("floor", "floor", (0.0, 0.0))];
    let root = fixtures::Fixture::new(&format!("rig_tile_walks_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .bounded_composition("tile_a", (1.0, 2.4, 1.0), &tile)
        .bounded_composition("tile_b", (1.0, 2.4, 1.0), &tile)
        .bounded_composition("tile_c", (1.0, 2.4, 1.0), &tile)
        .build("m");
    std::fs::copy(workspace_asset("grimoire/rigs.ron"), root.join("assets/grimoire/rigs.ron")).expect("rigs.ron");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
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

fn rig(app: &App) -> usize {
    app.world().resource::<BenchState>().selected
}

fn page(app: &App) -> Option<usize> {
    app.world().resource::<Build>().browsing
}

/// **Down walks the rigs through the operation, the list is a ring, and Shift strides five**; an agent walks the same
/// cursor with the same rule.
#[test]
fn the_rig_keys_and_an_agent_walk_one_ring() {
    let mut app = editor("rigs", Mode::Anim);
    let count = app.world().resource::<BenchState>().names().len();
    assert_eq!(count, 10, "the shipped manifest lists ten rigs");
    assert_eq!(rig(&app), 0);
    tap(&mut app, &[], KeyCode::ArrowDown);
    assert_eq!(rig(&app), 1, "down walks to the next rig");
    let lines = logged(&app, "rig_walk");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "next", "{:?}", lines[0]);

    let said = apply(&mut app, "rig_walk", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(rig(&app), 2, "an agent's walk moves the editor's own cursor");
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowDown);
    assert_eq!(rig(&app), 7, "Shift+down strides five");
    apply(&mut app, "rig_walk", json!({"by": "next", "rows": 5}));
    assert_eq!(rig(&app), 2, "and the list is a ring: five past the seventh is the second");
    tap(&mut app, &[], KeyCode::ArrowUp);
    apply(&mut app, "rig_walk", json!({"by": "prev"}));
    assert_eq!(rig(&app), 0, "up and an agent's `prev` walk back");
    apply(&mut app, "rig_walk", json!({"by": "prev"}));
    assert_eq!(rig(&app), count - 1, "and wrap from the first rig to the last");
    let refused = apply(&mut app, "rig_walk", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
}

/// **Down walks the Tiles page through the operation, and the walk clamps at both ends**; an agent walks it back.
#[test]
fn the_tile_page_keys_and_an_agent_walk_one_cursor_and_clamp_at_the_ends() {
    let mut app = editor("tiles", Mode::Tiles);
    assert_eq!(page(&app), Some(1), "the page opens on the first tile; `+ New Tile` is row zero");
    tap(&mut app, &[], KeyCode::ArrowDown);
    assert_eq!(page(&app), Some(2), "down walks to the next tile");
    let lines = logged(&app, "tile_page_walk");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "next", "{:?}", lines[0]);

    let said = apply(&mut app, "tile_page_walk", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(page(&app), Some(3), "an agent's walk moves the editor's own cursor");
    apply(&mut app, "tile_page_walk", json!({"by": "next"}));
    assert_eq!(page(&app), Some(3), "and it stops at the end of the page rather than wrapping");
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowUp);
    assert_eq!(page(&app), Some(0), "Shift+up strides five and stops on `+ New Tile`");
    let refused = apply(&mut app, "tile_page_walk", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
}

/// **A call with no tile page up is refused**, because there is no cursor to move: the page is a place the editor is in.
#[test]
fn an_agent_cannot_walk_a_tile_page_that_is_not_up() {
    let mut app = editor("no-page", Mode::Map);
    assert_eq!(page(&app), None, "the Map tab has no tile page up");
    let refused = apply(&mut app, "tile_page_walk", json!({"by": "next"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert_eq!(page(&app), None, "and nothing was moved");
}

/// **The four actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_four_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::PrevRig.operation(), Some("rig_walk"));
    assert_eq!(Action::NextRig.operation(), Some("rig_walk"));
    assert_eq!(Action::TilePrev.operation(), Some("tile_page_walk"));
    assert_eq!(Action::TileNext.operation(), Some("tile_page_walk"));
}
