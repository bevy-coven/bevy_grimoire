//! **A calibrated check can fail a run and an uncalibrated one cannot** (#112's three rules).
//!
//! The issue asks for a checker that runs named checks over captures and *"exits non-zero only when a
//! calibrated check, one at or above the bar, finds a failure. Uncalibrated checks only print."*
//!
//! That sentence is a policy, and a policy in a shell script is a thing nobody can prove. So it lives in
//! [`bevy_oracle::run_checks`] and this file is its statement: four runs over the same two checks,
//! with a recorded transport, covering the cases that decide whether a gate is safe to add a check to.
//!
//! Every test answers from a stand-in transport — no network, no model. The number it returns is a
//! constant this file states, so a threshold in a test reads as arithmetic rather than as a magic number
//! read off a recording.

use std::collections::BTreeMap;

use bevy_oracle::{Answer, Calibration, Check, Checks, Error, Evidence, Request, Response, Transport, Verdict, evidence_from, run_checks};

/// **The P(yes) every test is written against**, a constant so the thresholds read as arithmetic.
const P_YES: f64 = 0.8;

/// **A stand-in for the model that answers every `noul` with one probability.**
///
/// `Replay` is right for an exchange that must match byte for byte; this file varies the question per
/// test, so it answers by shape instead. It is still not a model: nothing leaves the process.
struct Fixed(f64);

impl Transport for Fixed {
    fn endpoint(&self) -> String {
        "fixed".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let answers = request.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: self.0 })).collect();
        Ok(Response { model: None, answers, usage: None })
    }
}

fn replay() -> Fixed {
    Fixed(P_YES)
}

fn calibration(agreement: f64) -> Calibration {
    Calibration {
        check: "floating".into(),
        model: "clef-flash".into(),
        set_size: 1,
        agreement,
        threshold: 0.5,
        date: "2026-11-02".into(),
    }
}

/// A check that found something, calibrated or not, and one that did not.
///
/// The question is asked of the recorded exchange, so both checks produce the same P(yes); what differs
/// is `agreement` (their authority) and `threshold` (whether that P is a finding).
fn checks(threshold: f64, agreement: Option<f64>) -> Checks {
    Checks {
        version: bevy_oracle::CHECKS_VERSION,
        checks: vec![Check {
            name: "floating".into(),
            question: "Is the piece floating?".into(),
            images: vec!["after".into()],
            facts: false,
            threshold,
            after_edit: false,
            calibration: agreement.map(calibration),
        }],
    }
}

fn evidence(_: &Check) -> Result<Evidence, String> {
    let mut images = BTreeMap::new();
    images.insert("after".to_owned(), "data:image/png;base64,AAAA".to_owned());
    Ok(evidence_from(&images, None))
}

fn run(checks: &Checks) -> Verdict {
    run_checks(checks, &replay(), &evidence, 0.9)
}

/// **The recorded exchange's P(yes)**, read once so the thresholds below are stated against it rather
/// than against a guessed number.
fn p_yes() -> f64 {
    P_YES
}

#[test]
fn a_calibrated_check_that_finds_something_fails_the_run() {
    let p = p_yes();
    // A threshold under the recorded P means "found"; the calibration is above the bar.
    let verdict = run(&checks(p - 0.01, Some(1.0)));
    assert!(verdict.findings[0].found, "the check found something at threshold {:.2}", p - 0.01);
    assert_eq!(verdict.blocking.len(), 1, "a calibrated check at the bar may block");
    assert!(verdict.fails(), "so the run fails");
}

#[test]
fn the_same_finding_from_an_uncalibrated_check_only_prints() {
    let p = p_yes();
    let verdict = run(&checks(p - 0.01, None));
    assert!(verdict.findings[0].found, "the check found the same thing");
    assert!(verdict.blocking.is_empty(), "but a check that has not earned its authority cannot block");
    assert_eq!(verdict.advisory.len(), 1);
    assert!(!verdict.fails(), "an uncalibrated check never fails the run");
    assert!(verdict.report().contains("advisory"), "it is printed: {}", verdict.report());
}

#[test]
fn a_calibrated_check_below_the_bar_only_prints() {
    let p = p_yes();
    // Calibrated, but its agreement is under the 0.9 bar this run uses.
    let verdict = run(&checks(p - 0.01, Some(0.4)));
    assert!(verdict.findings[0].found);
    assert!(verdict.blocking.is_empty(), "0.4 agreement is below the 0.9 bar");
    assert!(!verdict.fails(), "so it cannot fail a gate yet");
}

#[test]
fn a_calibrated_check_that_finds_nothing_does_not_fail_the_run() {
    let p = p_yes();
    let verdict = run(&checks(p + 0.01, Some(1.0)));
    assert!(!verdict.findings[0].found, "nothing found at threshold {:.2}", p + 0.01);
    assert!(verdict.blocking.is_empty());
    assert!(!verdict.fails());
}

#[test]
fn a_check_whose_capture_is_missing_is_an_error_and_never_a_failure() {
    let p = p_yes();
    let checks = checks(p - 0.01, Some(1.0));
    // No images at all: `Check::request` refuses rather than answering from less.
    let none = |_: &Check| Ok(evidence_from(&BTreeMap::new(), None));
    let verdict = run_checks(&checks, &replay(), &none, 0.9);
    assert!(verdict.findings.is_empty(), "nothing was found, because nothing was asked");
    assert_eq!(verdict.errors.len(), 1, "the missing capture is named");
    assert!(verdict.errors[0].1.contains("after"), "and says which: {:?}", verdict.errors);
    assert!(!verdict.fails(), "an error is not evidence of a problem in the picture");
}

/// **The committed clip set reads, has both labels, and names every image it needs.**
///
/// The set is data (`fixtures/clip_samples.ron`), so the things that can be wrong with it are the things a
/// file can do: not parse, be one-sided, or point at images that are not there. The first two are refused by
/// the loader; the third is checked here against the manifest's own directory, which is the one thing a
/// test can verify without the clip images (they are extracted on demand, not committed).
#[test]
fn the_committed_clip_set_is_two_sided_and_well_formed() {
    use bevy_oracle::{LABELLED_FILE, load_labelled};
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(LABELLED_FILE);
    let set = load_labelled(&manifest).unwrap_or_else(|e| panic!("{e}"));
    assert!(set.len() >= 20, "a set this small cannot measure a check: {} samples", set.len());
    let matches = set.iter().filter(|s| s.truth).count();
    let mismatches = set.len() - matches;
    assert!(matches > 0 && mismatches > 0, "{matches} matches, {mismatches} mismatches");
    assert!(
        (matches as f64 / set.len() as f64 - 0.5).abs() < 0.2,
        "a set that is {matches}/{len} puts a check's false-negative rate and its false-positive rate on \
         very different footings; this one should be about half and half",
        len = set.len()
    );
    // Every sample's title is a whole line, not an id: a title that is `268-warm` tests nothing.
    for s in &set {
        assert!(s.title.len() > 20, "`{}` has a title that is not a line: {:?}", s.id, s.title);
    }
    // And a mismatch is a *different* title from the one its frames belong to.
    for s in set.iter().filter(|s| !s.truth) {
        let own = s.id.strip_suffix("-mismatch").unwrap_or(&s.id);
        let twin = set.iter().find(|t| t.id == own && t.truth).expect("every mismatch has its match");
        assert_ne!(s.title, twin.title, "`{}` is labelled a mismatch but carries its own line's title", s.id);
    }
}
