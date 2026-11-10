//! **`cauldron migrate` converts only the kinds this build reads as YAML** (GD-03, #365).
//!
//! A kind whose reader still parses RON must be **left exactly where it is** by the command, because
//! converting it would write a file nothing reads and archive the one everything does — a project that opens
//! empty, with its content sitting in `.dark_factory/migrated-ron/`. The library-wide [`migrate::migrate`]
//! converts every kind it can and refuses the rest; the command a person types is
//! [`migrate::migrate_moved`], and both share one body.
//!
//! **Which kinds are left alone today are the ones above this crate.** Every kind `bevy_grimoire_core`
//! declares a type for is on [`migrate::READS_YAML`], so a `kits.ron` converts. What does not is
//! `editor.ron` (the editor's settings), `checks.ron` (the checks crate) and a family crate's own settings —
//! each is read by the crate that declares its type, and only that crate can say whether the YAML holds the
//! same value. Those must not block the work this command *can* do.
//!
//! **The losslessness half is tested apart**, in `a_project_migrates_from_ron_to_yaml.rs`, because a test of
//! "what converts" and a test of "what the conversion keeps" fail for different reasons.

use std::path::{Path, PathBuf};

use bevy_grimoire_migrate as migrate;

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
fn the_command_converts_a_kind_this_crate_reads_and_leaves_a_kind_another_crate_owns() {
    let root = scratch("mixed");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));

    // `kits` is this crate's and reads YAML; `editor` is the editor's and does not.
    let kits_ron = "(version: 3, bind: [], bash: [], authoring: None)";
    std::fs::write(grimoire.join("kits.ron"), kits_ron).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("editor.ron"), "(version: 1)").unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate_moved(&root, migrate::COMMAND)
        .unwrap_or_else(|p| panic!("the command converts what it knows: {}", p.line()));

    assert_eq!(
        report.converted.iter().map(|c| c.rom.as_str()).collect::<Vec<_>>(),
        ["assets/grimoire/kits.ron"],
        "only the kind this crate reads is converted"
    );
    let kits_yaml = text_at(&root, &format!("{}/kits.yaml", migrate::GRIMOIRE_DIR)).expect("`kits.yaml` is written");
    assert!(kits_yaml.contains("version:"), "the converted file is the value's YAML:\n{kits_yaml}");
    assert_eq!(
        text_at(&root, &format!("{}/kits.ron", migrate::GRIMOIRE_DIR)),
        None,
        "the RON it came from is archived, not left beside its YAML"
    );
    assert_eq!(
        text_at(&root, &format!("{}/editor.ron", migrate::GRIMOIRE_DIR)).as_deref(),
        Some("(version: 1)"),
        "a kind another crate owns is left exactly as it was"
    );
    assert_eq!(
        text_at(&root, &format!("{}/editor.yaml", migrate::GRIMOIRE_DIR)),
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
    // **The kinds whose types live above this crate.** A command that refused these would refuse every
    // project in the world until each crate migrated, which is the opposite of a migration a person can run
    // one step at a time.
    std::fs::write(grimoire.join("editor.ron"), "(version: 1)").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("checks.ron"), "(version: 1, checks: [])").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("agent.ron"), "(version: 1, command: Some(\"agent\"), args: [])")
        .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate_moved(&root, migrate::COMMAND)
        .unwrap_or_else(|p| panic!("a kind another crate owns does not block the command: {}", p.line()));

    assert_eq!(report.converted.len(), 1, "the one kind this command can convert: {:?}", report.converted);
    assert!(text_at(&root, "agent.yaml").is_some(), "`agent.yaml` is written");
    assert_eq!(text_at(&root, &format!("{}/editor.ron", migrate::GRIMOIRE_DIR)).as_deref(), Some("(version: 1)"));
    assert_eq!(
        text_at(&root, &format!("{}/checks.ron", migrate::GRIMOIRE_DIR)).as_deref(),
        Some("(version: 1, checks: [])")
    );

    let _ = std::fs::remove_dir_all(&root);
}
