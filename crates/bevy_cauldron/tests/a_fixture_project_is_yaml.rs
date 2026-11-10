//! **A fixture project is YAML, and the migration has nothing to do on it** (GD-03, #365).
//!
//! This file used to prove the library by converting a fixture's RON into YAML. That job is done: the
//! fixture builder writes every file through the one YAML writer from a value, so a fixture is YAML from the
//! first byte and `cauldron migrate` finds nothing to convert in it. **The property the old test guarded is
//! still guarded, at its new place**: a project the editor can open is a project whose every file reads as the
//! type the editor reads it as, and a migration run over it changes nothing and refuses nothing.
//!
//! The other half of what this file used to say — that the migration could not be reached from the binary,
//! because the readers still parsed RON — is gone with the cutover: every kind reads YAML, the command converts
//! the kinds that read YAML and leaves the rest alone, and a project holding a `.ron` file is refused at open
//! naming the command. Those live in `bevy_grimoire_core/tests/{the_migrate_command_moves_only_the_kinds_that_read_yaml,
//! every_project_root_kind_is_yaml}.rs` and in this crate's `project` tests.

use std::path::PathBuf;

use crate::fixtures::Fixture;
use bevy_grimoire_core::migrate;

/// **A fixture project of this test's own**, named for the test and this process.
fn a_fixture_project(tag: &str) -> PathBuf {
    Fixture::new(&format!("yamlcut_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (2.0, 2.0))
        .build("m")
}

/// Every file under `root` whose name ends `.yaml`, with `/`, relative to `root`.
fn yaml_files(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_fixture_project_is_yaml_and_holds_no_ron_project_file() {
    let root = a_fixture_project("yaml");

    // **The builder writes YAML.** A fixture written as RON would be a project this build refuses, so the
    // suite that opens one would be proving the editor against a shape nobody ships.
    let files = yaml_files(&root);
    assert!(files.len() >= 5, "every project file is written: {files:?}");
    assert!(
        files.iter().any(|f| f.ends_with("maps/m.map.yaml")),
        "the open map is there: {files:?}"
    );
    assert!(
        files.iter().any(|f| f.ends_with("vocab.yaml")) && files.iter().any(|f| f.ends_with("kits.yaml")),
        "the project root's own files are there: {files:?}"
    );

    // **A migration over it converts nothing**, because there is nothing left in the old spelling — and it
    // refuses nothing, which is the check that no file the builder writes is one the cutover missed.
    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(
        report.converted.is_empty(),
        "a YAML fixture has nothing to migrate, and this found {:?}",
        report.lines()
    );

    // **The map reads as the type the editor's own reader uses**, from the file on disk: the text starting
    // without a parenthesis proves nothing, and this is the value a person's project holds.
    let map_text = std::fs::read_to_string(root.join(migrate::GRIMOIRE_DIR).join("maps/m.map.yaml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let map: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::yaml::from_str(&map_text, "maps/m.map.yaml").unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(map.name, "m");
    assert_eq!(map.placements.len(), 1, "the fixture's one placement survived:\n{map_text}");

    // And the editor opens it: the whole project, through the door every test and every person goes through.
    bevy_cauldron::project::Project::open(&root, None).unwrap_or_else(|e| panic!("the fixture project opens: {e}"));

    let _ = std::fs::remove_dir_all(&root);
}
