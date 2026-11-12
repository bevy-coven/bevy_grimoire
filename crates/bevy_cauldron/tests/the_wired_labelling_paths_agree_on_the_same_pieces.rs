//! **The wired labelling path suggests exactly what the recorded judgement names** (D5, #113).
//!
//! There was a second path here until 2026-11-11: `chat` (`vlm::label_with_retry`'s parse of JSON)
//! beside `decide` (`decide_labels::label_with_decisions`) over the same pieces, through the
//! functions `labels.rs` really called. The owner approved removing the chat path on 2026-11-11, so
//! the assertions below — which were always about the decision path — are the whole of this file.
//!
//! Owner exception (2026-11-11):
//! `the_wired_labelling_paths_agree_on_the_same_pieces.rs::the_decision_path_suggests_every_token_the_recorded_judgement_names`
//! — only its NAME changed (it used to say both paths agree); its assertions are the same and it is
//! the only path left to assert.
//!
//! The model is replayed, never called, so what is compared is the **interpretation**: which
//! vocabulary tokens the path ends up suggesting from a judgement. That is the part a person's
//! labels are about.

use std::collections::BTreeMap;

use bevy_cauldron::decide_labels::{DEFAULT_THRESHOLD, label_with_decisions};
use bevy_cauldron::label_suggestion::PromptCtx;
use bevy_grimoire_core::vocab::{Vocabularies, Vocabulary};
use bevy_oracle::{Answer, Error, Request, Response, Transport};

/// **The recorded judgement**, in the shape the path is handed it: one `noul` per `axis:token`,
/// answered with P(yes) from the tokens a person's label names.
struct Recorded {
    /// The tokens this judgement names.
    yes_tokens: Vec<&'static str>,
}

impl Transport for Recorded {
    fn endpoint(&self) -> String {
        "recorded".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let mut answers = BTreeMap::new();
        for (id, question) in &request.questions {
            // **Answer each question in its own kind.** A `noul` wants P(yes); a `choice` wants one
            // option and a probability per option. The wired path asks both — the per-token
            // questions are `noul`, and `front` is a `choice` over the faces — so a transport that
            // answers everything as a `noul` is refused, which is what `Response::check_answers`
            // is for.
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
                    // The first option, with the judgement having nothing to say about faces.
                    let options: Vec<&String> = criteria.keys().collect();
                    let pick = options.first().map(|o| (*o).clone()).unwrap_or_default();
                    let probabilities = options
                        .iter()
                        .map(|o| ((*o).clone(), if **o == pick { 0.9 } else { 0.1 / (options.len().max(2) - 1) as f64 }))
                        .collect();
                    answers.insert(
                        id.clone(),
                        Answer::Choice { choice: pick, probabilities, confidence: 0.9 },
                    );
                }
                other => panic!("the wired path asked a question this harness has no answer for: {other:?}"),
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

/// A PNG the encoder accepts. Two are needed because the labeller renders two angles; the content is
/// irrelevant to a replayed transport.
fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([120, 90, 60, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("a 2x2 PNG writes");
    out.into_inner()
}

#[test]
fn the_decision_path_suggests_every_token_the_recorded_judgement_names() {
    // **A chair's judgement**: seating, a place to sit, brown, seats one person.
    let recorded = Recorded { yes_tokens: vec!["seating", "seat", "seat-one", "brown"] };
    let (suggestion, provenance) = label_with_decisions(
        &recorded,
        &[png(), png()],
        &vocab(),
        &ctx("chair"),
        DEFAULT_THRESHOLD,
        "2026-11-03".to_owned(),
    )
    .expect("a replayed decision endpoint labels a piece");

    assert_eq!(suggestion.kind, vec!["seating"], "the kind the judgement named");
    assert_eq!(suggestion.effects, vec!["seat-one"]);
    assert_eq!(suggestion.offers_surfaces, vec!["seat"]);
    assert_eq!(suggestion.look, vec!["brown"]);

    // **The provenance names the model and the day**, so a reviewer reading `library.yaml` can tell
    // which judgements came from a model and when — the reason `label_suggestion::Provenance`
    // carries `model`, off the answer, rather than the endpoint's configured model name.
    assert_eq!(provenance.model, "recorded", "the model that answered is the one recorded");
    assert_eq!(provenance.date, "2026-11-03");

    // **Nothing the judgement did not name leaked in**, which is the invariant that matters: a
    // probability of 0.02 is below the threshold and no token is suggested on it.
    for (axis, tokens) in [
        ("kind", &suggestion.kind),
        ("effects", &suggestion.effects),
        ("look", &suggestion.look),
        ("surfaces", &suggestion.offers_surfaces),
    ] {
        assert!(
            !tokens.iter().any(|t| t == "light" || t == "metal" || t == "table-top"),
            "`{axis}` suggests {tokens:?}, which the judgement did not name"
        );
    }
}

#[test]
fn a_replayed_judgement_that_names_nothing_suggests_nothing() {
    // The other half: every P(yes) is under the threshold, so the path suggests no token at all —
    // it does not fall back to the most probable one. A labeller that always names something is a
    // labeller nobody can trust.
    let recorded = Recorded { yes_tokens: vec![] };
    let (suggestion, _) = label_with_decisions(
        &recorded,
        &[png(), png()],
        &vocab(),
        &ctx("mystery"),
        DEFAULT_THRESHOLD,
        "2026-11-03".to_owned(),
    )
    .expect("it still answers, with nothing to suggest");
    assert!(suggestion.kind.is_empty(), "kind: {:?}", suggestion.kind);
    assert!(suggestion.effects.is_empty(), "effects: {:?}", suggestion.effects);
    assert!(suggestion.look.is_empty(), "look: {:?}", suggestion.look);
    assert!(suggestion.offers_surfaces.is_empty(), "surfaces: {:?}", suggestion.offers_surfaces);
}
