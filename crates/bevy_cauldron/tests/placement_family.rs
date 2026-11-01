//! **A brazier is a placement that carries its `bevy_lights` `Flicker`** (#103): the settings are saved
//! on the row, set through one operation, and undone with the same log a person's Cmd+Z reads.
#![cfg(feature = "bevy_lights")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, Ctx) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (root, Ctx { project: Some(project), open: Some(open), session: name.to_owned(), ..Ctx::default() })
}

fn saved(root: &std::path::Path) -> bevy_grimoire_core::map::Map {
    let text = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron")).unwrap();
    ron::from_str(&text).unwrap()
}

#[test]
fn a_flicker_set_on_a_piece_is_saved_on_its_row_and_undone() {
    let (root, mut ctx) = editor("piece_flicker");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let flicker = "(base: 900.0, phase: 1.3, depth: 0.15)";
    let done = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_lights", "value": flicker})).unwrap();
    assert!(matches!(done, Applied::Done { .. }), "{done:?}");
    let row = &saved(&root).placements[0];
    let read: bevy_lights::Flicker = ron::from_str(&row.family["bevy_lights"]).unwrap();
    assert_eq!((read.base, read.phase, read.depth), (900.0, 1.3, 0.15));

    ops::undo(&mut ctx, None).unwrap();
    assert!(saved(&root).placements[0].family.is_empty(), "undo takes the settings back off the row");
}

#[test]
fn settings_the_crates_own_type_cannot_read_are_refused_and_nothing_is_written() {
    let (root, mut ctx) = editor("piece_flicker_bad");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let r = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_lights", "value": "(base: \"bright\")"}));
    let said = match r {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(other) => panic!("a wrongly typed field was accepted: {other:?}"),
    };
    assert!(said.contains("not a bevy_lights Flicker"), "{said}");
    assert!(saved(&root).placements[0].family.is_empty());
}
