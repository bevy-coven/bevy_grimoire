//! **A note's resolution may cite a picture only if a board has it** (#124; SG-03, #402): the agent names the
//! picture it worked from with `board picture <path>`, and a path that is on no board is refused, not recorded.
//!
//! Owner exception (2026-11-13): this file cited `reference rN` ids, which went with `references.yaml`; the check is kept, by path.

use bevy_grimoire_core::notes::{self, Notes, State};
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

fn run(ctx: &mut Ctx, op: &str, p: Value) -> Result<String, String> {
    match ops::dispatch(ctx, op, &p) {
        Ok(Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

#[test]
fn a_resolve_citing_a_picture_no_board_has_is_refused() {
    let tag = format!("note_cites_reference_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>().clone();
    let path = notes::path_for_map(&open.map_path);
    let mut c = Ctx { project: Some(app.world().resource::<Project>().clone()), open: Some(open), session: "test".to_owned(), source: Source::Agent, ..Ctx::default() };
    run(&mut c, "note_add", json!({"cells": [0, 0, 2, 2], "text": "like the courthouse"})).unwrap();
    run(&mut c, "note_send", json!({})).unwrap();
    let id = Notes::load(&path).unwrap().notes[0].id.clone();
    run(&mut c, "note_claim", json!({"id": id, "agent": "omp", "at": 1000})).unwrap();
    run(&mut c, "board_set", json!({"action": "add", "path": "refs/court.png", "caption": "front of the courthouse"})).unwrap();

    let resolve = |summary: &str| json!({"id": id, "agent": "omp", "summary": summary, "captures": ["a.png", "b.png"], "at": 1002});
    let err = run(&mut c, "note_resolve", resolve("matched board picture refs/other.png: columns")).unwrap_err();
    assert!(err.contains("refs/other.png"), "{err}");
    assert!(matches!(Notes::load(&path).unwrap().get(&id).unwrap().state, State::Claimed { .. }), "a refused resolve changed the note");

    run(&mut c, "note_resolve", resolve("matched board picture refs/court.png: front of the courthouse")).unwrap();
    assert!(matches!(Notes::load(&path).unwrap().get(&id).unwrap().state, State::Resolved { .. }));
}
