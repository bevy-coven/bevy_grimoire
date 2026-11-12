//! **Undo on a row of the Agent workspace's history** (NV-10, #387; series getting around, #377).
//!
//! Each of the agent's last changes in the Agent workspace has a Look (pinned in `the_agent_workspace_holds_the_familiar.rs`) and an **Undo**.
//! The undo is the operation layer's own: the inverse the log recorded goes through the registry as the person's undo, the map is put back,
//! and the log gets a line that names what it took back. What is measured:
//!
//! 1. **Undo on the newest row puts the piece back** (its facing returns to what it was before the change) and the row leaves the history.
//! 2. **A refusal is said, never swallowed**: the older change cannot be taken back while a newer one in its scope still stands, and the status
//!    line carries the operation layer's own sentence while the map keeps what it had.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::agent_view::{AgentChangeRow, AgentChangeUndo};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

const SETTLE_FRAMES: usize = 12;
/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;

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
        .place_as("table@0", "table", (4.0, 4.0))
        .side(64.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    updates(&mut app, 8);
    // The editor does not follow the agent here (the `follow_agent` setting off): a followed change would carry the person to the Map under
    // the history being measured.
    let file = std::env::temp_dir().join(format!("agent_undo_settings_{name}_{}.yaml", std::process::id()));
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

fn yaw(app: &App) -> f32 {
    app.world().resource::<OpenMap>().map.placements.iter().find(|p| p.id == "table@0").map(|p| p.yaw).unwrap_or_else(|| panic!("`table@0` is not on the map"))
}

/// The Undo buttons of the history's rows, newest first (the order the rows are drawn in).
fn undo_buttons(app: &mut App) -> Vec<(Entity, String)> {
    let world = app.world_mut();
    world.query::<(Entity, &AgentChangeUndo)>().iter(world).map(|(e, u)| (e, u.0.clone())).collect()
}

fn history_rows(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<AgentChangeRow>>().iter(world).filter(|t| t.0.starts_with("placement_set_yaw")).count()
}

fn press(app: &mut App, button: Entity) {
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: button });
    updates(app, SETTLE_FRAMES);
}

/// **Undo on the newest row puts the piece back, and the row is gone.**
#[test]
fn undo_on_the_newest_row_takes_that_change_back() {
    let (mut app, _settings) = editor("agent_undo_newest");
    switch_to(&mut app, Workspace::Agent);
    let start = yaw(&app);
    turn(&mut app, 0.5);
    turn(&mut app, 1.0);
    updates(&mut app, SETTLE_FRAMES);
    assert!((yaw(&app) - 1.0).abs() < 1e-3, "the agent's second turn is on the map");
    assert_eq!(history_rows(&mut app), 2, "both changes are in the history");

    let buttons = undo_buttons(&mut app);
    assert_eq!(buttons.len(), 2, "every row has an Undo");
    press(&mut app, buttons[0].0);

    assert!((yaw(&app) - 0.5).abs() < 1e-3, "Undo on the newest row put the facing back to what the first change left: {}", yaw(&app));
    assert_eq!(history_rows(&mut app), 1, "the change that was taken back is not the agent's change any more");
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(note.contains(&buttons[0].1), "the status line names the change it took back: {note:?}");
    assert_ne!(start, yaw(&app), "the first change still stands");
}

/// **The older change cannot be taken back first**, and the refusal is the operation layer's own sentence on the status line.
#[test]
fn undo_on_an_older_row_while_a_newer_change_stands_says_why_and_changes_nothing() {
    let (mut app, _settings) = editor("agent_undo_older");
    switch_to(&mut app, Workspace::Agent);
    turn(&mut app, 0.5);
    turn(&mut app, 1.0);
    updates(&mut app, SETTLE_FRAMES);
    let buttons = undo_buttons(&mut app);
    assert_eq!(buttons.len(), 2);

    press(&mut app, buttons[1].0);

    assert!((yaw(&app) - 1.0).abs() < 1e-3, "a refused undo changes nothing: {}", yaw(&app));
    assert_eq!(history_rows(&mut app), 2, "and both changes still stand in the history");
    let said = app.world().resource::<EditorState>().status.problem_text().to_owned();
    assert!(said.contains("undo stopped at") && said.contains(&buttons[1].1), "the refusal is said and names the change: {said:?}");
}
