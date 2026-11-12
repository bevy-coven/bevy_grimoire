//! **The Agent workspace: everything about the Familiar in one place** (NV-10, #387; series getting around, #377).
//!
//! Opening Agent shows the AGENT block at full size with the waiting wish in it, the agent's last three changes
//! (read from the operation log, so a switch of workspace does not lose them, and only when the log changes), and a Look on
//! each that shows *that change's own pieces* on the Map. The Agent icon on the activity bar carries the count of what waits:
//! one list, shown here and counted there.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ComputedNode;

use bevy_cauldron::activity_bar::ActivityBar;
use bevy_cauldron::agent_view::{AgentChangeLook, AgentChangeRow, HISTORY_ROWS, oplog_reads};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::confirm::Confirm;
use bevy_cauldron::familiar::{Familiar, FamiliarItem};
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::view::Rig;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{Activate, ValueChange};

use crate::fixtures;

const SETTLE_FRAMES: usize = 12;
/// Frames a reveal may take: the switch, the panel's first build and the camera's ease.
const REVEAL_FRAMES: usize = 90;
/// One frame of the test clock, in seconds: a reveal is carried a frame at a time.
const FRAME_S: f32 = 1.0 / 60.0;
/// Where the table stands in the 64 m map, far from where the camera is put, so a Look that did nothing cannot pass.
const TABLE_AT: (f32, f32) = (28.0, -28.0);
/// How near the camera's focus must come to the table for the change to be framed, in metres.
const FRAMED_WITHIN_M: f32 = 6.0;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// Removes the test's own user settings file when the test ends, however it ends.
struct SettingsFile(std::path::PathBuf);

impl Drop for SettingsFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn editor(name: &str) -> (App, SettingsFile) {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("table", "alpha")
        .place_as("table@0", "table", TABLE_AT)
        .side(64.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    updates(&mut app, 8);
    // **The editor does not follow the agent in these tests** (the `follow_agent` setting off, in a user settings file of the test's own): a
    // followed change would carry the person to the Map under the history being measured. Look is the person's own ask and goes there
    // whatever the setting (`look_now`).
    let file = std::env::temp_dir().join(format!("agent_ws_settings_{name}_{}.ron", std::process::id()));
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(file.clone()));
    bevy_cauldron::setting_catalogue::write_key(&file, "follow_agent", Some(bevy_cauldron::setting_catalogue::Value::On(false))).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    (app, SettingsFile(file))
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("`{method}` is not registered");
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"));
    updates(app, 4);
    answer
}

fn turn(app: &mut App, yaw: f32) {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_yaw", "parameters": {"id": "table@0", "yaw": yaw}, "dry_run": false}),
    );
}

fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    updates(app, SETTLE_FRAMES);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "`{id}` did not open the {} workspace", workspace.title());
}

fn words(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
    q.iter(world)
        .filter(|(t, shown, node)| shown.get() && !t.0.trim().is_empty() && node.size().x > 0.0 && node.size().y > 0.0)
        .map(|(t, _, _)| t.0.trim().to_owned())
        .collect()
}

fn agent_badge(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut q = world.query::<&ActivityBar>();
    let bar = q.iter(world).next().unwrap_or_else(|| panic!("the activity bar is not drawn"));
    bar.shown.iter().find(|(w, _)| *w == Workspace::Agent).map(|(_, n)| *n).unwrap_or_else(|| panic!("Agent is not on the bar: {:?}", bar.shown))
}

fn history_rows(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<AgentChangeRow>>().iter(world).filter(|t| t.0.starts_with("placement_set_yaw")).count()
}

fn look_button(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<AgentChangeLook>>().iter(world).next().unwrap_or_else(|| panic!("no Look button"))
}

/// **Put the camera on the opposite corner of the map**, so only a real Look brings the table in.
fn camera_away(app: &mut App) -> Vec3 {
    let away = Vec3::new(-TABLE_AT.0, 0.0, -TABLE_AT.1);
    {
        let mut rig = app.world_mut().resource_mut::<Rig>();
        rig.focus = away;
        rig.height = 8.0;
    }
    updates(app, SETTLE_FRAMES);
    away
}

/// How far the camera's focus is from the table on the ground, in metres.
fn focus_to_table(app: &App) -> f32 {
    let focus = app.world().resource::<Rig>().focus;
    Vec2::new(focus.x - TABLE_AT.0, focus.z - TABLE_AT.1).length()
}

fn press_key(app: &mut App, code: KeyCode, text: &str) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Character(text.into()),
            state,
            text: (state == ButtonState::Pressed).then(|| text.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

#[test]
fn the_agent_workspace_shows_a_waiting_wish_and_the_agents_last_three_changes_and_look_frames_the_change_in_the_map() {
    let (mut app, _settings) = editor("agent_ws");
    assert!(Workspace::Agent.is_built(), "the Agent workspace is registered");
    // **Open Agent first**, with a clean map (a switch out of the Map with unsaved edits asks first); the agent then works while it is open.
    switch_to(&mut app, Workspace::Agent);
    // **Hand-backs are answered in the block here**, so presenting a result raises no modal over the screen.
    app.world_mut().resource_mut::<bevy_cauldron::familiar::HandBackHere>().0 = true;
    // **Four changes by the agent**, so the history has to choose the last three.
    for yaw in [0.5_f32, 1.0, 1.5, 2.0] {
        turn(&mut app, yaw);
    }

    // **A waiting wish**: a hand-back, and a wish that cites it.
    let presented = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({"title": "the courtyard", "summary": "A courtyard with its east wall placed.", "operation_ids": []}),
    );
    let after = presented["presentation"].as_str().unwrap_or_else(|| panic!("no presentation: {presented}")).to_owned();
    call(
        &mut app,
        "grimoire/wish",
        serde_json::json!({
            "after": {"presentation": after},
            "task": "place a wall along the courtyard's east side",
            "hardship": "three calls to grimoire_state to find the wall's piece id",
            "tool": "grimoire_find_piece: searches the library by label and answers the ids.",
            "example_call": "grimoire_find_piece({\"label\": \"wall\"})",
        }),
    );
    updates(&mut app, SETTLE_FRAMES);

    let blocks = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<Familiar>>().iter(world).count()
    };
    assert_eq!(blocks, 1, "the AGENT block is in the Agent workspace, once");
    let shown = words(&mut app);
    for label in ["AGENT", "HEARD", "NEWEST", "HISTORY"] {
        assert!(shown.iter().any(|w| w == label), "the Agent workspace does not show `{label}`: {shown:#?}");
    }
    assert!(shown.iter().any(|w| w.contains("grimoire_find_piece")), "the waiting wish is not in the Agent workspace: {shown:#?}");
    assert_eq!(agent_badge(&mut app), 1, "the Agent icon counts the one waiting wish");
    assert_eq!(history_rows(&mut app), HISTORY_ROWS, "the history lists the agent's last {HISTORY_ROWS} changes");

    // **The log is not read again while it does not change.**
    let reads = oplog_reads();
    updates(&mut app, 60);
    assert_eq!(oplog_reads(), reads, "the history read the operation log {} more time(s) while it did not change", oplog_reads() - reads);
    // **And is read when it does**: a fifth change moves the list and reads once.
    turn(&mut app, 2.5);
    updates(&mut app, SETTLE_FRAMES);
    assert!(oplog_reads() > reads, "a change to the log is read");
    let rows_now = history_rows(&mut app);
    let listed: Vec<String> = {
        let world = app.world_mut();
        world.query_filtered::<&Text, With<AgentChangeRow>>().iter(world).map(|t| t.0.clone()).collect()
    };
    let at = *app.world().resource::<Workspace>();
    assert_eq!(rows_now, HISTORY_ROWS, "still the last {HISTORY_ROWS}; the history lists {listed:?}; the workspace is {}", at.title());
    let again = oplog_reads();
    updates(&mut app, 60);
    assert_eq!(oplog_reads(), again, "and not read again once it is");

    // **Look frames this change's own piece.** The agent's edits leave the map unsaved, and a switch with unsaved edits asks first
    // ("Leave this map?"); the person has saved, so Look goes straight there.
    bevy_cauldron::app_verbs::save_the_open_map(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    camera_away(&mut app);
    assert!(focus_to_table(&app) > 3.0 * FRAMED_WITHIN_M, "the camera starts away from the table");
    let look = look_button(&mut app);
    app.world_mut().commands().trigger(Activate { entity: look });
    updates(&mut app, REVEAL_FRAMES);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "Look on a change takes the person to the Map");
    assert!(focus_to_table(&app) < FRAMED_WITHIN_M, "and frames the change's own piece: the camera is {:.1} m from the table", focus_to_table(&app));
}

/// **Look asks "Leave this map?" before anything moves**, and a "Stay" leaves the camera where it was.
#[test]
fn look_with_unsaved_edits_asks_first_and_a_stay_leaves_the_camera_alone() {
    let (mut app, _settings) = editor("agent_ws_stay");
    switch_to(&mut app, Workspace::Agent);
    app.world_mut().resource_mut::<bevy_cauldron::familiar::HandBackHere>().0 = true;
    turn(&mut app, 0.5);
    updates(&mut app, SETTLE_FRAMES);
    // **The map has an unsaved edit**: the agent's operations write through to disk, so the edit is made in the open map's memory, as a person's
    // would be (and a switch out of the Map saves, so the only way to be here with unsaved edits is an edit made while here).
    app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>().map.placements[0].yaw += 0.5;
    updates(&mut app, 2);
    assert!(app.world().resource::<bevy_cauldron::project::OpenMap>().dirty(), "the map has unsaved edits");
    let away = camera_away(&mut app);
    let look = look_button(&mut app);
    app.world_mut().commands().trigger(Activate { entity: look });
    updates(&mut app, SETTLE_FRAMES);
    let (problems, note, at) = {
        let state = app.world().resource::<bevy_cauldron::editor::EditorState>();
        (state.status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>(), state.status.note_text().to_owned(), *app.world().resource::<Workspace>())
    };
    assert!(
        app.world().resource::<Confirm>().is_open(),
        "Look with unsaved edits asks before it goes; workspace {}, problems {problems:?}, note {note:?}",
        at.title()
    );
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Agent, "and goes nowhere until it is answered");
    assert_eq!(app.world().resource::<Rig>().focus, away, "the camera has not moved while the question is up");

    press_key(&mut app, KeyCode::KeyN, "n");
    updates(&mut app, REVEAL_FRAMES);
    assert!(!app.world().resource::<Confirm>().is_open(), "Stay answers the question");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Agent, "Stay stays");
    assert_eq!(app.world().resource::<Rig>().focus, away, "and the camera is where it was: a refused Look moved nothing");
}

#[test]
fn the_compact_block_is_still_a_toggle_of_the_map_and_the_agent_workspace_waits_for_nothing() {
    assert!(Workspace::Agent.missing().is_none(), "a registered workspace waits for nothing");
    let (mut app, _settings) = editor("agent_ws_map");
    let blocks = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<Familiar>>().iter(world).count()
    };
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map);
    assert_eq!(blocks(&mut app), 0, "the Map shows no AGENT block until it is asked for");
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    assert_eq!(blocks(&mut app), 1, "View > Show agent puts the compact block in the Map's inspector");
    app.world_mut().commands().trigger(ValueChange { source: row, value: false, is_final: true });
    updates(&mut app, 6);
    assert_eq!(blocks(&mut app), 0, "and takes it out again");
}
