//! **Cmd+Z takes back the newest change, and a map resize is a change.**
//!
//! B-008. The size field dispatches `map_set_settings` through the registry, and its change went to
//! the operation log but never onto the editor's undo stack. So after a person laid a run of fence
//! and then resized the map, Cmd+Z skipped the resize and took back the fence: an older, unrelated
//! edit vanished and the change the person meant to undo stayed.
//!
//! These drive the human front end end to end: the size field is opened through its own `Activate`,
//! the digits and Enter arrive as `KeyboardInput`, and Cmd+Z / Cmd+Shift+Z are real key presses.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::editor::Placement;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
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

/// **A chord**: the platform modifier (and Shift, when asked) held while `key` is pressed.
fn chord(app: &mut App, key: KeyCode, shift: bool) {
    let held: Vec<KeyCode> = std::iter::once(bevy_cauldron::keys::MOD_KEYS[0])
        .chain(shift.then_some(KeyCode::ShiftLeft))
        .collect();
    for k in &held {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(*k);
    }
    tap(
        app,
        Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        key,
    );
    for k in &held {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(*k);
    }
    app.update();
}

/// **Resize one axis through the size field**, as a person does: open it, type, Enter.
fn resize(app: &mut App, axis: char, metres: &str) {
    bevy_cauldron::editor::size_field_for_test(app.world_mut(), axis);
    app.update();
    for c in metres.chars() {
        let code = match c {
            '0' => KeyCode::Digit0,
            '1' => KeyCode::Digit1,
            '2' => KeyCode::Digit2,
            '3' => KeyCode::Digit3,
            '4' => KeyCode::Digit4,
            '5' => KeyCode::Digit5,
            '6' => KeyCode::Digit6,
            '7' => KeyCode::Digit7,
            '8' => KeyCode::Digit8,
            '9' => KeyCode::Digit9,
            _ => panic!("not a digit: {c}"),
        };
        tap(app, Key::Character(c.to_string().into()), code);
    }
    tap(app, Key::Enter, KeyCode::Enter);
}

fn map(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

/// **Run, then resize, then Cmd+Z: the resize goes, the run stays.** Then Cmd+Z again takes the
/// run, and Cmd+Shift+Z twice puts both back in order.
#[test]
fn cmd_z_after_a_resize_takes_back_the_resize_and_not_an_older_edit() {
    let mut app = editor("undoresize");
    let start = map(&app);
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let with_run = map(&app).placements.len();
    assert!(
        with_run > start.placements.len(),
        "the run laid nothing, so there is nothing to compare"
    );

    resize(&mut app, 'y', "44");
    assert_eq!(
        map(&app).bounds.1,
        44.0,
        "the size field did not resize the map, so the test drives nothing"
    );

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        (map(&app).bounds.1, map(&app).placements.len()),
        (start.bounds.1, with_run),
        "Cmd+Z after a resize must take back the resize and leave the fence; it took back the \
         fence and left the resize, which is an older, unrelated edit disappearing"
    );
    assert_eq!(
        on_disk(&app).bounds.1,
        start.bounds.1,
        "the file still holds the resize"
    );

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).placements.len(),
        start.placements.len(),
        "the second Cmd+Z did not take the run back"
    );

    chord(&mut app, KeyCode::KeyZ, true);
    assert_eq!(
        map(&app).placements.len(),
        with_run,
        "the first redo did not put the run back"
    );
    chord(&mut app, KeyCode::KeyZ, true);
    assert_eq!(
        map(&app).bounds.1,
        44.0,
        "the second redo did not put the resize back"
    );
    assert_eq!(
        on_disk(&app).bounds.1,
        44.0,
        "the file did not get the redone resize"
    );
}

/// **Cmd+Z on a run writes the map it took back, so the watcher has nothing older to adopt.**
///
/// B-044. `Undo::Added` changed the in-memory map and never saved it, so the file on disk kept the
/// run. The 250 ms file watcher then read that stale file as an outside edit and put the run back
/// after the undo. The file and the editor must agree the moment Cmd+Z lands, and they must still
/// agree after the watcher has had a chance to poll.
#[test]
fn cmd_z_on_a_run_writes_the_map_so_the_watcher_has_nothing_to_adopt() {
    let mut app = editor("undorunwrite");
    let start = map(&app);
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    assert!(
        map(&app).placements.len() > start.placements.len(),
        "the run laid nothing, so there is nothing to undo"
    );

    // **The run reaches the file only through a later save.** A resize and its Cmd+Z write the map
    // with the run still in it, so the file holds the run when the run is taken back (the B-044 order).
    resize(&mut app, 'y', "44");
    chord(&mut app, KeyCode::KeyZ, false);
    assert!(
        map(&app).placements.len() > start.placements.len() && map(&app).bounds.1 == start.bounds.1,
        "Cmd+Z after the resize must take back only the resize and keep the run"
    );

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        on_disk(&app),
        map(&app),
        "Cmd+Z took the run back in memory, but the file still holds it"
    );

    std::thread::sleep(std::time::Duration::from_millis(300));
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        map(&app).placements.len(),
        start.placements.len(),
        "the watcher read the stale file and put the undone run back"
    );
}

/// **Cmd+Z and an agent's undo are one history.** If the agent already took the resize back, Cmd+Z
/// does not take it back twice, and says so where the author looks.
#[test]
fn cmd_z_does_not_undo_a_resize_the_agent_already_took_back() {
    let mut app = editor("undoresizeagent");
    let start = map(&app);
    resize(&mut app, 'x', "61");
    assert_eq!(map(&app).bounds.0, 61.0);

    // **The agent's undo, through the same registry function the MCP tool reaches.**
    let mut ctx = bevy_cauldron::ops::Ctx {
        root: None,
        project: Some(
            app.world()
                .resource::<bevy_cauldron::project::Project>()
                .clone(),
        ),
        open: Some(app.world().resource::<OpenMap>().clone()),
        source: bevy_cauldron::ops::Source::Agent,
        session: bevy_cauldron::ops::session_id(),
        proposal: None,
    };
    bevy_cauldron::ops::undo(&mut ctx, None)
        .unwrap_or_else(|e| panic!("the agent's undo: {e}"));
    let fresh = ctx.open.take().expect("the map comes back");
    *app.world_mut().resource_mut::<OpenMap>() = fresh;
    assert_eq!(map(&app).bounds.0, start.bounds.0);

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).bounds.0,
        start.bounds.0,
        "Cmd+Z re-applied or double-undid the resize"
    );
    let problems: Vec<String> = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .problems()
        .iter()
        .map(|p| p.line())
        .collect();
    assert!(
        problems.iter().any(|p| p.contains("took it back")),
        "the refusal did not reach the author's problem slot: {problems:?}"
    );
}

/// An editor with the agent's BRP methods registered, as the MCP server reaches them.
fn agent_editor(tag: &str) -> App {
    let mut app = editor(tag);
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn agent_call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
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

/// **B-019.** A person resizes the map, an agent turns a piece, and the person presses Cmd+Z.
/// The newest change is the agent's turn, so Cmd+Z takes the turn back and the resize stays. It
/// took the resize back and left the turn standing, because the agent's change never reached the
/// editor's stack.
#[test]
fn cmd_z_after_an_agents_turn_takes_back_the_turn_and_not_the_resize() {
    let mut app = agent_editor("undoagentturn");
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let start = map(&app);
    let id = start.placements[0].id.clone();
    let start_yaw = start.placements[0].yaw;
    let agent_yaw = if start_yaw == 33.0 { 34.0 } else { 33.0 };

    resize(&mut app, 'y', "44");
    assert_eq!(
        map(&app).bounds.1,
        44.0,
        "the size field did not resize the map"
    );

    let start_rot = screen_rotation(&mut app, &id);

    let turned = agent_call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_set_yaw",
            "parameters": {"id": id, "yaw": agent_yaw},
            "dry_run": false,
        }),
    );
    assert_eq!(turned["outcome"], "applied", "the agent's turn: {turned}");
    assert_eq!(map(&app).placements[0].yaw, agent_yaw);
    app.update();
    assert!(
        screen_rotation(&mut app, &id).angle_between(start_rot) > 0.005,
        "the agent's turn was saved, but the piece on screen did not turn"
    );

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).placements[0].yaw,
        start_yaw,
        "Cmd+Z after an agent's turn must take the turn back; it left the agent's change standing"
    );
    assert_eq!(
        map(&app).bounds.1,
        44.0,
        "Cmd+Z after an agent's turn took back the person's resize, an older change"
    );
    assert_eq!(
        on_disk(&app).placements[0].yaw,
        start_yaw,
        "the file still holds the agent's turn"
    );
    app.update();
    let off_screen = screen_rotation(&mut app, &id).angle_between(start_rot);
    assert!(
        off_screen < 0.005,
        "Cmd+Z took the turn back in the map and the file, but the piece on screen still shows \
         the agent's angle ({:.1} degrees off), so the picture and the map disagree",
        off_screen.to_degrees()
    );
}

/// **An agent's own undo leaves the stack honest.** The agent takes its turn back, so Cmd+Z must
/// not take it back a second time: it takes back the newest change still in effect, the resize.
#[test]
fn cmd_z_does_not_undo_an_agents_turn_the_agent_already_took_back() {
    let mut app = agent_editor("undoagentturnback");
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let start = map(&app);
    let id = start.placements[0].id.clone();
    let start_yaw = start.placements[0].yaw;
    let agent_yaw = if start_yaw == 33.0 { 34.0 } else { 33.0 };

    resize(&mut app, 'y', "44");
    let turned = agent_call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_set_yaw",
            "parameters": {"id": id, "yaw": agent_yaw},
            "dry_run": false,
        }),
    );
    let line = turned["operation_id"]
        .as_str()
        .expect("the turn's id")
        .to_owned();

    let took_back = agent_call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": line, "dry_run": false}),
    );
    assert_eq!(
        took_back["outcome"], "applied",
        "the agent's undo: {took_back}"
    );
    assert_eq!(map(&app).placements[0].yaw, start_yaw);

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).placements[0].yaw,
        start_yaw,
        "Cmd+Z re-applied or double-undid the agent's turn"
    );
    assert_eq!(
        map(&app).bounds.1,
        start.bounds.1,
        "Cmd+Z did not take back the newest change in effect, the resize"
    );
}

/// **An agent puts back a turn the person took back, and Cmd+Z then takes that turn back, not the
/// resize.** The put-back is the newest change in effect, so it is the one Cmd+Z must reach. A
/// put-back was never logged to the person's history, so Cmd+Z reached the resize, which the guard
/// refused, and the refusal popped the resize's entry: the resize could no longer be undone.
#[test]
fn cmd_z_after_an_agent_puts_back_a_turn_takes_that_turn_back_and_not_the_resize() {
    let mut app = agent_editor("undoagentputback");
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let start = map(&app);
    let id = start.placements[0].id.clone();
    let start_yaw = start.placements[0].yaw;
    let agent_yaw = if start_yaw == 33.0 { 34.0 } else { 33.0 };

    resize(&mut app, 'y', "44");
    let turned = agent_call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_set_yaw",
            "parameters": {"id": id, "yaw": agent_yaw},
            "dry_run": false,
        }),
    );
    assert_eq!(turned["outcome"], "applied", "the agent's turn: {turned}");

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).placements[0].yaw,
        start_yaw,
        "Cmd+Z did not take the turn back"
    );

    // **The line Cmd+Z wrote is the newest in the log**, and the only one an agent may name to put
    // the turn back.
    let oplog = app
        .world()
        .resource::<bevy_cauldron::project::Project>()
        .project_dir
        .join("oplog.jsonl");
    let text =
        std::fs::read_to_string(&oplog).unwrap_or_else(|e| panic!("{}: {e}", oplog.display()));
    let undo_line = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("a log line"))
        .expect("the log has lines")["operation_id"]
        .as_str()
        .expect("the id")
        .to_owned();

    let put_back = agent_call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": undo_line, "dry_run": false}),
    );
    assert_eq!(
        put_back["outcome"], "applied",
        "the agent's put-back: {put_back}"
    );
    assert_eq!(map(&app).placements[0].yaw, agent_yaw);

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).placements[0].yaw,
        start_yaw,
        "Cmd+Z after an agent's put-back must take the put-back back; it took nothing, because the \
         resize's entry was left where the guard refused it"
    );
    assert_eq!(
        map(&app).bounds.1,
        44.0,
        "Cmd+Z after an agent's put-back took back the resize, an older change"
    );

    chord(&mut app, KeyCode::KeyZ, false);
    assert_eq!(
        map(&app).bounds.1,
        start.bounds.1,
        "the second Cmd+Z did not reach the resize: the resize's entry was lost"
    );
}

/// **A placed piece's rotation as the screen draws it**, by its id.
fn screen_rotation(app: &mut App, id: &str) -> Quat {
    let mut q = app.world_mut().query::<(&Placement, &Transform)>();
    q.iter(app.world())
        .find(|(placement, _)| placement.0 == id)
        .map(|(_, transform)| transform.rotation)
        .unwrap_or_else(|| panic!("the piece `{id}` is not on screen"))
}
