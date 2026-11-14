//! **The tile in hand is moved, put flush and filled by operations, so a key and an agent's call are one** (E-002, #129 slice 29).
//!
//! Twelve verbs leave the allowlist of unmigrated actions, all of them on the Tiles tab with a tile open: the four arrows and
//! `[` and `]` (`BuildForward`, `BuildBack`, `BuildLeft`, `BuildRight`, `BuildUp`, `BuildDown`), which nudge the focused member
//! along its ladder; Shift and an arrow (`AlignForward`, `AlignBack`, `AlignLeft`, `AlignRight`), which puts it flush against a
//! side; Enter (`BuildDrop`), which drops the piece the list has picked; and Shift and Enter (`BuildSlot`), which drops a hole.
//! They dispatch `tile_member_nudge`, `tile_member_flush`, `tile_member_drop` and `tile_slot_drop`; an agent calls the same four
//! through `grimoire/operation_apply`, and the editor's own `Build` holds the answer either way. Nothing is written to
//! `compositions.yaml`: the tile in hand is a draft, and `Cmd+Z` on the tab takes each of these back.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::build::Build;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use bevy_grimoire_core::composition::Body;
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Tiles tab over a kit with a wall a metre square, a post a fifth of a metre square, a slot token and one
/// bounded tile (`bay`, a metre across) holding a single post. No tile is in hand yet.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("tile_pieces_{tag}_{}", std::process::id()))
        .sized_descriptor("wall", "alpha", 1.0, 1.0)
        .sized_descriptor("post", "alpha", 0.2, 0.2)
        .slot_token("opening")
        .bounded_composition("bay", (1.0, 2.4, 1.0), &[("pillar", "post", (0.0, 0.0))])
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

/// Where the focused member stands: its place on the tile and its lift.
fn stands(app: &App) -> ((f32, f32), f32) {
    let b = build(app);
    let m = &b.open.as_ref().expect("a tile in hand").members[b.focus];
    (m.at, m.lift)
}

/// The tile's own axis the arrow key `wish` (screen up is negative) points along from here, as the front end resolves it.
fn axis_of(app: &App, wish: Vec2) -> (i32, i32) {
    bevy_cauldron::build::step_in_view(wish, app.world().resource::<bevy_cauldron::view::Rig>().yaw)
}

/// **With no tile in hand there is nothing to shape**, so every call is refused and nothing moves.
#[test]
fn an_agent_cannot_nudge_flush_or_drop_into_a_tile_that_is_not_in_hand() {
    let mut app = editor("none");
    for (operation, parameters) in [
        ("tile_member_nudge", json!({"x": 1})),
        ("tile_member_flush", json!({"x": 1})),
        ("tile_member_drop", json!({"descriptor": "post"})),
        ("tile_slot_drop", json!({})),
    ] {
        let refused = apply(&mut app, operation, parameters);
        assert_eq!(refused["outcome"], "refused", "`{operation}` with no tile in hand: {refused}");
    }
    assert!(build(&app).open.is_none());
}

/// **An arrow nudges the focused member to the next stop on its ladder through the operation**, `]` and `[` lift it a layer,
/// and an agent names the tile's own axis. The ladder's outer stop and the flush position are the same number.
#[test]
fn the_arrows_and_an_agent_nudge_the_focused_member_along_its_ladder() {
    let mut app = editor("nudge");
    take_in_hand(&mut app);
    tap(&mut app, &[], KeyCode::Space);
    assert!(build(&app).placing, "Space takes the piece, so the arrows steer the tile");
    assert_eq!(stands(&app), ((0.0, 0.0), 0.0), "a member starts in the middle, on the floor");

    tap(&mut app, &[], KeyCode::ArrowUp);
    let (dx, dz) = axis_of(&app, Vec2::new(0.0, -1.0));
    let ((x, z), _) = stands(&app);
    assert!(
        (x != 0.0) == (dx != 0) && (z != 0.0) == (dz != 0) && (x != 0.0 || z != 0.0),
        "the up arrow moved the member along the axis the camera points it at ({dx}, {dz}): ({x}, {z})"
    );
    let lines = logged(&app, "tile_member_nudge");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!((lines[0].1["params"]["x"].as_i64(), lines[0].1["params"]["z"].as_i64()), (Some(dx as i64), Some(dz as i64)), "{:?}", lines[0]);

    // Back to the middle by an agent's own call, then out to the outer stop: it is where a flush puts the piece.
    let said = apply(&mut app, "tile_member_nudge", json!({"x": -dx, "z": -dz}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(stands(&app), ((0.0, 0.0), 0.0), "the ladder walks back to the middle");
    apply(&mut app, "tile_member_nudge", json!({"x": 1}));
    let outer = stands(&app).0;
    assert!(outer.0 > 0.0, "an agent nudges along the tile's own +X: {outer:?}");
    apply(&mut app, "tile_member_nudge", json!({"x": -1}));
    apply(&mut app, "tile_member_flush", json!({"x": 1}));
    assert_eq!(stands(&app).0, outer, "the ladder's outer stop and the flush position are one number");

    apply(&mut app, "tile_member_nudge", json!({"lift": 1}));
    assert!(stands(&app).1 > 0.0, "an agent lifts the member a layer");
    tap(&mut app, &[], KeyCode::BracketLeft);
    assert_eq!(stands(&app).1, 0.0, "[ lowers it, and the floor is the floor");
    assert_eq!(logged(&app, "tile_member_nudge").len(), 6, "one logged line per call and per key");
    tap(&mut app, &[], KeyCode::BracketLeft);
    assert_eq!(stands(&app).1, 0.0, "a member cannot be nudged under the tile it is in");
}

/// **Nothing is nudged that is not meant**: no direction, a step of two, a field that is not one, and an empty tile are
/// refused, and the tile is as it was.
#[test]
fn a_nudge_that_names_no_step_or_a_wild_one_is_refused() {
    let mut app = editor("nudge_refused");
    take_in_hand(&mut app);
    for parameters in [json!({}), json!({"x": 2}), json!({"z": -3}), json!({"y": 1}), json!({"x": "left"})] {
        let refused = apply(&mut app, "tile_member_nudge", parameters.clone());
        assert_eq!(refused["outcome"], "refused", "{parameters}: {refused}");
    }
    assert_eq!(stands(&app), ((0.0, 0.0), 0.0));
    apply(&mut app, "tile_clear", json!({}));
    let refused = apply(&mut app, "tile_member_nudge", json!({"x": 1}));
    assert_eq!(refused["outcome"], "refused", "an empty tile has nothing to move: {refused}");
}

/// **Shift and an arrow put the focused member flush through the operation**, and an agent names the side. A hole has no width,
/// so it is refused, and one side at a time is the rule.
#[test]
fn shift_and_an_arrow_and_an_agent_put_the_member_flush() {
    let mut app = editor("flush");
    take_in_hand(&mut app);
    tap(&mut app, &[], KeyCode::Space);
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowLeft);
    let (dx, dz) = axis_of(&app, Vec2::new(-1.0, 0.0));
    let ((x, z), _) = stands(&app);
    assert!(
        (x != 0.0) == (dx != 0) && (z != 0.0) == (dz != 0) && (x != 0.0 || z != 0.0),
        "Shift and the left arrow put the member flush along ({dx}, {dz}): ({x}, {z})"
    );
    let lines = logged(&app, "tile_member_flush");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    let said = apply(&mut app, "tile_member_flush", json!({"z": 1}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(stands(&app).0.1 > 0.0, "an agent puts it flush against +Z");
    for parameters in [json!({}), json!({"x": 1, "z": 1}), json!({"x": 2})] {
        let refused = apply(&mut app, "tile_member_flush", parameters.clone());
        assert_eq!(refused["outcome"], "refused", "a flush names one side: {parameters}: {refused}");
    }

    apply(&mut app, "tile_slot_drop", json!({}));
    let refused = apply(&mut app, "tile_member_flush", json!({"x": 1}));
    assert_eq!(refused["outcome"], "refused", "a hole has no width to put flush: {refused}");
    // And from the keyboard it is a problem, which sticks, and not a receipt the next keystroke erases.
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowLeft);
    let status = &app.world().resource::<bevy_cauldron::tiles::ImportState>().status;
    assert!(
        status.has_problem() && status.problem_text().contains("a hole has no width to put flush"),
        "Shift and an arrow on a hole files the operation's refusal as a problem: {:?}",
        status.problem_text()
    );
}

/// **Enter drops the piece the list has picked through the operation**, with the focus on it and the piece in hand, and an
/// agent names the piece. A piece the library does not carry is refused.
#[test]
fn enter_and_an_agent_drop_a_piece_into_the_tile() {
    let mut app = editor("drop");
    take_in_hand(&mut app);
    // The piece the list has picked, as a click on its row leaves it: the Meshes shelf is the other tab's.
    app.world_mut().resource_mut::<bevy_cauldron::tiles::ImportState>().selected_library_id = Some("post".to_owned());
    app.update();
    assert!(!build(&app).placing);
    tap(&mut app, &[], KeyCode::Enter);
    assert_eq!(members(&app), 2, "Enter drops the picked piece into the tile");
    assert!(build(&app).placing, "a drop leaves the piece in hand, so the arrows adjust it");
    let dropped = &build(&app).open.as_ref().expect("a tile in hand").members[build(&app).focus];
    assert!(matches!(&dropped.body, Body::Descriptor { id, .. } if id == "post"), "the focus follows the drop: {dropped:?}");
    let lines = logged(&app, "tile_member_drop");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["descriptor"], "post", "{:?}", lines[0]);

    let said = apply(&mut app, "tile_member_drop", json!({"descriptor": "wall"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(members(&app), 3, "an agent drops a piece of its own choosing");
    let refused = apply(&mut app, "tile_member_drop", json!({"descriptor": "not_a_piece"}));
    assert_eq!(refused["outcome"], "refused", "a member must name a library piece: {refused}");
    let refused = apply(&mut app, "tile_member_drop", json!({}));
    assert_eq!(refused["outcome"], "refused", "a drop names its piece: {refused}");
    assert_eq!(members(&app), 3, "and a refusal leaves the tile as it was");
}

/// **Shift and Enter drop a hole through the operation**, and an agent names the token it accepts.
#[test]
fn shift_enter_and_an_agent_drop_a_hole_into_the_tile() {
    let mut app = editor("slot");
    take_in_hand(&mut app);
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::Enter);
    assert_eq!(members(&app), 2, "the hole key drops a hole into the tile");
    let hole = &build(&app).open.as_ref().expect("a tile in hand").members[build(&app).focus];
    assert!(matches!(&hole.body, Body::Slot { accepts } if accepts == "opening"), "it takes the axis's first token: {hole:?}");
    let lines = logged(&app, "tile_slot_drop");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    let said = apply(&mut app, "tile_slot_drop", json!({"accepts": "opening"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(members(&app), 3, "an agent drops a hole that names its token");
    let refused = apply(&mut app, "tile_slot_drop", json!({"accepts": "no_such_token"}));
    assert_eq!(refused["outcome"], "refused", "a token the slot axis does not declare is refused: {refused}");
    assert_eq!(members(&app), 3);
}

/// **`Cmd+Z` on the Tiles tab takes back what an agent's nudge and drop did**, as it takes back the keys': the operations carry
/// no undo step of their own, and the tab's history watches the tile whoever changed it.
#[test]
fn cmd_z_takes_back_an_agents_nudge_and_an_agents_drop() {
    let mut app = editor("undo");
    take_in_hand(&mut app);
    let undo = |app: &mut App| tap(app, &[bevy_cauldron::keys::MOD_KEYS[0]], KeyCode::KeyZ);
    apply(&mut app, "tile_member_nudge", json!({"x": 1}));
    app.update();
    app.update();
    assert!(stands(&app).0.0 > 0.0);
    undo(&mut app);
    assert_eq!(stands(&app).0, (0.0, 0.0), "Cmd+Z puts the member back where it was");

    apply(&mut app, "tile_member_drop", json!({"descriptor": "wall"}));
    app.update();
    app.update();
    assert_eq!(members(&app), 2);
    undo(&mut app);
    assert_eq!(members(&app), 1, "Cmd+Z takes the dropped piece out");
}

/// **The twelve actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_twelve_actions_name_the_operations_they_dispatch() {
    for action in [Action::BuildForward, Action::BuildBack, Action::BuildLeft, Action::BuildRight, Action::BuildUp, Action::BuildDown] {
        assert_eq!(action.operation(), Some("tile_member_nudge"), "{action:?}");
    }
    for action in [Action::AlignForward, Action::AlignBack, Action::AlignLeft, Action::AlignRight] {
        assert_eq!(action.operation(), Some("tile_member_flush"), "{action:?}");
    }
    assert_eq!(Action::BuildDrop.operation(), Some("tile_member_drop"));
    assert_eq!(Action::BuildSlot.operation(), Some("tile_slot_drop"));
}

/// **`Cmd+Z` and `Cmd+Shift+Z` on the Tiles tab step the tab's history through the operation** (E-002, #129 slice 30), and an agent
/// steps the same history: one logged line per key, the tile back where it was, and nothing to step is refused.
#[test]
fn cmd_z_and_an_agent_step_the_tab_history_through_one_operation() {
    let mut app = editor("history");
    take_in_hand(&mut app);
    let mods = [bevy_cauldron::keys::MOD_KEYS[0]];
    apply(&mut app, "tile_member_nudge", json!({"x": 1}));
    app.update();
    app.update();
    let nudged = stands(&app).0;
    assert!(nudged.0 > 0.0);

    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!(stands(&app).0, (0.0, 0.0), "Cmd+Z takes the nudge back");
    let lines = logged(&app, "tile_history_step");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "back", "{:?}", lines[0]);

    tap(&mut app, &[mods[0], KeyCode::ShiftLeft], KeyCode::KeyZ);
    assert_eq!(stands(&app).0, nudged, "Cmd+Shift+Z puts it back");
    let lines = logged(&app, "tile_history_step");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1].1["params"]["by"], "forward", "{:?}", lines[1]);

    let said = apply(&mut app, "tile_history_step", json!({"by": "back"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(stands(&app).0, (0.0, 0.0), "an agent steps the editor's own history back");
    let said = apply(&mut app, "tile_history_step", json!({"by": "forward"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(stands(&app).0, nudged, "and forward");
    let refused = apply(&mut app, "tile_history_step", json!({"by": "forward"}));
    assert_eq!(refused["outcome"], "refused", "there is nothing left to put back: {refused}");
    let refused = apply(&mut app, "tile_history_step", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");

    // Back past the first edit lands on no tile at all, as it always did, and then there is nothing to undo.
    apply(&mut app, "tile_history_step", json!({"by": "back"}));
    apply(&mut app, "tile_history_step", json!({"by": "back"}));
    assert!(build(&app).open.is_none(), "the history reaches back to before the tile was opened");
    let refused = apply(&mut app, "tile_history_step", json!({"by": "back"}));
    assert_eq!(refused["outcome"], "refused", "nothing to undo: {refused}");
}

/// **The two history actions name the operation they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_two_history_actions_name_the_operation_they_dispatch() {
    assert_eq!(Action::UndoBuild.operation(), Some("tile_history_step"));
    assert_eq!(Action::RedoBuild.operation(), Some("tile_history_step"));
}
