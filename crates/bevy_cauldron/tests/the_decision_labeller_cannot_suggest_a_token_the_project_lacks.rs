//! **The decision labeller can never suggest a vocabulary token the project lacks** (D5, #113's third
//! acceptance item).
//!
//! The labeller used to ask a chat model for free-form JSON and validate it afterwards. The decision
//! path instead asks **closed** `choice` questions whose options *are* the project's vocabulary, so a
//! token outside it is not something the model can name — the request itself has no room for it.
//!
//! That is the whole point of the change: the old path's validation was a sieve over generated text,
//! and every token it rejected was a reprompt. This pins the invariant directly, on the request the
//! path builds, and on the answer it reads back.

use bevy_grimoire_core::vocab::{Token, Vocabularies, Vocabulary};

use bevy_cauldron::label_decide::{axes, request_for, suggestion_from};

/// A project vocabulary with a deliberately small `kind` axis, so a token outside it is easy to name.
fn vocab() -> Vocabularies {
    let v = |names: &[&str]| Vocabulary {
        tokens: names
            .iter()
            .map(|n| Token { name: (*n).to_owned(), note: format!("{n}, for the test"), implies: Vec::new() })
            .collect(),
    };
    Vocabularies {
        version: bevy_grimoire_core::vocab::VOCAB_VERSION,
        kind: v(&["seating", "table"]),
        effects: v(&["emit", "seat-one"]),
        look: v(&["brown", "metal"]),
        surfaces: v(&["seat", "top"]),
        edge: v(&["wall", "open"]),
        capabilities: v(&["sit"]),
        ..Default::default()
    }
}

#[test]
fn every_option_a_choice_offers_is_a_token_the_project_has() {
    let v = vocab();
    let request = request_for(&v, "a wooden chair");

    assert!(!request.questions.is_empty(), "the labeller asks at least one question");

    for (id, question) in &request.questions {
        if let bevy_oracle::Question::Choice { criteria, .. } = question {
            // The options come from the vocabulary, so every one of them must be in it.
            let axis = axes().into_iter().find(|a| a.1 == *id).map(|a| a.0).unwrap_or_else(|| {
                panic!("question `{id}` names no axis the labeller builds")
            });
            for option in criteria.keys() {
                assert!(
                    v.axis(axis).contains(option),
                    "question `{id}` offers `{option}`, which is not a `{axis:?}` token the project \
                     has. A closed question cannot suggest what the vocabulary lacks, and that is the \
                     point of asking it this way instead of parsing generated JSON"
                );
            }
        }
    }
}

#[test]
fn a_token_the_project_lacks_can_never_be_read_back_as_a_suggestion() {
    let v = vocab();
    // **An answer naming an option the request never offered.** A well-behaved model cannot produce
    // one; a misbehaving one, a stale vocabulary, or a hand-recorded fixture can. The path must
    // refuse it rather than carry it into a descriptor's provenance.
    let mut answers = std::collections::BTreeMap::new();
    answers.insert(
        "kind".to_owned(),
        bevy_oracle::Answer::Choice {
            choice: "throne".to_owned(),
            probabilities: [("throne".to_owned(), 0.9), ("seating".to_owned(), 0.1)].into(),
            confidence: 0.9,
        },
    );
    let error = suggestion_from(&v, answers).expect_err(
        "an answer whose choice is `throne`, a token the project does not have, must be refused: a \
         suggestion that reaches a descriptor is a piece no rule can match",
    );
    assert!(
        error.contains("throne"),
        "the refusal names the token it would not accept, so an author can fix it: {error}"
    );

    // And a vocabulary answer is accepted, with the probability the model gave it.
    let mut good = std::collections::BTreeMap::new();
    good.insert(
        "kind".to_owned(),
        bevy_oracle::Answer::Choice {
            choice: "seating".to_owned(),
            probabilities: [("seating".to_owned(), 0.8), ("table".to_owned(), 0.2)].into(),
            confidence: 0.8,
        },
    );
    let suggestion = suggestion_from(&v, good).expect("`seating` is a kind the project has");
    assert_eq!(suggestion.get("kind").map(String::as_str), Some("seating"));
}
