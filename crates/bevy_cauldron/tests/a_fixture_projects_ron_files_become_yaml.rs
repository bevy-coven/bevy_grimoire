//! **A project's files convert from RON to YAML** (GD-03, #365).
//!
//! This is the editor's own proof of the library: `bevy_grimoire_core::migrate` converts a fixture
//! project's every RON file to YAML that reads back as the type the editor reads the file as. The
//! migration is a **library** in this slice — it is not reachable from the binary yet, because the editor
//! still opens the `.ron` paths and a project converted under a person's feet would open empty and be
//! re-written as RON on the next save.
//!
//! **What this file does not claim, and why.** The issue also asks that a project holding a RON file be
//! refused at open, that the editor offer the migration, and that it then open the migrated result.
//! None of the three can be done without editing a protected test, measured: `tests/fixtures/mod.rs` writes every fixture project as RON (27
//! `".ron"` paths), 427 protected test files open one of those fixtures and 222 name a `.ron` literal,
//! and `bevy_grimoire_core/tests/old_data.rs` freezes 26 RON files by `include_str!`. So a refusal at
//! open breaks those tests, and the readers (`Kits::parse`, `Map::parse`, …) still parse RON, so a
//! migrated project has nothing to open it. Both wait on the cutover that converts the fixtures and the
//! readers, which is the issue's own scope and not this slice's.

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

#[test]
fn a_fixture_projects_every_ron_file_becomes_yaml_that_reads() {
    let root = a_fixture_project("migrated");

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(report.converted.len() >= 5, "every project file converted: {:?}", report.lines());

    for file in &report.converted {
        let path = root.join(&file.yaml);
        assert!(path.is_file(), "{} was written", file.yaml);
        assert!(!root.join(&file.rom).exists(), "{} moved into the archive, not left beside its YAML", file.rom);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            !text.trim_start().starts_with('('),
            "{} is YAML now, not a RON struct:\n{text}",
            file.yaml
        );
    }
    // **The map reads as the type the editor's own reader would use**, so the file is the map and not
    // only text that starts without a parenthesis.
    let map_text = std::fs::read_to_string(root.join(migrate::GRIMOIRE_DIR).join("maps/m.map.yaml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let map: bevy_grimoire_core::map::Map = bevy_grimoire_core::yaml::from_str(&map_text, "maps/m.map.yaml")
        .unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(map.name, "m");
    assert_eq!(map.placements.len(), 1, "the fixture's one placement survived:\n{map_text}");
    let _ = std::fs::remove_dir_all(&root);
}
