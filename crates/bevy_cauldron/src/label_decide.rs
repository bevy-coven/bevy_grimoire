//! **Kit labelling through a decision model** (D5, #113): closed `choice` questions over the project's
//! own vocabulary, instead of a chat model's free-form JSON that is validated afterwards.
//!
//! # Why this exists
//!
//! `vlm.rs` asks a chat model for JSON and validates it against the closed vocabulary, with a reprompt
//! on rejection. The vocabulary is *already* a closed set — it is a vocabulary — so the round trip
//! through generated text is a sieve whose holes cost a reprompt each. A decision model answers typed
//! questions with a probability per allowed option and generates nothing, so:
//!
//! * **a token the project lacks cannot be named.** The options *are* the vocabulary; a request has no
//!   room for `throne` when the project's kinds are `seating` and `table`. That is #113's third
//!   acceptance item, and [`suggestion_from`] still refuses one, because a stale vocabulary or a
//!   misbehaving server could send an option nobody offered.
//! * **nothing is parsed.** Every answer is a probability, so there is no prose to reject.
//!
//! # What it does not do
//!
//! It does not write a descriptor. Like `vlm.rs`, the output is a [`Suggestion`] the commit door turns
//! into data, stamped `By::Model`, so a person's own axes stay theirs.

use std::collections::BTreeMap;

use bevy_grimoire_core::vocab::Vocabularies;
use bevy_oracle::{Answer, Question, Request};

/// The axes this path labels, as `(field name, question id)`.
///
/// **`look` is deliberately absent from the questions and present in the vocabulary.** It is the
/// "search by eye" axis — gameplay never matches on it — and asking a 9B model to decide a piece is
/// `brown` from four renders is the weakest question in the set. A person sets it; the model may
/// still be asked for it later under its own calibration.
pub fn axes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("kind", "kind"),
        ("effects", "effects"),
        ("surfaces", "surfaces"),
        ("capabilities", "capabilities"),
    ]
}

/// **The request**: one `choice` per axis, its options exactly the project's tokens.
///
/// `state` carries the piece's own words — its descriptor id and whatever the caller knows about it —
/// because a decision has no memory between calls and nothing else tells it what it is looking at.
pub fn request_for(vocab: &Vocabularies, state: &str) -> Request {
    let mut request = Request::new(state);
    for (field, id) in axes() {
        let tokens: BTreeMap<String, String> = vocab
            .axis(field)
            .tokens
            .iter()
            .map(|t| (t.name.clone(), t.note.clone()))
            .collect();
        // A `choice` needs at least two options; an axis a project has not filled is not asked, since
        // a one-option question is a statement rather than a decision.
        if tokens.len() >= 2 {
            request
                .questions
                .insert(id.to_owned(), Question::Choice { instructions: String::new(), criteria: tokens });
        }
    }
    request
}

/// **Read the answers as a suggestion**, refusing any token the project does not have.
///
/// The refusal is the invariant the whole path exists for. A `choice` answer carries a most-probable
/// option and a probability for each; an option outside the vocabulary means the answer belongs to a
/// different project (a stale cache, a hand-written fixture, a misbehaving server), and carrying it
/// into a descriptor would put a piece on a map that no rule can match.
pub fn suggestion_from(
    vocab: &Vocabularies,
    answers: BTreeMap<String, Answer>,
) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (field, id) in axes() {
        let Some(answer) = answers.get(id) else { continue };
        let choice = match answer {
            Answer::Choice { choice, .. } => choice,
            other => {
                return Err(format!(
                    "question `{id}` answered as {other:?}, and this axis is a choice over the \
                     project's vocabulary"
                ))
            }
        };
        if !vocab.axis(field).contains(choice) {
            return Err(format!(
                "`{choice}` is not a `{field}` token this project has, so it cannot be suggested: the \
                 map's rules would have nothing to match it against"
            ));
        }
        out.insert(field.to_owned(), choice.clone());
    }
    Ok(out)
}
