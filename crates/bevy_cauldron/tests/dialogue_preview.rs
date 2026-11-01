//! **A conversation is previewed as a walk, before it is played** (#106): `dialogue_preview` reads the
//! project's dialogue script and answers with the lines a chosen path says and the graph's own shape.
#![cfg(feature = "bevy_speech_bubbles")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

const SCRIPT: &str = r#"(conversations: {"hello": (start: "a", nodes: {
    "a": Line(speaker: "Mara", text: "Well met.", next: Some("b")),
    "b": Choice(speaker: "Mara", prompt: "Stay?", options: [(text: "Yes", next: "c"), (text: "No", next: "d")]),
    "c": Line(speaker: "Mara", text: "Good.", next: None),
    "d": Line(speaker: "Mara", text: "Farewell.", next: None),
    "orphan": Line(speaker: "Mara", text: "Never said.", next: None),
})})"#;

fn open(name: &str) -> Ctx {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_speech_bubbles\"])").unwrap();
    std::fs::write(root.join("family/bevy_speech_bubbles.ron"), SCRIPT).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    Ctx { project: Some(project), session: name.to_owned(), ..Ctx::default() }
}

fn preview(ctx: &mut Ctx, params: serde_json::Value) -> Result<serde_json::Value, String> {
    match ops::dispatch(ctx, "dialogue_preview", &params) {
        Ok(Applied::Done { outcome, .. }) => Ok(outcome.after.unwrap()),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

#[test]
fn the_chosen_path_is_said_in_order_and_the_unreachable_node_is_named() {
    let mut ctx = open("dialogue_preview_path");
    let out = preview(&mut ctx, json!({"conversation": "hello", "choices": [1]})).unwrap();
    let said: Vec<&str> = out["lines"].as_array().unwrap().iter().map(|l| l["text"].as_str().unwrap()).collect();
    assert_eq!(said, ["Well met.", "Stay?", "Farewell."]);
    assert_eq!(out["ended"], true);
    assert_eq!(out["unreachable"], json!(["orphan"]));
}

#[test]
fn a_choice_left_unmade_stops_the_walk_and_offers_its_options() {
    let mut ctx = open("dialogue_preview_wait");
    let out = preview(&mut ctx, json!({"conversation": "hello"})).unwrap();
    assert_eq!(out["ended"], false);
    assert_eq!(out["waiting_at"]["node"], "b");
    assert_eq!(out["waiting_at"]["options"], json!(["Yes", "No"]));
}

#[test]
fn an_unknown_conversation_or_choice_index_is_refused_naming_what_exists() {
    let mut ctx = open("dialogue_preview_bad");
    let e = preview(&mut ctx, json!({"conversation": "nope"})).unwrap_err();
    assert!(e.contains("hello"), "{e}");
    let e = preview(&mut ctx, json!({"conversation": "hello", "choices": [5]})).unwrap_err();
    assert!(e.contains("2 options"), "{e}");
}
