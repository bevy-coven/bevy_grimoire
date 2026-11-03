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
            // **An instruction is required, and the server says so**: a `choice` with empty
            // instructions is refused outright (`the instructions and the options of a question must
            // not be empty`), which a calibration run found on its first request. The words say what
            // the axis *means*, so the model is answering the same question the vocabulary file
            // documents rather than guessing from the token names.
            request.questions.insert(
                id.to_owned(),
                Question::Choice {
                    instructions: format!("Which `{field}` does this piece have, in this project's vocabulary?"),
                    criteria: tokens,
                },
            );
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

/// **Which path labels a kit** (D5's second acceptance item): the `choice` questions here, or the old
/// free-form JSON through `vlm.rs`.
///
/// Both exist until the measured agreement says the new one is equal or better, and this is what
/// decides between them — never a guess in the calling code. `GRIMOIRE_LABEL_PATH` names one:
/// `decide` (the default) or `vlm`. An **unset** variable is `decide`, because a new install is not
/// a migration and the decision path is the one this repository is moving to; a value that names
/// neither is refused rather than silently defaulted, because a typo that quietly keeps the old path is
/// how a migration stalls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Path {
    /// Closed `choice` questions through `bevy_oracle`.
    Decide,
    /// A chat model's JSON through `vlm.rs`. Kept until the measurement allows its removal.
    Vlm,
}

impl Path {
    /// The environment variable that chooses.
    pub const VAR: &'static str = "GRIMOIRE_LABEL_PATH";

    /// Read one value. `None` is `Decide`; a name neither path has is an error naming both.
    pub fn named(value: Option<&str>) -> Result<Path, String> {
        match value.map(str::trim) {
            None | Some("") | Some("decide") => Ok(Path::Decide),
            Some("vlm") => Ok(Path::Vlm),
            Some(other) => Err(format!(
                "{}={other} names no labelling path; say `decide` (closed questions) or \
                 `vlm` (the old free-form JSON)",
                Self::VAR
            )),
        }
    }

    /// Read it from the environment, the way the editor does.
    pub fn from_env() -> Result<Path, String> {
        Self::named(std::env::var(Self::VAR).ok().as_deref())
    }
}

/// **One labelled piece**: its own words, and the axes a person gave it.
///
/// The set D5's first acceptance item measures against. It is data (`fixtures/label_kit.ron`), not
/// code, because the labels are the person's and a change to them is an edit rather than a commit to a
/// test.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LabelledPiece {
    /// What the piece is, in the words a labeller is given. This is the `state` of the request.
    pub state: String,
    /// The person's own answer per axis field, `kind` and `surfaces` and so on. An axis the person did
    /// not label is absent and does not count against either path.
    pub labels: BTreeMap<String, String>,
}

/// **How many of the person's labels a path got right**, over the axes it answered.
///
/// `answers` is what the path produced for the piece; `piece.labels` is the person's. An axis the
/// person left unlabelled is skipped, and one the path left unlabelled counts wrong — a path that
/// declines to answer did not agree.
pub fn agreement(piece: &LabelledPiece, answers: &BTreeMap<String, Answer>) -> (usize, usize) {
    let mut right = 0;
    let mut asked = 0;
    for (field, truth) in &piece.labels {
        asked += 1;
        let Some((id, _)) = axes().into_iter().find(|(f, _)| f == field) else { continue };
        let Some(answer) = answers.get(id) else { continue };
        if let Answer::Choice { choice, .. } = answer
            && choice == truth
        {
            right += 1;
        }
    }
    (right, asked)
}

/// **The measured comparison**, as a file a test reads.
///
/// D5's first acceptance item is *"On the same labelled kit, the decision path's agreement with the
/// person's labels is measured against the old path's (test reads both records)"*. A run writes this;
/// `tests/the_two_labelling_paths_are_compared.rs` keeps it honest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Comparison {
    /// The labelled set the two paths answered.
    pub set: String,
    /// How many pieces it holds.
    pub pieces: usize,
    /// Right answers over axes answered, for each path.
    pub decide_right: usize,
    pub decide_asked: usize,
    pub vlm_right: usize,
    pub vlm_asked: usize,
    /// Which model answered, and when. Empty is a run that could not be made.
    pub decide_model: String,
    pub vlm_model: String,
    pub date: String,
}

impl Comparison {
    /// The decision path's agreement, 0..=1. Zero asked is zero: a path that answered nothing agreed
    /// with nothing.
    pub fn decide(&self) -> f64 {
        if self.decide_asked == 0 { 0.0 } else { self.decide_right as f64 / self.decide_asked as f64 }
    }
    /// The old path's agreement, 0..=1.
    pub fn vlm(&self) -> f64 {
        if self.vlm_asked == 0 { 0.0 } else { self.vlm_right as f64 / self.vlm_asked as f64 }
    }
}
