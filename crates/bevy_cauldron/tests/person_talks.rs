//! **A person holds a conversation the project's dialogue script defines** (#106): attaching one that is not
//! defined is refused, and the saved person reads back with it.
#![cfg(feature = "bevy_speech_bubbles")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::ops_people::people_of;
use bevy_cauldron::project::Project;
use serde_json::json;

use crate::fixtures;

const SCRIPT: &str = r#"(conversations: {"hello": (start: "a", nodes: {
    "a": Line(speaker: "Mara", text: "Well met.", next: None),
})})"#;

#[test]
fn a_person_keeps_a_defined_conversation_and_an_undefined_one_is_refused() {
    let root = fixtures::Fixture::new("person_talks").descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_speech_bubbles\"])").unwrap();
    std::fs::write(root.join("family/bevy_speech_bubbles.ron"), SCRIPT).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let mut ctx = Ctx { project: Some(project.clone()), session: "person_talks".to_owned(), ..Ctx::default() };

    let set = |ctx: &mut Ctx, args: serde_json::Value| ops::dispatch(ctx, "person_set", &args);
    let Applied::Done { .. } = set(&mut ctx, json!({"map": "m", "name": "Mara", "at": [1.0, 1.0], "conversation": "hello"})).unwrap() else {
        panic!("refused")
    };
    assert_eq!(people_of(&project, "m").people[0].conversation.as_deref(), Some("hello"));

    let err = set(&mut ctx, json!({"map": "m", "name": "Mara", "conversation": "nope"})).map(|_| ()).unwrap_err();
    assert!(format!("{err:?}").contains("no conversation `nope`") && format!("{err:?}").contains("hello"), "{err:?}");
    assert_eq!(people_of(&project, "m").people[0].conversation.as_deref(), Some("hello"), "a refusal leaves the person as they were");

    set(&mut ctx, json!({"map": "m", "name": "Mara", "conversation": ""})).unwrap();
    assert_eq!(people_of(&project, "m").people[0].conversation, None);
}
