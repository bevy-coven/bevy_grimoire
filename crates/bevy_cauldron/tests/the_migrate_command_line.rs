//! **`bevy_cauldron migrate <project folder>` as a person runs it** (GD-03, #365; review M5 d): the binary, its output, its exit code.

use std::path::PathBuf;
use std::process::Command;

#[allow(unused_imports)]
use crate::fixtures;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("migrate_cli_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets/grimoire")).unwrap_or_else(|e| panic!("{e}"));
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron")).args(args).output().unwrap_or_else(|e| panic!("{e}"));
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn it_converts_reports_each_file_and_archives_the_originals() {
    let root = scratch("ok");
    std::fs::write(root.join("assets/grimoire/kits.ron"), "// the binding\n(version: 3, bind: [], bash: [], authoring: None)").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/grimoire/editor.ron"), "(version: 1, text_scale: Some(1.5))").unwrap_or_else(|e| panic!("{e}"));
    let (ok, out, err) = run(&["migrate", &root.display().to_string()]);
    assert!(ok, "exit 0: {err}");
    assert!(out.contains("kits.ron -> assets/grimoire/kits.yaml") && out.contains("editor.ron -> assets/grimoire/editor.yaml"), "one line per file: {out}");
    assert!(out.contains("migrated-ron"), "and where the originals went: {out}");
    assert!(root.join("assets/grimoire/kits.yaml").exists() && root.join("assets/grimoire/editor.yaml").exists());
    assert!(root.join(".dark_factory/migrated-ron/assets/grimoire/kits.ron").exists());
    let (ok, out, _) = run(&["migrate", &root.display().to_string()]);
    assert!(ok && out.contains("nothing to migrate"), "a second run has nothing to do: {out}");
}

#[test]
fn a_file_that_does_not_read_stops_it_naming_the_file_with_a_failing_exit_and_nothing_written() {
    let root = scratch("broken");
    std::fs::write(root.join("assets/grimoire/kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/grimoire/editor.ron"), "(version: 1, text_scale: ").unwrap_or_else(|e| panic!("{e}"));
    let (ok, _, err) = run(&["migrate", &root.display().to_string()]);
    assert!(!ok, "a failing exit");
    assert!(err.contains("editor.ron"), "the file is named: {err}");
    assert!(!root.join("assets/grimoire/kits.yaml").exists(), "nothing is written");
}

#[test]
fn no_folder_is_a_usage_message_and_a_failing_exit() {
    let (ok, _, err) = run(&["migrate"]);
    assert!(!ok && err.contains("usage: bevy_cauldron migrate <project folder>"), "{err}");
}

/// **`bevy_cauldron migrate .` converts a registered data file** (review MUST-FIX 2).
///
/// The containment check compares a registered file's path against the project root *as typed*, and this command does not
/// canonicalise its argument (`args::root_of` does, which is why the tests above pass a plain path). So the shortest, most
/// natural way to run it — from inside the project — met "is outside the project" for every file a `data.yaml` names, and
/// refused a project it should have converted.
#[test]
fn a_dot_root_still_converts_the_files_a_registry_names() {
    let root = scratch("dot");
    std::fs::write(root.join("assets/grimoire/kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)").unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join("assets/grimoire/site")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/grimoire/site/data.ron"), "(version: 1, files: [(id: \"tuning\", path: \"mob.ron\", bounds: {})])").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/grimoire/site/mob.ron"), "(mob: (speed: 1.5))").unwrap_or_else(|e| panic!("{e}"));

    let out = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron")).args(["migrate", "."]).current_dir(&root).output().unwrap_or_else(|e| panic!("{e}"));
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "a `.` root is the project: {said}");
    assert!(!said.contains("outside the project"), "and its registered file is inside it: {said}");
    assert!(root.join("assets/grimoire/site/mob.yaml").exists(), "the tuning file is YAML: {said}");
    assert!(root.join("assets/grimoire/site/data.yaml").exists(), "and its registry names it: {said}");
}
