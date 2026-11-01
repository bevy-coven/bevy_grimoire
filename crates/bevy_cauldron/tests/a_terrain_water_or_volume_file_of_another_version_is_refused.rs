//! **A terrain, water plan or volume file of another version is refused by the operation that would
//! rewrite it, and the bytes stand** (#202, H5 step 4).
//!
//! `terrain_stroke`, `water_place` and `volume_edit` each read the file, change it and write the whole
//! file back. Before these formats carried a version, a file a newer build wrote read as RON and was
//! written back as this build's, which loses whatever the newer build put in it. The refusal is in the
//! type's own deserialiser (`bevy_grimoire_core::format_version::read`), so these operations, which read
//! with a plain `ron::from_str`, refuse without each having a check of its own.

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;
use std::path::PathBuf;

use crate::fixtures;

fn open(tag: &str) -> (Ctx, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let dir = project.grimoire_dir.clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, dir)
}

/// The sentence the refusal says, whether the operation raised it or answered with it.
fn refusal(ctx: &mut Ctx, op: &str, params: &serde_json::Value) -> String {
    match ops::dispatch(ctx, op, params) {
        Err(e) => e.0,
        Ok(Applied::Done { .. }) => panic!("`{op}` took a file of another version and wrote it back"),
        Ok(other) => format!("{other:?}"),
    }
}

#[test]
fn a_file_of_another_version_refuses_the_operation_and_stays_as_it_was() {
    // The operation, the folder its file lives in, the rest of a valid file after `version`, and its params.
    let cases = [
        (
            "terrain_stroke",
            "terrain",
            "origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, heights: [0.0, 0.0, 0.0, 0.0]",
            json!({"map": "m", "stroke": {"brush": "Raise", "radius_m": 2.0, "strength": 1.5, "points": [[0.5, 0.5]]}}),
        ),
        (
            "water_place",
            "water",
            "springs: []",
            json!({"map": "m", "place": {"Spring": {"x": 3.0, "z": 4.0, "rate_m3_s": 0.5, "radius_m": 1.5}}}),
        ),
        (
            "volume_edit",
            "volume",
            "edits: []",
            json!({"map": "m", "edit": {"shape": {"Sphere": {"center": [0.0, 0.0, 0.0], "radius_m": 2.0}}, "mode": "Add"}}),
        ),
    ];
    for (op, dir, rest, params) in cases {
        for version in [2u32, 0] {
            let (mut ctx, grimoire) = open(&format!("another_version_{dir}_{version}"));
            let file = grimoire.join(dir).join("m.ron");
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            let text = format!("(version: {version}, {rest})");
            std::fs::write(&file, &text).unwrap();

            let said = refusal(&mut ctx, op, &params);
            assert!(said.contains(&format!("version {version} is not the 1 this build reads")), "`{op}` at version {version} says: {said}");
            assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "`{op}` at version {version}: the bytes stand");
        }
    }
}

/// **A file with no `version` is the first one**, so every project already on disk keeps working, and the
/// first write after the edit carries the version it was missing.
#[test]
fn a_file_with_no_version_is_taken_and_written_back_with_one() {
    let (mut ctx, grimoire) = open("no_version_water");
    let file = grimoire.join("water").join("m.ron");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "(springs: [])").unwrap();

    let spring = json!({"map": "m", "place": {"Spring": {"x": 3.0, "z": 4.0, "rate_m3_s": 0.5, "radius_m": 1.5}}});
    match ops::dispatch(&mut ctx, "water_place", &spring) {
        Ok(Applied::Done { .. }) => {}
        other => panic!("a plan from before the version was refused: {other:?}"),
    }
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.contains("version: 1"), "the rewritten plan carries its version:\n{written}");
    let plan: bevy_grimoire_core::water::WaterPlan = ron::from_str(&written).unwrap();
    assert_eq!(plan.springs.len(), 1, "and the spring:\n{written}");
}
