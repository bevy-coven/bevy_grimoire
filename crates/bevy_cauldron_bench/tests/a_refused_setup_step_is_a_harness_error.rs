//! **A setup step the editor refused is a harness error, not a trial that ran** (#433, item 1).
//!
//! `work-the-open-note` passed its own oracle alone and failed 8 of 24 when its trials ran 8 at a time: the oracle's
//! `note_claim` found "no note n1". The map and notes file of a failed trial were untouched, so the task's SETUP
//! (`note_add`) never happened, and the runner did not know: the editor answers an operation call with a normal
//! result whose `outcome` is `refused`, and the setup loop only looked for a transport error. The answer is now read.

use bevy_cauldron_bench::run::refused_answer;
use serde_json::json;

/// **A refused answer is a refusal, quoting the editor's own sentence and the operation.**
#[test]
fn a_refused_answer_is_a_refusal() {
    let answer = json!({"operation": "note_add", "outcome": "refused", "refusal": "no map is open, so nothing was added",
        "problem": {"what": "no map is open"}});
    let said = refused_answer(&answer).expect("a refused answer is read as one");
    assert!(said.contains("note_add"), "it names the operation: {said}");
    assert!(said.contains("no map is open, so nothing was added"), "and the editor's sentence: {said}");
    let only_problem = json!({"operation": "note_send", "outcome": "refused", "problem": {"what": "notes: no note n1"}});
    assert!(refused_answer(&only_problem).is_some_and(|s| s.contains("notes: no note n1")), "the problem's text is used when there is no refusal");
}

/// **An applied answer is not one.**
#[test]
fn an_applied_answer_is_not_a_refusal() {
    assert!(refused_answer(&json!({"operation": "note_add", "outcome": "applied", "summary": "added n1"})).is_none());
    assert!(refused_answer(&json!(null)).is_none(), "an answer with nothing in it is not a refusal either");
    assert!(refused_answer(&json!({"operation": "x", "outcome": "dry_run"})).is_none());
}
