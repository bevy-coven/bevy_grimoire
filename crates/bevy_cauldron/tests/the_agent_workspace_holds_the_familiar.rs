//! **The Agent workspace: everything about the Familiar in one place** (NV-10, #387; series getting around, #377).
//!
//! Opening Agent shows the AGENT block at full size with the waiting wish in it, the agent's last three changes
//! (read from the operation log, so a switch of workspace does not lose them), and a Look on each that takes the
//! person to the Map. The Agent icon on the activity bar carries the count of what waits: one list, shown here and
//! counted there.

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_cauldron::activity_bar::ActivityBar;
use bevy_cauldron::agent_view::{AgentChangeLook, AgentChangeRow, HISTORY_ROWS};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::familiar::Familiar;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::Activate;

use crate::fixtures;

const SETTLE_FRAMES: usize = 12;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("table", "alpha")
        .place_as("table@0", "table", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    updates(&mut app, 8);
    app
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

#[test]
fn the_agent_workspace_shows_a_waiting_wish_and_the_agents_last_three_changes_and_look_goes_to_the_map() {
    let mut app = editor("agent_ws");
    assert!(Workspace::Agent.is_built(), "the Agent workspace is registered");
    // **Open Agent first**, with a clean map (a switch out of the Map with unsaved edits asks first); the agent then works while it is open.
    switch_to(&mut app, Workspace::Agent);
    // **Hand-backs are answered in the block here**, so presenting a result raises no modal over the screen (a question on screen owns the
    // frame, and a switch waits for its answer).
    app.world_mut().resource_mut::<bevy_cauldron::familiar::HandBackHere>().0 = true;
    // **Four changes by the agent**, so the history has to choose the last three.
    for yaw in [0.5_f32, 1.0, 1.5, 2.0] {
        call(
            &mut app,
            "grimoire/operation_apply",
            serde_json::json!({"operation": "placement_set_yaw", "parameters": {"id": "table@0", "yaw": yaw}, "dry_run": false}),
        );
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

    // **The last three of the four changes**, and no more.
    let rows = {
        let world = app.world_mut();
        world
            .query_filtered::<&Text, With<AgentChangeRow>>()
            .iter(world)
            .filter(|t| t.0.starts_with("placement_set_yaw"))
            .count()
    };
    assert_eq!(rows, HISTORY_ROWS, "the history lists the agent's last {HISTORY_ROWS} changes");

    // **Look** goes to the Map, where the change is framed. The agent's edits leave the map unsaved, and a switch with unsaved edits asks
    // first ("Leave this map?"), as every route to another workspace does; the person has saved, so Look goes straight there.
    bevy_cauldron::app_verbs::save_the_open_map(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    let look = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<AgentChangeLook>>().iter(world).next().unwrap_or_else(|| panic!("no Look button"))
    };
    app.world_mut().commands().trigger(Activate { entity: look });
    updates(&mut app, SETTLE_FRAMES);
    let problems: Vec<String> = app.world().resource::<bevy_cauldron::editor::EditorState>().status.problems().iter().map(|p| p.text.clone()).collect();
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "Look on a change takes the person to the Map; problems: {problems:?}");
}

#[test]
fn the_compact_block_is_still_a_toggle_of_the_map_and_the_agent_workspace_waits_for_nothing() {
    assert!(Workspace::Agent.missing().is_none(), "a registered workspace waits for nothing");
    let mut app = editor("agent_ws_map");
    // The Map shows no AGENT block until it is asked for (View > Show agent), as before.
    let blocks = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<Familiar>>().iter(world).count()
    };
    assert_eq!(blocks, 0, "the compact block is only there when asked for");
}
