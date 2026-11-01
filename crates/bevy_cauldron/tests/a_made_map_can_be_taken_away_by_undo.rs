//! **Cmd+Z takes a map away that `map_create` just made, and a redo puts it back byte for byte** (E-066).

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }
}

fn make(ctx: &mut Ctx, name: &str) -> String {
    match ops::dispatch(ctx, "map_create", &json!({"name": name, "bounds": [8.0, 4.0, 8.0], "note": "kept"})) {
        Ok(Applied::Done { operation_id, .. }) => operation_id,
        other => panic!("the create did not apply: {other:?}"),
    }
}

fn file(ctx: &Ctx, name: &str) -> std::path::PathBuf {
    ctx.project.as_ref().unwrap().maps_dir.join(format!("{name}.map.ron"))
}

#[test]
fn undo_removes_the_made_map_and_redo_restores_its_bytes() {
    let mut ctx = project("map_undo");
    let id = make(&mut ctx, "annexe");
    let path = file(&ctx, "annexe");
    let written = std::fs::read_to_string(&path).unwrap();
    let (_, undone) = ops::undo(&mut ctx, Some(&id)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    assert!(!path.exists(), "the undo left the made map's file in place");
    let Applied::Done { operation_id: head, .. } = undone else { panic!("the undo did not apply") };

    let (plan, _) = ops::undo(&mut ctx, Some(&head)).unwrap_or_else(|e| panic!("redo refused: {}", e.0));
    assert!(plan.restores);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), written, "the redo did not restore the file exactly");
}

#[test]
fn two_made_maps_are_taken_away_independently() {
    let mut ctx = project("map_undo_two");
    let first = make(&mut ctx, "east");
    make(&mut ctx, "west");
    ops::undo(&mut ctx, Some(&first)).unwrap_or_else(|e| panic!("the older creation is blocked by the newer: {}", e.0));
    assert!(!file(&ctx, "east").exists());
    assert!(file(&ctx, "west").exists(), "undoing one creation took the other map");
}

#[test]
fn an_agent_cannot_remove_a_map_by_calling_the_undo_form() {
    let mut ctx = project("map_undo_agent");
    make(&mut ctx, "keep");
    let r = ops::dispatch(&mut ctx, "map_create", &json!({"remove": "keep"}));
    assert!(!matches!(r, Ok(Applied::Done { .. })), "an agent deleted a map: {r:?}");
    assert!(file(&ctx, "keep").exists());
}
