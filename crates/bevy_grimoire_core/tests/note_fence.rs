//! **The agent's fence, its questions and its report** (#92): the editor enforces the area, the
//! ask-first thresholds and the resolve record; it does not trust the agent to.

use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::notes::{Change, LARGE_CHANGE, Notes, Resolution, State, What};

/// A claimed area note over cells (4,4)..(5,5), margin 1, so the reach is (3,3)..(6,6).
fn claimed() -> (Notes, String) {
    let mut n = Notes::default();
    let id = n.add_area(&Map::default(), (4, 4, 5, 5), 1.0, (0.0, 0.0, 0.0, 0.0, 0.0), "tidy").unwrap();
    n.send(&id).unwrap();
    n.claim(&id, "omp", 1000).unwrap();
    (n, id)
}

fn inside() -> Change {
    Change { cells: Some((4, 4, 5, 5)), pieces: 2, ..Change::default() }
}

#[test]
fn a_change_outside_the_area_plus_margin_is_refused_even_after_an_answer() {
    let (mut n, id) = claimed();
    n.fence(&id, "omp", &inside()).expect("inside the area is just done");
    let out = Change { cells: Some((4, 4, 7, 5)), ..inside() };
    assert!(n.fence(&id, "omp", &out).unwrap_err().contains("outside note"));
    n.ask(&id, "omp", "may I go further?", 1001).unwrap();
    n.answer(&id, "yes", 1002).unwrap();
    assert!(n.fence(&id, "omp", &out).is_err(), "an answer does not move the fence");
}

#[test]
fn a_change_in_the_margin_waits_for_an_answer() {
    let (mut n, id) = claimed();
    let margin = Change { cells: Some((3, 4, 5, 5)), ..inside() };
    let e = n.fence(&id, "omp", &margin).unwrap_err();
    assert!(e.contains("margin") && e.contains("note_ask"), "{e}");
    n.ask(&id, "omp", "the fence post is one cell out; move it?", 1001).unwrap();
    assert!(n.fence(&id, "omp", &margin).unwrap_err().contains("waiting"), "asking is not an answer");
    n.answer(&id, "go on", 1002).unwrap();
    n.fence(&id, "omp", &margin).expect("answered, so allowed");
}

#[test]
fn removing_a_person_placed_piece_refuses_and_tells_the_agent_to_ask() {
    let (n, id) = claimed();
    let c = Change { person_placed: vec!["p7".into()], ..inside() };
    let e = n.fence(&id, "omp", &c).unwrap_err();
    assert!(e.contains("p7") && e.contains("placed by a person") && e.contains("ask the person first"), "{e}");
}

#[test]
fn more_than_ten_pieces_or_an_unmapped_request_must_be_asked_about() {
    let (n, id) = claimed();
    let ten = Change { pieces: LARGE_CHANGE, ..inside() };
    n.fence(&id, "omp", &ten).expect("ten is still small");
    assert!(n.fence(&id, "omp", &Change { pieces: LARGE_CHANGE + 1, ..inside() }).is_err());
    let e = n.fence(&id, "omp", &Change { needs_unmapped: Some("a fountain".into()), ..inside() }).unwrap_err();
    assert!(e.contains("a fountain"), "{e}");
}

#[test]
fn an_answer_given_before_the_claim_does_not_count_for_the_next_holder() {
    let (mut n, id) = claimed();
    n.ask(&id, "omp", "q", 1001).unwrap();
    n.answer(&id, "a", 1002).unwrap();
    n.release(&id, Some("omp"), 1003).unwrap();
    n.claim(&id, "cursor", 1004).unwrap();
    let c = Change { pieces: 11, ..inside() };
    assert!(n.fence(&id, "cursor", &c).is_err());
}

#[test]
fn only_the_holder_is_fenced_in_and_only_the_holder_asks() {
    let (mut n, id) = claimed();
    assert!(n.fence(&id, "cursor", &inside()).unwrap_err().contains("claimed by omp"));
    assert!(n.ask(&id, "cursor", "q", 1001).is_err());
    n.ask(&id, "omp", "q", 1001).unwrap();
    assert!(n.ask(&id, "omp", "q2", 1002).is_err(), "one question stands at a time");
    assert_eq!(n.get(&id).unwrap().pending_question(), Some("q"));
    n.answer(&id, "a", 1003).unwrap();
    assert!(n.answer(&id, "again", 1004).is_err());
    let whats: Vec<_> = n.get(&id).unwrap().history.iter().map(|e| (e.by.as_str(), &e.what)).collect();
    assert!(whats.contains(&("person", &What::Answered { answer: "a".into() })), "{whats:?}");
}

#[test]
fn resolving_keeps_summary_assumptions_ops_and_two_captures() {
    let (mut n, id) = claimed();
    let rec = |captures: Vec<&str>| Resolution {
        summary: "swept the yard".into(),
        assumptions: vec!["the crates are clutter".into()],
        ops: vec!["op-12".into(), "op-13".into()],
        captures: captures.into_iter().map(String::from).collect(),
    };
    assert!(n.resolve_record(&id, "omp", rec(vec!["before.png"]), 1010).is_err(), "two captures");
    n.ask(&id, "omp", "remove the barrel?", 1011).unwrap();
    assert!(n.resolve_record(&id, "omp", rec(vec!["b.png", "a.png"]), 1012).is_err(), "a question is unanswered");
    n.answer(&id, "yes", 1013).unwrap();
    assert!(n.resolve_record(&id, "cursor", rec(vec!["b.png", "a.png"]), 1014).is_err(), "not the holder");
    n.resolve_record(&id, "omp", rec(vec!["b.png", "a.png"]), 1015).unwrap();
    let State::Resolved { ops, summary, assumptions, captures } = &n.get(&id).unwrap().state else { panic!() };
    assert_eq!((ops.len(), summary.as_str(), assumptions.len()), (2, "swept the yard", 1));
    assert_eq!(captures, &["b.png", "a.png"]);
    assert_eq!(n.get(&id).unwrap().status(), "done");
    let back = Notes::parse(&n.to_yaml().unwrap()).unwrap();
    assert_eq!(back, n, "the record survives the file");
}
