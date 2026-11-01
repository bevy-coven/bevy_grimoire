//! **`reference_set` keeps the pictures a person pointed at as project data** (#124): an add and a
//! removal each reach the file beside the map, are one undo step, and the ids are never reused.

use bevy_grimoire_core::references::{self, References};
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

fn id_of(a: Applied) -> String {
    match a {
        Applied::Done { operation_id, .. } => operation_id,
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
}

#[test]
fn a_reference_is_saved_removed_and_undone_without_reusing_an_id() {
    let root = fixtures::Fixture::new("references").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>().clone();
    let file = references::path_for_map(&open.map_path);
    let mut ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(open),
        session: "test".to_owned(),
        ..Ctx::default()
    };
    let add = |ctx: &mut Ctx, p: &str| ops::dispatch(ctx, "reference_set", &json!({"path": p, "caption": "like this"}));
    let first = id_of(add(&mut ctx, "refs/court.png").unwrap());
    assert_eq!(References::load(&file).unwrap().refs[0].id, "r1");

    // The same picture twice, a path outside the project, and a non-image are refused, nothing written.
    for bad in ["refs/court.png", "../x.png", "/etc/x.png", "notes.txt"] {
        let refused = !matches!(add(&mut ctx, bad), Ok(Applied::Done { .. }));
        assert!(refused, "{bad} was accepted");
    }
    assert_eq!(References::load(&file).unwrap().refs.len(), 1);

    // Undo the add: the file is empty and the counter is back, so r1 is issued again.
    ops::undo(&mut ctx, Some(&first)).unwrap();
    assert!(References::load(&file).unwrap().refs.is_empty());
    add(&mut ctx, "refs/other.jpg").unwrap();
    assert_eq!(References::load(&file).unwrap().refs[0].id, "r1");

    // A removed id is not given out again, and undoing the removal puts it back where it stood.
    add(&mut ctx, "refs/third.webp").unwrap();
    ops::dispatch(&mut ctx, "reference_set", &json!({"remove": "r1"})).unwrap();
    let ids = |f: &std::path::Path| References::load(f).unwrap().refs.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&file), ["r2"]);
    add(&mut ctx, "refs/fourth.gif").unwrap();
    assert_eq!(ids(&file), ["r2", "r3"]);
}

#[test]
fn undoing_a_removal_puts_the_reference_back_where_it_stood() {
    let root = fixtures::Fixture::new("references_undo").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>().clone();
    let file = references::path_for_map(&open.map_path);
    let mut ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(open),
        session: "test".to_owned(),
        ..Ctx::default()
    };
    for p in ["a.png", "b.png", "c.png"] {
        ops::dispatch(&mut ctx, "reference_set", &json!({"path": p})).unwrap();
    }
    let removal = id_of(ops::dispatch(&mut ctx, "reference_set", &json!({"remove": "r2"})).unwrap());
    ops::undo(&mut ctx, Some(&removal)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    let saved = References::load(&file).unwrap();
    assert_eq!(saved.refs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["r1", "r2", "r3"]);
    assert_eq!(saved.issued, 3);
}
