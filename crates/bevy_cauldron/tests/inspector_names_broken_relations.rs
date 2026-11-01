//! **The Map inspector says which intended relations the open map breaks** (#137 slice 5).
//!
//! `grimoire_state.relations` tells an agent which relations in `<map>.relations.ron` the map breaks;
//! a person on the Map tab saw nothing. So the inspector has a `RELATIONS` row: how many are unmet
//! and missing and the first broken one in words, `N met` dimmed when all hold, `none` with no
//! file, and the file named when it does not read. The file is re-read about once a second, so an
//! edit to it shows with no restart.

use bevy::prelude::*;
use bevy_cauldron::harness;
use std::path::{Path, PathBuf};

use crate::fixtures;

/// Every placement faces +z (yaw 0). The chair at (2, 2) has its table behind it at (2, 0); the
/// crate at (8, 8.5) stands in the door's swing at (8, 8).
fn editor(name: &str, relations: Option<&str>) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .descriptor("site/door", "alpha")
        .descriptor("site/crate", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .place_as("door", "site/door", (8.0, 8.0))
        .place_as("crate", "site/crate", (8.0, 8.5))
        .build("m");
    let file = root.join("assets/grimoire/maps/m.relations.ron");
    if let Some(r) = relations {
        write(&file, r);
    }
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    (app, file)
}

fn write(path: &Path, relations: &str) {
    std::fs::write(path, format!("(version: 1, relations: [{relations}])"))
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

/// The value on the `RELATIONS` row, and its laid-out size.
fn relations_row(app: &mut App) -> (String, Vec2) {
    let world = app.world_mut();
    let texts: Vec<(Entity, String, Option<Entity>, Vec2)> = world
        .query::<(Entity, &Text, Option<&ChildOf>, &ComputedNode)>()
        .iter(world)
        .map(|(e, t, p, c)| (e, t.0.clone(), p.map(|p| p.parent()), c.size()))
        .collect();
    let label = texts
        .iter()
        .find(|t| t.1 == "RELATIONS")
        .unwrap_or_else(|| panic!("no RELATIONS label: {:?}", texts.iter().map(|t| &t.1).collect::<Vec<_>>()));
    let row = label.2.unwrap_or_else(|| panic!("the RELATIONS label has no row"));
    let value = texts
        .iter()
        .find(|t| t.2 == Some(row) && t.0 != label.0)
        .unwrap_or_else(|| panic!("the RELATIONS row has no value"));
    (value.1.clone(), value.3)
}

const BROKEN: &str = r#"
    (of: "chair", kind: Faces("table")),
    (of: "chair", kind: Near(to: "table", within: 3.0)),
    (of: "door", kind: ClearInFront(depth: 2.0, width: 1.0)),
    (of: "lamp", kind: Near(to: "door", within: 2.0)),
"#;

#[test]
fn the_broken_count_and_the_first_broken_relation_read_on_the_row() {
    let (mut app, _) = editor(&format!("inspector_relations_broken_{}", std::process::id()), Some(BROKEN));
    let (value, size) = relations_row(&mut app);
    assert_eq!(value, "2 unmet, 1 missing: chair faces table");
    assert!(size.x > 0.0 && size.y > 0.0, "the value is laid out, so it is drawn: {size:?}");
}

#[test]
fn no_file_reads_none_and_all_met_reads_the_count() {
    let (mut app, _) = editor(&format!("inspector_relations_none_{}", std::process::id()), None);
    assert_eq!(relations_row(&mut app).0, "none");

    let (mut app, _) = editor(
        &format!("inspector_relations_met_{}", std::process::id()),
        Some(r#"(of: "chair", kind: Near(to: "table", within: 3.0))"#),
    );
    assert_eq!(relations_row(&mut app).0, "1 met");
}

#[test]
fn an_edit_to_the_file_shows_without_a_restart() {
    let (mut app, file) = editor(&format!("inspector_relations_live_{}", std::process::id()), None);
    assert_eq!(relations_row(&mut app).0, "none");
    write(&file, r#"(of: "door", kind: ClearInFront(depth: 2.0, width: 1.0))"#);
    std::thread::sleep(std::time::Duration::from_millis(1200));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(relations_row(&mut app).0, "1 unmet: door clear in front (crate in the way)");

    std::fs::write(&file, "(version: 1, relations: [oops])").unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(std::time::Duration::from_millis(1200));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(relations_row(&mut app).0, "m.relations.ron does not read");
}
