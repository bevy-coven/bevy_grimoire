//! **How a mesh is labelled: typed questions to a decision model** (#113, D5). Each vocabulary token
//! of an axis (`kind`, `effects`, `look`, `surfaces`) becomes one `noul` question, "does this asset
//! have this token?", and the model answers P(yes) for each. Nothing is generated and nothing is
//! parsed, so there is no prompt to write, no schema to tolerate and no reprompt loop.
//!
//! **A token the project lacks can never be suggested**: the question set is built from the
//! vocabulary and [`suggest`] reads back only the ids it built, so an answer for any other id is
//! ignored rather than trusted.
//!
//! **One path** (owner, 2026-11-11). D5 asked for the old chat path to be removed once this one
//! measured equal or better on a labelled kit. The only measurement that ever stood found the two
//! **wired** paths reading one recorded judgement the same way — every axis, token for token
//! (`the_two_paths_read_one_judgement_the_same_way`); the 92%-each percentage this file's history
//! mentions was withdrawn with pass 277's duplicate labeller and never re-made, because it would
//! need a live chat model. The owner approved the removal without it on 2026-11-11, so the chat path
//! is deleted in the same change as this note: there is no configuration that selects between two
//! labellers and none that falls back. The endpoint is one per run, from `GRIMOIRE_DECIDE_*`
//! ([`bevy_oracle::Config`]).

use crate::label_suggestion::{LabelFailure, PromptCtx, Provenance, Suggestion, face_token, mount_meaning};
use bevy_grimoire_core::descriptor::{Confidence, Face, Mount, mount_label, mount_options};
use bevy_grimoire_core::vocab::{Vocabularies, Vocabulary};
use bevy_oracle::{Config, Error, MAX_QUESTIONS, Question, Request, Response, Transport, ask};
use std::collections::{BTreeMap, BTreeSet};

/// **Where the decisions are asked.** One endpoint per run: the process environment first, then the
/// project root's `.env`, which is what a session loads on start and what a project carries with it.
/// There is no fallback and no second endpoint to fall back to.
pub fn endpoint_of(root: &std::path::Path) -> Config {
    let file = crate::label_suggestion::dotenv(root);
    Config::from_lookup(|k| std::env::var(k).ok().filter(|v| !v.is_empty()).or_else(|| file.get(k).cloned()))
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
        // **No identification, because this path does not make one.** It answers a probability per
        // token, so the only thing it could put here is the token list — and `what` is a SENTENCE
        // that `Suggestion::description` offers as the piece's description when `note` is empty.
        // Writing the tag list there meant every apply overwrote an author's description with
        // `"seating, light"` and stamped `Axis::Note` as the model's; the tokens belong on `kind`,
        // where a reader already looks for them, and this stays empty (review of #113).
        what: String::new(),
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

/// Label one mesh through `transport`. The provenance names **the model that answered**, off the
/// response's own `model` field rather than the name the config asked for; an endpoint that names
/// none is refused.
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
        let endpoint = transport.endpoint();
        responses.push(ask(transport, &request).map_err(|e| failure(&endpoint, e))?);
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

/// **Why a request produced none**, and what to do about it — where the remedy depends on where
/// the endpoint is **not**.
///
/// A refusal against a machine on this LAN is about the process: start the service, or point the
/// setting at the machine that has it. A refusal out on the internet folds DNS, TLS, a proxy and
/// the remote's own health into one verdict, and local advice there sends the reader at the wrong
/// layer — so a remote endpoint gets the transport's own words and nothing else. This is the same
/// judgement [`probe`] makes when it declines to dial a remote address at all, and it is why
/// [`remedy_for`] exists rather than one sentence for every failure.
fn failure(endpoint: &str, e: Error) -> LabelFailure {
    match e {
        Error::Unreachable { .. } => LabelFailure::Unreachable {
            transport: e.to_string(),
            remedy: remedy_for(endpoint),
        },
        other => LabelFailure::Refused(other.to_string()),
    }
}

/// **What to do about a refused connection to this URL**, or `None` when the answer would be a
/// guess. `None` for an address [`is_near`] rejects: out there the transport's own words are the
/// better message, and naming a local service would send the reader at the wrong layer.
fn remedy_for(url: &str) -> Option<String> {
    let (host, port) = host_port(url)?;
    use std::net::ToSocketAddrs;
    let ip = (host.as_str(), port).to_socket_addrs().ok()?.next()?.ip();
    is_near(ip).then(|| {
        format!(
            "set GRIMOIRE_DECIDE_ENDPOINT to a running decision endpoint \
             (tools/decide/serve-clef-flash.sh) — nothing is serving {host}:{port}"
        )
    })
}

/// **Is anybody home, and are they ready?**
///
/// Three answers, because two of the failures want opposite responses from the caller and lumping
/// them together is what made a batch of 778 meshes fail 778 times in a row:
///
/// - [`Reach::Ready`] — the socket answered.
/// - [`Reach::Warming`] — something is listening but not serving yet. A cold model takes tens of
///   seconds to load, so this is an ordinary state on the first request of a session, not a fault.
///   The caller waits; it does not cancel.
/// - [`Reach::Unreachable`] — nothing is listening. The remedy travels with the verdict rather than
///   being left for the reader to remember.
///
/// **A TCP connect, not a question.** The question is whether the endpoint exists, and asking it
/// with a real decision costs a model load and up to the configured timeout to learn something a
/// refused socket says instantly.
///
/// **A machine on this LAN is worth probing; a machine on the internet is not.** A refusal out there
/// folds DNS, TLS, proxies and the remote's own health into one verdict, so the address is resolved
/// and the probe is skipped when it is not loopback or private — the request's own error is the
/// better message then. It is decided on the resolved address rather than the spelling of the host,
/// so a name that maps to the LAN is treated as the LAN.
#[derive(Debug, Clone, PartialEq)]
pub enum Reach {
    Ready,
    Warming(String),
    Unreachable(String),
}

/// **The preflight.** Cheap, synchronous, and run before a batch commits to a queue.
pub fn probe(cfg: &Config) -> Reach {
    let Some((host, port)) = host_port(&cfg.endpoint) else {
        // An address this cannot parse is not evidence of anything; let the real request judge it.
        return Reach::Ready;
    };
    use std::net::ToSocketAddrs;
    let Ok(mut addrs) = (host.as_str(), port).to_socket_addrs() else {
        return Reach::Unreachable(format!(
            "`{host}` does not resolve — set GRIMOIRE_DECIDE_ENDPOINT to the machine serving the model"
        ));
    };
    let Some(addr) = addrs.next() else {
        return Reach::Unreachable(format!("`{host}` resolves to nothing"));
    };
    if !is_near(addr.ip()) {
        return Reach::Ready;
    }
    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(400)) {
        Ok(_) => Reach::Ready,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            Reach::Unreachable(format!(
                "nothing is listening on {host}:{port} — start the decision service there \
                 (tools/decide/serve-clef-flash.sh), or point GRIMOIRE_DECIDE_ENDPOINT at the \
                 machine that is serving it"
            ))
        }
        // Reachable-but-not-answering. A host that is up with a model still loading times out here,
        // and that is a wait rather than a fault.
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Reach::Warming(format!(
            "{host}:{port} is slow to answer — the model may still be loading"
        )),
        Err(e) => Reach::Unreachable(format!(
            "{host}:{port} cannot be reached: {e} — check GRIMOIRE_DECIDE_ENDPOINT"
        )),
    }
}

/// The host and port a URL dials, for the connect probe. `None` when the URL is not one this can
/// take apart — in which case the caller should just try the request rather than refuse on a guess.
pub fn host_port(url: &str) -> Option<(String, u16)> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?']).next().unwrap_or(rest);
    let https = url.starts_with("https://");
    // Strip any userinfo, then split host from port. IPv6 literals are not dialled by this tool.
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    match authority.rsplit_once(':') {
        Some((host, port)) => port.parse().ok().map(|p| (host.to_owned(), p)),
        None => Some((authority.to_owned(), if https { 443 } else { 80 })),
    }
}

/// **Is this address one a refused connection tells the truth about?**
///
/// Loopback and this LAN, yes: nothing in between can turn a running service into a refusal, so
/// "refused" means "not serving" and the preflight is worth its 400 ms. Anything further away, no.
fn is_near(ip: std::net::IpAddr) -> bool {
    ip.is_loopback()
        || match ip {
            std::net::IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
            std::net::IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
        }
}

/// **Load the model, so a batch does not pay for it inside its first real request.**
///
/// [`probe`] answers "is the socket open"; this answers "is the model resident". They are different
/// questions with very different costs: a TCP connect is instant, and a cold 20–30 GB model takes
/// long enough that the load was once being paid *inside* mesh 1 of 778 and read as a hang.
///
/// So the wait is spent here, once, where the caller can say what it is waiting for. The request is
/// the smallest one the endpoint will accept: one question, no images. A `noul` needs no state to
/// answer, and the content is irrelevant to the point, which is to make the server load the model.
pub fn warm(transport: &dyn Transport) -> Result<(), LabelFailure> {
    let request = Request::new("ready").with_question("ready", Question::noul("Answer yes."));
    ask(transport, &request).map_err(|e| failure(&transport.endpoint(), e))?;
    Ok(())
}

/// **How well a suggestion agrees with a person's labels**: micro-averaged over the four axes, as
/// precision, recall and F1 of the set of `(axis, token)` pairs. It is the measurement D5's decision
/// to keep one path rests on, and it is kept so a recalibration on a new kit is the same arithmetic.
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
    use bevy_oracle::{Answer, Replay};

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

    /// **One path, and no setting that selects another.**
    ///
    /// The chat path is gone (owner, 2026-11-11), and what is left to pin is the *absence*: a project
    /// whose `.env` still says `GRIMOIRE_LABEL_PATH=chat` — every project the old default touched —
    /// must label through the decision endpoint, not silently fall back to a path that no longer
    /// exists. `GRIMOIRE_LABEL_PATH` is not read anywhere any more, so the value is inert.
    #[test]
    fn no_setting_selects_a_second_path() {
        let dir = std::env::temp_dir().join(format!("label_path_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.join(".env"), "GRIMOIRE_LABEL_PATH=chat\n")
            .unwrap_or_else(|e| panic!("{e}"));
        let endpoint = endpoint_of(&dir);
        assert_eq!(
            endpoint.endpoint,
            bevy_oracle::Config::from_lookup(|_| None).endpoint,
            "`GRIMOIRE_LABEL_PATH=chat` selected nothing: the decision endpoint is the one path"
        );
        std::fs::remove_dir_all(&dir).ok();
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

    /// **The address the probe dials**, including the shapes that made a naive split wrong.
    #[test]
    fn a_url_yields_the_host_and_port_it_dials() {
        assert_eq!(
            host_port("http://127.0.0.1:8093/v1/systemone"),
            Some(("127.0.0.1".to_owned(), 8093))
        );
        // No port: the scheme decides, which is the case a Cloudflare endpoint takes.
        assert_eq!(
            host_port("https://api.cloudflare.com/x"),
            Some(("api.cloudflare.com".to_owned(), 443))
        );
        assert_eq!(host_port("http://example.test/v1"), Some(("example.test".to_owned(), 80)));
        // A path containing a colon must not be read as a port.
        assert_eq!(host_port("http://localhost:8093/a:b"), Some(("localhost".to_owned(), 8093)));
    }

    /// **Nothing listening on a local port is a REFUSAL with a remedy**, not a mystery.
    ///
    /// A batch reported the same transport failure once per queued mesh — 778 of them — because it
    /// only ever asked whether the endpoint was *configured*. Port 1 is reserved and never
    /// listening, which is a refusal every platform agrees on.
    #[test]
    fn a_dead_local_port_is_unreachable_and_says_how_to_fix_it() {
        let cfg = Config { endpoint: "http://127.0.0.1:1/v1/systemone".to_owned(), ..Config::default() };
        match probe(&cfg) {
            Reach::Unreachable(why) => {
                assert!(why.contains("GRIMOIRE_DECIDE_ENDPOINT"), "the remedy travels with the verdict: {why}");
                assert!(why.contains("serve-clef-flash"), "and names how to serve it: {why}");
            }
            other => panic!("a dead port must refuse, got {other:?}"),
        }
    }

    /// **A remote endpoint is not probed by connecting.** DNS and TLS make a refusal mean several
    /// different things there, and the request's own error is the better message — so a batch is
    /// never blocked by a probe that cannot be trusted.
    #[test]
    fn a_remote_endpoint_is_left_to_the_request_itself() {
        let cfg = Config {
            endpoint: "https://api.cloudflare.com/client/v4/accounts/x/ai/run/@cf/cloudflare/clef".to_owned(),
            ..Config::default()
        };
        assert_eq!(probe(&cfg), Reach::Ready, "no connect probe for a remote host");
    }

    /// **A listening socket is ready**, whatever it later says about the model.
    #[test]
    fn something_listening_reads_as_ready() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| panic!("cannot bind a local test socket: {e}"));
        let port = listener.local_addr().unwrap_or_else(|e| panic!("no local addr: {e}")).port();
        let cfg = Config { endpoint: format!("http://127.0.0.1:{port}/v1/systemone"), ..Config::default() };
        assert_eq!(probe(&cfg), Reach::Ready);
    }

    /// **The preflight covers loopback and this LAN, and not the internet.**
    #[test]
    fn the_preflight_covers_loopback_and_this_lan_but_not_the_internet() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
        let v4 = |a, b, c, d| IpAddr::V4(Ipv4Addr::new(a, b, c, d));
        for near in [
            v4(127, 0, 0, 1),
            v4(192, 168, 1, 113), // bmb
            v4(10, 0, 0, 5),
            v4(172, 16, 4, 1),
            v4(169, 254, 3, 2), // link-local, which is what a .local name can land on
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ] {
            assert!(is_near(near), "{near} is reachable without leaving the house");
        }
        for far in [v4(1, 1, 1, 1), v4(104, 18, 0, 1), v4(172, 32, 0, 1)] {
            assert!(!is_near(far), "{far} is on the internet — its refusal means too many things to preflight on");
        }
    }

    /// **A LAN host that is down is NOT reported ready.** The preflight decided what to dial by the
    /// spelling of the host once, so `192.168.…` answered `Ready` without a byte sent and a batch
    /// went back to discovering a dead endpoint one mesh at a time.
    #[test]
    fn a_lan_host_that_is_down_is_not_reported_as_ready() {
        // **A port that was open and is now closed**, so a connect to it is refused.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port to borrow");
        let port = listener.local_addr().expect("the borrowed port").port();
        drop(listener);
        let cfg = Config {
            endpoint: format!("http://192.168.1.113:{port}/v1/systemone"),
            ..Config::default()
        };
        match probe(&cfg) {
            Reach::Ready => panic!(
                "the preflight said a LAN host on a closed port was READY, without sending a single \
                 byte. The endpoint is down, and the batch would discover that one mesh at a time."
            ),
            Reach::Unreachable(_) | Reach::Warming(_) => {}
        }
    }

    /// **The warm-up asks one question and nothing else**, so a cold model is loaded before a batch
    /// pays for it inside mesh 1 — and an endpoint that is down refuses there, once, with the remedy.
    #[test]
    fn the_warm_up_asks_one_question_and_names_an_unreachable_endpoint() {
        let request = Request::new("ready").with_question("ready", Question::noul("Answer yes."));
        let response = Response {
            model: Some("clef-flash".into()),
            answers: BTreeMap::from([("ready".to_owned(), yes(0.9))]),
            usage: None,
        };
        let transport = Replay::new(request, response);
        warm(&transport).expect("an answered warm-up is a warm model");

        struct Down;
        impl Transport for Down {
            fn endpoint(&self) -> String {
                "http://127.0.0.1:1/v1/systemone".into()
            }
            fn send(&self, _: &Request) -> Result<Response, Error> {
                Err(Error::Unreachable {
                    endpoint: self.endpoint(),
                    detail: "Connection refused".into(),
                })
            }
        }
        let e = warm(&Down).expect_err("a dead endpoint must refuse the warm-up");
        assert!(e.is_unreachable(), "so the batch stops rather than reporting it per mesh: {e}");
        assert!(e.to_string().contains("GRIMOIRE_DECIDE_ENDPOINT"), "{e}");
    }

    /// **A refusal against a remote endpoint must not be handed local advice.**
    ///
    /// Every `Error::Unreachable` used to get the same remedy — *"set GRIMOIRE_DECIDE_ENDPOINT to a
    /// running decision endpoint (tools/decide/serve-clef-flash.sh)"* — and for a Cloudflare or
    /// Ollama endpoint that is advice about a machine the reader is not on. It is the same judgement
    /// [`probe`] already makes by refusing to dial a remote address at all: out there a refusal folds
    /// DNS, TLS, proxies and the remote's own health together, and the transport's own words are the
    /// better message.
    ///
    /// This was `vlm.rs`'s `a_refusal_names_its_remedy_wherever_it_is_met` — *"a cloud endpoint must
    /// not be handed local advice"* — which went with the chat path although the rule is not about
    /// chat at all.
    #[test]
    fn a_remote_endpoint_is_never_handed_local_advice() {
        struct Down(&'static str);
        impl Transport for Down {
            fn endpoint(&self) -> String {
                self.0.to_owned()
            }
            fn send(&self, _: &Request) -> Result<Response, Error> {
                Err(Error::Unreachable { endpoint: self.0.to_owned(), detail: "Connection refused".into() })
            }
        }

        // A local endpoint: the remedy names the setting and how to serve one.
        let local = label_with_decisions(
            &Down("http://127.0.0.1:8093/v1/systemone"),
            &[vec![1u8], vec![2u8]],
            &vocab(),
            &ctx(),
            DEFAULT_THRESHOLD,
            "2026-11-11".to_owned(),
        )
        .expect_err("a dead endpoint labels nothing");
        let LabelFailure::Unreachable { remedy: Some(remedy), .. } = &local else {
            panic!("a local refusal is unreachable WITH a remedy, got {local:?}");
        };
        assert!(remedy.contains("GRIMOIRE_DECIDE_ENDPOINT"), "{remedy}");
        assert!(remedy.contains("serve-clef-flash"), "{remedy}");

        // A remote endpoint: the transport's own words, and no local advice at all.
        let remote = label_with_decisions(
            &Down("https://api.cloudflare.com/client/v4/accounts/x/ai/run/@cf/cloudflare/clef"),
            &[vec![1u8], vec![2u8]],
            &vocab(),
            &ctx(),
            DEFAULT_THRESHOLD,
            "2026-11-11".to_owned(),
        )
        .expect_err("a dead endpoint labels nothing");
        let LabelFailure::Unreachable { remedy, .. } = &remote else {
            panic!("an unreachable is unreachable, got {remote:?}");
        };
        assert!(
            remedy.is_none(),
            "a remote endpoint was handed local advice: {remedy:?}"
        );
        assert!(
            !remote.to_string().contains("serve-clef-flash")
                && !remote.to_string().contains("GRIMOIRE_DECIDE_ENDPOINT"),
            "and the sentence carries none either: {remote}"
        );
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
