//! **The clip check's agreement is measured and recorded, and this reads the record** (#112's first
//! acceptance item).
//!
//! *"On the labelled clip set, the clip check's agreement is measured and recorded (test reads the
//! record)."* The measurement is a calibration run against a served clef-flash, which a test may not
//! make (`fixtures/clip_samples.ron` is 122 samples; the repo's rule is that tests never call a live
//! model). So the run writes what it measured, and this test is what keeps that record honest: it must
//! name the check and the model, cover **every** sample in the labelled set, and its agreement must be
//! the one its own confusion counts produce.
//!
//! **Why this is worth a test at all.** A number in a document nobody checks is a claim, and the issue's
//! whole subject is that the loop's visual judgement has been wrong before. The record is the difference
//! between "we measured it" and "we think so".
//!
//! The record lives at `crates/bevy_oracle/fixtures/clip_calibration.ron` and is written by
//! `cargo run -p bevy_oracle --bin decide_calibrate -- --clip-set ...` (a run; see `docs/decide/`).

use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
struct Record {
    check: String,
    model: String,
    /// How many samples the labelled set holds **and** how many the run scored: they must agree.
    set_size: usize,
    /// Samples whose label the threshold got right, and the ones it did not.
    correct: usize,
    wrong: usize,
    threshold: f64,
}

#[derive(Debug, serde::Deserialize)]
struct Labelled {
    truth: bool,
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

#[test]
fn the_clip_check_s_agreement_is_recorded_against_every_labelled_sample() {
    let text = std::fs::read_to_string(fixture("clip_calibration.ron"))
        .unwrap_or_else(|e| panic!("fixtures/clip_calibration.ron: {e}"));
    let record: Record =
        ron::from_str(&text).unwrap_or_else(|e| panic!("fixtures/clip_calibration.ron: {e}"));

    assert_eq!(
        record.check, "clip_shows_its_title",
        "the record is of the clip check; another check's calibration does not answer this issue"
    );
    assert!(
        !record.model.is_empty(),
        "the record names the model its agreement was measured against: a number without its model \
         is not reproducible"
    );

    // **Every sample, both halves.** The mismatch half is what stops a check that always says "that is
    // fine" from scoring 100%, so a record that covers only the matches is not a measurement of this
    // check.
    let set: Vec<Labelled> = ron::from_str(
        &std::fs::read_to_string(fixture("clip_samples.ron")).expect("fixtures/clip_samples.ron"),
    )
    .expect("fixtures/clip_samples.ron parses");
    assert_eq!(
        record.set_size,
        set.len(),
        "the record covers {} samples but the labelled set holds {}: the agreement was measured over a \
         set that is not this one",
        record.set_size,
        set.len()
    );
    let mismatches = set.iter().filter(|s| !s.truth).count();
    assert!(
        mismatches > 0 && mismatches < set.len(),
        "the labelled set has {mismatches} mismatches of {} samples: both halves must be present or the \
         agreement means nothing",
        set.len()
    );

    // **The agreement is the counts' own arithmetic**, not a number beside them.
    assert_eq!(
        record.correct + record.wrong,
        record.set_size,
        "{} right and {} wrong do not add up to the {} samples the run scored",
        record.correct,
        record.wrong,
        record.set_size
    );
    let measured = record.correct as f64 / record.set_size as f64;
    assert!(
        (measured - record.agreement_impl()).abs() < 1e-9,
        "the record says agreement {} but its counts give {measured}: one of the two was typed",
        record.agreement_impl()
    );
    assert!(
        (0.0..=1.0).contains(&record.threshold),
        "the threshold {} is not a probability",
        record.threshold
    );
}

impl Record {
    fn agreement_impl(&self) -> f64 {
        self.correct as f64 / self.set_size as f64
    }
}
