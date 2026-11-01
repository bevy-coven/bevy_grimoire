use crate::Error;
use serde::{Deserialize, Serialize};

/// One labelled example: the model's P(yes) and the truth a person labelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub p_yes: f64,
    pub truth: bool,
}

/// What a check earned. Written beside the check; only a check at or above the bar may block the gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub check: String,
    pub model: String,
    pub set_size: usize,
    /// Fraction of the set the threshold classifies as labelled, 0..=1.
    pub agreement: f64,
    /// P(yes) at or above this is a yes.
    pub threshold: f64,
    pub date: String,
}

impl Calibration {
    /// Whether this check may block the loop's gate: its agreement meets `bar`.
    pub fn is_blocking(&self, bar: f64) -> bool {
        self.agreement >= bar
    }
}

/// Pick the threshold that best separates yes from no on a labelled set.
///
/// Candidates are 0, 1 (stricter than any P) and the midpoints between neighbouring distinct scores;
/// the one with the most agreements wins. Ties go to the candidate nearest 0.5, so a set that any
/// threshold in a range separates equally gets the one least sensitive to drift. A set with no
/// samples, a NaN score or only one label cannot calibrate: a threshold from one class is meaningless.
pub fn calibrate(
    check: &str,
    model: &str,
    date: &str,
    samples: &[Sample],
) -> Result<Calibration, Error> {
    if samples.is_empty() {
        return Err(Error::Calibration(format!("check `{check}` has no labelled samples")));
    }
    if samples.iter().any(|s| !(0.0..=1.0).contains(&s.p_yes)) {
        return Err(Error::Calibration(format!("check `{check}` has a P(yes) outside 0..=1")));
    }
    let yes = samples.iter().filter(|s| s.truth).count();
    if yes == 0 || yes == samples.len() {
        return Err(Error::Calibration(format!(
            "check `{check}`: every sample is labelled {}; a threshold needs both yes and no",
            if yes == 0 { "no" } else { "yes" }
        )));
    }
    let mut ps: Vec<f64> = samples.iter().map(|s| s.p_yes).collect();
    ps.sort_by(|a, b| a.partial_cmp(b).expect("range-checked, so not NaN"));
    ps.dedup();
    let mut candidates = vec![0.0];
    candidates.extend(ps.windows(2).map(|w| (w[0] + w[1]) / 2.0));
    candidates.push(1.0 + f64::EPSILON);
    let agree = |t: f64| samples.iter().filter(|s| (s.p_yes >= t) == s.truth).count();
    let best = candidates
        .into_iter()
        .map(|t| (agree(t), t))
        .max_by(|a, b| a.0.cmp(&b.0).then((b.1 - 0.5).abs().partial_cmp(&(a.1 - 0.5).abs()).unwrap()))
        .expect("at least two candidates");
    Ok(Calibration {
        check: check.into(),
        model: model.into(),
        set_size: samples.len(),
        agreement: best.0 as f64 / samples.len() as f64,
        threshold: best.1.min(1.0),
        date: date.into(),
    })
}
