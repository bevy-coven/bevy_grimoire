//! **A key that changes what a person is looking through is the operation an agent calls, and the agent's call
//! reaches the editor** (E-002, #129).
//!
//! `tool_arm`, `view_cycle_grid`, `snip_arm` and `view_toggle_journal` were registered for the keys and said
//! to be reachable over MCP, but `grimoire/operation_apply` built its context with **no view**, so an agent's
//! call was refused with "no view is in hand" and nothing changed: the registry listed four verbs the agent
//! could not use. The apply method now carries the editor's own view in, applies the answer, and the same is
//! true of `view_toggle_ghost`, the rig bench's measured-over-declared overlay that `G` toggles.
//!
//! **And two keys left the allowlist.** `B` (`MoveMode`) arms the move tool and `G` (`ToggleGhost`) toggles the
//! ghost, each through the operation, each with **one log line from the person** and no refusal painted over
//! the editor: a context with no project cannot append its line, and the key's status then said the change
//! "could not be logged" while the tool was armed all the same (#224's shape).

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::anim_stage::BenchAb;
use bevy_cauldron::editor::{EditorState, Rung, Tool};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::snip::SnipEdit;
use bevy_cauldron::tiles::Mode;
use bevy_grimoire_core::grid::SnapLevel;
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("view_verbs_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
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

/// **The key as a person presses it**: down and up in one frame, with no modifier.
fn tap(app: &mut App, key: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.update();
}

/// The log lines of `operation`, as `(who, line)`.
fn logged(app: &App, operation: &str) -> Vec<(String, Value)> {
    let root = app.world().resource::<Project>().root.clone();
    let text = std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(&root)).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == operation)
        .map(|v| (v["source"].as_str().unwrap_or_default().to_owned(), v))
        .collect()
}

fn tool(app: &App) -> Tool {
    app.world().resource::<EditorState>().tool
}

/// **An agent arms the move tool, and the editor holds it**; the same call again puts it back.
#[test]
fn an_agent_arms_a_tool_through_the_registry_and_the_editor_holds_it() {
    let mut app = editor("agent_tool");
    assert_eq!(tool(&app), Tool::Place, "a session opens placing");
    let said = apply(&mut app, "tool_arm", json!({"tool": "move"}));
    assert_eq!(said["outcome"], "applied", "an agent's tool_arm is applied, not refused for want of a view: {said}");
    assert_eq!(tool(&app), Tool::Move, "and the editor's own tool is the move tool: {said}");
    let said = apply(&mut app, "tool_arm", json!({"tool": "move"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(tool(&app), Tool::Place, "arming it again puts it back, as the key does");
}

/// **An agent cycles the grid, and the rung the editor draws at moves.**
#[test]
fn an_agent_cycles_the_grid_and_the_rung_moves() {
    let mut app = editor("agent_grid");
    let before = app.world().resource::<Rung>().0;
    assert_eq!(before, SnapLevel::Tile, "a session opens at the whole tile");
    let said = apply(&mut app, "view_cycle_grid", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(app.world().resource::<Rung>().0, SnapLevel::Fine, "the editor's rung is the operation's answer: {said}");
}

/// **An agent arms the snip**, which is the editor's capture mode.
#[test]
fn an_agent_arms_the_snip_and_the_editor_is_armed() {
    let mut app = editor("agent_snip");
    let said = apply(&mut app, "snip_arm", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(app.world().resource::<SnipEdit>().armed, "the editor's snip is armed: {said}");
}

/// **`B` arms the move tool through the operation**: one log line from the person, the tool is armed, and
/// the status is a receipt and not a refusal.
#[test]
fn b_arms_the_move_tool_through_the_operation_and_it_is_logged_once() {
    let mut app = editor("key_move");
    assert_eq!(Action::MoveMode.operation(), Some("tool_arm"), "the key names the operation an agent calls");
    tap(&mut app, KeyCode::KeyB);
    assert_eq!(tool(&app), Tool::Move, "B arms the move tool");
    let lines = logged(&app, "tool_arm");
    assert_eq!(lines.len(), 1, "one press is one log line, said by the person: {lines:?}");
    assert_eq!(lines[0].0, "human", "{lines:?}");
    assert_eq!(lines[0].1["params"]["tool"], "move", "{lines:?}");
    let status = app.world().resource::<EditorState>().status.clone();
    assert!(status.problems().is_empty(), "arming the tool paints no refusal over the editor: {:?}", status.problems());
}

/// **`G` toggles the bench's ghost through the operation**, and an agent's call toggles the same thing back.
#[test]
fn g_toggles_the_ghost_through_the_operation_and_an_agent_toggles_it_back() {
    let mut app = editor("key_ghost");
    app.world_mut().insert_resource(Mode::Anim);
    for _ in 0..6 {
        app.update();
    }
    assert!(!app.world().resource::<BenchAb>().0, "the ghost is off to begin with");
    assert_eq!(Action::ToggleGhost.operation(), Some("view_toggle_ghost"), "the key names the operation an agent calls");

    tap(&mut app, KeyCode::KeyG);
    assert!(app.world().resource::<BenchAb>().0, "G stages the ghost");
    let lines = logged(&app, "view_toggle_ghost");
    assert_eq!(lines.len(), 1, "one press is one log line: {lines:?}");
    assert_eq!(lines[0].0, "human", "{lines:?}");
    let status = app.world().resource::<EditorState>().status.clone();
    assert!(status.problems().is_empty(), "the toggle paints no refusal over the editor: {:?}", status.problems());

    let said = apply(&mut app, "view_toggle_ghost", json!({}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied: {said}");
    assert!(!app.world().resource::<BenchAb>().0, "and takes the ghost down again: {said}");
    let lines = logged(&app, "view_toggle_ghost");
    assert_eq!(lines.len(), 2, "the agent's call is the second line: {lines:?}");
    assert_eq!(lines[1].0, "agent", "{lines:?}");
}

/// **The ghost chip and the key are one toggle**: a click on the chip dispatches the operation too, so no
/// second place decides what a toggle means.
#[test]
fn the_ghost_has_no_second_toggle_in_the_source() {
    let stage = std::fs::read_to_string("src/anim_stage.rs").expect("anim_stage.rs is there");
    assert!(
        !stage.contains("ab.0 = !ab.0"),
        "`anim_stage.rs` flips the ghost by hand: the chip and the key must both dispatch `view_toggle_ghost`, \
         or there are two rules for one toggle"
    );
}

/// **An agent toggles the journal panel**, and the panel's own `Display` is what moves.
#[test]
fn an_agent_toggles_the_journal_and_the_panel_moves() {
    let mut app = editor("agent_journal");
    let display = |app: &mut App| {
        let mut panels = app.world_mut().query_filtered::<&Node, With<bevy_cauldron::chrome::JournalPanel>>();
        let all: Vec<Display> = panels.iter(app.world()).map(|n| n.display).collect();
        assert!(!all.is_empty(), "the editor has a journal panel to toggle");
        all[0]
    };
    assert_eq!(display(&mut app), Display::None, "the journal starts down");
    let said = apply(&mut app, "view_toggle_journal", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_ne!(display(&mut app), Display::None, "the journal is up: {said}");
    let said = apply(&mut app, "view_toggle_journal", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(display(&mut app), Display::None, "and down again: {said}");
}

/// **`Shift+B` arms the clone tool without a refusal painted over the editor.** The clone key dispatched
/// `tool_arm` with a context that carried no project, so the operation's log line had nowhere to go and the
/// status said the change "could not be logged" while the tool was armed all the same.
#[test]
fn shift_b_arms_the_clone_tool_and_is_logged_once_without_a_refusal() {
    let mut app = editor("key_clone");
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    tap(&mut app, KeyCode::KeyB);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
    assert_eq!(tool(&app), Tool::Clone, "Shift+B arms the clone tool");
    let lines = logged(&app, "tool_arm");
    assert_eq!(lines.len(), 1, "one press is one log line: {lines:?}");
    assert_eq!(lines[0].0, "human", "{lines:?}");
    let status = app.world().resource::<EditorState>().status.clone();
    assert!(status.problems().is_empty(), "the clone key paints no refusal over the editor: {:?}", status.problems());
}

/// **An operation that refuses for want of a view says it needs one**, so the door that carries a view in
/// does not skip a verb: the five view verbs are exactly the operations that name `changes_view`.
#[test]
fn an_operation_that_needs_a_view_says_so() {
    let ctx = bevy_cauldron::ops::Ctx::default();
    let mut says: Vec<&'static str> = Vec::new();
    for op in bevy_cauldron::ops::registry() {
        if op.changes_view() {
            says.push(op.name());
        }
        if let Err(refusal) = op.plan(&ctx, &json!({})) {
            assert!(
                !refusal.0.contains("no view is in hand") || op.changes_view(),
                "`{}` refuses for want of a view and does not say `changes_view`, so an agent's call is refused: {}",
                op.name(),
                refusal.0
            );
        }
    }
    says.sort_unstable();
    // Assumed owner exception (2026-11-11): the list gains the three operations #237 registers, which carry the
    // outliner's eye and locks in `Ctx::view`, and the two #129 slice 22 registers for the composition picker's
    // cursor; each names `changes_view`, so the door carries a view in for them.
    assert_eq!(
        says,
        [
            "camera_pan",
            "camera_turn",
            "compose_carousel_step",
            "compose_focus",
            "compose_walk",
            "composition_picker_toggle",
            "composition_picker_walk",
            "filter_focus",
            "kit_rescan",
            "labels_abandon",
            "labels_suggest",
            "labels_walk",
            "lattice_cursor_step",
            "layer_set_visible",
            "pack_fold",
            "placement_set_hidden",
            "placement_set_locked",
            "rig_check_all",
            "rig_history_step",
            "rig_walk",
            "shelf_focus",
            "shelf_walk",
            "snip_arm",
            "stage_camera",
            "stage_play",
            "stage_scrub",
            "tile_clear",
            "tile_close",
            "tile_grid_deepen",
            "tile_hand_arm",
            "tile_history_step",
            "tile_member_drop",
            "tile_member_flush",
            "tile_member_focus",
            "tile_member_nudge",
            "tile_member_remove",
            "tile_member_turn",
            "tile_new",
            "tile_open",
            "tile_page_enter",
            "tile_page_leave",
            "tile_page_walk",
            "tile_slot_drop",
            "tool_arm",
            "view_cycle_grid",
            "view_toggle_ghost",
            "view_toggle_journal"
        ]
    );
}
