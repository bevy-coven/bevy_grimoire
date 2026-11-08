//! **The AGENT block says when the agent was last heard, whose the newest change is, and takes the agent's back** (UI-60, #246).
//!
//! View > Show agent puts a block at the top of the Map door's INSPECTOR. It says what the editor can know about an agent and
//! nothing it cannot: the call it last heard and when, and whether the newest thing on the undo stack is the agent's or the
//! person's. Its Undo button is the Undo chord, greyed unless the newest change is the agent's (a button that undid the
//! person's own edit under the agent's name would be a lie), and its switch is the View row's twin.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_cauldron::editor::{EditorState, place_for_test};
use bevy_cauldron::familiar::{AgentPause, Familiar, FamiliarHeard, FamiliarItem, FamiliarNewest, FamiliarSwitch, FamiliarUndo, PauseItem};
use bevy_cauldron::harness;
use bevy_gooey::{Activate, Checked, ValueChange};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("table", "alpha")
        .place_as("table@0", "table", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    updates(&mut app, 8);
    app
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// The person chose (or unchose) View > Show agent.
fn show_the_block(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

fn blocks(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<Familiar>>().iter(world).count()
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

fn turn_to(app: &mut App, yaw: f32) -> serde_json::Value {
    call(app, "grimoire/operation_apply", serde_json::json!({"operation": "placement_set_yaw", "parameters": {"id": "table@0", "yaw": yaw}, "dry_run": false}))
}

fn yaw_of(app: &App) -> f32 {
    app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements.iter().find(|p| p.id == "table@0").map(|p| p.yaw).expect("the table")
}

fn heard(app: &mut App) -> String {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<FamiliarHeard>>().single(world).expect("one heard line").0.clone()
}

fn newest(app: &mut App) -> String {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<FamiliarNewest>>().single(world).expect("one newest line").0.clone()
}

fn undo_button(app: &mut App) -> (Entity, bool) {
    let world = app.world_mut();
    let (button, disabled) = world
        .query_filtered::<(Entity, Has<InteractionDisabled>), With<FamiliarUndo>>()
        .single(world)
        .expect("one Undo button");
    (button, disabled)
}

fn press_undo(app: &mut App) {
    let (button, _) = undo_button(app);
    app.world_mut().commands().trigger(Activate { entity: button });
    // One chord goes down per two frames, and the dispatcher judges it then.
    updates(app, 12);
}

#[test]
fn the_block_is_there_only_when_asked_for_and_the_row_follows_it() {
    let mut app = editor("agent_block_row");
    assert_eq!(blocks(&mut app), 0, "nothing is built until it is asked for");
    show_the_block(&mut app, true);
    assert_eq!(blocks(&mut app), 1);
    let ticked = {
        let world = app.world_mut();
        world.query_filtered::<Has<Checked>, With<FamiliarItem>>().single(world).expect("one row")
    };
    assert!(ticked, "the row is ticked while the block is there");
    show_the_block(&mut app, false);
    assert_eq!(blocks(&mut app), 0, "and unticked, it is gone");
}

#[test]
fn it_says_who_made_the_newest_change_and_undo_takes_the_agents_back_and_never_the_persons() {
    let mut app = editor("agent_block_undo");
    show_the_block(&mut app, true);
    assert!(heard(&mut app).contains("Not heard"), "no call yet: `{}`", heard(&mut app));
    assert!(newest(&mut app).contains("No change"), "{}", newest(&mut app));
    assert!(undo_button(&mut app).1, "nothing to undo: the button is greyed");

    let before = yaw_of(&app);
    let answer = turn_to(&mut app, 90.0);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert!(heard(&mut app).contains("operation_apply"), "the call it made: `{}`", heard(&mut app));
    assert!(newest(&mut app).contains("the agent's"), "{}", newest(&mut app));
    assert!(!undo_button(&mut app).1, "the newest change is the agent's: the button is live");

    press_undo(&mut app);
    assert_eq!(yaw_of(&app), before, "Undo took the agent's change back");
    assert!(newest(&mut app).contains("No change"), "and the newest is no longer it: `{}`", newest(&mut app));
    assert!(undo_button(&mut app).1, "greyed again");

    // The person's own edit on top of the agent's: the button must not undo it.
    turn_to(&mut app, 45.0);
    place_for_test(app.world_mut(), "table", (3.5, 3.5));
    updates(&mut app, 4);
    let depth = app.world().resource::<EditorState>().undo_depth();
    assert!(newest(&mut app).contains("yours"), "{}", newest(&mut app));
    assert!(undo_button(&mut app).1, "the newest change is the person's: the button is greyed");
    press_undo(&mut app);
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), depth, "a press on a greyed button undoes nothing");
}

#[test]
fn the_switch_in_the_block_is_the_view_rows_twin() {
    let mut app = editor("agent_block_switch");
    show_the_block(&mut app, true);
    let switch = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarSwitch>>().single(world).expect("one switch")
    };
    app.world_mut().commands().trigger(ValueChange { source: switch, value: true, is_final: true });
    updates(&mut app, 4);
    assert!(app.world().resource::<AgentPause>().0, "the switch pauses the agent");
    let ticked = {
        let world = app.world_mut();
        world.query_filtered::<Has<Checked>, With<PauseItem>>().single(world).expect("one row")
    };
    assert!(ticked, "and the View row says so");

    let refused = turn_to(&mut app, 90.0);
    assert_eq!(refused["outcome"], "refused", "{refused}");

    // The row turns it off, and the switch in the block follows.
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PauseItem>>().single(world).expect("one row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: false, is_final: true });
    updates(&mut app, 4);
    assert!(!app.world().resource::<AgentPause>().0);
    let on = {
        let world = app.world_mut();
        world.query_filtered::<Has<Checked>, With<FamiliarSwitch>>().single(world).expect("one switch")
    };
    assert!(!on, "the switch in the block is off with it");
}
