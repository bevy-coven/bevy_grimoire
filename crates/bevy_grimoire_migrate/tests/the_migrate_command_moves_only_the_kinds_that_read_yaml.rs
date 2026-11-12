//! **`cauldron migrate` converts every kind a project names, and a RON file of any kind is refused at open** (GD-03, #365).
//!
//! This file proved the opposite while the cutover moved one kind at a time: that the command left `editor.ron`, `checks.ron`,
//! a forest and a family crate's settings exactly where they were. Those kinds read YAML now (the editor and the oracle read
//! nothing else), so a project holding one was neither refused at open nor converted by the command: it opened as if empty.
//! Owner exception (2026-11-12): the assertions follow the issue's Decision, "after this slice nothing reads RON, and a project
//! that still has RON files is refused at open".
//!
//! **A kind whose type lives above this crate is converted by value**: the RON is read into a tree (a variant keeps its name, an
//! `Option` is its value or null), written as YAML, and the YAML is read back and compared with the tree. The comparison is
//! the check; no type is guessed at.

use std::path::{Path, PathBuf};

use bevy_grimoire_migrate as migrate;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-migrate-all-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

fn text_at(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap_or(root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
}

/// **The kinds the reload table names whose type is not this crate's**, each as a RON file an old project held.
const EDITOR_KINDS: [(&str, &str); 6] = [
    ("assets/grimoire/editor.ron", "(version: 1, text_scale: Some(1.25), keymap: {\"file.save\": (key: S)})"),
    ("assets/grimoire/checks.ron", "(version: 1, checks: [])"),
    ("assets/grimoire/site/forest/yard.ron", "(version: 1, species: [(name: \"oak\", kind: Pine(height_m: 4.0))])"),
    ("assets/grimoire/site/people/yard.ron", "(version: 1, people: [(name: \"Mara\", at: (2.0, 2.0))])"),
    ("family/bevy_gooey.ron", "(version: 1, gap: 5.0, ink: Srgba(red: 1.0, green: 0.5, blue: 0.0, alpha: 1.0))"),
    ("assets/grimoire/site/trees/oak.tree.ron", "(version: 1, trunk_m: 3.0, seed: None)"),
];

#[test]
fn a_project_holding_an_editor_kind_in_ron_is_refused_at_open_naming_the_file_and_the_command() {
    for (rel, text) in EDITOR_KINDS {
        let root = scratch("refused");
        put(&root, rel, text);
        let problem = migrate::leftover(&root, "bevy_cauldron migrate /p")
            .unwrap_or_else(|p| panic!("{}", p.line()))
            .unwrap_or_else(|| panic!("`{rel}` is RON and the project is not refused"));
        let said = problem.line();
        assert!(said.contains(rel), "the file is named: {said}");
        assert!(said.contains("bevy_cauldron migrate /p"), "and the command: {said}");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn the_command_converts_every_editor_kind_by_value_and_archives_the_originals() {
    let root = scratch("converted");
    for (rel, text) in EDITOR_KINDS {
        put(&root, rel, text);
    }
    let report = migrate::convert_project(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), EDITOR_KINDS.len(), "{:?}", report.converted);
    for (rel, _) in EDITOR_KINDS {
        let yaml = rel.replace(".ron", ".yaml");
        assert!(text_at(&root, &yaml).is_some(), "`{yaml}` is written");
        assert_eq!(text_at(&root, rel), None, "`{rel}` is archived, not left beside its YAML");
        assert!(root.join(migrate::ARCHIVE_DIR).join(rel).exists(), "and it is in the archive");
    }
    let editor = text_at(&root, "assets/grimoire/editor.yaml").unwrap_or_default();
    assert!(editor.contains("text_scale: 1.25"), "an `Option` is its value, not `Some(..)`:\n{editor}");
    let forest = text_at(&root, "assets/grimoire/site/forest/yard.yaml").unwrap_or_default();
    assert!(forest.contains("!Pine"), "a variant keeps its name as a tag:\n{forest}");
    assert!(migrate::leftover(&root, migrate::COMMAND).unwrap_or_else(|p| panic!("{}", p.line())).is_none(), "nothing is left to refuse");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_ron_file_that_does_not_read_stops_the_whole_command_and_writes_nothing() {
    let root = scratch("broken");
    put(&root, "assets/grimoire/kits.ron", "(version: 3, bind: [], bash: [], authoring: None)");
    put(&root, "assets/grimoire/editor.ron", "(version: 1, text_scale: ");
    let refused = migrate::convert_project(&root).expect_err("a file that is not RON stops the command");
    assert!(refused.line().contains("editor.ron"), "{}", refused.line());
    assert!(text_at(&root, "assets/grimoire/kits.ron").is_some(), "the project is as it was");
    assert!(text_at(&root, "assets/grimoire/kits.yaml").is_none(), "and nothing was written");
    let _ = std::fs::remove_dir_all(&root);
}
