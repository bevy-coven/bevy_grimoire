//! **`tools/decide/check.py`'s engine, in Rust** (#112): run named checks over a directory of
//! captures and decide what a failure means.
//!
//! The issue asks for a command-line checker that:
//!
//! * runs named checks on a capture directory;
//! * exits non-zero **only** when a *calibrated* check finds a failure — a check at or above the bar;
//! * an uncalibrated check only prints.
//!
//! Those three rules are what this module decides, and they are decided here rather than in a script so
//! a test can prove them: a Python driver would be the only place the policy lived, and the repo's
//! rule is that a check which cannot fail is not a check. The CLI in `tools/decide/check.py` is a thin
//! front end over `run_checks` — it reads the evidence, calls this, and maps the verdict to an exit
//! code.
//!
//! **Nothing here calls a model.** `run_checks` takes a [`Transport`]; a test passes a [`Replay`], and
//! the CLI passes an [`Http`] only when it is asked to (`--ask`). That is the same split
//! `crate::Check::run` already draws, lifted to the whole set.

use std::collections::BTreeMap;

use crate::check::{Check, Checks, Evidence, Finding};
use crate::{Error, Transport};

/// **What a run of checks concluded**, and why it is or is not allowed to fail a gate.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// Every check that ran, in the order `Checks` holds them.
    pub findings: Vec<Finding>,
    /// Checks that could not run (a refused request, a missing capture), by name and why. These never
    /// fail a gate: an error is not evidence of a problem in the picture.
    pub errors: Vec<(String, String)>,
    /// The calibrated checks that found something — the only findings that may fail a run.
    pub blocking: Vec<Finding>,
    /// Findings from checks that have not earned the bar: printed, never fatal.
    pub advisory: Vec<Finding>,
    /// The bar a check must meet to block. Passed in so a test can move it without editing a constant.
    pub bar: f64,
}

impl Verdict {
    /// **Whether this run should fail.** True only for a calibrated check that found something.
    ///
    /// An uncalibrated finding is `advisory` and is printed; a missing capture is an error and is
    /// printed. That is the issue's rule, and it is the reason a fresh check can be added to a
    /// project without being able to break anyone's build on its first day.
    pub fn fails(&self) -> bool {
        !self.blocking.is_empty()
    }

    /// One line per finding, then one per error, for a person reading a terminal.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for f in &self.findings {
            let earned = if f.found { "FOUND" } else { "ok" };
            let authority = if self.blocking.iter().any(|b| b.check == f.check) {
                "blocking"
            } else {
                "advisory"
            };
            out.push_str(&format!(
                "{:10} {}: p(yes) {:.2} vs {:.2} ({authority})\n",
                earned, f.check, f.p_yes, f.threshold
            ));
        }
        for (name, why) in &self.errors {
            out.push_str(&format!("error      {name}: {why}\n"));
        }
        out
    }
}

/// **Run every check `Checks` holds against `evidence`.**
///
/// `bar` is the agreement a check must have measured to block (the issue's *"a check must earn its
/// authority"*); a project's `checks.yaml` carries its own calibration, and the caller decides the bar.
/// `evidence_for` names the images each check wants, because the CLI and a test load them differently
/// — a test holds them in memory, the CLI reads a directory.
pub fn run_checks(checks: &Checks, transport: &dyn Transport, evidence_for: &dyn Fn(&Check) -> Result<Evidence, String>, bar: f64) -> Verdict {
    let mut verdict = Verdict { findings: Vec::new(), errors: Vec::new(), blocking: Vec::new(), advisory: Vec::new(), bar };
    for check in &checks.checks {
        let evidence = match evidence_for(check) {
            Ok(e) => e,
            Err(why) => {
                verdict.errors.push((check.name.clone(), why));
                continue;
            }
        };
        match check.run(transport, &evidence) {
            Ok(finding) if finding.found => {
                if check.may_block(bar) {
                    verdict.blocking.push(finding.clone());
                } else {
                    verdict.advisory.push(finding.clone());
                }
                verdict.findings.push(finding);
            }
            Ok(finding) => verdict.findings.push(finding),
            Err(e) => verdict.errors.push((check.name.clone(), describe(&e))),
        }
    }
    verdict
}

/// The evidence a check asks for, from images already in hand.
///
/// A check names its images (`before`, `after`, …); a directory of captures is keyed by the same names.
/// A name the caller does not have is `Err` naming it, so a check never answers from less than it asked.
pub fn evidence_from(images: &BTreeMap<String, String>, facts: Option<serde_json::Value>) -> Evidence {
    Evidence { images: images.clone(), facts }
}

fn describe(e: &Error) -> String {
    match e {
        Error::MissingEvidence { check, missing } => format!("{check} needs {missing}"),
        other => format!("{other}"),
    }
}
