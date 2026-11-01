//! **An agent lists sent notes, claims one and gets what it needs to work it** (#91), through the
//! registry and `grimoire/state`'s notes section: the same surface an MCP client reads.

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, State};
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::{OpenMap, Project};
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

fn file(ctx: &Ctx) -> Notes {
    Notes::load(&notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap()
}

fn run(ctx: &mut Ctx, op: &str, p: Value) -> Result<String, String> {
    match ops::dispatch(ctx, op, &p) {
        Ok(Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

fn two_sent(ctx: &mut Ctx) {
    run(ctx, "note_add", json!({"cells": [0, 0, 2, 2], "text": "tidy the wall"})).unwrap();
    run(ctx, "note_add", json!({"cells": [8, 8, 9, 9], "text": "far away"})).unwrap();
    run(ctx, "note_send", json!({})).unwrap();
}

#[test]
fn claim_renew_release_are_operations_with_one_undo_each() {
    let mut ctx = ctx("note_claims_ops");
    two_sent(&mut ctx);
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "omp", "at": 1000})).unwrap();
    let err = run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "cursor", "at": 1001})).unwrap_err();
    assert!(err.contains("claimed by omp"), "{err}");
    run(&mut ctx, "note_claim", json!({"id": "n2", "agent": "cursor", "at": 1001})).expect("a distant note is free");
    assert_eq!(file(&ctx).notes[0].holder(), Some("omp"));

    run(&mut ctx, "note_renew", json!({"id": "n1", "agent": "omp", "at": 1100})).unwrap();
    assert!(matches!(file(&ctx).notes[0].state, State::Claimed { until: 1400, .. }));
    ops::undo(&mut ctx, None).unwrap();
    assert!(matches!(file(&ctx).notes[0].state, State::Claimed { until: 1300, .. }), "undo takes back the renewal");

    // An agent releases only as itself; the person's release needs no name.
    assert!(run(&mut ctx, "note_release", json!({"id": "n1", "at": 1200})).is_err());
    run(&mut ctx, "note_release", json!({"id": "n1", "agent": "omp", "at": 1200})).unwrap();
    assert_eq!(file(&ctx).notes[0].state, State::Open);
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(file(&ctx).notes[0].holder(), Some("omp"), "undo of a release gives the lease back");
}

#[test]
fn an_expired_lease_is_free_to_claim_again_with_a_controlled_clock() {
    let mut ctx = ctx("note_claims_expiry");
    two_sent(&mut ctx);
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "omp", "at": 1000})).unwrap();
    let err = run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "cursor", "at": 1299})).unwrap_err();
    assert!(err.contains("omp"), "{err}");
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "cursor", "at": 1300})).expect("300 s later the lease is gone");
    assert_eq!(file(&ctx).notes[0].holder(), Some("cursor"));
}

#[test]
fn state_lists_sent_notes_and_the_bundle_has_what_an_agent_needs() {
    let mut ctx = ctx("note_claims_bundle");
    two_sent(&mut ctx);
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "omp", "at": 1000})).unwrap();
    let open = ctx.open.as_ref().unwrap();
    let lib = ctx.project.as_ref().map(|p| &p.library);

    let v = bevy_cauldron::ops_notes::state_json(open, lib, Some("n1"), 1001);
    let sent = v["sent"].as_array().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["holder"], "omp");
    assert_eq!(sent[1]["holder"], Value::Null);

    let b = &v["bundle"];
    assert_eq!(b["text"], "tidy the wall");
    assert_eq!(b["reach_cells"], json!([-1, -1, 3, 3]), "the area plus one cell");
    assert_eq!(b["pieces"][0]["kind"], "site/wall");
    assert!(b["palette"].as_array().unwrap().iter().any(|d| d["id"] == "site/wall"));
    assert!(b["camera"].is_object() && b["picture"]["rect"].is_array());
    assert_eq!(b["earlier_notes"][0]["id"], "n2");

    let later = bevy_cauldron::ops_notes::state_json(open, lib, None, 1000 + 300);
    assert_eq!(later["sent"][0]["holder"], Value::Null, "a lease that ran out shows as sent again");
}

#[test]
fn the_holder_asks_the_person_answers_and_the_resolve_record_is_kept() {
    let mut c = ctx("note_talk");
    run(&mut c, "note_add", json!({"cells": [0, 0, 2, 2], "text": "tidy the wall"})).unwrap();
    run(&mut c, "note_send", json!({})).unwrap();
    let id = file(&c).notes[0].id.clone();
    run(&mut c, "note_claim", json!({"id": id, "agent": "omp", "at": 1000})).unwrap();
    assert!(run(&mut c, "note_ask", json!({"id": id, "agent": "other", "question": "q?", "at": 1001})).is_err());
    run(&mut c, "note_ask", json!({"id": id, "agent": "omp", "question": "Remove the barrel?", "at": 1001})).unwrap();
    assert_eq!(file(&c).get(&id).unwrap().pending_question(), Some("Remove the barrel?"));
    let done = json!({"id": id, "agent": "omp", "summary": "tidied", "assumptions": ["wall stays"], "ops": ["op-1"], "captures": ["a.png", "b.png"], "at": 1002});
    assert!(run(&mut c, "note_resolve", done.clone()).is_err(), "an unanswered question blocks the resolve");
    run(&mut c, "note_answer", json!({"id": id, "answer": "yes", "at": 1003})).unwrap();
    run(&mut c, "note_resolve", done).unwrap();
    let n = file(&c).get(&id).unwrap().clone();
    match n.state {
        State::Resolved { ops, summary, assumptions, captures } => {
            assert_eq!((ops, summary, assumptions, captures), (vec!["op-1".to_owned()], "tidied".to_owned(), vec!["wall stays".to_owned()], vec!["a.png".to_owned(), "b.png".to_owned()]));
        }
        other => panic!("{other:?}"),
    }
}
