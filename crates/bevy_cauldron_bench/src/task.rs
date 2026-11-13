//! **One benchmark task, as its file states it** (BM-01, #411).
//!
//! A task is `bench/tasks/<id>/task.yaml`, read through the workspace's one YAML reader
//! ([`bevy_grimoire_core::yaml`]) so it meets the rules every project file meets: an unknown field is
//! refused with its file, line and field path, a key given twice is refused, and a refusal says what to
//! do. Nothing here is optional-with-a-guess: a task that names no prompt, no map or no checks is a task
//! that measures nothing, and it is refused when the suite loads rather than skipped.
//!
//! # The two budgets, and why they are constants
//!
//! A trial stops at [`TASK_WALL_S_DEFAULT`] seconds or [`TASK_TOOL_CALLS_DEFAULT`] tool calls. They are
//! named constants rather than literals here and in the runner, so the number on a trial's record and
//! the number in this file cannot drift.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::problem::{Detail, Location, Problem};
use serde::{Deserialize, Serialize};

use crate::check::Check;

/// **How long a trial may run**, in seconds, when a task states no budget.
///
/// Ten minutes: long enough for a model to look at a picture, think and place three pieces, short
/// enough that a suite of twenty tasks in five trials is not an afternoon.
pub const TASK_WALL_S_DEFAULT: u32 = 600;

/// **How many tool calls a trial may make**, when a task states no budget.
///
/// Sixty: a task that needs more than sixty calls is a task whose prompt does not say what it wants.
pub const TASK_TOOL_CALLS_DEFAULT: u32 = 60;

/// **The name of the file a task is read from**, inside its own directory.
pub const TASK_FILE: &str = "task.yaml";

/// **Which half of a suite a task belongs to.**
///
/// `dev` is the tasks a person iterates against; `held_out` is the tasks a change is scored on, and the
/// split exists so a prompt can be tuned without tuning the measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Visible while a harness is being built and tuned.
    Dev,
    /// Scored, not tuned against.
    HeldOut,
}

impl Split {
    /// The word the file writes.
    pub fn name(self) -> &'static str {
        match self {
            Split::Dev => "dev",
            Split::HeldOut => "held_out",
        }
    }
}

/// **One step of a trial**: an operation, or the hand-back that ends a turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// The operation's name, exactly as the editor's registry and an MCP tool spell it, or
    /// [`crate::PRESENT_STEP`] for a hand-back.
    pub op: String,
    /// What it is called with, verbatim — the same JSON an agent would send.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// **The budgets a trial is stopped at.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Wall-clock seconds an agent is given.
    #[serde(default = "default_wall_s")]
    pub wall_s: u32,
    /// Tool calls an agent is given.
    #[serde(default = "default_tool_calls")]
    pub tool_calls: u32,
}

fn default_wall_s() -> u32 {
    TASK_WALL_S_DEFAULT
}

fn default_tool_calls() -> u32 {
    TASK_TOOL_CALLS_DEFAULT
}

impl Default for Budget {
    fn default() -> Self {
        Budget { wall_s: TASK_WALL_S_DEFAULT, tool_calls: TASK_TOOL_CALLS_DEFAULT }
    }
}

/// **One task.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// The task's id: **the directory's own name**, so nothing has to remember a second spelling.
    pub id: String,
    /// What a person reading a report sees.
    pub title: String,
    /// Which half of the suite this task is.
    pub split: Split,
    /// What the task is about — `place`, `relations`, `refusal`. For reading a report, never for scoring.
    #[serde(default)]
    pub tags: Vec<String>,
    /// **The map the trial opens**: a map file's stem under `assets/grimoire/maps/`.
    pub map: String,
    /// **Everything the agent is told.** A trial gives an agent this and nothing else.
    pub prompt: String,
    /// Operations applied before the agent starts, so the scene it meets is the task's.
    #[serde(default)]
    pub setup: Vec<Step>,
    /// **A known answer**: operations that, applied through the editor's dispatch, leave every check
    /// passing. It is what proves the task solvable without a model, and it is what the runner's
    /// `oracle` agent replays.
    #[serde(default)]
    pub solution: Vec<Step>,
    /// **Every one of these must pass** on what the run left.
    pub checks: Vec<Check>,
    /// When the trial is stopped.
    #[serde(default)]
    pub budget: Budget,
}

impl Task {
    /// **Read the task in `dir`** — a `<id>/` directory holding a `task.yaml`.
    ///
    /// Refuses, naming the file and line, a task whose file does not read; a task whose `id` is not the
    /// directory's name; a task that says no prompt or no checks; and a task whose `map` is not a map
    /// the fixture or the project holds.
    ///
    /// `root` is the project the task runs against, or the repository holding `bench/fixture/`; `known`
    /// is every operation name the caller's registry knows, because whether an operation exists is a
    /// question this crate cannot answer for itself.
    #[allow(clippy::result_large_err)]
    pub fn load(dir: &Path, root: &Path, known: &[&str]) -> Result<Task, Problem> {
        let path = dir.join(TASK_FILE);
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(&path).map_err(|e| {
            Problem::with(
                format!("{shown} could not be read"),
                Detail::default()
                    .at(Location::file(shown.clone()))
                    .because(e.to_string())
                    .fix("check the task directory holds a task.yaml a person can read"),
            )
        })?;
        let task: Task = bevy_grimoire_core::yaml::from_str(&text, &shown)?;
        let lines = bevy_grimoire_core::yaml::lines(&text);
        task.validate(dir, root, known, &shown, &lines)?;
        Ok(task)
    }

    /// **The checks a task's own answer must satisfy**, which is every check it states.
    pub fn solution_checks(&self) -> &[Check] {
        &self.checks
    }

    /// Everything about a parsed task that depends on where it sits and what the project holds.
    ///
    /// **`Problem` is the workspace's one refusal type**, which every surface renders, so this answers it
    /// rather than a boxed copy: a box here would make a task's refusal a different shape from a file
    /// reader's and an operation dispatcher's, and the size is the shape's.
    #[allow(clippy::result_large_err)]
    fn validate(
        &self,
        dir: &Path,
        root: &Path,
        known: &[&str],
        shown: &str,
        lines: &std::collections::BTreeMap<String, usize>,
    ) -> Result<(), Problem> {
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if self.id != dir_name {
            return Err(Problem::with(
                "the task's id is not its directory's name",
                Detail::default()
                    .at(Location::file(shown.to_owned()).field(Some("id".to_owned())))
                    .because(format!("`{}` sits in `{dir_name}/`", self.id))
                    .fix(format!("call it `{dir_name}`, or put it in `{}/`", self.id)),
            ));
        }
        if self.prompt.trim().is_empty() {
            return Err(refuse_at(
                shown,
                "prompt",
                lines.get("prompt").copied(),
                "the task's prompt is empty, so an agent would be given nothing to do",
                "write what the task asks for",
            ));
        }
        if self.checks.is_empty() {
            return Err(refuse_at(
                shown,
                "checks",
                lines.get("checks").copied(),
                "the task states no checks, so no run of it could be wrong",
                "state at least one check",
            ));
        }
        // **Every step names an operation something knows.** An operation this build does not have is a
        // task that cannot be replayed, and skipping it would make the task's own solution lie.
        for (list, steps) in [("setup", &self.setup), ("solution", &self.solution)] {
            for (n, step) in steps.iter().enumerate() {
                if step.op == crate::PRESENT_STEP || known.iter().any(|k| *k == step.op) {
                    continue;
                }
                let field = format!("{list}[{n}].op");
                return Err(refuse_at(
                    shown,
                    &field,
                    lines.get(&format!("{list}[{n}]")).copied(),
                    &format!(
                        "`{}` is not an operation this build knows, and `{list}[{n}]` is the step that \
                         names it",
                        step.op
                    ),
                    "name one of the registry's operations, or `grimoire_present` for the hand-back",
                ));
            }
        }
        // **A map, and every file a check names, has to be there.** A check about a file nothing holds
        // would pass or fail for a reason the task does not state.
        let held = |rel: &str| fixture_or_project(root, rel).is_some();
        if !held(&format!("assets/grimoire/maps/{}.map.yaml", self.map)) {
            return Err(refuse_at(
                shown,
                "map",
                lines.get("map").copied(),
                &format!(
                    "`{}` is not a map this task can open — neither {} nor the project holds it",
                    self.map,
                    crate::fixture::FIXTURE_OF_GRIMOIRE
                ),
                "name a map file under assets/grimoire/maps/, or add it to bench/fixture/",
            ));
        }
        for (n, check) in self.checks.iter().enumerate() {
            for file in check.files_named() {
                if !held(&format!("assets/grimoire/{file}")) {
                    return Err(refuse_at(
                        shown,
                        &format!("checks[{n}]"),
                        lines.get(&format!("checks[{n}]")).copied(),
                        &format!("`{file}` is not a file this task can check — nothing holds it"),
                        "name a file under assets/grimoire/, or add it to bench/fixture/",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// **The file a task's path names, in the fixture first and then in the project**, or `None`.
///
/// The fixture is asked first because that is the order a trial lays them: the project is copied, the
/// fixture goes over the copy, and a file the fixture holds is the file a trial has.
pub fn fixture_or_project(root: &Path, rel: &str) -> Option<PathBuf> {
    for base in [root.join(crate::FIXTURE_DIR), root.to_path_buf()] {
        let path = base.join(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// **A refusal at a task's own line and field**, with both in the sentence a person reads.
///
/// **The line is said in `what`, not only carried on `Location`**, because `Location`'s `Display` renders the
/// file and the field and never the line — so a refusal that only set `Location::line` named the file and the
/// field and left the line to a reader who happened to inspect the value. Every other reader of a project
/// file says its line in the sentence for the same reason, and #411's acceptance asks for it by name.
fn refuse_at(shown: &str, field: &str, line: Option<usize>, why: &str, fix: &str) -> Problem {
    let mut at = Location::file(shown.to_owned()).field(Some(field.to_owned()));
    if let Some(line) = line {
        at = at.line(line);
    }
    let what = match line {
        Some(line) => format!("the task was refused at line {line}"),
        None => "the task was refused".to_owned(),
    };
    Problem::with(
        what,
        Detail::default().at(at).because(why.to_owned()).fix(fix.to_owned()),
    )
}
