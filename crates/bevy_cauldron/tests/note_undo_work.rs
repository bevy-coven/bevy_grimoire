//! **Reviewing a done note** (#93), through the registry: Keep, and Undo this note as one step that
//! leaves later unrelated changes alone and refuses when a later one depends on the work.

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, State, What};
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

fn ctx(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (30.5, 30.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        session: "test".to_owned(),
        source: Source::Agent,
        ..Ctx::default()
    }
}

fn run(ctx: &mut Ctx, op: &str, p: Value) -> Result<(String, String), String> {
    match ops::dispatch(ctx, op, &p) {
        Ok(Applied::Done { outcome, operation_id, .. }) => Ok((outcome.summary, operation_id)),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

fn file(ctx: &Ctx) -> Notes {
    Notes::load(&notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap()
}

fn ids(ctx: &Ctx) -> Vec<String> {
    ctx.open.as_ref().unwrap().map.placements.iter().map(|p| p.id.clone()).collect()
}

/// A note an agent worked: three pieces placed, then resolved naming those operations.
fn done(ctx: &mut Ctx) -> (String, Vec<String>) {
    run(ctx, "note_add", json!({"cells": [4, 4, 6, 6], "text": "three walls"})).unwrap();
    run(ctx, "note_send", json!({})).unwrap();
    run(ctx, "note_claim", json!({"id": "n1", "agent": "omp", "at": 1000})).unwrap();
    let mut ops_run = Vec::new();
    for x in [4.5, 5.5, 6.5] {
        ops_run.push(run(ctx, "placement_add", json!({"piece": "site/wall", "at": [x, 4.5]})).unwrap().1);
    }
    run(ctx, "note_resolve", json!({"id": "n1", "agent": "omp", "summary": "walls", "ops": ops_run, "captures": ["b.png", "a.png"], "at": 1010})).unwrap();
    ("n1".to_owned(), ops_run)
}

#[test]
fn undoing_a_note_removes_its_pieces_in_one_step_and_keeps_an_unrelated_change() {
    let mut ctx = ctx("note_undo_work_one_step");
    let (id, _) = done(&mut ctx);
    run(&mut ctx, "placement_add", json!({"piece": "site/wall", "at": [20.5, 20.5]})).unwrap();
    assert_eq!(ids(&ctx).len(), 5);
    run(&mut ctx, "note_undo_work", json!({"id": id, "at": 2000})).unwrap();
    let left = ids(&ctx);
    assert_eq!(left.len(), 2, "the three pieces are gone, the fixture's and the manual one stay: {left:?}");
    let n = file(&ctx);
    let note = n.get(&id).unwrap();
    assert_eq!(note.state, State::Open);
    assert!(note.stands(), "the sign is back");
    assert_eq!(note.history.last().unwrap().what, What::WorkUndone);

    // One Cmd+Z puts all three back, and the note is done again.
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(ids(&ctx).len(), 5);
    assert!(matches!(file(&ctx).get(&id).unwrap().state, State::Resolved { .. }));
}

#[test]
fn a_piece_moved_by_hand_after_the_note_refuses_naming_the_move() {
    let mut ctx = ctx("note_undo_work_refuses");
    let (id, _) = done(&mut ctx);
    let target = ids(&ctx).last().cloned().unwrap();
    let (_, turn) = run(&mut ctx, "placement_set_yaw", json!({"id": target, "yaw": 90.0})).unwrap();
    let err = run(&mut ctx, "note_undo_work", json!({"id": id, "at": 2000})).unwrap_err();
    assert!(err.contains(&turn), "names the move {turn}: {err}");
    assert_eq!(ids(&ctx).len(), 4, "a refusal changes nothing");
    assert!(matches!(file(&ctx).get(&id).unwrap().state, State::Resolved { .. }));
}

#[test]
fn keep_archives_the_note_and_one_undo_brings_it_back_to_done() {
    let mut ctx = ctx("note_keep_undo");
    let (id, _) = done(&mut ctx);
    run(&mut ctx, "note_keep", json!({"id": id, "at": 2000})).unwrap();
    let n = file(&ctx);
    assert!(matches!(n.get(&id).unwrap().state, State::Kept { .. }));
    assert!(!n.get(&id).unwrap().stands(), "the sign is gone");
    ops::undo(&mut ctx, None).unwrap();
    assert!(matches!(file(&ctx).get(&id).unwrap().state, State::Resolved { .. }));
}
