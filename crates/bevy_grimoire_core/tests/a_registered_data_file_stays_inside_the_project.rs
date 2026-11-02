//! **A registered data file is data, never a command.** `data_set` writes the files `data.ron` registers,
//! and nothing stopped an entry from naming `agent.ron` or `play.ron`, the files that name a program the
//! editor starts. Registered, `data_set` could rewrite the command a person had not read (the review of
//! 2026-11-02). A game's data may live outside the grimoire folder, as `../data/mob.ron` does, so the rule
//! is about what the file is, not where it is.

use bevy_grimoire_core::data_files::DataFiles;

fn registry(path: &str) -> String {
    format!("(version: 1, files: [(id: \"x\", path: \"{path}\")])")
}

#[test]
fn the_files_that_name_a_program_cannot_be_registered_wherever_they_are() {
    for path in ["agent.ron", "../agent.ron", "../../play.ron", "a/b/play.ron"] {
        let err = DataFiles::parse(&registry(path)).expect_err(path);
        assert!(err.contains("`x`") && err.contains(path) && err.contains("program"), "{path}: the refusal names the entry, the path and why: {err}");
    }
}

#[test]
fn a_games_own_data_may_live_outside_the_folder() {
    for path in ["mob.ron", "rooms/a.ron", "../data/mob.ron", "../../game/tuning.ron", "my_play.ron", "agent.ron.bak"] {
        DataFiles::parse(&registry(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
}
