//! **No RON comment is dropped quietly, in any of its forms** (GD-03, #365; review M4).
//!
//! `//` above a field was carried; a trailing same-line comment (`version: 1, // the first`) was never recorded, and a `/* .. */`
//! block comment was skipped without a record (a one-line `/* x */` even swallowed the lines after it). Each is either placed
//! above its field in the YAML or listed in the report.

use std::path::{Path, PathBuf};

use bevy_grimoire_migrate as migrate;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-migrate-comments-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(migrate::GRIMOIRE_DIR)).unwrap_or_else(|e| panic!("{e}"));
    dir
}

fn migrated(tag: &str, ron: &str) -> (migrate::Report, String, PathBuf) {
    let root = scratch(tag);
    let file = root.join(migrate::GRIMOIRE_DIR).join("standards.ron");
    std::fs::write(&file, ron).unwrap_or_else(|e| panic!("{e}"));
    let report = migrate::convert_project(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let written = std::fs::read_to_string(Path::new(&root).join(migrate::GRIMOIRE_DIR).join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));
    (report, written, root)
}

#[test]
fn a_trailing_comment_is_carried_above_its_field() {
    let (report, written, root) = migrated("trailing", "(\n    version: 1, // the first version\n    rules: [],\n)\n");
    assert!(written.contains("# the first version\nversion: 1"), "above the field it was written beside:\n{written}");
    assert!(report.unplaced.is_empty(), "{:?}", report.lines());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_block_comment_is_carried_above_its_field() {
    let (report, written, root) = migrated("block", "(\n    version: 1,\n    /* why the rules are empty:\n       none apply yet */\n    rules: [],\n)\n");
    assert!(written.contains("# why the rules are empty:\n# none apply yet\nrules:"), "both lines, above the field:\n{written}");
    assert!(report.unplaced.is_empty(), "{:?}", report.lines());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_one_line_block_comment_does_not_swallow_the_lines_after_it() {
    let (report, written, root) = migrated("oneline", "(\n    /* a */\n    version: 1,\n    // b\n    rules: [],\n)\n");
    assert!(written.contains("# a\nversion: 1"), "{written}");
    assert!(written.contains("# b\nrules:"), "the comment after it is still found, because `version` was read as a field:\n{written}");
    assert!(report.unplaced.is_empty(), "{:?}", report.lines());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_trailing_comment_that_has_no_field_to_stand_above_is_listed() {
    let (report, _, root) = migrated("closer", "(\n    version: 1,\n    rules: [],\n) // end of the file\n");
    assert!(report.unplaced.iter().any(|u| u.comment.contains("end of the file")), "listed, never dropped: {:?}", report.lines());
    let _ = std::fs::remove_dir_all(root);
}
