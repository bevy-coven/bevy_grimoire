use crate::{Calibration, Error, Question, Request, Transport, ask};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The editor's own checks, as shipped (`fixtures/checks.ron`). A project's `checks.ron` has the same shape.
pub const BUILTIN_CHECKS_RON: &str = include_str!("../fixtures/checks.ron");

/// The project's own checks, beside `kits.ron` and `rules.ron`.
pub const PROJECT_CHECKS_FILE: &str = "checks.ron";

/// A named yes/no question about what an operation changed, with what it needs and what it earned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    /// The yes/no question: yes means something is wrong.
    pub question: String,
    /// Names of the images it needs (`before`, `after`, `closeup`, `view`, ...), in the order sent.
    #[serde(default)]
    pub images: Vec<String>,
    /// Whether it needs the JSON facts (operations run, pieces, camera).
    #[serde(default)]
    pub facts: bool,
    /// P(yes) at or above this is a finding.
    pub threshold: f64,
    /// Run after each agent operation (never while a person is editing).
    #[serde(default)]
    pub after_edit: bool,
    /// Set once a labelled set of real captures measured it.
    #[serde(default)]
    pub calibration: Option<Calibration>,
}

/// The format version of `checks.ron` (#202). A file with no `version` is the first.
///
/// This crate has no `bevy_grimoire_core` dependency (it is engine-free and no game links it), so the two
/// small functions below say what `bevy_grimoire_core::format_version` says for the formats core owns.
pub const CHECKS_VERSION: u32 = 1;

/// The version of a file from before the version existed: the first.
fn first_version() -> u32 {
    1
}

/// A `version` that must be [`CHECKS_VERSION`]: any other number is refused, saying which version this
/// build reads, so an author knows a file from a newer release is not damaged.
fn read_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let found = u32::deserialize(deserializer)?;
    if found == CHECKS_VERSION {
        Ok(found)
    } else {
        Err(serde::de::Error::custom(format!("version {found} is not the {CHECKS_VERSION} this build reads")))
    }
}

/// The `checks.ron` file: checks by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checks {
    /// The format version: [`CHECKS_VERSION`]. A file from before it existed has none and reads as the
    /// first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "first_version", deserialize_with = "read_version")]
    pub version: u32,
    pub checks: Vec<Check>,
}

impl Default for Checks {
    fn default() -> Self {
        Checks { version: CHECKS_VERSION, checks: Vec::new() }
    }
}

impl Checks {
    pub fn from_ron(text: &str) -> Result<Self, Error> {
        let c: Checks = ron::from_str(text).map_err(|e| Error::Checks(e.to_string()))?;
        let mut seen = std::collections::BTreeSet::new();
        for check in &c.checks {
            if !seen.insert(check.name.as_str()) {
                return Err(Error::Checks(format!("two checks are named `{}`", check.name)));
            }
            if !(0.0..=1.0).contains(&check.threshold) {
                return Err(Error::Checks(format!("check `{}`: threshold {} is outside 0..=1", check.name, check.threshold)));
            }
            if check.images.len() > crate::MAX_IMAGES {
                return Err(Error::Checks(format!("check `{}` needs more than {} images", check.name, crate::MAX_IMAGES)));
            }
        }
        Ok(c)
    }

    pub fn to_ron(&self) -> Result<String, Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| Error::Checks(e.to_string()))
    }

    /// The editor's own checks.
    pub fn builtin() -> Self {
        Self::from_ron(BUILTIN_CHECKS_RON).expect("fixtures/checks.ron parses (a test reads it)")
    }

    /// The editor's checks with the project's own `assets/grimoire/checks.ron` over them, read fresh so an
    /// edit takes effect at once. No file is the editor's checks alone; an unreadable or invalid one is an error
    /// naming the path.
    pub fn for_project(project: &std::path::Path) -> Result<Self, String> {
        let path = project.join("assets").join("grimoire").join(PROJECT_CHECKS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Self::builtin().merged(Self::from_ron(&text).map_err(|e| format!("{}: {e}", path.display()))?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::builtin()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Check> {
        self.checks.iter().find(|c| c.name == name)
    }

    /// `other`'s checks over these: a game's check of the same name replaces the editor's.
    pub fn merged(mut self, other: Checks) -> Self {
        for c in other.checks {
            match self.checks.iter_mut().find(|m| m.name == c.name) {
                Some(slot) => *slot = c,
                None => self.checks.push(c),
            }
        }
        self
    }
}

/// What a check is run on: the captures by name (data URLs) and the JSON facts.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    pub images: BTreeMap<String, String>,
    pub facts: Option<serde_json::Value>,
}

/// One check's result. A finding is advisory: it never blocks or undoes an operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub p_yes: f64,
    pub threshold: f64,
    /// `p_yes >= threshold`.
    pub found: bool,
}

impl Finding {
    /// "floating 0.93 > 0.70", as the journal writes it; `None` when nothing was found.
    pub fn message(&self) -> Option<String> {
        self.found.then(|| format!("{} {:.2} > {:.2}", self.check, self.p_yes, self.threshold))
    }
}

const QUESTION_ID: &str = "check";

impl Check {
    /// The request for this check. A check that needs an image or the facts and was not given them is an
    /// error naming what is missing: it never answers from less.
    pub fn request(&self, evidence: &Evidence) -> Result<Request, Error> {
        let mut missing: Vec<&str> = self
            .images
            .iter()
            .filter(|n| !evidence.images.contains_key(*n))
            .map(String::as_str)
            .collect();
        if self.facts && evidence.facts.is_none() {
            missing.push("facts");
        }
        if !missing.is_empty() {
            return Err(Error::MissingEvidence { check: self.name.clone(), missing: missing.join(", ") });
        }
        let mut state = format!("Check `{}`.", self.name);
        if !self.images.is_empty() {
            state += &format!(" Images, in order: {}.", self.images.join(", "));
        }
        if let (true, Some(f)) = (self.facts, &evidence.facts) {
            state += &format!(" Facts: {f}");
        }
        let mut req = Request::new(state).with_question(QUESTION_ID, Question::noul(self.question.clone()));
        for n in &self.images {
            req = req.with_image(evidence.images[n].clone());
        }
        Ok(req)
    }

    /// Ask `transport` this check's question and compare P(yes) with the threshold.
    pub fn run(&self, transport: &dyn Transport, evidence: &Evidence) -> Result<Finding, Error> {
        let response = ask(transport, &self.request(evidence)?)?;
        let p_yes = response.answers[QUESTION_ID].p_yes().expect("ask checked the answer is a noul");
        Ok(Finding { check: self.name.clone(), p_yes, threshold: self.threshold, found: p_yes >= self.threshold })
    }

    /// Whether this check may block the loop's gate: it earned a calibration at or above `bar`.
    pub fn may_block(&self, bar: f64) -> bool {
        self.calibration.as_ref().is_some_and(|c| c.is_blocking(bar))
    }
}
