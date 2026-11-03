//! **The clip check** (#112): does a changelog clip show what its line claims?
//!
//! The loop judges its own clips by looking at them, and that judgement has been wrong more than once —
//! `AGENTS.md`'s changelog notes record a `tour.gif` captioned "at noon" over a HUD reading 10.00 h, and a
//! hero caption promising a flame catch its frames never show. The issue asks for the clip check to be a
//! *named check* with **its own labelled set**, built from the changelog's real clips and titles plus
//! mismatched pairs, and for its agreement to be measured and recorded.
//!
//! This module is the check's shape, and the part a test can prove without a model:
//!
//! * **What evidence it takes.** The still, and up to `FRAMES` frames from the MP4 — four images, which is
//!   the API's limit. A clip whose still or frames cannot be read is an error naming which; it is never
//!   answered from less.
//! * **What it asks.** One `noul`: *does what these frames show match the line's title?* The title goes in
//!   the state beside the images, because a question without the claim it is testing is a question a model
//!   can only answer by guessing.
//! * **What a labelled sample is.** A title with the frames of the clip that line points at, labelled
//!   `true` (they match) or `false` (a mismatched pair — the same frames under a different line's title).
//!   The mismatched half is what stops a check passing by always saying yes.
//!
//! **The measurement needs a served model** — `calibrate` over the labelled set — and this repository's
//! host has none (`127.0.0.1:8093` refuses). So the set is built here as data, and the calibration that
//! reads it is the next pass with a model's job.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::check::{Check, Evidence};
use crate::types::Request;

/// How many frames a clip check sends, besides the still: the API takes at most four images and the still
/// is the fourth.
pub const FRAMES: usize = 3;

/// The question id, so the answer is named in one place.
pub const QUESTION_ID: &str = "shows";

/// **The clip check**, as a [`Check`] a project's `checks.ron` carries.
///
/// `name` is the check's own name (`clip_shows_its_title`); the images it needs are named by
/// [`still`] and [`frame`], so a capture directory holds `still.png`, `frame0.png`, `frame1.png` and
/// `frame2.png`.
pub fn clip_check(name: &str) -> Check {
    let mut images = vec![still()];
    images.extend((0..FRAMES).map(frame));
    Check {
        name: name.to_owned(),
        // The question says what "yes" means, which is the crate's rule for every check: yes is a problem.
        question: "Do these frames fail to show what the changelog line says they show?".to_owned(),
        images,
        facts: false,
        // A clip that does not show its claim is a problem, so the threshold starts where "more likely
        // than not". It is advisory until the labelled set moves it.
        threshold: 0.5,
        after_edit: false,
        calibration: None,
    }
}

/// The still's evidence name.
pub fn still() -> String {
    "still".to_owned()
}

/// One frame's evidence name, by index.
pub fn frame(i: usize) -> String {
    format!("frame{i}")
}

/// **The request a clip check makes**, given the line it is testing.
///
/// The title is carried in the state rather than in the question, because the four images are one clip and
/// the claim is what makes the question answerable: *"does this show: <title>?"* is the issue's own wording.
pub fn request(check: &Check, title: &str, evidence: &Evidence) -> Result<Request, crate::Error> {
    let mut with_title = check.clone();
    with_title.question = format!(
        "Do these frames from one changelog clip fail to show what its line says they show? The line says: \
         {title}"
    );
    with_title.request(evidence)
}

/// **One labelled sample**: a line's title, and the four capture names that go with it.
///
/// `truth` is `true` when the frames really are the clip that line points at, and `false` for a mismatched
/// pair — a clip's frames under another line's title. The second is the half that stops a check which always
/// answers "yes, that is fine" from scoring 100 %.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Labelled {
    pub id: String,
    pub title: String,
    /// The directory holding `still.png`, `frame0.png`, … for this sample.
    pub dir: PathBuf,
    pub truth: bool,
}

/// **The labelled set, as a file beside the checks**: one line per sample, oldest first.
///
/// A set is data, so it is a file and not a constant: a person adding a mismatch runs the same path the
/// loop does, and a pass can see what a threshold was chosen from.
pub const LABELLED_FILE: &str = "clip_samples.ron";

/// Read a labelled set, refusing a file whose samples cannot be read as evidence.
pub fn load_labelled(path: &Path) -> Result<Vec<Labelled>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut samples: Vec<Labelled> =
        ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    // **A relative `dir` is relative to the manifest**, so a committed manifest works from any clone and
    // from any working directory. An absolute one is left alone, which is what a scratch set uses.
    if let Some(base) = path.parent() {
        for s in &mut samples {
            if s.dir.is_relative() {
                s.dir = base.join(&s.dir);
            }
        }
    }
    if samples.is_empty() {
        return Err(format!("{}: no samples; a set needs both a match and a mismatch", path.display()));
    }
    // **Both labels or nothing.** `any` and `all` are the two one-sided cases; a set needs one of each,
    // because a check measured only on matches scores 100 % by always saying "that is fine".
    let any_match = samples.iter().any(|s| s.truth);
    let any_mismatch = samples.iter().any(|s| !s.truth);
    if any_match && any_mismatch {
        return Ok(samples);
    }
    Err(format!(
        "{}: every sample is labelled the same way; a check measured only on matches cannot tell a \
         truthful clip from a lying one",
        path.display()
    ))
}

/// The evidence a sample's directory holds, by the names a clip check asks for.
///
/// A name the directory lacks is `Ok(None)`, so the check's own `MissingEvidence` names it once.
pub fn evidence(dir: &Path) -> Evidence {
    let mut images = BTreeMap::new();
    for name in std::iter::once(still()).chain((0..FRAMES).map(frame)) {
        if let Ok(bytes) = std::fs::read(dir.join(format!("{name}.png"))) {
            images.insert(name, super::b64::data_url(&bytes));
        }
    }
    Evidence { images, facts: None }
}

/// **Run the labelled set against a transport**, and return the samples it scored, ready for
/// [`crate::calibrate`].
///
/// This is the one function a calibration run needs and a pass without a model cannot finish: it makes the
/// request for each sample (the still, three frames and the title), asks, and returns P(yes) beside the
/// sample's truth. Keeping it here rather than in the binary is what lets the request and its evidence be
/// tested without a model.
pub fn score_set(
    check: &Check,
    samples: &[Labelled],
    transport: &dyn crate::Transport,
) -> Result<Vec<crate::Sample>, crate::Error> {
    let mut out = Vec::with_capacity(samples.len());
    for s in samples {
        let ev = evidence(&s.dir);
        let req = request(check, &s.title, &ev)?;
        let response = crate::ask(transport, &req)?;
        // **A missing answer is an error, not a panic** (#112). The model occasionally returns an
        // `answers` map without this question — a truncated generation, or a server that decided the
        // state was empty. Indexing panicked on the 30th sample of a calibration run and lost every
        // score already measured; naming it lets the caller say which sample and carry on.
        // **The id the request actually sent.** `Check::request` names its question with `check.rs`'s
        // `QUESTION_ID` (`"check"`); this module's `QUESTION_ID` (`"shows"`) is the *clip* question's name
        // in prose and was never what went on the wire. Reading `"shows"` out of an answer keyed
        // `"check"` failed on the first sample of a real calibration run.
        let p_yes = response
            .answers
            .get(crate::check::QUESTION_ID)
            .and_then(|a| a.p_yes())
            .ok_or_else(|| crate::Error::BadResponse {
                endpoint: transport.endpoint(),
                detail: format!(
                    "sample `{}` did not answer `{QUESTION_ID}` with a noul; it answered: {:?}",
                    s.id,
                    response.answers.keys().collect::<Vec<_>>()
                ),
            })?;
        out.push(crate::Sample { p_yes, truth: s.truth });
    }
    Ok(out)
}


#[cfg(test)]
mod tests {
    //! The shape, not the model: how many images, what is asked, and what a set with one label is refused.

    use super::*;

    #[test]
    fn a_clip_check_sends_a_still_and_three_frames_and_names_the_line_it_tests() {
        let check = clip_check("clip_shows_its_title");
        assert_eq!(check.images, vec!["still", "frame0", "frame1", "frame2"]);
        assert_eq!(check.images.len(), FRAMES + 1, "the API's four-image limit, with the still as the fourth");
        assert!(check.question.contains("fail to show"), "yes must mean a problem: {}", check.question);
    }

    #[test]
    fn the_request_carries_the_lines_title() {
        // The claim is part of the state: a model asked "does this match?" without it answers from nothing.
        let check = clip_check("clip");
        let mut images = BTreeMap::new();
        for n in std::iter::once(still()).chain((0..FRAMES).map(frame)) {
            images.insert(n, "data:image/png;base64,AA==".to_owned());
        }
        let ev = Evidence { images, facts: None };
        let req = request(&check, "a bench for the stallkeepers", &ev).expect("every image is there");
        // `Check::request` puts the question in the questions map; the title rides in it, because that is
        // what makes the answer mean anything.
        let asked = format!("{:?}", req.questions);
        assert!(asked.contains("a bench for the stallkeepers"), "questions: {asked}");
        assert_eq!(req.images.len(), 4);
    }

    /// **`score_set` reads the answer under the id the request sent** (#112).
    ///
    /// `Check::request` names its question with `check::QUESTION_ID`; this module's own `QUESTION_ID`
    /// is `"shows"` and was what `score_set` looked for. On the first real calibration run — 122 samples
    /// against a served clef-flash — every answer came back keyed `"check"`, so the lookup missed and the
    /// run panicked on sample one. A recording makes the mismatch visible without a model.
    #[test]
    fn a_scored_sample_reads_the_answer_the_request_asked_for() {
        use crate::Response;
        use std::collections::BTreeMap;

        let dir = std::env::temp_dir().join(format!("clip_score_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in std::iter::once(still()).chain((0..FRAMES).map(frame)) {
            std::fs::write(dir.join(format!("{name}.png")), b"\x89PNG not really").unwrap();
        }
        let samples = vec![Labelled { id: "s".into(), title: "a bench".into(), dir, truth: true }];

        // A response under the id the check actually sends, as the served model answers it.
        let mut answers = BTreeMap::new();
        answers.insert(
            crate::check::QUESTION_ID.to_owned(),
            crate::Answer::Noul { noul: 0.9 },
        );
        let response = Response { model: Some("test".into()), answers, usage: None };
        let asked = request(&clip_check("clip"), "a bench", &evidence(&samples[0].dir)).unwrap();
        let transport = crate::Replay::new(asked, response);

        let scored = score_set(&clip_check("clip"), &samples, &transport)
            .expect("an answer under `check` is the answer this check asked for");
        assert_eq!(scored.len(), 1);
        assert!(scored[0].truth, "the sample is a match and must score as one");
        assert!((scored[0].p_yes - 0.9).abs() < 1e-9, "p_yes was {}", scored[0].p_yes);
    }

    #[test]
    fn a_set_that_is_all_matches_or_all_mismatches_is_refused() {
        // A set with one label cannot calibrate: `agreement` would be 100 % whatever the model said.
        let dir = std::env::temp_dir().join(format!("clip_set_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LABELLED_FILE);
        let one = |truth: bool| {
            format!(
                "[(id: \"a\", title: \"t\", dir: \"d\", truth: {truth})]"
            )
        };
        std::fs::write(&path, one(true)).unwrap();
        assert!(load_labelled(&path).is_err(), "every sample a match must be refused");
        std::fs::write(&path, one(false)).unwrap();
        assert!(load_labelled(&path).is_err(), "every sample a mismatch must be refused");
        std::fs::write(
            &path,
            "[(id: \"a\", title: \"t\", dir: \"d\", truth: true), (id: \"b\", title: \"u\", dir: \"e\", truth: false)]",
        )
        .unwrap();
        assert_eq!(load_labelled(&path).unwrap().len(), 2, "one of each is a set");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
