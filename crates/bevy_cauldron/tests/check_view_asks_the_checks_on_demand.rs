//! **A person's Check view click owes the after-edit checks once, and says so when nothing is found (#111).**

use bevy_cauldron::after_edit::{AfterEditQueue, CHECK_VIEW, EditReport};

#[test]
fn a_request_queues_one_owed_run_named_check_view() {
    let mut q = AfterEditQueue::default();
    q.request_view();
    assert_eq!(q.pending.len(), 1);
    assert_eq!(q.pending[0].op, CHECK_VIEW);
}

#[test]
fn a_clear_run_still_answers_the_person_who_asked() {
    let clear = EditReport { seq: 0, op: CHECK_VIEW.into(), findings: vec![], skipped: vec![], errors: vec![] };
    assert_eq!(clear.journal_lines(), vec![format!("{CHECK_VIEW}: 0 checks, nothing found")]);
    let agent = EditReport { seq: 3, op: "placement_add".into(), findings: vec![], skipped: vec![], errors: vec![] };
    assert!(agent.journal_lines().is_empty(), "an agent's clear run stays silent");
}
