//! **An agent's plan: a checklist with a status per item, kept beside the map** (E-069).
//!
//! An agent that means to do several things shows them first, so the person watching can see what it
//! intends before it acts and how far it has got. The plan is data, not editor state: one file per map,
//! **beside the map**: `maps/town.map.yaml` has `maps/town.plan.yaml`, written through
//! [`crate::ron_surgery::save_atomic`] like the map and the notes are, so a [`crate::live::FileWatch`]
//! over the project sees it change.
//!
//! The plan is replaced whole. The agent sends the list as it now stands, so there is no item id to
//! keep honest and no update that can miss; the operation that does it (`plan_set`) keeps the previous
//! list as its undo.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// The file format's version, written first so a later change can be migrated.
pub const PLAN_VERSION: u32 = 1;

/// Where one item of the plan stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Not started.
    #[default]
    Pending,
    /// The step the agent is on now.
    Doing,
    Done,
    /// Dropped without being done.
    Skipped,
}

/// One step the agent means to take.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanItem {
    pub text: String,
    #[serde(default)]
    pub status: Status,
}

/// The plan of one map: its items in the order the agent means to take them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: u32,
    #[serde(default)]
    pub items: Vec<PlanItem>,
}

impl Default for Plan {
    fn default() -> Self {
        Plan { version: PLAN_VERSION, items: Vec::new() }
    }
}

/// The plan file that belongs to a map file: `town.map.yaml` → `town.plan.yaml`, in the same directory.
/// A map path that does not end `.map.yaml` gets `.plan.yaml` appended to its stem.
pub fn path_for_map(map_path: &Path) -> PathBuf {
    let name = map_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name
        .strip_suffix(crate::map::MAP_SUFFIX)
        .or_else(|| name.strip_suffix(".yaml"))
        .unwrap_or(&name);
    map_path.with_file_name(format!("{stem}.plan.yaml"))
}

impl Plan {
    /// A plan of these items, or the first item that says nothing.
    pub fn new(items: Vec<PlanItem>) -> Result<Plan, String> {
        if let Some(at) = items.iter().position(|i| i.text.trim().is_empty()) {
            return Err(format!("plan: item {} has no text, and an item says what the agent will do", at + 1));
        }
        Ok(Plan { version: PLAN_VERSION, items })
    }

    pub fn parse(text: &str) -> Result<Plan, String> {
        let plan: Plan = crate::yaml::from_str(text, "plan").map_err(|p| format!("plan: parse: {}", p.line()))?;
        if plan.version != PLAN_VERSION {
            return Err(format!("plan: version {} is not the {PLAN_VERSION} this build reads", plan.version));
        }
        Ok(plan)
    }

    /// **The file as the one YAML writer writes it.**
    pub fn to_yaml(&self) -> Result<String, String> {
        crate::yaml::to_string(self).map_err(|p| format!("plan: {}", p.line()))
    }

    /// Reads the plan file; a file that is not there is an empty plan, not an error.
    pub fn load(path: &Path) -> Result<Plan, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Plan::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Plan::default()),
            Err(e) => Err(format!("plan: {}: {e}", path.display())),
        }
    }

    /// Writes the plan file atomically. An empty plan and no file writes nothing, so a map that never
    /// had a plan never grows one; an empty plan over a file empties it.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.items.is_empty() && !path.exists() {
            return Ok(());
        }
        crate::yaml::save(path, &self.to_yaml()?)
    }

    /// How many items are in each status, as `(pending, doing, done, skipped)`.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let n = |s: Status| self.items.iter().filter(|i| i.status == s).count();
        (n(Status::Pending), n(Status::Doing), n(Status::Done), n(Status::Skipped))
    }
}
