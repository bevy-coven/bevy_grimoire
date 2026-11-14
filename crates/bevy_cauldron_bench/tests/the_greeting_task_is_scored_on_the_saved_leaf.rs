//! **The gatekeeper's greeting is scored on the leaf the agent saved, never on what it said about it** (SG-10, #409).
//!
//! `games/sample/bench/tasks/write-the-gatekeepers-greeting` scores six lines of the sample game's style on `gatekeeper.greeting.text`. This grades the
//! real task, with no editor and no model, on a copy of the sample game's project laid out the way a trial lays it: the starting leaf, a leaf an agent
//! wrote well, and a leaf it wrote badly, each with a hand-back that says the opposite of the leaf. A line that moved with the hand-back would be the
//! asymmetry between the two arms the measurement cannot have: an agent that was told the forbidden words can write "avoided `welcome`" in its summary.

use std::path::{Path, PathBuf};

use bevy_cauldron_bench::{fixture, Grading, Handback, Snapshot, Task};

/// The map the task opens, and the leaf it scores.
const MAP: &str = "courtyard";
const LEAF: &str = "gatekeeper.greeting.text";
const FILE: &str = "assets/grimoire/furniture/data/gatekeeper.yaml";

/// **Positions in the task's `checks`**: the guard, then the three Do lines, then the three Don't lines.
fn do_lines() -> std::ops::RangeInclusive<usize> {
    1..=3
}
fn dont_lines() -> std::ops::RangeInclusive<usize> {
    4..=6
}

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/sample")
}

fn task() -> Task {
    let root = sample();
    Task::load(&root.join("bench/tasks/write-the-gatekeepers-greeting"), &root, &["data_set"])
        .unwrap_or_else(|p| panic!("the greeting task did not load: {}", p.line()))
}

/// **A trial's project**, laid out as the runner lays it, with this test's name and the process id in its path.
fn trial(test: &str, task: &Task) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-greeting-leaf-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    fixture::lay_out_task(&sample(), &task.dir, &dir).unwrap_or_else(|e| panic!("the fixture was not laid out: {e}"));
    dir
}

/// **Write the greeting into the saved leaf**, with the core's own leaf edit (what `data_set` does).
fn write_greeting(project: &Path, text: &str) {
    let file = project.join(FILE);
    let old = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let new = bevy_grimoire_core::data_files::set_leaf(&old, LEAF, &serde_json::json!(text), None).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&file, new).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
}

/// **What each line's check says on this project and hand-back**, in the order of the task's checks.
fn lines(task: &Task, project: &Path, handback: Option<&str>) -> Vec<bool> {
    let before = Snapshot::read(project, MAP, Vec::new(), Default::default());
    let _ = &before;
    let backs: Vec<Handback> = handback.map(|summary| Handback { summary: summary.to_owned(), ..Handback::default() }).into_iter().collect();
    let left = Snapshot::read(project, MAP, backs, Default::default());
    Grading::of(task, &before, &left).verdicts.iter().map(|v| v.passed).collect()
}

/// **Nothing written**: the Do lines fail, and the Don't lines hold because the starting leaf says nothing.
#[test]
fn a_greeting_never_written_satisfies_only_what_it_never_says() {
    let task = task();
    let project = trial("nothing", &task);
    let said = lines(&task, &project, None);
    assert_eq!(said.len(), 7, "the guard and six lines: {said:?}");
    assert!(!said[0], "the guard fails: nothing went through the editor");
    assert!(do_lines().all(|i| !said[i]), "no Do line holds on a greeting that says nothing: {said:?}");
    assert!(dont_lines().all(|i| said[i]), "the three Don't lines hold on a greeting that says nothing: {said:?}");
    let _ = std::fs::remove_dir_all(&project);
}

/// **A greeting written well satisfies all six lines, whatever the hand-back says**, even one that names every forbidden word.
#[test]
fn a_good_greeting_satisfies_every_line_whatever_the_handback_says() {
    let task = task();
    let project = trial("good", &task);
    write_greeting(&project, "Evening, friend. The toll is two coins, and the gate shuts at dusk.");
    let said = lines(&task, &project, Some("I avoided the word welcome and the crown, and used no exclamation mark!"));
    assert!(do_lines().chain(dont_lines()).all(|i| said[i]), "the saved leaf is right, so every line holds: {said:?}");
    let _ = std::fs::remove_dir_all(&project);
}

/// **A greeting written badly fails all six lines, whatever the hand-back says**, even one that states every Do word and no Don't word.
#[test]
fn a_bad_greeting_fails_every_line_whatever_the_handback_says() {
    let task = task();
    let project = trial("bad", &task);
    write_greeting(&project, "Welcome, traveller! The crown lies beyond the gate!");
    let said = lines(&task, &project, Some("The gatekeeper greets with a short line about the toll, calls the visitor friend, and says the gate shuts at dusk."));
    assert!(do_lines().chain(dont_lines()).all(|i| !said[i]), "the saved leaf is wrong, so no line holds: {said:?}");
    let _ = std::fs::remove_dir_all(&project);
}
