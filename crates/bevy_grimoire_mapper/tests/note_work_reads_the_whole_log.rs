//! **A note's undo and an agent's fence read the operation log the way a plain undo does** (#207): a
//! line that does not read is refused naming its line and field, not skipped. A skipped later change
//! hides a newer edit from the overlap check, and a skipped agent line makes its pieces a person's.

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, State};
use bevy_grimoire_mapper::ops::{self, Applied, Ctx, Source};
use bevy_grimoire_mapper::ops_notes::fence_refusal;
use bevy_grimoire_mapper::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

fn ctx(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (30.5, 30.5)).build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
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
        Ok(Applied::Done { operation_id, .. }) => Ok(operation_id),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// Appends a line the log's reader cannot read as an entry: `seq` of the wrong type.
fn damage_the_log(ctx: &Ctx) {
    use std::io::Write;
    let path = ops::oplog_path(ctx.project.as_ref().unwrap());
    let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(f, "{{\"seq\":\"not a number\"}}").unwrap();
}

#[test]
fn a_note_undo_refuses_when_a_later_log_line_does_not_read_and_changes_nothing() {
    let mut ctx = ctx("note_work_reads_the_whole_log_undo");
    run(&mut ctx, "note_add", json!({"cells": [4, 4, 6, 6], "text": "a wall"})).unwrap();
    run(&mut ctx, "note_send", json!({})).unwrap();
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "omp", "at": 1000})).unwrap();
    let op = run(&mut ctx, "placement_add", json!({"piece": "site/wall", "at": [4.5, 4.5]})).unwrap();
    run(&mut ctx, "note_resolve", json!({"id": "n1", "agent": "omp", "summary": "wall", "ops": [op], "captures": ["b.png", "a.png"], "at": 1010})).unwrap();
    damage_the_log(&ctx);
    let pieces = ctx.open.as_ref().unwrap().map.placements.len();

    let err = run(&mut ctx, "note_undo_work", json!({"id": "n1", "at": 2000})).unwrap_err();
    assert!(err.contains("the log does not read") && err.contains("line ") && err.contains(".seq"), "names the damaged line and field: {err}");
    assert!(err.contains("Nothing was changed"), "{err}");
    assert_eq!(ctx.open.as_ref().unwrap().map.placements.len(), pieces, "no piece was taken back");
    let n = Notes::load(&notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap();
    assert!(matches!(n.get("n1").unwrap().state, State::Resolved { .. }), "the note is still done");
}

#[test]
fn an_agent_fence_refuses_when_the_log_does_not_read_because_it_cannot_tell_whose_pieces_they_are() {
    let mut ctx = ctx("note_work_reads_the_whole_log_fence");
    run(&mut ctx, "note_add", json!({"cells": [0, 0, 2, 2], "text": "tidy"})).unwrap();
    run(&mut ctx, "note_send", json!({})).unwrap();
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "omp"})).unwrap();
    let add = json!({"piece": "site/wall", "at": [0.5, 0.5]});
    assert!(fence_refusal(&ctx, "omp", "placement_add", &add).is_none(), "inside the area, with a log that reads");
    damage_the_log(&ctx);
    let why = fence_refusal(&ctx, "omp", "placement_add", &add).expect("an unreadable log refuses");
    assert!(why.contains("the log does not read") && why.contains(".seq") && why.contains("Nothing was changed"), "{why}");
}
