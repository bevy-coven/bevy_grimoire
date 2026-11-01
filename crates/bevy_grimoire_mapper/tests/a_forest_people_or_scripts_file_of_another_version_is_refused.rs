//! **A forest, people or triggers file of another version is refused by whoever reads it, and the bytes
//! stand** (#202, H5 step 4).
//!
//! `forest_scatter`, `person_set` and `trigger_set` read a map's file, change one row and write the whole
//! file back. Before these formats carried a version, a file a newer build wrote read as RON (or, since
//! each type denies unknown fields, was refused as damaged, which says nothing about what is wrong) and a
//! file from before the version had no way to say so. The refusal is in the type's own deserialiser
//! (`bevy_grimoire_core::format_version::read`), so every reader of the type refuses without a check of
//! its own, and a refusal names the field `version` and the version this build reads.

use bevy_grimoire_core::reload_check;
use bevy_grimoire_mapper::ops::{self, Applied, Ctx};
use bevy_grimoire_mapper::ops_forest::Forest;
use bevy_grimoire_mapper::ops_people::People;
use bevy_grimoire_mapper::ops_scripts::Scripts;
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::fixtures;

fn open(tag: &str) -> (Ctx, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_grimoire_mapper::project::Project>().clone();
    let dir = project.grimoire_dir.clone();
    let open = app.world().resource::<bevy_grimoire_mapper::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: tag.to_owned(), ..Ctx::default() }, dir)
}

/// The sentence the refusal says, whether the operation raised it or answered with it.
fn refusal(ctx: &mut Ctx, op: &str, params: &Value) -> String {
    match ops::dispatch(ctx, op, params) {
        Err(e) => e.0,
        Ok(Applied::Done { .. }) => panic!("`{op}` took a file of another version and wrote it back"),
        Ok(other) => format!("{other:?}"),
    }
}

/// The operation, the folder its file lives in, the rest of a valid file after `version`, and a call that
/// works on a file of the right version.
fn operations() -> Vec<(&'static str, &'static str, &'static str, Value)> {
    #[cfg_attr(not(feature = "bevy_flora"), allow(unused_mut))]
    let mut ops = vec![
        ("person_set", "people", "people: []", json!({"map": "m", "name": "Ola", "at": [2.0, 2.0]})),
        ("trigger_set", "scripts", "triggers: []", json!({"map": "m", "id": "t", "on": "Activity(\"work\")", "then": "let x = 1;"})),
    ];
    // A forest is planted only in a build with `bevy_flora`; the refusal comes before the stroke is looked at.
    #[cfg(feature = "bevy_flora")]
    ops.push((
        "forest_scatter",
        "forest",
        "trees: []",
        json!({"map": "m", "plant": {"points": [[0.0, 0.0], [12.0, 0.0]], "radius_m": 4.0, "spacing_m": 3.0, "seed": 3, "species": [{"name": "oak", "weight": 1.0}]}}),
    ));
    ops
}

#[test]
fn a_file_of_another_version_refuses_the_operation_and_stays_as_it_was() {
    for (op, dir, rest, params) in operations() {
        for version in [2u32, 0] {
            let (mut ctx, grimoire) = open(&format!("another_version_{dir}_{version}"));
            let file = grimoire.join(dir).join("m.ron");
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            let text = format!("(version: {version}, {rest})");
            std::fs::write(&file, &text).unwrap();

            let said = refusal(&mut ctx, op, &params);
            assert!(said.contains(&format!("version {version} is not the 1 this build reads")), "`{op}` at version {version} says: {said}");
            assert!(said.contains("(version)"), "`{op}` at version {version} names the field: {said}");
            assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "`{op}` at version {version}: the bytes stand");
        }
    }
}

/// **The same refusal for a reader that is not an operation**: the forest view reads the file with
/// `reload_check::read`, and a forest cannot be written without `bevy_flora`, so the type is read directly.
#[test]
fn the_types_themselves_refuse_another_version_and_name_the_field() {
    fn refused<T: serde::de::DeserializeOwned>(kind: &'static str, rest: &str) {
        for version in [2u32, 0, 7] {
            let text = format!("(version: {version}, {rest})");
            let r = match reload_check::read::<T>(kind, "x.ron", &text) {
                Err(r) => r,
                Ok(_) => panic!("`{kind}` at version {version} was taken as this build's"),
            };
            assert_eq!(r.field.as_deref(), Some("version"), "`{kind}` at version {version}: {}", r.sentence());
            assert!(r.reason.contains(&format!("version {version} is not the 1 this build reads")), "`{kind}` at version {version} says: {}", r.reason);
        }
    }
    refused::<Forest>("forest", "trees: []");
    refused::<People>("people", "people: []");
    refused::<Scripts>("scripts", "triggers: []");
}

/// **A file with no `version` is the first one**, so every project already on disk keeps working, and the
/// first write after the edit carries the version it was missing, beside the rows that were there.
#[test]
fn a_file_with_no_version_is_taken_and_written_back_with_one() {
    let cases = [
        ("person_set", "people", "(people: [(name: \"Mara\", at: (1.0, 2.0))])", "Mara", json!({"map": "m", "name": "Ola", "at": [2.0, 2.0]}), "Ola"),
        (
            "trigger_set",
            "scripts",
            "(triggers: [(id: \"first\", on: \"Activity(\\\"work\\\")\", then: \"let x = 1;\")])",
            "first",
            json!({"map": "m", "id": "t", "on": "Activity(\"work\")", "then": "let x = 1;"}),
            "\"t\"",
        ),
    ];
    for (op, dir, old, old_row, params, new_row) in cases {
        // Without `bevy_society` a trigger is refused whatever the file says (see
        // `a_file_that_does_not_read_is_never_overwritten`), so the type-level test below covers it.
        if op == "trigger_set" && !cfg!(feature = "bevy_society") {
            continue;
        }
        let (mut ctx, grimoire) = open(&format!("no_version_{dir}"));
        let file = grimoire.join(dir).join("m.ron");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, old).unwrap();

        match ops::dispatch(&mut ctx, op, &params) {
            Ok(Applied::Done { .. }) => {}
            other => panic!("`{op}`: a file from before the version was refused: {other:?}"),
        }
        let written = std::fs::read_to_string(&file).unwrap();
        assert!(written.contains("version: 1"), "`{op}`: the rewritten file carries its version:\n{written}");
        assert!(written.contains(old_row) && written.contains(new_row), "`{op}`: the rows that were there and the new one:\n{written}");
    }
}

/// **The same, for a reader that is not an operation**: an old file is the first version, with its rows.
#[test]
fn a_file_from_before_the_version_reads_as_the_first() {
    let forest: Forest = ron::from_str("(trees: [])").unwrap();
    let people: People = ron::from_str("(people: [(name: \"Mara\", at: (1.0, 2.0))])").unwrap();
    let scripts: Scripts = ron::from_str("(triggers: [(id: \"first\", on: \"Activity(\\\"work\\\")\", then: \"let x = 1;\")])").unwrap();
    assert_eq!((forest.version, people.version, scripts.version), (1, 1, 1));
    assert_eq!((people.people.len(), scripts.triggers.len()), (1, 1), "and the rows are there");
    // Written again, the version it was missing comes with it, and the text reads back as the same value.
    let again: People = ron::from_str(&ron::to_string(&people).unwrap()).unwrap();
    assert_eq!(again, people);
}

/// **A new file is written at the version this build reads**, for the type that has no operation here.
#[test]
fn a_new_file_is_written_with_its_version() {
    for (kind, text) in [
        ("forest", ron::to_string(&Forest::default()).unwrap()),
        ("people", ron::to_string(&People::default()).unwrap()),
        ("scripts", ron::to_string(&Scripts::default()).unwrap()),
    ] {
        assert!(text.starts_with("(version:1") || text.starts_with("(version: 1"), "`{kind}` is written with its version first: {text}");
    }
}
