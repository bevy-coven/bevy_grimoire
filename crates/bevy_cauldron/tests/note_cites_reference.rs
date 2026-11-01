//! **A note's resolution may cite a reference only if the map has it** (#124): the agent names the
//! picture it worked from with `reference rN`, and an id that names nothing is refused, not recorded.

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
fn a_resolve_citing_a_reference_the_map_lacks_is_refused() {
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
    run(&mut c, "reference_set", json!({"path": "refs/court.png", "caption": "front of the courthouse"})).unwrap();

    let resolve = |summary: &str| json!({"id": id, "agent": "omp", "summary": summary, "captures": ["a.png", "b.png"], "at": 1002});
    let err = run(&mut c, "note_resolve", resolve("matched reference r2 (refs/other.png): columns")).unwrap_err();
    assert!(err.contains("r2"), "{err}");
    assert!(matches!(Notes::load(&path).unwrap().get(&id).unwrap().state, State::Claimed { .. }), "a refused resolve changed the note");

    run(&mut c, "note_resolve", resolve("matched reference r1 (refs/court.png): front of the courthouse")).unwrap();
    assert!(matches!(Notes::load(&path).unwrap().get(&id).unwrap().state, State::Resolved { .. }));
}
