//! **One recorded judgement, read by both wired paths; they agree on every token it names** (D5, #113).
//!
//! `labels.rs` runs one of two paths, chosen by `GRIMOIRE_LABEL_PATH`:
//!
//! * `chat` — `vlm::parse_reply` over the JSON a chat model writes;
//! * `decide` — `decide_labels::label_with_decisions` over a probability per token.
//!
//! This feeds **the same judgement** to both — the tokens a person's label names — and compares what
//! each suggests. Neither calls a model: the chat path is given the JSON that judgement would be
//! written as, and the decision path a transport that answers P(yes) from the same token list.
//!
//! The point of the comparison is the *interpretation*. `parse_reply` must reject what the vocabulary
//! lacks and re-sort into vocabulary order; the decision path must respect the threshold. If the two
//! disagree, one of them is reading the same judgement differently from the other, and the person
//! cannot tell which path labelled their piece.

use std::collections::BTreeMap;

use bevy_cauldron::decide_labels::{DEFAULT_THRESHOLD, label_with_decisions};
use bevy_cauldron::vlm::{parse_reply, validate, PromptCtx};
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

/// The JSON the chat path is handed for a judgement naming `tokens`.
fn reply(tokens: &[&str]) -> String {
    let of = |axis: &[&str]| {
        let named: Vec<&str> = tokens.iter().copied().filter(|t| axis.contains(t)).collect();
        format!("{:?}", named).replace('\'', "\"")
    };
    format!(
        r#"{{"what":"a piece","kind":{},"effects":{},"look":{},"offers_surfaces":{}}}"#,
        of(&["seating", "light"]),
        of(&["emit", "seat-one"]),
        of(&["brown", "metal"]),
        of(&["table-top", "seat"]),
    )
}

#[test]
fn both_paths_suggest_the_same_tokens_for_one_recorded_judgement() {
    // **A chair's judgement**, named the way a person's label would name it.
    let tokens = ["seating", "seat-one", "seat", "brown"];

    // The chat path: parse the JSON that judgement is written as, then validate it against the
    // vocabulary — the two steps `vlm::label_with_retry` runs on a reply.
    let raw = parse_reply(&reply(&tokens)).expect("the reply is the schema's JSON");
    let chat = validate(raw, &vocab()).expect("every token named is in the vocabulary");

    // The decision path: the same judgement as P(yes) per token.
    let (decide, _) = label_with_decisions(
        &Recorded { yes_tokens: tokens.to_vec() },
        &[png(), png()],
        &vocab(),
        &ctx("chair"),
        DEFAULT_THRESHOLD,
        "2026-11-03".to_owned(),
    )
    .expect("a replayed decision endpoint labels the piece");

    let sorted = |v: &Vec<String>| {
        let mut c = v.clone();
        c.sort();
        c
    };
    assert_eq!(
        sorted(&chat.kind),
        sorted(&decide.kind),
        "kind: chat {:?} vs decide {:?}",
        chat.kind,
        decide.kind
    );
    assert_eq!(sorted(&chat.effects), sorted(&decide.effects), "effects differ");
    assert_eq!(sorted(&chat.look), sorted(&decide.look), "look differs");
    assert_eq!(
        sorted(&chat.offers_surfaces),
        sorted(&decide.offers_surfaces),
        "surfaces differ"
    );
}

#[test]
fn both_paths_refuse_a_token_the_project_lacks_the_same_way() {
    // A reply naming `throne` is rejected by `validate`, which is the chat path's guard…
    let raw = parse_reply(r#"{"what":"a chair","kind":["throne"]}"#).expect("schema-valid JSON");
    let refused = validate(raw, &vocab()).expect_err("`throne` is not a kind this project has");
    assert!(refused.contains("throne"), "the refusal names the token: {refused}");

    // …and the decision path cannot name it at all: the request's options *are* the vocabulary, so
    // `throne` is not among the questions and no answer can carry it. That asymmetry is the point of
    // D5 — one path rejects after generation, the other has nothing to reject.
    let request = bevy_cauldron::decide_labels::build_requests(&vocab(), &ctx("chair"), &[]);
    let ids: Vec<&String> = request.iter().flat_map(|r| r.questions.keys()).collect();
    assert!(
        !ids.iter().any(|id| id.ends_with(":throne")),
        "the decision path asked about `throne`, which the project does not have"
    );
}
