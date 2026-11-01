//! **A note left on Shift-clicked pieces names those pieces, not a rectangle** (#89): the Note tool
//! sends `{"pieces": [...]}` when pieces were picked, and the saved note keeps exactly them.

use bevy_grimoire_core::notes::{self, Notes};
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

#[test]
fn a_note_on_two_pieces_keeps_those_two_and_one_undo_takes_it_off() {
    let root = fixtures::Fixture::new("note_pieces")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .place("site/wall", (9.5, 9.5))
        .place("site/wall", (20.5, 20.5))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>().clone();
    let mut ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(open.clone()),
        session: "test".to_owned(),
        ..Ctx::default()
    };
    let ids: Vec<String> = open.map.placements.iter().take(2).map(|p| p.id.clone()).collect();
    let r = ops::dispatch(&mut ctx, "note_add", &json!({"pieces": ids, "text": "swap these"})).unwrap();
    assert!(matches!(r, Applied::Done { .. }));
    let path = notes::path_for_map(&open.map_path);
    let file = || Notes::load(&path).unwrap();
    let saved = file();
    assert_eq!(saved.notes.len(), 1);
    let mut got = saved.notes[0].site.as_ref().unwrap().pieces.clone();
    got.sort();
    let mut want = ids.clone();
    want.sort();
    assert_eq!(got, want, "the note holds the two picked pieces and not the third");
    assert!(saved.notes[0].is_draft());
    ops::undo(&mut ctx, None).unwrap();
    assert!(file().notes.is_empty());
}
