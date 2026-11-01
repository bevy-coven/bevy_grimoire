//! **A note is left, edited and removed through the registry, and each is one undo** (#89).

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes};
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

fn ctx(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
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

fn done(r: Result<Applied, ops::Refusal>) -> String {
    match r.unwrap() {
        Applied::Done { operation_id, .. } => operation_id,
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
}

#[test]
fn a_note_is_added_edited_removed_and_each_is_one_undo() {
    let mut ctx = ctx("notes_ops");
    // A 4x3-cell rectangle holding the one placed piece.
    done(ops::dispatch(&mut ctx, "note_add", &json!({"cells": [0, 0, 3, 2], "text": "widen this"})));
    let saved = file(&ctx);
    assert_eq!(saved.notes.len(), 1, "the note is in the notes file beside the map");
    let note = &saved.notes[0];
    assert!(note.is_draft());
    assert_eq!(note.site.as_ref().unwrap().cells, (0, 0, 3, 2));
    assert_eq!(note.site.as_ref().unwrap().pieces.len(), 1);
    let id = note.id.clone();

    done(ops::dispatch(&mut ctx, "note_edit", &json!({"id": id, "text": "widen it twice"})));
    assert_eq!(file(&ctx).notes[0].text, "widen it twice");
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(file(&ctx).notes[0].text, "widen this", "one undo takes the edit back");

    done(ops::dispatch(&mut ctx, "note_remove", &json!({"id": id})));
    assert!(file(&ctx).notes.is_empty());
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(file(&ctx).notes.len(), 1, "one undo puts the removed note back");
    // The edit and the removal are both taken back, so the next undo is the add's.
    ops::undo(&mut ctx, None).unwrap();
    assert!(file(&ctx).notes.is_empty(), "undoing the add takes the note off");
}

#[test]
fn an_undo_only_parameter_is_refused_and_a_note_needs_text() {
    let mut ctx = ctx("notes_refusals");
    assert!(ops::dispatch(&mut ctx, "note_add", &json!({"take_back": "n1", "text": "x"})).is_err());
    assert!(ops::dispatch(&mut ctx, "note_add", &json!({"cells": [0, 0, 1, 1], "text": "  "})).is_err());
    assert!(file(&ctx).notes.is_empty());
}
