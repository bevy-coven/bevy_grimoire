//! **A `destructible.ron` from another version is refused by both calls and is never written back as this
//! build's** (#202, H5 step 4).
//!
//! `destructible_set` reads the marks, changes one piece and writes the whole file back, and
//! `destructible_hit` reads them. A file that a newer build wrote reads as RON, so before this it was taken
//! as this build's and written back at this build's version, which loses whatever the newer build put in
//! it. Now both refuse, naming the file and the `version` field, and the bytes stand.
#![cfg(feature = "bevy_carnage")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;
use std::path::PathBuf;

use crate::fixtures;

const PIECE: &str = "site/cart";

fn open(tag: &str) -> (Ctx, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor(PIECE, "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, root)
}

fn call(ctx: &mut Ctx, op: &str, params: serde_json::Value) -> Result<(), String> {
    match ops::dispatch(ctx, op, &params) {
        Ok(Applied::Done { .. }) => Ok(()),
        Ok(other) => Err(format!("{other:?}")),
        Err(e) => Err(e.0),
    }
}

#[test]
fn a_marks_file_of_another_version_refuses_both_calls_and_stays_as_it_was() {
    for (what, text) in [("a newer build's", "(version: 2, pieces: [\"site/other\"])"), ("version zero", "(version: 0, pieces: [\"site/other\"])")] {
        let (mut ctx, root) = open(&format!("marks_version_{}", what.replace([' ', '\''], "_")));
        let marks = root.join("destructible.ron");
        std::fs::write(&marks, text).unwrap();
        let refusals = [
            call(&mut ctx, "destructible_set", json!({"piece": PIECE, "on": true})).expect_err(what),
            call(&mut ctx, "destructible_set", json!({"piece": PIECE, "on": false})).expect_err(what),
            call(&mut ctx, "destructible_hit", json!({"piece": PIECE, "point": [0.0, 0.25, 0.0], "radius": 0.3})).expect_err(what),
        ];
        for refusal in refusals {
            assert!(refusal.contains("destructible.ron (version)") && refusal.contains("is not the 1 this program reads"), "{what}: {refusal}");
            assert!(refusal.contains("left as it is"), "{what}: the refusal says the file stands: {refusal}");
        }
        assert_eq!(std::fs::read(&marks).unwrap(), text.as_bytes(), "{what}: the bytes stand");
        assert!(!root.join("destructible/site__cart.bake.ron").exists(), "{what}: nothing was baked over a marks file this build does not read");
    }
}
