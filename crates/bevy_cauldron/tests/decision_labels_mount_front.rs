//! **The decision path labels `mount` and `front` too** (#113): a `choice` over exactly the mounts the
//! project can satisfy, and over the four faces only when the mesh could not measure its own front.

use bevy_grimoire_core::descriptor::{mount_label, Face, Mount};
use bevy_grimoire_core::vocab::{Vocabularies, Vocabulary};
use bevy_grimoire_decide::{Answer, Response};
use bevy_cauldron::decide_labels::{build_requests, suggest, DEFAULT_THRESHOLD};
use bevy_cauldron::vlm::PromptCtx;
use std::collections::BTreeMap;

fn vocab() -> Vocabularies {
    Vocabularies {
        kind: Vocabulary::of(&[("seating", "to sit on")]),
        surfaces: Vocabulary::of(&[("table-top", "flat top"), ("shelf", "a shelf")]),
        ..Vocabularies::default()
    }
}

fn ctx(front_measured: Option<Option<Face>>) -> PromptCtx {
    PromptCtx {
        id: "chair".into(),
        mesh: "chair.glb".into(),
        footprint: None,
        height: None,
        mount_now: String::new(),
        kind_now: vec![],
        effects_now: vec![],
        look_now: vec![],
        offers_now: vec![],
        note_now: None,
        rooms_in_use: vec![],
        groups_in_use: vec![],
        front_measured,
    }
}

fn choice(winner: &str, others: &[String]) -> Answer {
    let mut probabilities: BTreeMap<String, f64> = others.iter().map(|o| (o.clone(), 0.01)).collect();
    probabilities.insert(winner.to_owned(), 0.9);
    Answer::Choice { choice: winner.to_owned(), probabilities, confidence: 0.9 }
}

fn options(request: &bevy_grimoire_decide::Request, id: &str) -> Vec<String> {
    match &request.questions[id] {
        bevy_grimoire_decide::Question::Choice { criteria, .. } => criteria.keys().cloned().collect(),
        other => panic!("`{id}` is {other:?}"),
    }
}

#[test]
fn the_mount_is_one_of_the_projects_own_mounts() {
    let v = vocab();
    let requests = build_requests(&v, &ctx(None), &[]);
    let offered = options(&requests[0], "mount");
    assert!(offered.contains(&mount_label(Some(&Mount::OnSurface { class: "shelf".into() }))));
    assert!(!offered.contains(&"on throne".to_owned()));

    let mut r = Response { model: Some("clef-flash".into()), answers: BTreeMap::new(), usage: None };
    r.answers.insert("mount".into(), choice("on shelf", &offered));
    assert_eq!(suggest(&v, &[r.clone()], DEFAULT_THRESHOLD).suggestion.mount, Some(Mount::OnSurface { class: "shelf".into() }));

    // An option nobody offered is never taken, even when the model volunteers it with certainty:
    // only offered options are read, so the winner is the best of those.
    let mut r2 = Response { model: Some("m".into()), answers: BTreeMap::new(), usage: None };
    r2.answers.insert("mount".into(), choice("on throne", &offered));
    let got = suggest(&v, &[r2], DEFAULT_THRESHOLD).suggestion.mount;
    assert!(got.is_none_or(|m| offered.contains(&mount_label(Some(&m)))));

    // `unset` is an answer: the mount is left alone.
    let mut r3 = Response { model: Some("m".into()), answers: BTreeMap::new(), usage: None };
    r3.answers.insert("mount".into(), choice("unset", &offered));
    assert_eq!(suggest(&v, &[r3], DEFAULT_THRESHOLD).suggestion.mount, None);
}

#[test]
fn a_front_is_asked_only_when_the_mesh_could_not_measure_one() {
    let v = vocab();
    assert!(build_requests(&v, &ctx(None), &[])[0].questions.contains_key("front"));
    assert!(!build_requests(&v, &ctx(Some(Some(Face::East))), &[])[0].questions.contains_key("front"));
    assert!(!build_requests(&v, &ctx(Some(None)), &[])[0].questions.contains_key("front"));

    let faces = options(&build_requests(&v, &ctx(None), &[])[0], "front");
    let mut r = Response { model: Some("m".into()), answers: BTreeMap::new(), usage: None };
    r.answers.insert("front".into(), choice("west", &faces));
    assert_eq!(suggest(&v, &[r], DEFAULT_THRESHOLD).suggestion.front, Some(Face::West));
}
