//! **A person can bake a `bevy_elements` terrain preset from the Terrain field** (#99; the `terrain_preset`
//! operation had no UI entry point in a build with the feature, so the parity rule
//! `every_operation_has_a_ui_entry_point_and_the_other_way_round` was red there).
//!
//! The field's `preset <name> <seed> <cell_m> <cells_x> <cells_z>` line is the operation an agent's `apply`
//! calls, with the person's source in the log, and its refusals are the operation's.
#![cfg(feature = "bevy_elements")]

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, preset_terrain};
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_grimoire_core::heightfield::Heightfield;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

fn type_line(app: &mut App, raw: &str) {
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let mut state = app.world_mut().resource_mut::<EditorState>();
    preset_terrain(Some(&project), Some(&open), &mut state, raw);
}

fn terrain_file(app: &App) -> std::path::PathBuf {
    app.world().resource::<Project>().grimoire_dir.join("terrain").join("m.ron")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One typed line is one `terrain_preset` line stamped as a person's, the ground is the preset's, and the
/// status says what was done.**
#[test]
fn a_typed_preset_line_bakes_the_ground_through_the_operation() {
    let mut app = editor("preset-typed");
    type_line(&mut app, "preset hills 7 8 32 24");
    let field: Heightfield = ron::from_str(&std::fs::read_to_string(terrain_file(&app)).expect("the bake wrote a terrain")).unwrap();
    assert_eq!((field.width, field.depth, field.cell_m), (32, 24, 8.0), "the size typed is the size baked");
    let lines = log(&app);
    assert_eq!(lines.len(), 1, "one line is one log line: {lines:?}");
    assert_eq!(lines[0].op, "terrain_preset");
    assert_eq!(format!("{:?}", lines[0].source), "Human", "a person's bake must be stamped as a person's");
    assert!(app.world().resource::<EditorState>().status.note_text().contains("from a preset"), "the receipt is said");
}

/// **A line that is not a preset, or names one the crate lacks, is a problem and writes nothing.**
#[test]
fn a_bad_preset_line_is_a_problem_and_writes_nothing() {
    let mut app = editor("preset-bad");
    for raw in ["preset hills", "preset hills x 8 32 24", "preset volcano 1 8 32 24", "preset hills 1 8 1 24"] {
        type_line(&mut app, raw);
        assert!(!app.world().resource::<EditorState>().status.problem_text().is_empty(), "`{raw}` raised no problem");
    }
    assert!(!terrain_file(&app).exists(), "a refused line wrote a terrain");
    assert!(log(&app).is_empty(), "a refused line logged a change");
}
