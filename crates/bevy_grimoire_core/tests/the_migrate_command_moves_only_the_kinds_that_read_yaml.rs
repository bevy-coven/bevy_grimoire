//! **`cauldron migrate` converts only the kinds this build reads as YAML** (GD-03, #365).
//!
//! The cutover is one kind at a time. A kind whose reader still parses RON (its `reload::KINDS` row still
//! says `kits.ron`) must be **left exactly where it is** by the command, because converting it would write
//! a `kits.yaml` nothing reads and archive the `kits.ron` that everything does — a project that opens
//! empty, with its content sitting in `.dark_factory/migrated-ron/`. The library-wide [`migrate::migrate`]
//! converts every kind it can and is what a complete cutover runs; the command a person types is the
//! narrower [`migrate::migrate_moved`], and both share one body.
//!
//! **The two halves are tested apart on purpose.** `a_project_migrates_from_ron_to_yaml.rs` proves the
//! conversion is lossless for every kind this crate has a type for; this file proves the *command* does
//! not run ahead of the readers, which is the one way a migration can destroy a project.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::migrate;

/// A throwaway project folder of this test's own: its name and the process id, so a test binary that runs
/// two of these never shares a path.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-migrate-moved-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// The text of a project file at `rel` under `root`, or `None` when it is not there.
fn text_at(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

#[test]
fn the_command_leaves_a_kind_whose_reader_is_still_ron_and_converts_the_one_that_moved() {
    let root = scratch("mixed");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));

    // `play` reads YAML now; `kits` does not yet. The two are the whole question.
    let kits_ron = "(version: 1, bind: [], bash: [], authoring: None)";
    std::fs::write(grimoire.join("kits.yaml"), kits_ron).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        root.join("play.yaml"),
        "(version: 1, command: [\"game\", \"{map}\"], port: 15702, build: [])",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate_moved(&root, migrate::COMMAND)
        .unwrap_or_else(|p| panic!("the command converts what it knows: {}", p.line()));

    assert_eq!(
        report.converted.iter().map(|c| c.rom.as_str()).collect::<Vec<_>>(),
        ["play.yaml"],
        "only the kind whose reader moved is converted"
    );
    let play_yaml = text_at(&root, "play.yaml").expect("`play.yaml` is written");
    // The text is the checked text, so the version it holds is the one the RON held.
    assert!(play_yaml.contains("command:"), "the converted file is the value's YAML:\n{play_yaml}");
    assert_eq!(text_at(&root, "play.yaml"), None, "the RON it came from is archived, not left beside it");
    assert_eq!(
        text_at(&root, &format!("{}/kits.yaml", migrate::GRIMOIRE_DIR)).as_deref(),
        Some(kits_ron),
        "a kind this build still reads as RON is left exactly as it was"
    );
    assert_eq!(
        text_at(&root, &format!("{}/kits.yaml", migrate::GRIMOIRE_DIR)),
        None,
        "and nothing is written that no reader would read"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_command_is_not_blocked_by_a_kind_this_crate_cannot_convert() {
    let root = scratch("unowned");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // `editor_settings` lives in the editor and `checks` in the checks crate: this crate has no type for
    // either, so converting them is not its work and must not stop the work it can do.
    std::fs::write(grimoire.join("editor.yaml"), "(version: 1)").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("checks.ron"), "(version: 1, checks: [])").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("agent.yaml"), "(version: 1, command: Some(\"agent\"), args: [])").unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate_moved(&root, migrate::COMMAND)
        .unwrap_or_else(|p| panic!("a kind another crate owns does not block the command: {}", p.line()));

    assert_eq!(report.converted.len(), 1, "the one kind this command can convert: {:?}", report.converted);
    assert!(text_at(&root, "agent.yaml").is_some(), "`agent.yaml` is written");
    assert_eq!(text_at(&root, &format!("{}/editor.yaml", migrate::GRIMOIRE_DIR)).as_deref(), Some("(version: 1)"));
    assert_eq!(
        text_at(&root, &format!("{}/checks.ron", migrate::GRIMOIRE_DIR)).as_deref(),
        Some("(version: 1, checks: [])")
    );

    let _ = std::fs::remove_dir_all(&root);
}
