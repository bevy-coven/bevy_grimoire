//! **Tab in the note box leaves an `auto` note** (E-070): the draft's `auto` reaches `note_add`, the saved
//! note carries it, and a note left without Tab does not.

use bevy_grimoire_core::notes::{self, Notes};
use bevy_cauldron::editor::{NoteDraft, note_add_params};
use bevy_cauldron::ops::{self, Ctx};
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

#[test]
fn a_draft_with_auto_saves_an_auto_note_and_one_without_does_not() {
    let root = fixtures::Fixture::new("note_box_auto").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
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
    let mut draft = NoteDraft { area: [0, 0, 2, 2], editing: None, text: "decide the height".into(), pieces: Vec::new(), auto: true };
    ops::dispatch(&mut ctx, "note_add", &note_add_params(&draft, None)).unwrap();
    draft.auto = false;
    draft.text = "ask me".into();
    ops::dispatch(&mut ctx, "note_add", &note_add_params(&draft, None)).unwrap();
    let saved = Notes::load(&notes::path_for_map(&open.map_path)).unwrap();
    let auto_of = |text: &str| saved.notes.iter().find(|n| n.text == text).unwrap().auto;
    assert!(auto_of("decide the height"));
    assert!(!auto_of("ask me"));
}
