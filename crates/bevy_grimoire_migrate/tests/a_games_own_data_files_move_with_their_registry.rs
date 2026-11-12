//! **A game's own data files are converted with the registry that lists them** (GD-03, #365; review M2).
//!
//! `<kit>/data.ron` registers tuning files (`path: "mob.ron"`). The registry converts as its type and its `.ron` paths become
//! `.yaml`; each registered file converts by value (the game owns its shape), read back and compared; the originals are archived.
//! After it `data_set`'s reader (`leaves`) finds the leaves in the YAML, and a registered file that does not read stops the whole
//! migration with nothing written.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::data_files::leaves;
use bevy_grimoire_migrate as migrate;

const REGISTRY: &str = "(version: 1, files: [(id: \"tuning\", path: \"mob.ron\", bounds: {\"mob.speed\": (min: Some(0.0), max: Some(9.0))})])";
const TUNING: &str = "// tuning\n(mob: (speed: 1.50, name: \"orc\", hp: 7), fast: false)\n";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-migrate-data-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets/grimoire/site")).unwrap_or_else(|e| panic!("{e}"));
    dir
}

fn put(root: &Path, rel: &str, text: &str) {
    std::fs::write(root.join(rel), text).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn the_registry_and_its_files_move_together_and_the_registry_names_the_yaml() {
    let root = scratch("ok");
    put(&root, "assets/grimoire/site/data.ron", REGISTRY);
    put(&root, "assets/grimoire/site/mob.ron", TUNING);
    let report = migrate::convert_project(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 2, "the registry and the one file it names: {:?}", report.lines());

    let registry = std::fs::read_to_string(root.join("assets/grimoire/site/data.yaml")).unwrap_or_default();
    assert!(registry.contains("path: mob.yaml"), "the registry names the YAML file now:\n{registry}");
    let tuning = std::fs::read_to_string(root.join("assets/grimoire/site/mob.yaml")).unwrap_or_default();
    assert!(tuning.contains("# tuning") && tuning.contains("speed: 1.5"), "the file is its YAML, comment kept:\n{tuning}");
    assert!(leaves(&tuning).unwrap_or_default().iter().any(|(path, _)| path == "mob.speed"), "and `data_set`'s reader finds its leaves");
    for rel in ["assets/grimoire/site/data.ron", "assets/grimoire/site/mob.ron"] {
        assert!(!root.join(rel).exists() && root.join(migrate::ARCHIVE_DIR).join(rel).exists(), "{rel} is archived");
    }
    assert!(migrate::leftover(&root, migrate::COMMAND).unwrap_or_else(|p| panic!("{}", p.line())).is_none());
}

#[test]
fn a_registered_file_that_does_not_read_stops_everything_and_writes_nothing() {
    let root = scratch("broken");
    put(&root, "assets/grimoire/site/data.ron", REGISTRY);
    put(&root, "assets/grimoire/site/mob.ron", "(mob: (speed: ");
    let refused = migrate::convert_project(&root).expect_err("a data file that is not RON stops the migration");
    assert!(refused.line().contains("site/mob.ron"), "{}", refused.line());
    assert!(root.join("assets/grimoire/site/data.ron").exists() && !root.join("assets/grimoire/site/data.yaml").exists(), "nothing was written");
}

#[test]
fn a_registered_file_outside_the_project_is_refused_not_moved() {
    let root = scratch("outside");
    put(&root, "assets/grimoire/site/data.ron", "(version: 1, files: [(id: \"x\", path: \"../../../../elsewhere.ron\")])");
    let refused = migrate::convert_project(&root).expect_err("a path out of the project is refused");
    assert!(refused.line().contains("outside the project"), "{}", refused.line());
}

/// **A registry already converted that still names a `.ron` file is refused at open** (GD-03, #365; review M2).
///
/// The registry's own conversion rewrites the paths it holds, so the only way to arrive here is a project
/// whose `data.yaml` was written by hand, or one a person half-migrated: the registry reads, nothing refuses
/// it, and the first `data_set` on that file fails in the editor instead of the open naming the command.
#[test]
fn a_registry_that_names_a_ron_file_still_is_refused_at_open() {
    let root = scratch("registry_names_ron");
    put(&root, "assets/grimoire/site/data.yaml", "version: 1\nfiles:\n- id: tuning\n  path: mob.ron\n  bounds:\n    mob.speed:\n      min: 0.0\n      max: 9.0\n");
    put(&root, "assets/grimoire/site/mob.ron", TUNING);
    let problem = migrate::leftover(&root, "bevy_cauldron migrate /p")
        .unwrap_or_else(|p| panic!("{}", p.line()))
        .unwrap_or_else(|| panic!("`mob.ron` is registered and still RON, and the project is not refused"));
    let said = problem.line();
    assert!(said.contains("site/mob.ron"), "the registered file is named: {said}");
    assert!(said.contains("bevy_cauldron migrate /p"), "and the command: {said}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **And the command converts it**: a registry in YAML with a `.ron` path is walked like the RON one.
#[test]
fn the_command_converts_a_file_a_yaml_registry_still_names_as_ron() {
    let root = scratch("yaml_registry_converts");
    put(&root, "assets/grimoire/site/data.yaml", "version: 1\nfiles:\n- id: tuning\n  path: mob.ron\n  bounds:\n    mob.speed:\n      min: 0.0\n      max: 9.0\n");
    put(&root, "assets/grimoire/site/mob.ron", TUNING);
    let report = migrate::convert_project(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 2, "the registry (for its path) and the file it names: {:?}", report.lines());
    let registry = std::fs::read_to_string(root.join("assets/grimoire/site/data.yaml")).unwrap_or_default();
    assert!(registry.contains("path: mob.yaml"), "the registry names the YAML now:\n{registry}");
    assert!(root.join("assets/grimoire/site/mob.yaml").exists());
    assert!(root.join(migrate::ARCHIVE_DIR).join("assets/grimoire/site/mob.ron").exists(), "the original is archived");
    assert!(migrate::leftover(&root, migrate::COMMAND).unwrap_or_else(|p| panic!("{}", p.line())).is_none());
    let _ = std::fs::remove_dir_all(&root);
}
