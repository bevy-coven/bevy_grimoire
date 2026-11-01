//! **A `checks.ron` of another version is refused, and a file from before the version is the first**
//! (#202, H5 step 4).
//!
//! The decision layer reads the project's `checks.ron` on every run and the editor decides it on every
//! edit. Before the file carried a version, a file a newer build wrote either read as this build's or was
//! refused as damaged without saying what was wrong. The refusal is in the type's own deserialiser, so
//! `Checks::from_ron`, `Checks::for_project` and the editor's `reload_check::read` all refuse the same way.

use bevy_grimoire_decide::Checks;

#[test]
fn a_file_of_another_version_is_refused_naming_the_version_this_build_reads() {
    for version in [2u32, 0, 7] {
        let said = match Checks::from_ron(&format!("(version: {version}, checks: [])")) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("a checks.ron at version {version} was taken as this build's"),
        };
        assert!(said.contains(&format!("version {version} is not the 1 this build reads")), "version {version}: {said}");
    }
}

/// A project's file from before the version has none, and is the first.
#[test]
fn a_file_with_no_version_is_the_first_and_is_written_back_with_one() {
    let old = Checks::from_ron("(checks: [])").expect("a checks.ron from before the version reads");
    let written = old.to_ron().unwrap();
    assert!(written.contains("version: 1"), "a rewritten file carries its version:\n{written}");
    assert_eq!(Checks::from_ron(&written).unwrap(), old, "and reads back as the same checks");
}

/// The editor's own checks are the first version too, and a project file's checks merge over them.
#[test]
fn the_editors_own_checks_carry_the_version() {
    let builtin = Checks::builtin();
    assert!(builtin.to_ron().unwrap().contains("version: 1"));
    assert!(Checks::default().to_ron().unwrap().contains("version: 1"), "a new, empty file is written at the version this build reads");
}
