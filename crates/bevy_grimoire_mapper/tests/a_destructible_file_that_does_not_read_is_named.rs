//! **A destructible marks file or a bake that does not read is refused naming the file and the field, and
//! is never overwritten** (#200, H3).
//!
//! `destructible_set` reads `destructible.ron`, changes one piece and writes the whole file back, and
//! `destructible_hit` reads it and the piece's cached bake. The marks file read with the parser's
//! position and no field, and a bake that did not parse was baked again over the damaged bytes with no
//! word. Now each of the three bad files (corrupt, a field of the wrong type, a required field missing)
//! refuses both calls naming the file and the field, the bytes stay, and once the file is fixed (or, for a
//! bake, removed) the same call applies and keeps the rows that were there. A bake that reads but was made
//! from another mesh is stale, not damaged: it is baked again.
#![cfg(feature = "bevy_carnage")]

use bevy_grimoire_mapper::ops::{self, Applied, Ctx};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::fixtures;

const PIECE: &str = "site/cart";

fn open(tag: &str) -> (Ctx, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor(PIECE, "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_grimoire_mapper::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, root)
}

fn set(ctx: &mut Ctx, on: bool) -> Result<ops::Outcome, String> {
    match ops::dispatch(ctx, "destructible_set", &json!({"piece": PIECE, "on": on})) {
        Ok(Applied::Done { outcome, .. }) => Ok(outcome),
        Ok(other) => Err(format!("{other:?}")),
        Err(e) => Err(e.0),
    }
}

fn hit(ctx: &mut Ctx) -> Result<(), String> {
    match ops::dispatch(ctx, "destructible_hit", &json!({"piece": PIECE, "point": [0.0, 0.25, 0.0], "radius": 0.3})) {
        Ok(Applied::Done { .. }) => Ok(()),
        Ok(other) => Err(format!("{other:?}")),
        Err(e) => Err(e.0),
    }
}

fn marks(root: &Path) -> PathBuf {
    root.join("destructible.ron")
}

fn bake(root: &Path) -> PathBuf {
    root.join("destructible/site__cart.bake.ron")
}

/// The three bad marks files: `(what is wrong, the text, the field the refusal must name)`.
const BAD_MARKS: [(&str, &str, Option<&str>); 3] = [
    ("corrupt", "(version: 1, pieces: [\"site/cart\"", None),
    ("wrong type", "(version: \"one\", pieces: [\"site/cart\"])", Some("version")),
    ("missing field", "(version: 1)", Some("pieces")),
];

/// The three bad bakes, and a damaged chunk where a hand edit lands.
const BAD_BAKES: [(&str, &str, Option<&str>); 4] = [
    ("corrupt", "(version: 1, key: 5, chunks: [(points: [(0.0, 0.0, 0.0)]", None),
    ("wrong type", "(version: 1, key: \"k\", chunks: [])", Some("key")),
    ("missing field", "(version: 1, chunks: [])", Some("key")),
    ("damaged chunk", "(version: 1, key: 5, chunks: [(points: [], faces: []), (points: 7, faces: [])])", Some("chunks[1].points")),
];

fn names(refusal: &str, file: &str, field: Option<&str>, what: &str) {
    assert!(refusal.contains(file), "{what}: the refusal names `{file}`: {refusal}");
    if let Some(field) = field {
        assert!(refusal.contains(&format!("({field})")), "{what}: the refusal names the field `{field}`: {refusal}");
    }
    assert!(refusal.contains("left as it is"), "{what}: the refusal says the file stands: {refusal}");
}

#[test]
fn a_marks_file_that_does_not_read_refuses_both_calls_and_stays_as_it_was() {
    for (what, text, field) in BAD_MARKS {
        let (mut ctx, root) = open(&format!("marks_bad_{}", what.replace(' ', "_")));
        std::fs::write(marks(&root), text).unwrap();
        let before = text.as_bytes();
        for on in [true, false] {
            let refusal = set(&mut ctx, on).expect_err(what);
            names(&refusal, "destructible.ron", field, what);
        }
        let refusal = hit(&mut ctx).expect_err(what);
        names(&refusal, "destructible.ron", field, what);
        assert_eq!(std::fs::read(marks(&root)).unwrap(), before, "{what}: the bytes stand");
        assert!(!bake(&root).exists(), "{what}: nothing was baked over a marks file that does not read");
    }
}

#[test]
fn once_the_marks_are_fixed_the_same_call_applies_and_keeps_the_pieces_that_were_there() {
    let (mut ctx, root) = open("marks_fixed");
    std::fs::write(marks(&root), BAD_MARKS[1].1).unwrap();
    set(&mut ctx, true).expect_err("refused while the file is bad");
    std::fs::write(marks(&root), "(version: 1, pieces: [\"site/other\"])").unwrap();
    let done = set(&mut ctx, true).expect("the fixed file applies");
    assert_eq!(done.after.as_ref().unwrap()["baked"], true, "{done:?}");
    let text = std::fs::read_to_string(marks(&root)).unwrap();
    assert!(text.contains("site/other") && text.contains("site/cart"), "the piece that was there stays: {text}");
}

#[test]
fn a_bake_that_does_not_read_refuses_both_calls_and_stays_as_it_was() {
    for (what, text, field) in BAD_BAKES {
        let (mut ctx, root) = open(&format!("bake_bad_{}", what.replace(' ', "_")));
        set(&mut ctx, true).expect("a first mark bakes");
        std::fs::write(bake(&root), text).unwrap();
        let marked = std::fs::read(marks(&root)).unwrap();
        // Taking the mark off reads no bake, so only marking (which bakes) and a hit refuse.
        let refusal = set(&mut ctx, true).expect_err(what);
        names(&refusal, "destructible/site__cart.bake.ron", field, what);
        let refusal = hit(&mut ctx).expect_err(what);
        names(&refusal, "destructible/site__cart.bake.ron", field, what);
        assert_eq!(std::fs::read(bake(&root)).unwrap(), text.as_bytes(), "{what}: the bake was not baked over");
        assert_eq!(std::fs::read(marks(&root)).unwrap(), marked, "{what}: the marks were not written either");
    }
}

#[test]
fn a_removed_bake_is_baked_again_and_a_stale_one_too() {
    let (mut ctx, root) = open("bake_stale");
    set(&mut ctx, true).expect("a first mark bakes");
    let good = std::fs::read(bake(&root)).unwrap();
    std::fs::write(bake(&root), "(version: 1, key: 5, chunks: [])").unwrap();
    // Reads, but was made from something else: stale, so baked again, to the same bytes as the first time.
    let again = set(&mut ctx, true).expect("a stale bake is baked again");
    assert_eq!(again.after.as_ref().unwrap()["baked"], true, "{again:?}");
    assert_eq!(std::fs::read(bake(&root)).unwrap(), good);
    // Removed after damage: the way out the refusal names.
    std::fs::write(bake(&root), BAD_BAKES[0].1).unwrap();
    hit(&mut ctx).expect_err("damaged");
    std::fs::remove_file(bake(&root)).unwrap();
    hit(&mut ctx).expect("a removed bake is baked again");
    assert_eq!(std::fs::read(bake(&root)).unwrap(), good);
}
