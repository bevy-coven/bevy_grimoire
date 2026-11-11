//! **One judgement, read by the one wired labelling path; it names exactly the tokens it was given**
//! (D5, #113).
//!
//! `labels.rs` labels through `decide_labels::label_with_decisions`: one `noul` question per
//! vocabulary token, answered with a probability, plus a `choice` for the mount and (only when the
//! mesh has no measurable front) one for the front.
//!
//! This feeds the path **the judgement a person's label names** and asks what it suggests. Nothing
//! calls a model: the transport answers P(yes) from the same token list.
//!
//! **What changed, and why it is not a weakening.** Until 2026-11-11 this file compared two paths —
//! the chat path's `parse_reply` + `validate` against the decision path's threshold — because the
//! issue's rule was that the old path is removed only once the new one measures equal. **This file
//! was that measurement**: the two wired paths read one recorded judgement the same way, on every
//! axis. The owner approved removing the chat path on 2026-11-11 without waiting for a live-model
//! percentage, so there is no second path to compare against. Every check that is not about the chat
//! path is kept below: the tokens suggested are exactly the ones the judgement named, and a token the
//! project lacks is never asked about, so it can never be suggested.
//!
//! Owner exception (2026-11-11): `the_two_paths_read_one_judgement_the_same_way.rs
//! ::both_paths_suggest_the_same_tokens_for_one_recorded_judgement` — the chat path's half of the
//! comparison is deleted with the path it measured; the decision path's half is kept and asserted
//! unchanged. Owner exception (2026-11-11):
//! `the_two_paths_read_one_judgement_the_same_way.rs::both_paths_refuse_a_token_the_project_lacks_the_same_way`
//! — the chat path's rejection is deleted with `validate`; the decision path's half (the request
//! carries no room for a foreign token) is kept and asserted unchanged.

use std::collections::BTreeMap;

use bevy_cauldron::decide_labels::{DEFAULT_THRESHOLD, build_requests, label_with_decisions};
use bevy_cauldron::label_suggestion::PromptCtx;
use bevy_grimoire_core::vocab::{Vocabularies, Vocabulary};
use bevy_oracle::{Answer, Error, Request, Response, Transport};

struct Recorded {
    yes_tokens: Vec<&'static str>,
}

impl Transport for Recorded {
    fn endpoint(&self) -> String {
        "recorded".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let mut answers = BTreeMap::new();
        for (id, question) in &request.questions {
            match question {
                bevy_oracle::Question::Noul { .. } => {
                    let p = if self.yes_tokens.iter().any(|t| id.ends_with(&format!(":{t}"))) {
                        0.97
                    } else {
                        0.02
                    };
                    answers.insert(id.clone(), Answer::Noul { noul: p });
                }
                bevy_oracle::Question::Choice { criteria, .. } => {
                    let options: Vec<&String> = criteria.keys().collect();
                    let pick = options.first().map(|o| (*o).clone()).unwrap_or_default();
                    let probabilities = options
                        .iter()
                        .map(|o| {
                            let p = if **o == pick { 0.9 } else { 0.1 / (options.len().max(2) - 1) as f64 };
                            ((*o).clone(), p)
                        })
                        .collect();
                    answers.insert(
                        id.clone(),
                        Answer::Choice { choice: pick, probabilities, confidence: 0.9 },
                    );
                }
                other => panic!("no answer for {other:?}"),
            }
        }
        Ok(Response { model: Some("recorded".into()), answers, usage: None })
    }
}

fn vocab() -> Vocabularies {
    Vocabularies {
        kind: Vocabulary::of(&[("seating", "to sit on"), ("light", "gives light")]),
        effects: Vocabulary::of(&[("emit", "makes light"), ("seat-one", "one person sits")]),
        look: Vocabulary::of(&[("brown", "brown"), ("metal", "metal")]),
        surfaces: Vocabulary::of(&[("table-top", "flat top"), ("seat", "a place to sit")]),
        ..Vocabularies::default()
    }
}

fn ctx(id: &str) -> PromptCtx {
    PromptCtx {
        id: id.into(),
        mesh: format!("{id}.glb"),
        footprint: Some((0.5, 0.5)),
        height: Some(0.9),
        mount_now: String::new(),
        kind_now: vec![],
        effects_now: vec![],
        look_now: vec![],
        offers_now: vec![],
        note_now: None,
        rooms_in_use: vec![],
        groups_in_use: vec![],
        front_measured: None,
    }
}

fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([120, 90, 60, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("a 2x2 PNG writes");
    out.into_inner()
}

#[test]
fn the_path_names_exactly_the_tokens_the_recorded_judgement_named() {
    // **A chair's judgement**, named the way a person's label would name it.
    let tokens = ["seating", "seat-one", "seat", "brown"];

    let (decide, _) = label_with_decisions(
        &Recorded { yes_tokens: tokens.to_vec() },
        &[png(), png()],
        &vocab(),
        &ctx("chair"),
        DEFAULT_THRESHOLD,
        "2026-11-03".to_owned(),
    )
    .expect("a replayed decision endpoint labels the piece");

    // Compared as sets, because the path's own order is the request's (vocabulary order) and this
    // assertion is about WHICH tokens, not which first.
    let sorted = |v: &Vec<String>| {
        let mut c = v.clone();
        c.sort();
        c
    };
    assert_eq!(sorted(&decide.kind), vec!["seating".to_owned()], "kind");
    assert_eq!(sorted(&decide.effects), vec!["seat-one".to_owned()], "effects");
    assert_eq!(sorted(&decide.look), vec!["brown".to_owned()], "look");
    assert_eq!(sorted(&decide.offers_surfaces), vec!["seat".to_owned()], "surfaces");
}

#[test]
fn a_token_the_project_lacks_has_no_room_in_the_request() {
    // A judgement naming `throne` cannot be read as one, because the request's options *are* the
    // vocabulary: `throne` is not among the questions and no answer can carry it. That asymmetry is
    // the point of D5 — the decision path has nothing to reject, because nothing foreign can arrive.
    let request = build_requests(&vocab(), &ctx("chair"), &[]);
    let ids: Vec<&String> = request.iter().flat_map(|r| r.questions.keys()).collect();
    assert!(
        !ids.iter().any(|id| id.ends_with(":throne")),
        "the decision path asked about `throne`, which the project does not have"
    );
    // And the guard is not vacuous: an answer that volunteers it anyway is ignored rather than
    // trusted, because `suggest` reads back only the ids it built.
    let mut answers = BTreeMap::new();
    for request in &request {
        for id in request.questions.keys() {
            answers.insert(id.clone(), Answer::Noul { noul: 0.01 });
        }
    }
    answers.insert("kind:seating".to_owned(), Answer::Noul { noul: 0.97 });
    answers.insert("kind:throne".to_owned(), Answer::Noul { noul: 0.99 });
    let d = bevy_cauldron::decide_labels::suggest(
        &vocab(),
        &[Response { model: Some("recorded".into()), answers, usage: None }],
        DEFAULT_THRESHOLD,
    );
    assert_eq!(d.suggestion.kind, vec!["seating".to_owned()], "`throne` was accepted");
}
