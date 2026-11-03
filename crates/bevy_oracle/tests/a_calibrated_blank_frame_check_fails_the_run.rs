//! **A calibrated blank-frame check fails a run on a blank frame and passes on a good capture; an
//! uncalibrated check never fails** (#112's second and third acceptance items).
//!
//! *"`check.py` fails on a fixture blank frame with a calibrated blank-frame check, and passes on a
//! good capture (test with replay). An uncalibrated check never fails the run (test)."*
//!
//! The model is replayed, never called: `Replay` answers every question the way a recorded clef-flash
//! would, which is the repo's own rule ("tests never loop live VLM calls"). What is under test is the
//! **policy** — which finding may fail a run — and that policy is `bevy_oracle::run_checks`.
//!
//! The fixtures are two real frames: `fixtures/blank/view.png` is one flat colour with no scene in it,
//! and `fixtures/blank/good.png` is the same size with structure in it. The check is the built-in
//! `blank_frame` (`fixtures/checks.ron`), whose question is exactly "is the image blank".

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_oracle::{
    Answer, Check, Checks, Evidence, Request, Response, Transport, Verdict, calibrate, run_checks,
};

/// **The recorded answer**, keyed by the check's name, so one fixture serves both cases: a blank frame
/// the model called blank, and a good capture it called good. A live clef-flash answers in the same
/// shape; this is the playback of one.
struct Replay {
    /// P(yes) per check name.
    p: BTreeMap<String, f64>,
}

impl Transport for Replay {
    fn endpoint(&self) -> String {
        "replay".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        // One check runs per case here, so its single recorded P answers every question the request
        // carries. `Request` names no check: the state and the image are what it sends.
        let p = self.p.values().next().copied().unwrap_or(0.0);
        let answers = request
            .questions
            .keys()
            .map(|id| (id.clone(), Answer::Noul { noul: p }))
            .collect();
        Ok(Response { model: None, answers, usage: None })
    }
}

use bevy_oracle::Error;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(path)
}

/// The built-in checks with `blank_frame` decided by `p`, and a calibration when one is given.
fn checks(p: f64, calibration: Option<bevy_oracle::Calibration>) -> Checks {
    let mut set = Checks::builtin();
    set.checks.retain(|c| c.name == "blank_frame");
    set.checks[0].calibration = calibration;
    let _ = p;
    set
}

/// A blank frame's evidence: the one image `blank_frame` asks for.
fn evidence(image: &str) -> Evidence {
    let bytes = std::fs::read(fixture(&format!("blank/{image}.png"))).expect("the fixture PNG");
    let mut images = BTreeMap::new();
    images.insert("view".to_owned(), bevy_oracle::data_url(&bytes));
    Evidence { images, facts: None }
}

/// A calibration that has earned the right to block: agreement well over the 0.9 bar.
fn earned() -> bevy_oracle::Calibration {
    let samples: Vec<bevy_oracle::Sample> = (0..20)
        .map(|i| bevy_oracle::Sample { p_yes: if i % 2 == 0 { 0.9 } else { 0.1 }, truth: i % 2 == 0 })
        .collect();
    calibrate("blank_frame", "clef-flash", "2026-11-03", &samples).expect("a two-sided set calibrates")
}

#[test]
fn a_calibrated_blank_frame_check_fails_the_run_and_passes_on_a_good_capture() {
    // **The blank frame.** The model said blank (P = 0.97, over the check's own 0.70 threshold).
    let blank = run_checks(
        &checks(0.0, Some(earned())),
        &Replay { p: BTreeMap::from([("blank_frame".to_owned(), 0.97)]) },
        &|_: &Check| Ok(evidence("view")),
        0.9,
    );
    assert!(
        blank.fails(),
        "a calibrated blank_frame at 0.97 on a blank frame must fail the run: {:?}",
        blank.blocking
    );

    // **The good capture.** The same check, the same calibration, P = 0.02: no finding, no failure.
    let good = run_checks(
        &checks(0.0, Some(earned())),
        &Replay { p: BTreeMap::from([("blank_frame".to_owned(), 0.02)]) },
        &|_: &Check| Ok(evidence("good")),
        0.9,
    );
    assert!(
        !good.fails() && good.blocking.is_empty(),
        "a good capture must pass: {:?}",
        good.blocking
    );
    assert_eq!(
        good.findings.len(),
        1,
        "the check ran and answered; it was not skipped, which would make this vacuous"
    );
}

#[test]
fn the_same_blank_frame_from_an_uncalibrated_check_never_fails_the_run() {
    // **No calibration**, the same P = 0.97 on the same blank frame: it is named, and it cannot block.
    let verdict: Verdict = run_checks(
        &checks(0.0, None),
        &Replay { p: BTreeMap::from([("blank_frame".to_owned(), 0.97)]) },
        &|_: &Check| Ok(evidence("view")),
        0.9,
    );
    assert!(!verdict.fails(), "an uncalibrated check never fails the run");
    assert!(verdict.blocking.is_empty());
    assert!(
        !verdict.advisory.is_empty(),
        "the finding is still printed as advisory, or the person loses it"
    );
}
