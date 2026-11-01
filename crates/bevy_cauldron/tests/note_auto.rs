//! **`note_add` carries "decide these yourself"** (E-070): the flag reaches the saved note.

use bevy_grimoire_core::notes::{self, Notes};
use bevy_cauldron::ops::{self, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

#[test]
fn an_auto_note_is_saved_auto() {
    let root = fixtures::Fixture::new("note_auto").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
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
    ops::dispatch(&mut ctx, "note_add", &json!({"cells": [0, 0, 1, 1], "text": "a courtyard", "auto": true})).unwrap();
    ops::dispatch(&mut ctx, "note_add", &json!({"cells": [0, 0, 1, 1], "text": "a lane"})).unwrap();
    let saved = Notes::load(&notes::path_for_map(&open.map_path)).unwrap();
    assert_eq!(saved.notes.iter().map(|n| n.auto).collect::<Vec<_>>(), [true, false]);
}
