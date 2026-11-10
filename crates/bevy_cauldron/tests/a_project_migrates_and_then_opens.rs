//! **`cauldron migrate` converts a project's files from RON to YAML** (GD-03, #365).
//!
//! Proved through the seam a person uses: the argv parser the command line and the launcher both
//! speak ([`bevy_cauldron::args`]) reports the subcommand and the root it names, and the migration it
//! runs converts a fixture project's every RON file to YAML that reads.
//!
//! **What this file does not claim, and why.** The issue also asks that a project holding a RON file be
//! refused at open, and that the editor then open the migrated result. Neither can be done without
//! editing a protected test, measured: `tests/fixtures/mod.rs` writes every fixture project as RON (27
//! `".ron"` paths), 427 protected test files open one of those fixtures and 222 name a `.ron` literal,
//! and `bevy_grimoire_core/tests/old_data.rs` freezes 26 RON files by `include_str!`. So a refusal at
//! open breaks those tests, and the readers (`Kits::parse`, `Map::parse`, …) still parse RON, so a
//! migrated project has nothing to open it. Both wait on the cutover that converts the fixtures and the
//! readers, which is the issue's own scope and not this slice's.

use std::path::PathBuf;

use crate::fixtures::Fixture;
use bevy_cauldron::args;
use bevy_grimoire_core::migrate;

/// **A fixture project of this test's own**, named for the test and this process.
fn a_fixture_project(tag: &str) -> PathBuf {
    Fixture::new(&format!("yamlcut_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (2.0, 2.0))
        .build("m")
}

#[test]
fn the_migrate_subcommand_is_read_off_the_arguments_and_names_its_root() {
    let argv = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    assert!(args::asks_to_migrate(&argv(&["migrate", "/a/project"])));
    assert!(args::asks_to_migrate(&argv(&["migrate"])));
    assert!(!args::asks_to_migrate(&argv(&["/a/project", "m"])), "a project opens, it does not migrate");
    assert!(!args::asks_to_migrate(&argv(&[])), "no argument: the launcher");

    let root = a_fixture_project("argv");
    // `args::migrate_root` canonicalises, as every other root the editor takes does.
    let named = args::migrate_root(&argv(&["migrate", &root.to_string_lossy()]))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(named, root.canonicalize().unwrap_or(root.clone()), "the root it names");
    let refusal = args::migrate_root(&argv(&["migrate"])).expect_err("no root is refused");
    assert!(refusal.contains("migrate"), "the refusal names the subcommand: {refusal}");
    let _ = std::fs::remove_dir_all(&root);
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
