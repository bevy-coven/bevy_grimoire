//! **With the AGENT block open and "Answer hand-backs here" on, the agent's hand-back is answered in it: Accept, Undo, Dismiss** (UI-60b, #340).
//!
//! `grimoire/present` used to ask only in a modal (Keep or Undo all), which covers the agent's other work while it waits. With
//! the block open and the switch on the question is asked there instead, beside what the agent did: what was made, what the person said, what
//! the agent assumed and where to look, and three answers. With the block closed, or the switch off, it is the modal as ever, and a block closed on
//! a waiting question hands it to the modal, so a question never waits where nobody can see it.

use bevy::prelude::*;
use bevy_cauldron::confirm::{Asked, Confirm};
use bevy_cauldron::familiar::{Familiar, FamiliarItem, HandBackAnswer, HandBackCard, HandBackHere, HandBackSwitch};
use bevy_cauldron::harness;
use bevy_cauldron::present::Reply;
use bevy_gooey::{Activate, ValueChange};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("table", "alpha").place_as("table@0", "table", (0.0, 0.0)).build("m");
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

fn show_the_block(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

/// The person turned "Answer hand-backs here" on (or off) in the block.
fn answer_here(app: &mut App, on: bool) {
    let switch = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<HandBackSwitch>>().single(world).expect("the block carries the switch")
    };
    app.world_mut().commands().trigger(ValueChange { source: switch, value: on, is_final: true });
    updates(app, 4);
    assert_eq!(app.world().resource::<HandBackHere>().0, on);
}

fn try_call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("`{method}` is not registered");
    };
    let answer = app.world_mut().run_system_with(id, Some(params)).unwrap_or_else(|e| panic!("{method}: {e:?}"));
    updates(app, 4);
    answer.map_err(|e| format!("{e:?}"))
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    try_call(app, method, params).unwrap_or_else(|e| panic!("{method}: {e}"))
}

fn turn_to(app: &mut App, yaw: f32) -> serde_json::Value {
    call(app, "grimoire/operation_apply", serde_json::json!({"operation": "placement_set_yaw", "parameters": {"id": "table@0", "yaw": yaw}, "dry_run": false}))
}

fn yaw_of(app: &App) -> f32 {
    app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements.iter().find(|p| p.id == "table@0").map(|p| p.yaw).expect("the table")
}

/// The agent turns the table and hands the turn back.
fn hand_back(app: &mut App, yaw: f32) -> Result<serde_json::Value, String> {
    let done = turn_to(app, yaw);
    assert_eq!(done["outcome"], "applied", "{done}");
    try_call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "the turned table",
            "summary": "Turned the table a quarter.",
            "specified": ["turn the table"],
            "inferred": ["a quarter turn means ninety degrees"],
            "check": [{"what": "the table faces the window", "placements": ["table@0"]}],
            "operation_ids": [done["operation_id"]],
        }),
    )
}

fn answer_of(app: &mut App) -> serde_json::Value {
    call(app, "grimoire/state", serde_json::json!({}))["presentation"]["answer"].clone()
}

fn cards(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<HandBackCard>>().iter(world).count()
}

fn modal_up(app: &App) -> bool {
    app.world().resource::<Confirm>().asking(Asked::Presentation)
}

fn press(app: &mut App, reply: Reply) {
    let button = {
        let world = app.world_mut();
        world.query::<(Entity, &HandBackAnswer)>().iter(world).find(|(_, answer)| answer.0 == reply).map(|(entity, _)| entity).expect("the card has that button")
    };
    app.world_mut().commands().trigger(Activate { entity: button });
    updates(app, 6);
}

fn card_text(app: &mut App) -> String {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn with_the_block_closed_the_hand_back_is_the_modal_as_ever() {
    let mut app = editor("hand_back_modal");
    hand_back(&mut app, 90.0).expect("presented");
    assert!(modal_up(&app), "the question is the modal's");
    assert_eq!(cards(&mut app), 0, "and there is no card");
}

#[test]
fn with_the_block_open_but_the_switch_off_it_is_still_the_modal() {
    let mut app = editor("hand_back_off");
    show_the_block(&mut app, true);
    hand_back(&mut app, 90.0).expect("presented");
    assert!(modal_up(&app), "the person has not asked for it here");
    assert_eq!(cards(&mut app), 0);
}

#[test]
fn with_the_block_open_it_is_asked_there_and_says_what_was_made_assumed_and_worth_checking() {
    let mut app = editor("hand_back_card");
    show_the_block(&mut app, true);
    answer_here(&mut app, true);
    hand_back(&mut app, 90.0).expect("presented");
    assert!(!modal_up(&app), "the block holds the question, so the modal does not cover it");
    assert_eq!(cards(&mut app), 1);
    let text = card_text(&mut app);
    for words in ["Turned the table a quarter.", "turn the table", "a quarter turn means ninety degrees", "the table faces the window", "Accept", "Undo 1 operation", "Dismiss"] {
        assert!(text.contains(words), "the card says `{words}`: {text}");
    }
    assert_eq!(answer_of(&mut app), serde_json::Value::Null, "nothing is answered yet");
    let again = hand_back(&mut app, 180.0);
    assert!(again.is_err_and(|why| why.contains("waiting in the AGENT block")), "a second hand-back waits for the first");
}

#[test]
fn accept_keeps_it_and_dismiss_keeps_it_unread() {
    let mut app = editor("hand_back_keep");
    show_the_block(&mut app, true);
    answer_here(&mut app, true);
    hand_back(&mut app, 90.0).expect("presented");
    press(&mut app, Reply::Accept);
    assert_eq!(answer_of(&mut app), "kept");
    assert_eq!(cards(&mut app), 0, "the card is gone with the answer");
    assert_eq!(yaw_of(&app), 90.0, "keeping changes nothing");

    hand_back(&mut app, 180.0).expect("presented again");
    press(&mut app, Reply::Dismiss);
    assert_eq!(answer_of(&mut app), "dismissed");
    assert_eq!(yaw_of(&app), 180.0, "and neither does dismissing");
}

#[test]
fn undo_takes_every_operation_back() {
    let mut app = editor("hand_back_undo");
    show_the_block(&mut app, true);
    answer_here(&mut app, true);
    let before = yaw_of(&app);
    hand_back(&mut app, 90.0).expect("presented");
    assert_eq!(yaw_of(&app), 90.0);
    press(&mut app, Reply::Undo);
    assert_eq!(answer_of(&mut app), "undone");
    assert_eq!(yaw_of(&app), before, "the turn is taken back");
}

#[test]
fn closing_the_block_on_a_waiting_hand_back_hands_it_to_the_modal() {
    let mut app = editor("hand_back_closed");
    show_the_block(&mut app, true);
    answer_here(&mut app, true);
    hand_back(&mut app, 90.0).expect("presented");
    assert!(!modal_up(&app));
    show_the_block(&mut app, false);
    let world = app.world_mut();
    assert_eq!(world.query_filtered::<Entity, With<Familiar>>().iter(world).count(), 0);
    assert!(modal_up(&app), "the question is asked where it can be seen");
    assert_eq!(answer_of(&mut app), serde_json::Value::Null);
}
