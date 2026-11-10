//! **A project file is YAML, and a project still holding the old spelling is refused by name** (GD-03,
//! #365).
//!
//! The cutover is one kind at a time, so this file grows a test per kind as that kind moves: the kind's
//! path ends `.yaml`, its reader reads YAML, and the file its RON spelling used to be is not read at all.
//! A test here is the kind's own proof that nothing was left behind — a reader still parsing RON passes
//! every other test in the workspace, because every fixture is whatever spelling the reader wants.
//!
//! The refusal half is one test, over the whole project: an open is handed a folder holding a `.ron`
//! project file and must answer the file's name and the command that converts it, never open it.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::migrate;
use bevy_grimoire_core::play::PlayConfig;

/// A throwaway project folder of this test's own: its name and the process, so two tests in one binary
/// never share a path (the editor's tests run several files in one process).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "grimoire-yaml-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

#[test]
fn a_play_file_is_yaml() {
    let root = scratch("play");

    assert_eq!(
        bevy_grimoire_core::play::path_for_project(&root),
        root.join("play.yaml"),
        "the kind's path ends `.yaml`"
    );

    // The writer's own text, so the file read is the file the editor would write.
    let written = bevy_grimoire_core::yaml::to_string(&PlayConfig {
        version: bevy_grimoire_core::play::PLAY_VERSION,
        command: vec!["cargo".to_owned(), "run".to_owned()],
        cwd: None,
        port: bevy_grimoire_core::play::DEFAULT_PORT,
        build: Vec::new(),
    })
    .unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(written.starts_with("version: "), "the writer wrote a YAML mapping:\n{written}");

    std::fs::write(root.join("play.yaml"), &written).unwrap_or_else(|e| panic!("{e}"));
    let read = PlayConfig::load(&root).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read.command, ["cargo", "run"]);

    // **A `play.ron` is not read**, and the refusal names the file the project must now have.
    std::fs::remove_file(root.join("play.yaml")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("play.ron"), "(version: 1, command: [\"game\"])").unwrap_or_else(|e| panic!("{e}"));
    let refused = PlayConfig::load(&root).expect_err("a `play.ron` is not a `play.yaml`");
    assert!(refused.contains("play.yaml"), "the refusal names the file it wanted: {refused}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_project_holding_ron_files_is_refused_naming_the_command() {
    let root = scratch("leftover");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)\n")
        .unwrap_or_else(|e| panic!("{e}"));

    let problem = migrate::leftover(&root)
        .unwrap_or_else(|p| panic!("a folder with no project files: {}", p.line()))
        .expect("a `.ron` project file is refused");

    assert!(problem.line().contains("kits.ron"), "the refusal names the file: {}", problem.line());
    assert!(
        problem.line().contains(bevy_grimoire_core::migrate::COMMAND),
        "the refusal names the command that fixes it: {}",
        problem.line()
    );

    std::fs::write(grimoire.join("kits.yaml"), "version: 3\nbind: []\nbash: []\n")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::remove_file(grimoire.join("kits.ron")).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        migrate::leftover(&root).unwrap_or_else(|p| panic!("{}", p.line())).is_none(),
        "a YAML project is opened"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The root a fixture project lives under, for a test that reads one.
#[allow(dead_code)]
fn root_of(dir: &Path) -> PathBuf {
    dir.to_path_buf()
}
