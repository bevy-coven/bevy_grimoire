//! **`play.ron` names the build a change that needs Rust runs** (#371, GD-09).
//!
//! A project's `play.ron` names how the game starts. A change that needs a new component type or a new
//! system is not data, so the same file names the **build** that must succeed before the game can show
//! it — and `game_build` refuses a project that names none, in words that say what to write.
//!
//! # What is being proved
//!
//! 1. **A version 1 file still reads**, and has no build command: the projects that exist today were
//!    written before `build` was a field.
//! 2. **A build command takes `{root}`**, because the command runs in a directory the file chose and
//!    the compiler needs the project's own path.
//! 3. **The build is a command a person allows**, by the same rule as the game: the spec a trust grant
//!    is compared against is the template as the file writes it, with `{root}` unfilled.
//! 4. **A project that names no build says so, naming the file**, rather than reporting a build it did
//!    not run.

use std::path::Path;

use bevy_grimoire_core::play::{PlayConfig, PLAY_VERSION};

/// **1. A version 1 file still reads**, which is every project written before this field existed.
#[test]
fn a_version_one_file_still_reads_and_has_no_build() {
    let c = PlayConfig::from_ron(r#"(version: 1, command: ["g", "{map}"])"#).expect("a version 1 file reads");
    assert!(c.build.is_empty(), "a version 1 file has no build command: {:?}", c.build);
    assert!(c.build_spec().is_none(), "a project with no build names nothing a person could allow");
}

/// **4. A project that names no build says so, naming the file.**
#[test]
fn a_project_with_no_build_command_says_what_to_write() {
    let c = PlayConfig::from_ron(r#"(version: 2, command: ["g"])"#).expect("a game with no build reads");
    let refused = c.build_argv(Path::new("/p")).expect_err("a project with no build has nothing to run");
    assert!(refused.contains("build"), "the refusal does not name the field: {refused}");
    assert!(refused.contains("play.ron"), "the refusal does not name the file: {refused}");
}

/// **2 and 3. The command takes `{root}`, and the trust gate reads it as the file wrote it.**
#[test]
fn a_build_command_takes_the_root_and_is_what_a_person_allows() {
    let c = PlayConfig::from_ron(
        r#"(version: 2, command: ["g"], build: ["cargo", "build", "--manifest-path={root}/Cargo.toml"])"#,
    )
    .expect("a version 2 file reads");
    assert_eq!(
        c.build_argv(Path::new("/p")).expect("this project names a build"),
        ["cargo", "build", "--manifest-path=/p/Cargo.toml"]
    );
    let spec = c.build_spec().expect("a build command is allowed like the game's");
    assert_eq!(spec.program, "cargo");
    assert_eq!(
        spec.args,
        ["build", "--manifest-path={root}/Cargo.toml"],
        "the grant is compared against the template a person read, not one run's values"
    );
}

/// **The version this build writes is the one a new file carries**, and it is above the version it
/// still reads, so a file of the new version is never mistaken for an old one that happens to have a
/// `build` field.
#[test]
fn the_build_field_arrived_with_a_version_above_the_oldest_one_read() {
    assert_eq!(PLAY_VERSION, 2);
    // **A const block, so this is a compile-time fact** rather than an assertion a runtime could skip.
    const { assert!(bevy_grimoire_core::play::PLAY_VERSION_READS_FROM < PLAY_VERSION) };
    assert!(
        PlayConfig::from_ron(r#"(version: 3, command: ["g"])"#).is_err(),
        "a file from a later release is refused rather than read as this one"
    );
}
