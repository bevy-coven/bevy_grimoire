//! **A note keeps its history, and Send is one undo that returns the drafts** (#90).

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, State, What};
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

fn ctx(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        session: "test".to_owned(),
        ..Ctx::default()
    }
}

fn file(ctx: &Ctx) -> Notes {
    Notes::load(&notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap()
}

fn run(ctx: &mut Ctx, op: &str, p: serde_json::Value) {
    match ops::dispatch(ctx, op, &p).unwrap() {
        Applied::Done { .. } => {}
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
}

#[test]
fn history_is_stored_and_send_all_is_one_undo_back_to_drafts() {
    let mut ctx = ctx("notes_send");
    run(&mut ctx, "note_add", json!({"cells": [0, 0, 1, 1], "text": "one"}));
    run(&mut ctx, "note_add", json!({"cells": [4, 4, 5, 5], "text": "two"}));
    run(&mut ctx, "note_edit", json!({"id": "n1", "text": "one, wider"}));

    let n = file(&ctx);
    assert_eq!(
        n.notes[0].history.iter().map(|e| e.what.clone()).collect::<Vec<_>>(),
        vec![What::Written, What::Edited { old: "one".to_owned() }],
        "an edit records the text it replaced"
    );

    run(&mut ctx, "note_send", json!({}));
    let n = file(&ctx);
    assert!(n.notes.iter().all(|n| n.state == State::Open), "Send all sends every draft");
    assert_eq!(n.notes[1].history.last().unwrap().what, What::Sent);

    ops::undo(&mut ctx, None).unwrap();
    let n = file(&ctx);
    assert!(n.notes.iter().all(|n| n.is_draft()), "one undo returns both to draft");
    assert!(n.notes.iter().all(|n| !n.history.iter().any(|e| e.what == What::Sent)), "and forgets the send");
    assert_eq!(n.notes[0].history.len(), 2, "the earlier history is intact");

    assert!(ops::dispatch(&mut ctx, "note_send", &json!({"id": "n9"})).is_err());
    run(&mut ctx, "note_send", json!({"id": "n2"}));
    assert_eq!(file(&ctx).notes[0].state, State::Draft, "one id sends only that draft");
    assert_eq!(file(&ctx).notes[1].state, State::Open);
    assert!(ops::dispatch(&mut ctx, "note_send", &json!({"id": "n2"})).is_err(), "a sent note is not sent twice");
}
