//! **The editor holds an agent to its note** (#92): `grimoire/operation_apply` with an `agent` refuses,
//! before anything is written, an operation reaching outside the note's area plus one cell, and one
//! that removes a piece a person placed until the agent has asked and the person has answered.

use bevy_grimoire_core::map::Map;
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::ops_notes::fence_refusal;
use bevy_cauldron::project::{OpenMap, Project};
use bevy::prelude::*;
use serde_json::{Value, json};

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
        source: Source::Agent,
        ..Ctx::default()
    }
}

fn run(ctx: &mut Ctx, op: &str, p: Value) -> Result<String, String> {
    match ops::dispatch(ctx, op, &p) {
        Ok(Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

fn map(ctx: &Ctx) -> Map {
    ctx.open.as_ref().unwrap().map.clone()
}

/// A note over cells (0,0)..(2,2), sent and claimed by `omp` at the real clock.
fn claimed(ctx: &mut Ctx) {
    run(ctx, "note_add", json!({"cells": [0, 0, 2, 2], "text": "tidy the wall"})).unwrap();
    run(ctx, "note_send", json!({})).unwrap();
    run(ctx, "note_claim", json!({"id": "n1", "agent": "omp"})).unwrap();
}

#[test]
fn an_operation_outside_the_area_plus_margin_is_refused_and_changes_nothing() {
    let mut ctx = ctx("note_fence_outside");
    claimed(&mut ctx);
    let before = map(&ctx);
    let why = fence_refusal(&ctx, "omp", "placement_add", &json!({"piece": "site/wall", "at": [10.5, 10.5]}))
        .expect("ten cells away is outside the area plus its margin");
    assert!(why.contains("outside note n1") && why.contains("nothing was changed"), "{why}");
    assert_eq!(map(&ctx), before, "the trial left the open map alone");
    assert!(fence_refusal(&ctx, "cursor", "placement_add", &json!({"piece": "site/wall", "at": [10.5, 10.5]})).is_none(), "an agent holding nothing is not fenced here");
    assert!(fence_refusal(&ctx, "omp", "placement_add", &json!({"piece": "site/wall", "at": [0.5, 0.5]})).is_none(), "inside the area is just done");
}

#[test]
fn removing_a_person_placed_piece_waits_for_a_question_and_its_answer() {
    let mut ctx = ctx("note_fence_remove");
    claimed(&mut ctx);
    let id = map(&ctx).placements[0].id.clone();
    let remove = json!({"id": id});
    let why = fence_refusal(&ctx, "omp", "placement_remove", &remove).expect("the file's own piece is a person's");
    assert!(why.contains("placed by a person") && why.contains("note_ask"), "{why}");

    run(&mut ctx, "note_ask", json!({"id": "n1", "agent": "omp", "question": "remove the old wall?"})).unwrap();
    assert!(fence_refusal(&ctx, "omp", "placement_remove", &remove).unwrap().contains("waiting"), "asking is not an answer");
    run(&mut ctx, "note_answer", json!({"id": "n1", "answer": "yes"})).unwrap();
    assert!(fence_refusal(&ctx, "omp", "placement_remove", &remove).is_none(), "answered, so allowed");
}

#[test]
fn a_piece_the_agent_placed_is_its_own_to_remove() {
    let mut ctx = ctx("note_fence_own");
    claimed(&mut ctx);
    run(&mut ctx, "placement_add", json!({"piece": "site/wall", "at": [0.5, 0.5]})).unwrap();
    let mine = map(&ctx).placements.iter().find(|p| p.at == (0.5, 0.5)).expect("placed").id.clone();
    assert!(fence_refusal(&ctx, "omp", "placement_remove", &json!({"id": mine})).is_none());
}
