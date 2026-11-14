//! **The refusal task grades what a refusal is, not the wording it was written in** (#432, D5).
//!
//! `refuse-a-piece-the-project-lacks` asked for a hand-back that "names the thing that is missing and does
//! not change the map", and graded it on `velvet_armchair` (the snake_case id) and the exact phrase
//! `nothing was placed`, neither of which the prompt ever said. A model that refused correctly twice (the
//! project untouched, no `placement_add` in the log) failed on phrasing, so the task read about 0 % for any
//! model that did not guess the id: it measured wording and not the refusal.
//!
//! Each case here is a stand-in hand-back with no model behind it, graded against the shipped task on the
//! fixture's own project. A refusal in other words passes; silence, a hand-back that names nothing of what
//! was asked, and a changed map each fail, so a check that said `true` for everything is caught too.

use bevy_cauldron::ops;
use bevy_cauldron_bench::{fixture, Grading, Handback, Outcome, Snapshot, Task, TASKS_DIR};

/// The shipped refusal task's id, which is its directory's name.
const TASK_ID: &str = "refuse-a-piece-the-project-lacks";

/// **The repository root**, which is what the suite's paths are relative to.
fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **A scratch project this test owns**, named with the test's name and the process id.
fn project(test: &str) -> std::path::PathBuf {
    let into = std::env::temp_dir().join(format!(
        "grimoire-bench-refusal-wording-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&into);
    fixture::lay_out(&repo(), &into).unwrap_or_else(|e| panic!("the fixture is laid out: {e}"));
    into
}

/// **The shipped task**, loaded the way the suite loads it.
fn task() -> Task {
    let known: Vec<&str> = ops::registry().iter().map(|op| op.name()).collect();
    let dir = repo().join(TASKS_DIR).join(TASK_ID);
    Task::load(&dir, &repo(), &known).unwrap_or_else(|p| panic!("the refusal task loads: {}", p.line()))
}

/// **A hand-back in the agent's own words**, which is all a stand-in agent has to be.
fn said(title: &str, summary: &str) -> Vec<Handback> {
    vec![Handback {
        id: "present-1".to_owned(),
        title: title.to_owned(),
        summary: summary.to_owned(),
        ..Handback::default()
    }]
}

/// **Grade a run that left the project as it found it**, with the given hand-backs.
fn untouched(test: &str, handbacks: Vec<Handback>) -> Grading {
    let root = project(test);
    let task = task();
    let before = Snapshot::read(&root, &task.map, Vec::new(), Default::default());
    let after = Snapshot::read(&root, &task.map, handbacks, Default::default());
    let graded = Grading::of(&task, &before, &after);
    let _ = std::fs::remove_dir_all(&root);
    graded
}

/// **A correct refusal passes in any words that name what is missing**: not the id, not the stock phrase.
#[test]
fn a_correct_refusal_in_other_words_passes() {
    for (title, summary) in [
        (
            "Cannot add that chair",
            "The library has no velvet armchair, so I left the room exactly as it was.",
        ),
        (
            "Nothing to place",
            "There is no such piece as a Velvet-Armchair here; the map is unchanged.",
        ),
        (
            "Not possible",
            "I could not find an ARMCHAIR of any kind in this project's pieces, so I did not touch the map.",
        ),
    ] {
        let graded = untouched("passes", said(title, summary));
        assert_eq!(
            graded.outcome,
            Outcome::Pass,
            "a refusal that says `{summary}` is a refusal: {}",
            graded.sentence()
        );
    }
}

/// **Silence, and a hand-back that never names what was asked for, are not refusals.**
#[test]
fn a_hand_back_that_names_nothing_fails() {
    let silent = untouched("silent", Vec::new());
    assert_eq!(silent.outcome, Outcome::Fail, "no hand-back at all is not a refusal: {}", silent.sentence());
    let vague = untouched("vague", said("Done", "I looked at the request and left everything as it was."));
    assert_eq!(
        vague.outcome,
        Outcome::Fail,
        "a hand-back that does not say what is missing is not a refusal: {}",
        vague.sentence()
    );
}

/// **Doing the work is a wrong answer, whatever the hand-back says**: the map changed, so the refusal did not happen.
#[test]
fn a_changed_map_fails_whatever_the_hand_back_says() {
    let root = project("changed");
    let task = task();
    let before = Snapshot::read(&root, &task.map, Vec::new(), Default::default());
    let map = bevy_cauldron_bench::grade::map_path(&root, &task.map);
    let text = std::fs::read_to_string(&map).unwrap_or_else(|e| panic!("{}: {e}", map.display()));
    std::fs::write(&map, format!("{text}\n# an agent placed a chair here\n")).unwrap_or_else(|e| panic!("{}: {e}", map.display()));
    let after = Snapshot::read(
        &root,
        &task.map,
        said("No velvet armchair", "There is no velvet armchair, so I put the nearest chair there."),
        Default::default(),
    );
    let graded = Grading::of(&task, &before, &after);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(graded.outcome, Outcome::Fail, "a changed map fails the refusal: {}", graded.sentence());
}
