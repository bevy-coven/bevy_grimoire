//! **The tile in hand is shaped by operations, so a key and an agent's call are one** (E-002, #129 slice 28).
//!
//! Seven verbs leave the allowlist of unmigrated actions, all of them on the Tiles tab with a tile open: `BuildArm`
//! (`Space` takes the piece, or puts it back), `MemberPrev` and `MemberNext` (`,` and `.` walk the focus through the
//! members), `BuildRung` (`J` makes the grid finer by thirds, wrapping), `BuildTurn` (`R` turns the focused member a
//! quarter), `BuildDropMember` (the remove key takes it out) and `ClearTile` (Shift and the remove key empty the tile).
//! They dispatch `tile_hand_arm`, `tile_member_focus`, `tile_grid_deepen`, `tile_member_turn`, `tile_member_remove` and
//! `tile_clear`; an agent calls the same six through `grimoire/operation_apply`, and the editor's own `Build` holds the
//! answer either way. Nothing is written to `compositions.yaml`: the tile in hand is a draft, and saving it is `Cmd+S`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::build::Build;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Tiles tab over a kit with one bounded tile of two members. No tile is in hand yet.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("tile_hand_{tag}_{}", std::process::id()))
        .sized_descriptor("wall", "alpha", 1.0, 1.0)
        .bounded_composition("bay", (1.0, 2.4, 1.0), &[("north", "wall", (0.0, 0.0)), ("south", "wall", (0.0, 0.25))])
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Tiles).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Take the kit's tile into hand**, as `Enter` on the Tiles page does, and stand on the Meshes page where the keys act.
fn take_in_hand(app: &mut App) {
    let bay = app.world().resource::<Project>().compositions.compositions[0].clone();
    {
        let mut build = app.world_mut().resource_mut::<Build>();
        build.open = Some(bay);
        build.browsing = None;
        build.focus = 0;
        build.placing = false;
    }
    for _ in 0..3 {
        app.update();
    }
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

fn build(app: &App) -> &Build {
    app.world().resource::<Build>()
}

fn members(app: &App) -> usize {
    build(app).open.as_ref().map_or(0, |c| c.members.len())
}

/// **With no tile in hand there is nothing to shape**, so every call is refused and nothing moves.
#[test]
fn an_agent_cannot_shape_a_tile_that_is_not_in_hand() {
    let mut app = editor("none");
    for (operation, parameters) in [
        ("tile_hand_arm", json!({})),
        ("tile_member_focus", json!({"by": "next"})),
        ("tile_grid_deepen", json!({})),
        ("tile_member_turn", json!({})),
        ("tile_member_remove", json!({})),
        ("tile_clear", json!({})),
    ] {
        let refused = apply(&mut app, operation, parameters);
        assert_eq!(refused["outcome"], "refused", "`{operation}` with no tile in hand: {refused}");
    }
    assert!(build(&app).open.is_none());
    assert!(!build(&app).placing);
}

/// **`Space` takes the piece and puts it back through the operation**, and an agent names the state it wants.
#[test]
fn space_and_an_agent_take_and_put_back_the_piece() {
    let mut app = editor("arm");
    take_in_hand(&mut app);
    assert!(!build(&app).placing);
    tap(&mut app, &[], KeyCode::Space);
    assert!(build(&app).placing, "Space takes the piece in hand");
    let lines = logged(&app, "tile_hand_arm");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    let said = apply(&mut app, "tile_hand_arm", json!({"placing": false}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!(!build(&app).placing, "an agent puts the piece back");
    apply(&mut app, "tile_hand_arm", json!({}));
    assert!(build(&app).placing, "no `placing` toggles, as Space does");
    let refused = apply(&mut app, "tile_hand_arm", json!({"placing": "yes"}));
    assert_eq!(refused["outcome"], "refused", "a state that is not a boolean is refused: {refused}");
}

/// **`,` and `.` walk the focus and stop at the ends**, and an agent walks the same cursor.
#[test]
fn the_member_keys_and_an_agent_walk_one_focus_and_clamp_at_the_ends() {
    let mut app = editor("focus");
    take_in_hand(&mut app);
    let n = members(&app);
    assert_eq!(n, 2, "the fixture's tile holds two members");
    tap(&mut app, &[], KeyCode::Space);
    tap(&mut app, &[], KeyCode::Period);
    assert_eq!(build(&app).focus, 1, ". walks to the next member");
    let lines = logged(&app, "tile_member_focus");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].1["params"]["by"], "next", "{:?}", lines[0]);
    tap(&mut app, &[], KeyCode::Period);
    assert_eq!(build(&app).focus, n - 1, "and stops at the last one rather than wrapping");
    tap(&mut app, &[], KeyCode::Comma);
    assert_eq!(build(&app).focus, 0, ", walks back");
    let said = apply(&mut app, "tile_member_focus", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(build(&app).focus, 1, "an agent walks the editor's own cursor");
    apply(&mut app, "tile_member_focus", json!({"by": "prev"}));
    apply(&mut app, "tile_member_focus", json!({"by": "prev"}));
    assert_eq!(build(&app).focus, 0, "and stops at the first");
    let refused = apply(&mut app, "tile_member_focus", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
}

/// **`J` deepens the arrow ladder, wrapping**, through the operation an agent calls.
#[test]
fn j_and_an_agent_deepen_the_grid_and_it_wraps() {
    let mut app = editor("rung");
    take_in_hand(&mut app);
    let start = build(&app).depth;
    tap(&mut app, &[], KeyCode::KeyJ);
    assert_eq!(build(&app).depth, (start + 1) % bevy_cauldron::build::DEPTHS, "J steps the ladder one rung");
    let lines = logged(&app, "tile_grid_deepen");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    for _ in 0..bevy_cauldron::build::DEPTHS {
        apply(&mut app, "tile_grid_deepen", json!({}));
    }
    assert_eq!(build(&app).depth, (start + 1) % bevy_cauldron::build::DEPTHS, "a full turn of the ladder comes back to where it was");
}

/// **`R` turns the focused member a quarter through the operation**, and an agent turns it too.
#[test]
fn r_and_an_agent_turn_the_focused_member_a_quarter() {
    let mut app = editor("turn");
    take_in_hand(&mut app);
    let yaw = |app: &App| build(app).open.as_ref().map(|c| c.members[0].yaw).expect("a tile in hand");
    assert_eq!(yaw(&app), 0.0);
    tap(&mut app, &[], KeyCode::KeyR);
    assert_eq!(yaw(&app), 90.0, "R turns the focused member a quarter");
    let lines = logged(&app, "tile_member_turn");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    let said = apply(&mut app, "tile_member_turn", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(yaw(&app), 180.0, "an agent turns the same member");
    apply(&mut app, "tile_member_turn", json!({}));
    apply(&mut app, "tile_member_turn", json!({}));
    assert_eq!(yaw(&app), 0.0, "four quarters come back round");
}

/// **The remove key takes the focused member out and Shift empties the tile**, through the operations an agent calls.
#[test]
fn the_remove_keys_and_an_agent_take_members_out_and_empty_the_tile() {
    let mut app = editor("remove");
    take_in_hand(&mut app);
    tap(&mut app, &[], bevy_cauldron::keys::REMOVE_KEY);
    assert_eq!(members(&app), 1, "the remove key takes the focused member out");
    let lines = logged(&app, "tile_member_remove");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    let said = apply(&mut app, "tile_member_remove", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(members(&app), 0, "an agent takes the last one out");
    let refused = apply(&mut app, "tile_member_remove", json!({}));
    assert_eq!(refused["outcome"], "refused", "there is no member left to take out: {refused}");

    take_in_hand(&mut app);
    assert_eq!(members(&app), 2);
    tap(&mut app, &[KeyCode::ShiftLeft], bevy_cauldron::keys::REMOVE_KEY);
    assert_eq!(members(&app), 0, "Shift and the remove key empty the tile");
    assert_eq!(logged(&app, "tile_clear").len(), 1, "one key, one logged operation");
    take_in_hand(&mut app);
    apply(&mut app, "tile_clear", json!({}));
    assert_eq!(members(&app), 0, "an agent empties it too");
    assert_eq!(build(&app).focus, 0, "and the focus goes back to the first");
    let refused = apply(&mut app, "tile_clear", json!({}));
    assert_eq!(refused["outcome"], "refused", "an empty tile has nothing to clear: {refused}");
}

/// **The seven actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_seven_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::BuildArm.operation(), Some("tile_hand_arm"));
    assert_eq!(Action::MemberPrev.operation(), Some("tile_member_focus"));
    assert_eq!(Action::MemberNext.operation(), Some("tile_member_focus"));
    assert_eq!(Action::BuildRung.operation(), Some("tile_grid_deepen"));
    assert_eq!(Action::BuildTurn.operation(), Some("tile_member_turn"));
    assert_eq!(Action::BuildDropMember.operation(), Some("tile_member_remove"));
    assert_eq!(Action::ClearTile.operation(), Some("tile_clear"));
}

/// **`Cmd+Z` on the Tiles tab takes back what an agent's call did to the draft**: the operations carry no undo step of their
/// own (`Undo::None`), because the tab's own history watches the tile and records every change to it, whoever made it.
#[test]
fn cmd_z_takes_back_an_agents_clear_and_an_agents_removal() {
    let mut app = editor("undo");
    take_in_hand(&mut app);
    assert_eq!(members(&app), 2);
    let undo = |app: &mut App| tap(app, &[bevy_cauldron::keys::MOD_KEYS[0]], KeyCode::KeyZ);

    let said = apply(&mut app, "tile_clear", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    app.update();
    app.update();
    assert_eq!(members(&app), 0, "an agent emptied the tile");
    undo(&mut app);
    assert_eq!(members(&app), 2, "Cmd+Z brings both members back as one step");

    let said = apply(&mut app, "tile_member_remove", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    app.update();
    app.update();
    assert_eq!(members(&app), 1, "an agent took the focused member out");
    undo(&mut app);
    assert_eq!(members(&app), 2, "Cmd+Z puts it back");
}
