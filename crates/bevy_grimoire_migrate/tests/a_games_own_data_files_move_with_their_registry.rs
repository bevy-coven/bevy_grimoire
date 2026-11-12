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
