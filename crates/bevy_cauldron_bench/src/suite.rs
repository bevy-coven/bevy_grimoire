//! **A suite: every task a project states, read and refused as a whole** (BM-01, #411).
//!
//! A suite is a directory of `<id>/task.yaml` directories — [`crate::TASKS_DIR`] in this repository, or
//! `<root>/.dark_factory/bench/` in a game (BM-02's `--suite`). **Every task is read and checked before
//! any trial runs**, so a suite with one bad task is a suite that refuses rather than a suite that quietly
//! measures nineteen of twenty. A task an agent must refuse is still a task; a task the *suite* cannot read
//! is a file nobody can run.
//!
//! # Which operations exist is the caller's answer
//!
//! Whether `placement_set_yaw` is an operation this build has is a question only the editor can answer, so
//! [`Suite::load`] takes the registry's names and refuses a task naming one it does not have. That is what
//! makes a task file with an operation from a newer build a refusal at load rather than a trial that dies
//! halfway through its setup.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::problem::{Detail, Location, Problem};
use serde::{Deserialize, Serialize};

use crate::task::{Split, Task};

/// **The file a suite's arms are stated in**, beside its tasks: `bench/variants.yaml` (BM-02).
pub const VARIANTS_FILE: &str = "variants.yaml";

/// **Every task a project states.**
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Suite {
    /// The tasks, in id order, so two runs of one suite report in one order.
    pub tasks: Vec<Task>,
    /// Which directory they were read from, for a report.
    pub dir: PathBuf,
}

/// **A named arm of a suite**, read from [`VARIANTS_FILE`] (BM-02).
///
/// A variant is a way to run the *suite* rather than a different task: tools an agent must not be offered,
/// and project files laid over the copy so a prompt or a guide can differ per arm.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    /// The arm's name, as `--variant` spells it.
    pub name: String,
    /// **Tools that are not served at all**, by the MCP server's `--hide-tools`, so they are absent from the
    /// agent's tool list rather than refused when called.
    #[serde(default)]
    pub hide_tools: Vec<String>,
    /// **Project files laid over the trial's copy**, relative to the project root.
    #[serde(default)]
    pub overlay: Vec<String>,
}

/// **Every arm a suite states.**
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variants {
    /// The version the file is written at, so a file from a newer build is refused rather than misread.
    pub version: u32,
    /// The arms.
    #[serde(default)]
    pub variants: Vec<Variant>,
}

/// **The version of `variants.yaml` this build reads.**
pub const VARIANTS_VERSION: u32 = 1;

impl Suite {
    /// **Read every task under `dir`**, refusing the suite on the first task that cannot be run.
    ///
    /// `root` is the project the tasks run against — the repository, or a game's root — and `known` every
    /// operation name the caller's registry has.
    #[allow(clippy::result_large_err)]
    pub fn load(root: &Path, dir: &Path, known: &[&str]) -> Result<Suite, Problem> {
        let entries = std::fs::read_dir(dir).map_err(|e| {
            Problem::with(
                format!("{} could not be read", dir.display()),
                Detail::default()
                    .at(Location::file(dir.display().to_string()))
                    .because(e.to_string())
                    .fix("check the suite directory exists and holds one directory per task"),
            )
        })?;
        let mut task_dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        task_dirs.sort();
        let mut tasks = Vec::with_capacity(task_dirs.len());
        for task_dir in task_dirs {
            tasks.push(Task::load(&task_dir, root, known)?);
        }
        // **Two tasks with one id are refused**: a report naming `bed` would otherwise be a report about
        // whichever the filesystem listed first.
        for pair in tasks.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(Problem::with(
                    format!("two tasks are both called `{}`", pair[0].id),
                    Detail::default()
                        .at(Location::file(dir.display().to_string()))
                        .because("a task's id is its directory's name, so one of these was copied and not renamed")
                        .fix("rename one, or delete it if it was left behind"),
                ));
            }
        }
        if tasks.is_empty() {
            return Err(Problem::with(
                format!("{} holds no tasks", dir.display()),
                Detail::default()
                    .at(Location::file(dir.display().to_string()))
                    .because("a suite with no tasks measures nothing, and a run of it would report success on an empty set")
                    .fix(format!("add `<id>/{}/` directories under {}", crate::task::TASK_FILE, dir.display())),
            ));
        }
        Ok(Suite { tasks, dir: dir.to_path_buf() })
    }

    /// **The tasks of one half of the suite**, in id order.
    pub fn split(&self, split: Split) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.split == split).collect()
    }

    /// **The tasks named on a command line**, or every task when none is named; a name no task has is an
    /// error rather than an empty selection.
    pub fn select(&self, ids: &[String]) -> Result<Vec<&Task>, String> {
        if ids.is_empty() {
            return Ok(self.tasks.iter().collect());
        }
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            match self.tasks.iter().find(|t| &t.id == id) {
                Some(task) => out.push(task),
                None => {
                    return Err(format!(
                        "`{id}` is not a task in {} — it holds {}",
                        self.dir.display(),
                        self.tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", ")
                    ));
                }
            }
        }
        Ok(out)
    }

    /// **The arms a project states**, and the file they were read from, or none when it has no file.
    #[allow(clippy::result_large_err)]
    pub fn variants(root: &Path) -> Result<Vec<Variant>, Problem> {
        let path = variants_path(root);
        let shown = path.display().to_string();
        let Ok(text) = std::fs::read_to_string(&path) else {
            // **No file is an arm-less suite**, not a refusal: comparing two editor builds needs no
            // variant, and most projects will never write one.
            return Ok(Vec::new());
        };
        let file: Variants = bevy_grimoire_core::yaml::from_str(&text, &shown)?;
        if file.version > VARIANTS_VERSION {
            return Err(Problem::with(
                format!("{shown} is version {} but this build reads {VARIANTS_VERSION}", file.version),
                Detail::default()
                    .at(Location::file(shown.clone()).field(Some("version".to_owned())))
                    .because("a variants file from a newer build may name settings this build does not apply")
                    .fix("open the newer build, or write the version this build reads"),
            ));
        }
        for variant in &file.variants {
            if variant.name.trim().is_empty() {
                return Err(Problem::with(
                    format!("{shown} states an arm with no name"),
                    Detail::default()
                        .at(Location::file(shown.clone()).field(Some("variants".to_owned())))
                        .because("an arm is chosen by name, and an unnamed one cannot be")
                        .fix("give it a name"),
                ));
            }
            for rel in &variant.overlay {
                let path = root.join(rel);
                if !path.is_file() {
                    return Err(Problem::with(
                        format!("{shown}: arm `{}` lays over `{rel}`, which is not a file", variant.name),
                        Detail::default()
                            .at(Location::file(shown.clone()).field(Some("overlay".to_owned())))
                            .because(format!("{} is not on disk", path.display()))
                            .fix("point it at a file, relative to the project root"),
                    ));
                }
            }
        }
        Ok(file.variants)
    }
}

/// **Where a project's arms are stated**: `bench/variants.yaml` beside the tasks.
pub fn variants_path(root: &Path) -> PathBuf {
    root.join(crate::TASKS_DIR).join(VARIANTS_FILE)
}
