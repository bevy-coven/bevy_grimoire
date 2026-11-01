//! **Labelling through typed questions** (#113, D5): each vocabulary token of an axis (`kind`,
//! `effects`, `look`, `surfaces`) becomes one `noul` question, "does this asset have this token?",
//! and the decision model answers P(yes) for each. Nothing is generated and nothing is parsed, so the
//! chat path's reprompt loop has no counterpart here.
//!
//! **A token the project lacks can never be suggested**: the question set is built from the
//! vocabulary and [`suggest`] reads back only the ids it built, so an answer for any other id is
//! ignored rather than trusted.
//!
//! The chat path in [`crate::vlm`] stays until a measurement on a labelled kit (see [`agreement`])
//! says this one is equal or better; [`LabelPath`] selects between them by configuration.

use crate::vlm::{LabelFailure, PromptCtx, Provenance, Suggestion};
use crate::vlm::{face_token, mount_meaning};
use bevy_grimoire_core::descriptor::{Confidence, Face, Mount, mount_label, mount_options};
use bevy_grimoire_core::vocab::{Vocabularies, Vocabulary};
use bevy_grimoire_decide::{Error, MAX_QUESTIONS, Question, Request, Response, Transport, ask};
use std::collections::{BTreeMap, BTreeSet};

/// Which path labels a mesh. One per run: no fallback from one to the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LabelPath {
    /// A chat model answers free-form JSON, validated against the vocabulary (the original path).
    #[default]
    Chat,
    /// A decision model answers one typed question per token.
    Decide,
}

impl LabelPath {
    /// `GRIMOIRE_LABEL_PATH` = `chat` | `decide`; unset or empty is `chat`. Anything else is an error
    /// naming the two values, never a silent default.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<LabelPath, String> {
        match get("GRIMOIRE_LABEL_PATH").as_deref().map(str::trim) {
            None | Some("") | Some("chat") => Ok(LabelPath::Chat),
            Some("decide") => Ok(LabelPath::Decide),
            Some(other) => Err(format!("GRIMOIRE_LABEL_PATH is `{other}`; it is `chat` or `decide`")),
        }
    }
}

impl LabelPath {
    /// The process environment first, then the project root's `.env`, as [`crate::vlm::VlmConfig::load`] reads it.
    pub fn load(root: &std::path::Path) -> Result<LabelPath, String> {
        let file = crate::vlm::dotenv(root);
        Self::from_lookup(|k| std::env::var(k).ok().filter(|v| !v.is_empty()).or_else(|| file.get(k).cloned()))
    }
}

/// A token is suggested when the model's P(yes) reaches this. The default is the even bet; the
/// threshold a calibration on a labelled kit picks replaces it.
pub const DEFAULT_THRESHOLD: f64 = 0.5;

/// The axes the decision path labels, in the order a `Suggestion` lists them.
const AXES: [&str; 4] = ["kind", "effects", "look", "surfaces"];

fn axis_vocab<'a>(v: &'a Vocabularies, axis: &str) -> &'a Vocabulary {
    match axis {
        "kind" => &v.kind,
        "effects" => &v.effects,
        "look" => &v.look,
        _ => &v.surfaces,
    }
}

fn axis_meaning(axis: &str) -> &'static str {
    match axis {
        "kind" => "what the asset IS (its identity)",
        "effects" => "what the asset DOES to the world, or needs in order to",
        "look" => "what the asset LOOKS like (colour, material, wear)",
        _ => "what support surface class the asset OFFERS to things resting on it",
    }
}

/// The question id of one token: `axis:token`. Tokens never contain `:`, so it parses back.
fn question_id(axis: &str, token: &str) -> String {
    format!("{axis}:{token}")
}

const FACES: [Face; 4] = [Face::North, Face::East, Face::South, Face::West];
const MOUNT_ID: &str = "mount";
const FRONT_ID: &str = "front";
/// The option of the `mount` and `front` choices that says "none applies / unclear".
const NONE_OPTION: &str = "unset";

fn mount_options_for(vocab: &Vocabularies) -> Vec<Mount> {
    let surfaces: Vec<String> = vocab.surfaces.tokens.iter().map(|t| t.name.clone()).collect();
    mount_options(&surfaces)
}

/// One `choice` over exactly the mounts [`mount_options`] offers for this project (one per surface
/// class), keyed by [`mount_label`], plus `unset`. A mount the project cannot satisfy is not an option,
/// so it cannot be suggested.
fn mount_question(vocab: &Vocabularies) -> Question {
    let mut options: Vec<(String, String)> = mount_options_for(vocab)
        .iter()
        .map(|m| (mount_label(Some(m)), mount_meaning(m).to_owned()))
        .collect();
    options.push((NONE_OPTION.to_owned(), "unclear; leave the mount as it is".to_owned()));
    Question::choice("What kind of support does THIS asset need?", options)
}

fn front_question() -> Question {
    let mut options: Vec<(String, String)> = FACES
        .into_iter()
        .map(|f| (face_token(f).to_owned(), format!("the item's visual front faces {}", face_token(f))))
        .collect();
    options.push((NONE_OPTION.to_owned(), "no front: a symmetric prop".to_owned()));
    Question::choice(
        "Which face is the item's visual FRONT, the side it presents to the room (a sofa fronts where you sit, a screen where it is watched)?",
        options,
    )
}

/// The most probable option of a `choice` answer to `id`, if it is one of `allowed`.
fn pick(responses: &[Response], id: &str, allowed: &[String]) -> Option<String> {
    let answer = responses.iter().find_map(|r| r.answers.get(id))?;
    let mut best: Option<(String, f64)> = None;
    for name in allowed {
        if let Some(p) = answer.p_option(name)
            && best.as_ref().is_none_or(|(_, b)| p > *b)
        {
            best = Some((name.clone(), p));
        }
    }
    best.map(|(n, _)| n)
}

/// One request per [`MAX_QUESTIONS`] tokens, every request carrying the same two renders. `images`
/// are the data URLs of the front and rear renders.
pub fn build_requests(vocab: &Vocabularies, ctx: &PromptCtx, images: &[String]) -> Vec<Request> {
    let mut state = format!(
        "A 3D game asset `{}` (mesh `{}`), shown in two renders: three-quarter front, then three-quarter rear.",
        ctx.id, ctx.mesh
    );
    if let (Some((w, d)), Some(h)) = (ctx.footprint, ctx.height) {
        state.push_str(&format!(" Measured size: {w:.2} m wide, {d:.2} m deep, {h:.2} m tall."));
    }
    let questions: Vec<(String, Question)> = AXES
        .iter()
        .flat_map(|axis| {
            axis_vocab(vocab, axis).tokens.iter().map(move |t| {
                let note = if t.note.is_empty() { String::new() } else { format!(" Meaning: {}.", t.note) };
                let q = Question::noul(format!(
                    "Axis `{axis}` is {}. Does this asset have the token `{}`?{note} Answer yes only if it clearly applies; most tokens apply to nothing.",
                    axis_meaning(axis),
                    t.name
                ));
                (question_id(axis, &t.name), q)
            })
        })
        .collect();
    let mut questions = questions;
    questions.push((MOUNT_ID.to_owned(), mount_question(vocab)));
    // A measured front is a fact read off the vertex buffer and is not asked; only an unmeasurable
    // mesh has its front judged from the renders.
    if ctx.front_measured.is_none() {
        questions.push((FRONT_ID.to_owned(), front_question()));
    }
    questions
        .chunks(MAX_QUESTIONS)
        .map(|chunk| {
            let mut r = Request::new(state.clone());
            r.images = images.to_vec();
            for (id, q) in chunk {
                r = r.with_question(id.clone(), q.clone());
            }
            r
        })
        .collect()
}

/// What the decision path decided: the suggestion, and P(yes) of every token it asked about.
#[derive(Clone, Debug, PartialEq)]
pub struct Decided {
    pub suggestion: Suggestion,
    /// `(axis, token) -> P(yes)` for every asked token, kept so a calibration can pick a threshold.
    pub probabilities: BTreeMap<(String, String), f64>,
}

/// Fold the answers into a suggestion: on each axis, the tokens at or above `threshold`, in
/// vocabulary order. Only ids built from `vocab` are read.
pub fn suggest(vocab: &Vocabularies, responses: &[Response], threshold: f64) -> Decided {
    let mut probabilities = BTreeMap::new();
    for axis in AXES {
        for t in &axis_vocab(vocab, axis).tokens {
            let p = responses
                .iter()
                .find_map(|r| r.answers.get(&question_id(axis, &t.name)))
                .and_then(|a| a.p_yes());
            if let Some(p) = p {
                probabilities.insert((axis.to_owned(), t.name.clone()), p);
            }
        }
    }
    let picked = |axis: &str| -> Vec<String> {
        axis_vocab(vocab, axis)
            .tokens
            .iter()
            .filter(|t| probabilities.get(&(axis.to_owned(), t.name.clone())).is_some_and(|p| *p >= threshold))
            .map(|t| t.name.clone())
            .collect()
    };
    let kind = picked("kind");
    // How far the most doubtful answer sits from the threshold: a confident labelling is one with no
    // token near the line.
    let margin = probabilities.values().map(|p| (p - threshold).abs()).fold(f64::INFINITY, f64::min);
    let confidence = if probabilities.is_empty() || margin < 0.2 {
        Confidence::Low
    } else if margin < 0.4 {
        Confidence::Medium
    } else {
        Confidence::High
    };
    let suggestion = Suggestion {
        what: if kind.is_empty() { String::new() } else { kind.join(", ") },
        effects: picked("effects"),
        look: picked("look"),
        offers_surfaces: picked("surfaces"),
        kind,
        mount: {
            let options = mount_options_for(vocab);
            let mut names: Vec<String> = options.iter().map(|m| mount_label(Some(m))).collect();
            names.push(NONE_OPTION.to_owned());
            pick(responses, MOUNT_ID, &names).and_then(|n| options.into_iter().find(|m| mount_label(Some(m)) == n))
        },
        front: {
            let names: Vec<String> = FACES.iter().map(|f| face_token(*f).to_owned()).chain([NONE_OPTION.to_owned()]).collect();
            pick(responses, FRONT_ID, &names).and_then(|n| FACES.into_iter().find(|f| face_token(*f) == n))
        },
        needs_turn: None,
        note: None,
        rooms: Vec::new(),
        group: None,
        confidence,
        token_proposals: Vec::new(),
    };
    Decided { suggestion, probabilities }
}

/// Label one mesh through `transport`. The provenance names the model that answered, as the chat
/// path's does; an endpoint that names none is refused.
pub fn label_with_decisions(
    transport: &dyn Transport,
    pngs: &[Vec<u8>; 2],
    vocab: &Vocabularies,
    ctx: &PromptCtx,
    threshold: f64,
    date: String,
) -> Result<(Suggestion, Provenance), LabelFailure> {
    use base64::Engine as _;
    let images: Vec<String> = pngs
        .iter()
        .map(|p| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(p)))
        .collect();
    let mut responses = Vec::new();
    for request in build_requests(vocab, ctx, &images) {
        responses.push(ask(transport, &request).map_err(failure)?);
    }
    let model = responses
        .iter()
        .find_map(|r| r.model.clone())
        .ok_or_else(|| LabelFailure::Refused(format!("decision endpoint {} named no model", transport.endpoint())))?;
    let mut decided = suggest(vocab, &responses, threshold);
    if let Some(measured) = ctx.front_measured {
        decided.suggestion.front = measured;
    }
    Ok((decided.suggestion, Provenance { model, date, attempts: 1 }))
}

fn failure(e: Error) -> LabelFailure {
    match e {
        Error::Unreachable { .. } => LabelFailure::Unreachable {
            transport: e.to_string(),
            remedy: Some("set GRIMOIRE_DECIDE_ENDPOINT to a running decision endpoint (tools/decide/serve-clef-flash.sh)".into()),
        },
        other => LabelFailure::Refused(other.to_string()),
    }
}

/// **How well a suggestion agrees with a person's labels**: micro-averaged over the four axes, as
/// precision, recall and F1 of the set of `(axis, token)` pairs. Both paths are measured by this one
/// function, so "equal or better" compares like with like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agreement {
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
}

impl Agreement {
    pub fn f1(&self) -> f64 {
        let denom = 2 * self.true_positive + self.false_positive + self.false_negative;
        if denom == 0 { 1.0 } else { 2.0 * self.true_positive as f64 / denom as f64 }
    }
}

/// The labels a person gave, per axis, against what `s` suggests.
pub fn agreement(person: &Suggestion, s: &Suggestion) -> Agreement {
    let pairs = |x: &Suggestion| -> BTreeSet<(&'static str, String)> {
        [("kind", &x.kind), ("effects", &x.effects), ("look", &x.look), ("surfaces", &x.offers_surfaces)]
            .into_iter()
            .flat_map(|(a, v)| v.iter().map(move |t| (a, t.clone())))
            .collect()
    };
    let (want, got) = (pairs(person), pairs(s));
    let tp = want.intersection(&got).count();
    Agreement { true_positive: tp, false_positive: got.len() - tp, false_negative: want.len() - tp }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_grimoire_decide::{Answer, Replay};

    fn vocab() -> Vocabularies {
        Vocabularies {
            kind: Vocabulary::of(&[("seating", "to sit on"), ("light", "gives light")]),
            effects: Vocabulary::of(&[("emit", "makes light")]),
            look: Vocabulary::of(&[("brown", "brown"), ("rusted", "rusty")]),
            surfaces: Vocabulary::of(&[("table-top", "flat top")]),
            ..Vocabularies::default()
        }
    }

    fn ctx() -> PromptCtx {
        PromptCtx {
            id: "chair".into(),
            mesh: "chair.glb".into(),
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

    fn yes(p: f64) -> Answer {
        Answer::Noul { noul: p }
    }

    #[test]
    fn config_selects_the_path() {
        let get = |v: &'static str| move |k: &str| (k == "GRIMOIRE_LABEL_PATH").then(|| v.to_owned());
        assert_eq!(LabelPath::from_lookup(|_| None), Ok(LabelPath::Chat));
        assert_eq!(LabelPath::from_lookup(get("chat")), Ok(LabelPath::Chat));
        assert_eq!(LabelPath::from_lookup(get("decide")), Ok(LabelPath::Decide));
        assert!(LabelPath::from_lookup(get("both")).unwrap_err().contains("`chat` or `decide`"));
    }

    #[test]
    fn a_token_the_project_lacks_is_never_suggested() {
        let v = vocab();
        let requests = build_requests(&v, &ctx(), &[]);
        assert_eq!(requests.len(), 1);
        let mut response = Response { model: Some("clef".into()), answers: BTreeMap::new(), usage: None };
        for id in requests[0].questions.keys() {
            response.answers.insert(id.clone(), yes(0.01));
        }
        response.answers.insert("kind:seating".into(), yes(0.97));
        // The model volunteers tokens nobody asked about, and a real axis with a foreign token.
        response.answers.insert("kind:throne".into(), yes(0.99));
        response.answers.insert("look:gilded".into(), yes(0.99));
        response.answers.insert("mood:grim".into(), yes(0.99));
        let d = suggest(&v, &[response], DEFAULT_THRESHOLD);
        assert_eq!(d.suggestion.kind, vec!["seating"]);
        assert!(d.suggestion.look.is_empty());
        assert!(d.probabilities.keys().all(|(a, t)| axis_vocab(&v, a).contains(t)));
    }

    #[test]
    fn a_vocabulary_larger_than_one_request_is_split_not_truncated() {
        let names: Vec<String> = (0..70).map(|i| format!("t{i}")).collect();
        let pairs: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
        let mut v = vocab();
        v.look = Vocabulary::of(&pairs);
        let requests = build_requests(&v, &ctx(), &[]);
        assert!(requests.len() >= 2);
        assert!(requests.iter().all(|r| r.questions.len() <= MAX_QUESTIONS));
        let asked: usize = requests.iter().map(|r| r.questions.len()).sum();
        assert_eq!(asked, 2 + 1 + 70 + 1 + 1 + 1); // + the `mount` and `front` choices
    }

    #[test]
    fn replayed_answers_become_a_stamped_suggestion() {
        let v = vocab();
        let pngs = [vec![1u8, 2, 3], vec![4u8, 5, 6]];
        let images: Vec<String> = {
            use base64::Engine as _;
            pngs.iter()
                .map(|p| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(p)))
                .collect()
        };
        let request = build_requests(&v, &ctx(), &images).remove(0);
        let mut answers = BTreeMap::new();
        for (id, q) in &request.questions {
            let answer = match q {
                // The `mount` and `front` choices (assumed owner exception: #113 adds them): answered
                // `unset`, which suggests nothing.
                Question::Choice { criteria, .. } => Answer::Choice {
                    choice: "unset".into(),
                    probabilities: criteria.keys().map(|k| (k.clone(), if k == "unset" { 0.9 } else { 0.01 })).collect(),
                    confidence: 0.9,
                },
                _ => yes(if id == "look:brown" || id == "kind:seating" { 0.95 } else { 0.03 }),
            };
            answers.insert(id.clone(), answer);
        }
        let response = Response { model: Some("clef-flash".into()), answers, usage: None };
        let transport = Replay::new(request, response);
        let (s, p) = label_with_decisions(&transport, &pngs, &v, &ctx(), DEFAULT_THRESHOLD, "2026-10-29".into()).unwrap();
        assert_eq!((s.kind, s.look), (vec!["seating".to_owned()], vec!["brown".to_owned()]));
        assert_eq!(s.confidence, Confidence::High);
        assert_eq!((p.model.as_str(), p.attempts), ("clef-flash", 1));
    }

    #[test]
    fn agreement_counts_pairs_across_axes() {
        let v = vocab();
        let mk = |kind: &[&str], look: &[&str]| {
            let mut r = Response { model: None, answers: BTreeMap::new(), usage: None };
            for t in kind {
                r.answers.insert(format!("kind:{t}"), yes(1.0));
            }
            for t in look {
                r.answers.insert(format!("look:{t}"), yes(1.0));
            }
            suggest(&v, &[r], 0.5).suggestion
        };
        let person = mk(&["seating"], &["brown", "rusted"]);
        let got = mk(&["seating", "light"], &["brown"]);
        let a = agreement(&person, &got);
        assert_eq!((a.true_positive, a.false_positive, a.false_negative), (2, 1, 1));
        assert!((a.f1() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(agreement(&person, &person).f1(), 1.0);
    }
}
