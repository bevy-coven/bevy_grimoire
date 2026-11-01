//! **Notes end to end** (#95): three notes on one map (one on empty ground, one on a piece, one
//! that removes a person's piece after a question), two agents holding notes at once, two results
//! kept and one undone. The map file on disk is then the first map plus what was kept, and the
//! piece the undone note removed is back at the row it held.

use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::notes::{self, Notes, State};
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

fn ctx(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (10.5, 10.5))
        .place("site/wall", (20.5, 20.5))
        .place("site/wall", (30.5, 30.5))
        .build("m");
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

fn run(ctx: &mut Ctx, op: &str, p: Value) -> String {
    match ops::dispatch(ctx, op, &p) {
        Ok(Applied::Done { operation_id, .. }) => operation_id,
        Ok(Applied::Refused(e)) | Err(e) => panic!("{op} {p} was refused: {}", e.0),
    }
}

/// The map as the file on disk says, one `(id, x, z)` per row in file order.
fn on_disk(ctx: &Ctx) -> Vec<(String, f32, f32)> {
    let path = &ctx.open.as_ref().unwrap().map_path;
    let map = Map::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    map.placements.iter().map(|p| (p.id.clone(), p.at.0, p.at.1)).collect()
}

#[test]
fn two_notes_kept_and_one_undone_leave_the_map_file_with_only_the_kept_work() {
    let mut ctx = ctx("notes_end_to_end");
    let before = on_disk(&ctx);
    let (stall, table) = (before[0].0.clone(), before[1].0.clone());

    // The person: a note on empty ground, one on a piece, one asking about another; then Send.
    run(&mut ctx, "note_add", json!({"cells": [2, 2, 4, 4], "text": "a wall here"}));
    run(&mut ctx, "note_add", json!({"pieces": [table], "text": "a wall beside this one"}));
    run(&mut ctx, "note_add", json!({"pieces": [stall], "text": "this one stands empty: clear it?"}));
    run(&mut ctx, "note_send", json!({}));

    // Two agents, each holding a note at once.
    run(&mut ctx, "note_claim", json!({"id": "n1", "agent": "omp", "at": 1000}));
    run(&mut ctx, "note_claim", json!({"id": "n2", "agent": "claude", "at": 1000}));
    let built = run(&mut ctx, "placement_add", json!({"piece": "site/wall", "at": [3.5, 3.5]}));
    let beside = run(&mut ctx, "placement_add", json!({"piece": "site/wall", "at": [21.5, 20.5]}));
    run(&mut ctx, "note_resolve", json!({"id": "n1", "agent": "omp", "summary": "a wall", "ops": [built], "captures": ["b.png", "a.png"], "at": 1010}));
    run(&mut ctx, "note_resolve", json!({"id": "n2", "agent": "claude", "summary": "a wall beside", "ops": [beside], "captures": ["b.png", "a.png"], "at": 1010}));

    // The third removes a person's piece, so it asks first and the person answers.
    run(&mut ctx, "note_claim", json!({"id": "n3", "agent": "claude", "at": 1020}));
    run(&mut ctx, "note_ask", json!({"id": "n3", "agent": "claude", "question": "remove it?", "at": 1021}));
    run(&mut ctx, "note_answer", json!({"id": "n3", "answer": "yes, clear it", "at": 1022}));
    let cleared = run(&mut ctx, "placement_remove", json!({"id": stall}));
    run(&mut ctx, "note_resolve", json!({"id": "n3", "agent": "claude", "summary": "cleared it", "ops": [cleared], "captures": ["b.png", "a.png"], "at": 1030}));
    assert!(!on_disk(&ctx).iter().any(|(id, ..)| *id == stall), "the removal reached the file");

    // The person keeps two and undoes the removal.
    run(&mut ctx, "note_keep", json!({"id": "n1", "at": 2000}));
    run(&mut ctx, "note_keep", json!({"id": "n2", "at": 2000}));
    run(&mut ctx, "note_undo_work", json!({"id": "n3", "at": 2001}));

    let after = on_disk(&ctx);
    assert_eq!(after[..before.len()], before[..], "every row the map had is back, the cleared piece at the row it held");
    let added: Vec<(f32, f32)> = after[before.len()..].iter().map(|(_, x, z)| (*x, *z)).collect();
    assert_eq!(added, vec![(3.5, 3.5), (21.5, 20.5)], "and the two kept notes' pieces are the only additions");

    let file = Notes::load(&notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap();
    assert!(matches!(file.get("n1").unwrap().state, State::Kept { .. }));
    assert!(matches!(file.get("n2").unwrap().state, State::Kept { .. }));
    assert_eq!(file.get("n3").unwrap().state, State::Open, "the undone note is sent again");

    // One Cmd+Z takes the undo back: the piece is cleared again and the note done again.
    ops::undo(&mut ctx, None).unwrap();
    assert!(!on_disk(&ctx).iter().any(|(id, ..)| *id == stall));
    let file = Notes::load(&notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap();
    assert!(matches!(file.get("n3").unwrap().state, State::Resolved { .. }));
}
