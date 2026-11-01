//! **A dialogue the crate's `validate_script` refuses is not written** (#106): the saved conversation is the
//! crate's own `DialogueScript`, and `family_set` is how a person or an agent edits it.
#![cfg(feature = "bevy_speech_bubbles")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

const SCRIPT: &str = r#"(conversations: {"hello": (start: "a", nodes: {
    "a": Line(speaker: "Mara", text: "Well met.", next: Some("b")),
    "b": Choice(speaker: "Mara", prompt: "Stay?", options: [(text: "Yes", next: "c")]),
    "c": Line(speaker: "Mara", text: "Good.", next: None),
})})"#;

/// The `conversations` field's value with the start node replaced: a conversation is edited whole.
fn with_start(start: &str) -> String {
    let body = SCRIPT.trim_start_matches("(conversations: ").trim_end_matches(')');
    body.replace("start: \"a\"", &format!("start: \"{start}\""))
}

fn open(name: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_speech_bubbles\"])").unwrap();
    let file = root.join("family/bevy_speech_bubbles.ron");
    std::fs::write(&file, SCRIPT).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, file)
}

#[test]
fn a_choice_pointing_nowhere_is_refused_with_the_crates_message_and_the_file_is_untouched() {
    let (mut ctx, file) = open("dialogue_bad");
    let err = ops::dispatch(
        &mut ctx,
        "family_set",
        &json!({"crate": "bevy_speech_bubbles", "field": "conversations", "value": with_start("nope")}),
    )
    .map(|_| ())
    .unwrap_err();
    assert!(format!("{err:?}").contains("start node 'nope' not found"), "{err:?}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SCRIPT);
}

#[test]
fn a_valid_edit_is_written_and_reads_back_as_the_crates_own_script() {
    let (mut ctx, file) = open("dialogue_ok");
    let done = ops::dispatch(
        &mut ctx,
        "family_set",
        &json!({"crate": "bevy_speech_bubbles", "field": "conversations", "value": with_start("b")}),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(done, Applied::Done { .. }));
    let script: bevy_speech_bubbles::DialogueScript = ron::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(script.conversation("hello").unwrap().start, "b");
}

fn placed(name: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_speech_bubbles\"])").unwrap();
    std::fs::write(root.join("family/bevy_speech_bubbles.ron"), SCRIPT).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: name.to_owned(), ..Ctx::default() }, root)
}

fn saved_family(root: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    let text = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron")).unwrap();
    let map: bevy_grimoire_core::map::Map = ron::from_str(&text).unwrap();
    map.placements[0].family.clone().into_iter().collect()
}

#[test]
fn a_conversation_attached_to_a_piece_is_saved_on_its_row_and_one_the_script_lacks_is_refused() {
    let (mut ctx, root) = placed("dialogue_attach");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let done = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_speech_bubbles", "value": "\"hello\""})).unwrap();
    assert!(matches!(done, Applied::Done { .. }), "{done:?}");
    assert_eq!(saved_family(&root)["bevy_speech_bubbles"], "\"hello\"");

    let r = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_speech_bubbles", "value": "\"goodbye\""}));
    let said = match r {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(other) => panic!("an undefined conversation was accepted: {other:?}"),
    };
    assert!(said.contains("goodbye") && said.contains("hello"), "{said}");
    assert_eq!(saved_family(&root)["bevy_speech_bubbles"], "\"hello\"", "a refusal leaves the row as it was");

    ops::undo(&mut ctx, None).unwrap();
    assert!(saved_family(&root).is_empty(), "undo takes the conversation back off the piece");
}

#[test]
fn barks_attached_to_a_piece_are_the_crates_own_bark_list_and_a_malformed_one_is_refused() {
    let (mut ctx, root) = placed("dialogue_barks");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let barks = r#"[(speaker: "Mara", kind: Speech, emotion: Joy, text: "Fine day."), (speaker: "Mara", kind: Thought, emotion: Neutral, text: "Rain soon.")]"#;
    let done = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_speech_bubbles", "value": barks})).unwrap();
    assert!(matches!(done, Applied::Done { .. }), "{done:?}");
    let saved: Vec<bevy_speech_bubbles::Bark> = ron::from_str(&saved_family(&root)["bevy_speech_bubbles"]).unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].text, "Fine day.");

    let bad = r#"[(speaker: "Mara", kind: Shout, emotion: Joy, text: "x")]"#;
    let r = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_speech_bubbles", "value": bad}));
    let said = match r {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(other) => panic!("a malformed bark was accepted: {other:?}"),
    };
    assert!(said.contains("Shout") || said.contains("bark"), "{said}");
    assert_eq!(saved_family(&root)["bevy_speech_bubbles"], barks);
}
