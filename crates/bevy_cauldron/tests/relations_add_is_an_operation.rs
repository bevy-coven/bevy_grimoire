//! **A person or an agent can add a relation to a map, not only hand-write the file** (#137, E-002).
//!
//! The relation vocabulary has been complete for weeks — eleven `relations::Kind` variants, from
//! `Faces` to `ReachableFrom` — and `relations_solve` mends a map against them. **The step before was
//! missing**: a relation could only be *added* by opening `.relations.ron` in a text editor, so the
//! constraint layer was authorable by a person with a text editor and by nobody else. It is now the
//! registered operation `relations_add`.
//!
//! Three claims, each of which can fail:
//!
//! 1. **A relation lands in the file** in the crate's own shape, and the map can then be mended against
//!    it — so an authored relation is a working constraint, not a comment.
//! 2. **A relation about a piece the map does not hold is refused**, and the file is untouched: the
//!    solver would skip it silently and the author would believe it held.
//! 3. **The undo takes back that relation by value**, so a second relation added in between cannot make
//!    it remove the wrong one.

use bevy::prelude::*;
use bevy_cauldron::ops;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/a", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    // **Two pieces on the map**, so a relation has something to be about.
    let project = app.world().resource::<Project>().clone();
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    for (id, x) in [("a", 0.0f32), ("b", 2.0)] {
        open.map.placements.push(Placed {
            id: id.to_owned(),
            descriptor: "site/a".to_owned(),
            at: (x, 0.0),
            yaw: 0.0,
            ..Placed::default()
        });
    }
    let _ = project;
    app
}

fn editor_ctx(app: &App) -> ops::Ctx {
    ops::Ctx {
        source: ops::Source::Agent,
        session: ops::session_id(),
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        ..ops::Ctx::default()
    }
}

fn relations_path(app: &App) -> std::path::PathBuf {
    relations::path_for_map(&app.world().resource::<OpenMap>().map_path)
}

fn file_relations(app: &App) -> Vec<relations::Relation> {
    std::fs::read_to_string(relations_path(app))
        .ok()
        .and_then(|t| relations::Relations::parse(&t).ok())
        .map(|r| r.relations)
        .unwrap_or_default()
}

/// **A relation lands in the file, and the map can be mended against it.**
#[test]
fn adding_a_relation_writes_it_in_the_crates_own_shape() {
    let app = editor("relations_add");
    assert!(!relations_path(&app).exists(), "the fixture starts with no relations file");

    let mut ctx = editor_ctx(&app);
    let answer = ops::dispatch(
        &mut ctx,
        "relations_add",
        &serde_json::json!({"of": "a", "kind": {"Near": {"to": "b", "within": 3.0}}, "hard": true}),
    );
    let ops::Applied::Done { outcome, .. } = answer.unwrap_or_else(|e| panic!("{}", e.0)) else {
        panic!("adding a relation to a real placement applies");
    };
    assert_eq!(outcome.summary, "`a` now keeps it");
    assert!(outcome.saved, "the file was written");

    let written = file_relations(&app);
    assert_eq!(written.len(), 1, "one relation in the file");
    assert_eq!(written[0].of, "a");
    assert_eq!(written[0].kind, relations::Kind::Near { to: "b".to_owned(), within: 3.0 });
    assert!(written[0].hard, "`hard` travels with it: the solver must not break it to mend a soft one");

    // **And it is a working constraint**: the file the operation wrote is one the solver reads.
    let text = std::fs::read_to_string(relations_path(&app)).expect("the file is there");
    let doc = relations::Relations::parse(&text).expect("and it reads back");
    let open = app.world().resource::<OpenMap>();
    let solved = relations::solve_map(&open.map.placements, &doc);
    assert_eq!(solved.unmet_before, 0, "a and b stand 2 m apart and the relation allows 3");
}

/// **A relation about a piece the map does not hold is refused, and the file is untouched.**
#[test]
fn a_relation_about_a_missing_piece_is_refused() {
    let app = editor("relations_add_missing");
    let mut ctx = editor_ctx(&app);
    let answer = ops::dispatch(
        &mut ctx,
        "relations_add",
        &serde_json::json!({"of": "nowhere", "kind": {"Faces": "b"}}),
    );
    match answer {
        Ok(ops::Applied::Refused(e)) | Err(e) => {
            assert!(e.0.contains("is not a placement on"), "the refusal says what is wrong: {}", e.0);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(
        !relations_path(&app).exists(),
        "a refused relation writes nothing: the file must not appear"
    );
}

/// **An unknown `kind` is refused with the shape, not a serde message.**
#[test]
fn an_unknown_relation_kind_is_refused() {
    let app = editor("relations_add_unknown");
    let mut ctx = editor_ctx(&app);
    let answer = ops::dispatch(
        &mut ctx,
        "relations_add",
        &serde_json::json!({"of": "a", "kind": {"Orbits": "b"}}),
    );
    match answer {
        Ok(ops::Applied::Refused(e)) | Err(e) => {
            assert!(e.0.contains("is not a relation this build understands"), "{}", e.0);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// **The undo takes back that relation by value**, so a second one added in between cannot make it
/// remove the wrong one.
#[test]
fn the_undo_removes_that_relation_and_not_another() {
    let app = editor("relations_add_undo");
    let near = serde_json::json!({"of": "a", "kind": {"Near": {"to": "b", "within": 3.0}}});
    let faces = serde_json::json!({"of": "b", "kind": {"Faces": "a"}});

    // The first add, keeping the inverse it hands back — which is the relation *by value*.
    let mut ctx = editor_ctx(&app);
    let answer = ops::dispatch(&mut ctx, "relations_add", &near);
    let ops::Applied::Done { outcome: first, .. } = answer.unwrap_or_else(|e| panic!("{}", e.0)) else {
        panic!("the first add applies");
    };
    let inverse = first.inverse.clone().expect("an add hands back its inverse");

    // A second relation, added after it: the undo must not touch this one.
    let mut ctx = editor_ctx(&app);
    let answer = ops::dispatch(&mut ctx, "relations_add", &faces);
    assert!(matches!(answer, Ok(ops::Applied::Done { .. })), "the second add applies: {answer:?}");
    assert_eq!(file_relations(&app).len(), 2, "two relations, in the order they were added");

    // Undo the FIRST one, in the operation's own undo form, by the value it handed back.
    let mut ctx = editor_ctx(&app);
    let answer = ops::dispatch(
        &mut ctx,
        "relations_add",
        &serde_json::json!({"of": "a", "kind": {"Near": {"to": "b", "within": 3.0}}, "remove": inverse}),
    );
    assert!(
        matches!(answer, Ok(ops::Applied::Done { .. })),
        "the undo form applies even though a relation was added after it: {answer:?}"
    );

    let left = file_relations(&app);
    assert_eq!(left.len(), 1, "one relation taken back: {left:?}");
    assert_eq!(left[0].of, "b", "and it is the *other* one, added after the undo's target: {left:?}");
    assert_eq!(left[0].kind, relations::Kind::Faces("a".to_owned()));
}
