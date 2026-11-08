//! **The AGENT block says when the agent was last heard, whose the newest change is, and takes the agent's back** (UI-60, #246).
//!
//! View > Show agent puts a block at the top of the Map door's INSPECTOR. It says what the editor can know about an agent and
//! nothing it cannot: the call it last heard and when, and whether the newest thing on the undo stack is the agent's or the
//! person's. Its Undo button is the Undo chord, greyed unless the newest change is the agent's (a button that undid the
//! person's own edit under the agent's name would be a lie), and its switch is the View row's twin.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_cauldron::editor::{EditorState, place_for_test};
use bevy_cauldron::familiar::{AgentPause, Familiar, FamiliarItem, FamiliarLine, FamiliarSwitch, FamiliarUndo, PauseItem};
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

fn line(app: &mut App, which: FamiliarLine) -> String {
    let world = app.world_mut();
    world
        .query::<(&Text, &FamiliarLine)>()
        .iter(world)
        .find(|(_, line)| **line == which)
        .map(|(text, _)| text.0.clone())
        .unwrap_or_else(|| panic!("the block has no {which:?} line"))
}

fn heard(app: &mut App) -> String {
    line(app, FamiliarLine::Heard)
}

fn newest(app: &mut App) -> String {
    line(app, FamiliarLine::Newest)
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

/// A key the way a person presses it: written as input, then one update, which is the path the question's modal reads.
fn press(app: &mut App, code: KeyCode) {
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

#[test]
fn what_the_agent_handed_back_is_waiting_on_the_person_and_then_says_how_they_answered() {
    let mut app = editor("agent_block_handback");
    show_the_block(&mut app, true);
    assert_eq!(line(&mut app, FamiliarLine::HandBack), "", "nothing handed back yet, so the line says nothing");

    let done = turn_to(&mut app, 90.0);
    assert_eq!(done["outcome"], "applied", "{done}");
    call(
        &mut app,
        "grimoire/present",
        serde_json::json!({
            "title": "the turned table",
            "summary": "Turned the table a quarter.",
            "specified": ["turn the table"],
            "operation_ids": [done["operation_id"]],
        }),
    );
    updates(&mut app, 4);
    let waiting = line(&mut app, FamiliarLine::HandBack);
    assert!(waiting.contains("the turned table") && waiting.contains("Waiting for your answer"), "{waiting}");
    assert!(heard(&mut app).contains("present"), "the hand-back is the call it last made: `{}`", heard(&mut app));

    // N keeps: the safe answer, which changes nothing.
    press(&mut app, KeyCode::KeyN);
    updates(&mut app, 4);
    let answered = line(&mut app, FamiliarLine::HandBack);
    assert!(answered.contains("You answered: kept"), "{answered}");
    assert_eq!(yaw_of(&app), 90.0, "keeping changes nothing");
}

/// **The block's lines stand one under the other, never over one another**: a line that wraps takes the room it wraps into. The
/// text is the longest it gets (the agent's call and its change), because a short line cannot overlap anything.
#[test]
fn the_blocks_lines_do_not_overlap_when_they_wrap() {
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    let mut app = editor("agent_block_layout");
    show_the_block(&mut app, true);
    let done = turn_to(&mut app, 90.0);
    assert_eq!(done["outcome"], "applied", "{done}");
    call(
        &mut app,
        "grimoire/present",
        serde_json::json!({"title": "the turned table, which is a long title", "summary": "Turned the table.", "operation_ids": [done["operation_id"]]}),
    );
    updates(&mut app, 12);
    let mut boxes: Vec<(FamiliarLine, f32, f32, String)> = {
        let world = app.world_mut();
        world
            .query::<(&FamiliarLine, &ComputedNode, &UiGlobalTransform, &Text)>()
            .iter(world)
            .map(|(line, node, at, text)| (*line, at.translation.y - node.size().y / 2.0, at.translation.y + node.size().y / 2.0, text.0.clone()))
            .collect()
    };
    boxes.sort_by(|a, b| a.1.total_cmp(&b.1));
    let order: Vec<FamiliarLine> = boxes.iter().map(|b| b.0).collect();
    assert_eq!(order, [FamiliarLine::Heard, FamiliarLine::Newest, FamiliarLine::HandBack], "{boxes:?}");
    for pair in boxes.windows(2) {
        assert!(pair[1].1 >= pair[0].2 - 0.5, "`{}` ({:?}) overlaps `{}` ({:?}): {boxes:?}", pair[0].3, (pair[0].1, pair[0].2), pair[1].3, (pair[1].1, pair[1].2));
    }
}
