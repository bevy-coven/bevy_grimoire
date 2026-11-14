//! **A hand-back the editor holds but the transcript does not show is graded from the editor's** (#434, BM-10).
//!
//! An agent can reach the editor by a route whose arguments the runner cannot read (omp's `eval`). The editor still holds
//! what it was handed: `grimoire/state`'s `presentation` carries the hand-back's title, summary and lists. So after the agent is
//! done the runner asks the editor, and **never scores "no hand-back was made" for a trial in which one was**:
//!
//! - the transcript shows a hand-back: it is graded as before;
//! - it shows none and the editor holds one: the editor's is graded (and the call is counted);
//! - a call the transcript could not read was a hand-back and the editor holds none, or cannot be asked: the trial is a HARNESS
//!   ERROR naming "tool calls the transcript did not show", because the runner cannot tell what was said;
//! - nothing suggests a call was hidden and the editor holds none: the agent made no hand-back, which is its answer.

use bevy_cauldron::ops;
use bevy_cauldron_bench::run::{reconcile_with_editor, AgentRun};
use bevy_cauldron_bench::{fixture, Grading, Handback, Outcome, Snapshot, Task, TASKS_DIR};
use serde_json::{json, Value};

fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("{e}"))
}

/// What `grimoire/state` answers after a hand-back, as far as the runner reads it.
fn state_with_hand_back() -> Value {
    json!({"project_root": "/p", "map": "bench_bedroom", "presentation": {
        "id": "present-1", "title": "Not placed: no velvet armchair in this project", "answer": null,
        "summary": "I did not change the map. The library has no velvet piece.",
        "specified": ["a velvet armchair at (-2.0, 1.5)"], "inferred": ["it would be a chair"], "check": ["the map is unchanged"],
        "followed": []}})
}

fn run_with(calls: &[&str], blind: &[&str]) -> AgentRun {
    AgentRun {
        tool_calls: calls.iter().map(|c| (*c).to_owned()).collect(),
        blind_calls: blind.iter().map(|c| (*c).to_owned()).collect(),
        ..AgentRun::default()
    }
}

/// **The editor's hand-back stands in for the one the transcript did not show**, words and all.
#[test]
fn the_editors_hand_back_is_taken_when_the_transcript_shows_none() {
    let mut run = run_with(&["grimoire_state", "grimoire_present"], &["grimoire_state", "grimoire_present"]);
    reconcile_with_editor(&mut run, Ok(state_with_hand_back())).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(run.handbacks.len(), 1, "one hand-back, the editor's");
    let hand = &run.handbacks[0];
    assert_eq!(hand.id, "present-1");
    assert_eq!(hand.title, "Not placed: no velvet armchair in this project");
    assert!(hand.summary.contains("no velvet piece"));
    assert_eq!(hand.specified, ["a velvet armchair at (-2.0, 1.5)"]);
    assert_eq!(hand.inferred, ["it would be a chair"]);
    assert_eq!(hand.check, ["the map is unchanged"]);
    assert!(run.handbacks_from_editor, "and the record says where it came from");
    // The call counts even when the transcript showed none: the editor proves it happened.
    let mut bare = run_with(&[], &[]);
    reconcile_with_editor(&mut bare, Ok(state_with_hand_back())).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(bare.tool_calls, ["grimoire_present"], "a hand-back the editor holds is a call that was made");
}

/// **A hand-back the transcript showed is left alone**, whatever the editor holds.
#[test]
fn a_hand_back_the_transcript_showed_is_not_replaced() {
    let mut run = run_with(&["grimoire_present"], &[]);
    run.handbacks.push(Handback { id: "present-1".into(), title: "the agent's own".into(), ..Handback::default() });
    reconcile_with_editor(&mut run, Ok(state_with_hand_back())).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(run.handbacks.len(), 1);
    assert_eq!(run.handbacks[0].title, "the agent's own");
    assert!(!run.handbacks_from_editor);
}

/// **An invisible hand-back the editor cannot confirm is a harness error, never "no hand-back"**.
#[test]
fn an_unconfirmed_invisible_hand_back_is_a_harness_error() {
    let blind = ["grimoire_present"];
    for (name, state) in [
        ("the editor holds none", Ok(json!({"presentation": null, "map": "bench_bedroom"}))),
        ("the editor cannot be asked", Err("grimoire/state: connection refused".to_owned())),
    ] {
        let mut run = run_with(&["grimoire_present"], &blind);
        let said = reconcile_with_editor(&mut run, state).expect_err(name);
        assert!(said.contains("tool calls the transcript did not show"), "{name}: {said}");
    }
}

/// **No hidden call and no editor hand-back is the agent's own answer**: nothing to reconcile.
#[test]
fn no_hand_back_anywhere_is_no_hand_back() {
    for state in [Ok(json!({"presentation": null})), Err("no editor".to_owned())] {
        let mut run = run_with(&["grimoire_state"], &[]);
        reconcile_with_editor(&mut run, state).unwrap_or_else(|e| panic!("{e}"));
        assert!(run.handbacks.is_empty());
    }
}

/// **The refusal task is graded from the editor's hand-back exactly as from a transcript's**: a correct refusal passes.
#[test]
fn a_refusal_the_editor_holds_passes_the_refusal_task() {
    let known: Vec<&str> = ops::registry().iter().map(|op| op.name()).collect();
    let task = Task::load(&repo().join(TASKS_DIR).join("refuse-a-piece-the-project-lacks"), &repo(), &known).unwrap_or_else(|p| panic!("{}", p.line()));
    let project = std::env::temp_dir().join(format!("grimoire-bench-unseen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    fixture::lay_out(&repo(), &project).unwrap_or_else(|e| panic!("{e}"));
    let mut run = run_with(&["grimoire_present"], &["grimoire_present"]);
    reconcile_with_editor(&mut run, Ok(state_with_hand_back())).unwrap_or_else(|e| panic!("{e}"));
    let before = Snapshot::read(&project, &task.map, Vec::new(), Default::default());
    let after = Snapshot::read(&project, &task.map, run.handbacks.clone(), Default::default());
    let graded = Grading::of(&task, &before, &after);
    let _ = std::fs::remove_dir_all(&project);
    assert_eq!(graded.outcome, Outcome::Pass, "{}", graded.sentence());
}
