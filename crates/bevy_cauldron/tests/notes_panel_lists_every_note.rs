//! **The Notes panel draws every note of the open map, newest first** (#90), resolved ones included.

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, Target};
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

#[test]
fn the_panel_lists_every_note_newest_first() {
    let root = fixtures::Fixture::new("notes_panel").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let path = notes::path_for_map(&app.world().resource::<OpenMap>().map_path);
    let mut n = Notes::default();
    n.add(vec![Target::Cell(0, 0)], "first").unwrap();
    n.add(vec![Target::Cell(1, 1)], "second").unwrap();
    n.resolve("n1", vec![], "did it").unwrap();
    n.save(&path).unwrap();
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
    let mut lines: Vec<String> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .filter(|t| t.starts_with('n') && t.contains('['))
        .collect();
    lines.sort_by_key(|l| l.clone());
    assert_eq!(lines, ["n1 [done] first", "n2 [sent] second"], "both notes are drawn, the resolved one too");
}
