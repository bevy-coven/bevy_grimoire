//! **A change a running program cannot take is a verdict of its own** (#200, H3).
//!
//! A kind the registry classes as `Restart` is held by a program from when it started. An edit to one is
//! neither applied nor refused: the file is fine, and the program keeps what it has. The third outcome,
//! `needs_restart`, says that, so a person or an agent reading `grimoire_state` knows the edit is on disk
//! and is not in use, rather than reading `applied` or a silence.

use bevy_grimoire_core::live::{KindVerdict, Outcome, Verdict};

#[test]
fn a_change_that_needs_a_restart_is_not_ok_and_survives_the_file_it_travels_in() {
    let dir = std::env::temp_dir().join(format!("grimoire_restart_verdict_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let kinds = vec![
        KindVerdict::applied("kits", "kit/kits.ron"),
        KindVerdict::needs_restart("rigs", "rigs.ron", "the animation tab holds the rigs it read when it opened"),
    ];
    let v = Verdict::with_kinds(3, "one file needs a restart", kinds);
    // The edit is not in use, so the reload did not land: `ok` is the answer an agent branches on.
    assert!(!v.ok, "a file that is not in use is not an applied reload");

    v.write(&dir).unwrap();
    let read = Verdict::read(&dir).unwrap().expect("the verdict was written");
    let rigs = read.kinds.iter().find(|k| k.kind == "rigs").expect("the rigs verdict is read back");
    assert_eq!(rigs.outcome, Outcome::NeedsRestart);
    assert_eq!(rigs.outcome.word(), "needs_restart");
    assert_eq!(rigs.field, None, "a valid file has no field it is wrong at");
    assert!(rigs.sentence().contains("rigs.ron") && rigs.sentence().contains("holds the rigs"), "{}", rigs.sentence());
    assert_eq!(read, v);

    let _ = std::fs::remove_dir_all(&dir);
}
