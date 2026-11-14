//! **What a run left, and what the checks make of it** (BM-01, #411).
//!
//! A [`Snapshot`] is a project directory read as data: every file's digest, the open map read as a
//! [`Map`], the library that gives its pieces their extents, the operation log's lines, the hand-backs
//! an agent made, and the problem passes the core runs. Everything a check asks is answered from one of
//! those, which is what makes a check independent of the operation that made the change.
//!
//! # Why a digest per file, and not "the value is equal"
//!
//! A rewrite that leaves the meaning alone is still a write: an agent that re-saves a map the task said
//! to leave alone has changed a file a person will see in a diff. So `unchanged` compares **bytes**,
//! through a SHA-256 per file, and only a file whose digest moved is reported.
//!
//! # The operation log is read by name and outcome only
//!
//! A check about the log asks "did an operation of this name apply", never "what were its parameters".
//! The parameters are the operation's own account of itself; the map is the measurement.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bevy_grimoire_core::map::Map;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::check::{grade, Check, ProblemKind, Verdict};

/// **Where a project keeps its files**, relative to the trial's root — the one path every reader here
/// uses, so the snapshot and the editor cannot disagree about where a map is.
pub const GRIMOIRE_DIR: &str = "assets/grimoire";

/// **Where the operation log is**, relative to a project root.
pub const OPLOG: &str = "assets/grimoire/oplog.jsonl";

/// **The one project file a run always writes**, whatever it did.
///
/// `unchanged` and `project_unchanged` compare **bytes**, and the dispatcher appends a line to the log for
/// every operation it applies — including the ones a task's `setup` applies before the agent starts. A
/// comparison that counted the log would therefore fail on every task, and would be measuring "did the
/// harness run" rather than "did the agent change the project". So the log is left out of both, and a
/// check that cares what the log says states it separately (`oplog_has`, `oplog_lacks`), where it is read
/// by op name and outcome rather than by digest.
pub const RUN_RECORD: &str = OPLOG;

/// **Where the editor keeps its own working files**, relative to a project root: its thumbnail cache, its
/// captures, its logs. **Never a project file**, and never what `project_unchanged` measures.
///
/// Measured on this host, 2026-11-13: a trial's read-only task failed `project_unchanged` naming 36
/// `target/grimoire_thumbs/*.png`, because the editor bakes the palette's thumbnails while the agent is
/// still thinking. A check that counted those would fail every task that ran longer than the bake, which is
/// every task a real model attempts — so `unchanged` and `project_unchanged` measure the project, and the
/// editor's own cache is not the project.
pub const EDITOR_SCRATCH: &str = "target";

/// **The files a project holds that a snapshot digests**: every regular file under the project, keyed by
/// its path relative to the root with `/` separators.
///
/// **Every file but the editor's own scratch** ([`EDITOR_SCRATCH`]), because an agent that moves a texture
/// or leaves a stray file in the project has changed it, and `project_unchanged` is the check that says so.
/// Dotfiles are skipped  -  a `.tmp` from an interrupted atomic write is not a project file  -  and so is
/// the operation log ([`RUN_RECORD`]), which is read by name and outcome instead.
pub fn project_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            // **The editor's own working directory is not the project**, see [`EDITOR_SCRATCH`].
            if dir == root && name == EDITOR_SCRATCH {
                continue;
            }
            walk(root, &path, out);
        } else if kind.is_file()
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// **One line of the operation log**, as much as a check needs: what was done, and whether it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    /// The operation's name.
    pub op: String,
    /// `applied` or `refused`.
    pub outcome: String,
    /// The source, `human` or `agent`.
    pub source: String,
}

impl LogLine {
    /// Whether this line records a change that happened.
    pub fn applied(&self) -> bool {
        self.outcome == "applied"
    }
}

/// **One hand-back an agent made**, as `grimoire/present` leaves it in the transcript the runner reads.
///
/// A presentation changes no file — it is a question on screen — so it reaches the grader as a value the
/// runner hands over, never as something a check could find on disk.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Handback {
    /// The id the editor gave it (`present-1`).
    pub id: String,
    /// What the agent called it.
    pub title: String,
    /// One paragraph: what was made.
    pub summary: String,
    /// What the person asked for, as the agent quoted it back.
    pub specified: Vec<String>,
    /// What the agent decided on its own.
    pub inferred: Vec<String>,
    /// The places it says are worth a look.
    pub check: Vec<String>,
}

impl Handback {
    /// **Everything this hand-back says, in one string**, which is what `handback_contains` matches
    /// against: a fact can be in the title, the summary or any of the three lists, because an agent that
    /// states it anywhere has stated it.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for part in std::iter::once(&self.title)
            .chain(std::iter::once(&self.summary))
            .chain(self.specified.iter())
            .chain(self.inferred.iter())
            .chain(self.check.iter())
        {
            out.push_str(part);
            out.push('\n');
        }
        out
    }
}

/// **A calibrated decision the harness made** about the result — `grimoire/decide`'s answer, kept so the
/// grader needs no model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    /// What the decision model said.
    pub answer: bool,
    /// How sure it was, `0.0..=1.0`.
    pub confidence: f32,
}

/// **Everything the problem passes found** in a map, in a sentence each, so a failing check can name one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// Edge faults, as `a/b`.
    pub faults: Vec<String>,
    /// Overlaps, as `a/b` with the ids sorted.
    pub overlaps: Vec<String>,
    /// Doorways, as `opening/inside`.
    pub doorways: Vec<String>,
    /// Clearances, as `keeper/intruder`.
    pub clearances: Vec<String>,
}

/// **A project as data**, read once by [`Snapshot::read`].
///
/// **A map that does not read is a failed run, not a partial one.** A run that leaves the map unparseable
/// has broken the project, so every check that asks something about the map fails and names the reason
/// ([`Self::map_problem`]). Substituting `Map::default()` and answering from it would *pass* `no_problem`
/// (nothing found in a map that does not exist) and report "no `bed_double` is on the map" as though the
/// agent had simply not placed it — the two answers a person would most want to tell apart.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Every file's path relative to the root, and its digest.
    pub files: BTreeMap<String, String>,
    /// The open map, when the file still reads.
    pub map: Map,
    /// The project's library, which gives a placement its extents.
    pub library: bevy_grimoire_core::library::Library,
    /// The operation log's lines, oldest first.
    pub oplog: Vec<LogLine>,
    /// Every hand-back the run made, oldest first.
    pub handbacks: Vec<Handback>,
    /// The decisions the harness made about this run, by the name the check asks for.
    pub decisions: BTreeMap<String, Decision>,
    /// What the problem passes found in [`Self::map`].
    pub found: Found,
    /// **Why the map did not read**, when it did not: a task whose agent left an unreadable map has failed
    /// it, and the reason is the interesting part. Every map-derived check reads this first.
    pub map_problem: Option<String>,
    /// **The project's root**, so a check that reads a file the snapshot does not digest into a value (a
    /// number in a data file, the ground's heights) reads it from where the run left it.
    pub root: PathBuf,
}

impl Snapshot {
    /// **Read the project at `root`.**
    ///
    /// `map_name` is the map the trial opened; `handbacks` and `decisions` are what the run made, which no
    /// file holds.
    pub fn read(root: &Path, map_name: &str, handbacks: Vec<Handback>, decisions: BTreeMap<String, Decision>) -> Snapshot {
        let mut files = BTreeMap::new();
        for rel in project_files(root) {
            // **The log is the run's own record**, excluded from byte comparisons (see [`RUN_RECORD`]).
            if rel == RUN_RECORD {
                continue;
            }
            let digest = digest_of(&root.join(&rel));
            files.insert(rel, digest);
        }
        let library = read_library(root);
        let map_path = map_path(root, map_name);
        let (map, map_problem) = match std::fs::read_to_string(&map_path) {
            Ok(text) => match Map::parse(&text) {
                Ok(map) => (with_runs_laid(map, &library), None),
                Err(why) => (Map::default(), Some(why)),
            },
            Err(e) => (Map::default(), Some(format!("{}: {e}", map_path.display()))),
        };
        let oplog = read_oplog(root);
        let found = if map_problem.is_none() {
            problems_of(&map, &library, root)
        } else {
            Found::default()
        };
        Snapshot { files, map, library, oplog, handbacks, decisions, found, map_problem, root: root.to_path_buf() }
    }

    /// **A project file's text, as the run left it**, by its path under `assets/grimoire/`; `None` when the
    /// project does not hold it or it cannot be read.
    pub fn text(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(GRIMOIRE_DIR).join(rel)).ok()
    }

    /// **One file's digest**, or `None` when the project does not hold it.
    pub fn digest(&self, rel: &str) -> Option<&String> {
        self.files.get(rel)
    }

    /// **The files whose bytes differ between two snapshots**, sorted, including files added and removed.
    pub fn differences(&self, other: &Snapshot) -> Vec<String> {
        let keys: BTreeSet<&String> = self.files.keys().chain(other.files.keys()).collect();
        keys.into_iter()
            .filter(|k| self.files.get(*k) != other.files.get(*k))
            .cloned()
            .collect()
    }
}

/// **What reading one project left**: a snapshot, the files the suite laid over it, and the checks'
/// verdicts.
#[derive(Clone, Debug)]
pub struct Left {
    /// The project as the run left it.
    pub after: Snapshot,
    /// The project as the setup left it — what `unchanged` and `project_unchanged` compare against.
    pub before: Snapshot,
}

/// **One trial's outcome**, which is what a report and a `trial.json` state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every check held.
    Pass,
    /// At least one check did not.
    Fail,
    /// **The harness itself failed**: the editor did not start, a port did not open, the transcript could
    /// not be read. Never `pass` and never `fail`, because neither is a fact about the agent.
    HarnessError,
}

impl Outcome {
    /// The word a report writes.
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::HarnessError => "harness_error",
        }
    }
}

/// **A graded run**: every check with its verdict, and the outcome those add up to.
#[derive(Clone, Debug)]
pub struct Grading {
    /// The task's id.
    pub task: String,
    /// Every check, in the order the task stated them.
    pub verdicts: Vec<Verdict>,
    /// `pass` when every verdict passed, `fail` otherwise.
    pub outcome: Outcome,
    /// **Why the outcome is `harness_error`**, when it is. Empty otherwise.
    pub reason: String,
}

impl Grading {
    /// **Grade every one of a task's checks** against what the setup left and what the run left.
    pub fn of(task: &crate::task::Task, before: &Snapshot, after: &Snapshot) -> Grading {
        let verdicts: Vec<Verdict> = task
            .checks
            .iter()
            .map(|check| grade(check, before, after))
            .collect();
        let outcome = if verdicts.iter().all(|v| v.passed) { Outcome::Pass } else { Outcome::Fail };
        Grading { task: task.id.clone(), verdicts, outcome, reason: String::new() }
    }

    /// **A run the harness could not grade**, with the reason it could not.
    pub fn harness_error(task: &str, reason: impl Into<String>) -> Grading {
        Grading {
            task: task.to_owned(),
            verdicts: Vec::new(),
            outcome: Outcome::HarnessError,
            reason: reason.into(),
        }
    }

    /// **The checks that did not pass**, for a report that has to name what is wrong.
    pub fn failed(&self) -> Vec<&Verdict> {
        self.verdicts.iter().filter(|v| !v.passed).collect()
    }

    /// How many checks passed, and how many there are.
    pub fn score(&self) -> (usize, usize) {
        (self.verdicts.iter().filter(|v| v.passed).count(), self.verdicts.len())
    }

    /// **The sentence the tests quote when a task is invalid**: the task, the outcome, and every failing
    /// check with the value it measured.
    pub fn sentence(&self) -> String {
        let mut out = format!("task `{}`: {}", self.task, self.outcome.name());
        if !self.reason.is_empty() {
            out.push_str(&format!(" ({})", self.reason));
        }
        for failed in self.failed() {
            out.push_str(&format!("\n  - `{}` failed: {} — measured {}", failed.check, failed.asks, failed.measured));
        }
        let (passed, total) = self.score();
        out.push_str(&format!("\n  {passed}/{total} checks passed"));
        out
    }
}

/// **The map's file**, for a map's stem.
pub fn map_path(root: &Path, map_name: &str) -> PathBuf {
    root.join(GRIMOIRE_DIR).join("maps").join(format!("{map_name}{}", bevy_grimoire_core::map::MAP_SUFFIX))
}

/// **A file's digest, as hex** — the project's one digest, so `unchanged` and any other comparison agree.
pub fn digest_of(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
        }
        // **A file that cannot be read is a digest no other file has**, so it always reads as changed.
        Err(e) => format!("unreadable: {e}"),
    }
}

/// **A map with its runs laid as the placements they stand for**, which is what a person sees and what a
/// check about a hedge of six has to count.
///
/// `run_add` writes a run (a piece, a pitch and two ends), never the posts: they are laid when the map is
/// read, by the core's own `Map::expanded`, as the editor draws them. A map with stamps is left as written,
/// because expanding those needs the project's compositions and no task here asks about one; a run that
/// does not lay is left as written too, so the check says what is on the map rather than nothing.
fn with_runs_laid(map: Map, library: &bevy_grimoire_core::library::Library) -> Map {
    if map.runs.is_empty() || !map.stamps.is_empty() {
        return map;
    }
    map.expanded(&[], library).unwrap_or(map)
}

/// **The project's library**, read through the core's own loader: every bound kit, layered and merged,
/// which is the library a placement's extents come from.
///
/// A project whose library does not read answers an empty one: a check that needed an extent then says
/// the piece is not in the library, which is the honest reading of a project in that state.
fn read_library(root: &Path) -> bevy_grimoire_core::library::Library {
    let grimoire = root.join(GRIMOIRE_DIR);
    let Ok(text) = std::fs::read_to_string(grimoire.join(bevy_grimoire_core::kits::KITS_FILE)) else {
        return bevy_grimoire_core::library::Library::default();
    };
    let Ok(kits) = bevy_grimoire_core::kits::Kits::parse(&text) else {
        return bevy_grimoire_core::library::Library::default();
    };
    match bevy_grimoire_core::kits::bound_library(&grimoire, &kits) {
        Ok(bound) => bound.library,
        Err(_) => bevy_grimoire_core::library::Library::default(),
    }
}

/// **The operation log's lines**, oldest first; a line that does not read is skipped, which is what the
/// editor's own reader does.
fn read_oplog(root: &Path) -> Vec<LogLine> {
    let Ok(text) = std::fs::read_to_string(root.join(OPLOG)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            Some(LogLine {
                op: value.get("op")?.as_str()?.to_owned(),
                outcome: value.get("outcome")?.as_str()?.to_owned(),
                source: value.get("source").and_then(|s| s.as_str()).unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// **The problem passes, from the core's own arithmetic** — overlap, edge faults, doorways and clearances.
///
/// The editor's `checks.rs` computes the same four over the same map with the same core functions; this is
/// a second caller of the core, not a second implementation. The grader cannot call the editor's because
/// that module is a Bevy app's, and an engine-free grader is what lets the gate prove a task.
fn problems_of(map: &Map, library: &bevy_grimoire_core::library::Library, root: &Path) -> Found {
    let faces = read_face_bands(root);
    let fault = |f: &bevy_grimoire_core::adjacency::Fault| format!("{}/{}", f.a, f.b);
    let pair = |(i, j): (usize, usize)| {
        match (map.placements.get(i), map.placements.get(j)) {
            (Some(a), Some(b)) => format!("{}/{}", a.id, b.id),
            _ => format!("{i}/{j}"),
        }
    };
    Found {
        faults: bevy_grimoire_core::adjacency::faults(map, library, faces).iter().map(fault).collect(),
        overlaps: bevy_grimoire_core::stack::overlaps(map, library).into_iter().map(pair).collect(),
        doorways: bevy_grimoire_core::stack::blocked_openings(map, library).into_iter().map(pair).collect(),
        clearances: bevy_grimoire_core::stack::blocked_clearances(map, library).into_iter().map(pair).collect(),
    }
}

/// **The project's face bands**, from `kits.yaml`; the default when the file does not say, which is what
/// a project written before the field existed meant.
fn read_face_bands(root: &Path) -> u32 {
    let path = root.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE);
    let Ok(text) = std::fs::read_to_string(path) else {
        return bevy_grimoire_core::kits::Lattice::default().face_bands;
    };
    bevy_grimoire_core::kits::Kits::parse(&text)
        .map(|k| k.lattice.face_bands)
        .unwrap_or_else(|_| bevy_grimoire_core::kits::Lattice::default().face_bands)
}

/// **Whether a check's subject is a map problem of a kind**, so a report can point at the check that
/// would have caught it.
pub fn problem_of_kind(found: &Found, kind: ProblemKind) -> bool {
    kind.count(found) > 0
}

/// **Every check in a task, graded against two snapshots** — the free function a caller with no `Task`
/// handy uses.
pub fn grade_all(checks: &[Check], before: &Snapshot, after: &Snapshot) -> Vec<Verdict> {
    checks.iter().map(|c| grade(c, before, after)).collect()
}

/// **The folders the editor's operations write**, relative to a project root, read from the reload contract's
/// file kinds (`bevy_grimoire_core::reload`) rather than guessed: the library and maps under
/// `assets/grimoire/`, a game's style tree and every place's dream board under `design/` (`board_set`,
/// `tree_set`), and the Rhai scripts, family, reference pictures and destructible bakes the reload contract
/// lists at the project root. A file outside all of them is not one an operation wrote.
pub const OPERATION_WRITE_ROOTS: &[&str] =
    &[GRIMOIRE_DIR, "design", "scripts", "family", "references", "destructible"];

/// **Single files at the project root an operation writes**, for the same reason.
pub const OPERATION_WRITE_FILES: &[&str] = &["family.yaml", "destructible.yaml"];

/// **Operations that write a file wherever it is**: `file_edit` and `files_replace` edit any project file, and
/// `data_set` edits a registered data file, which a game keeps outside the grimoire folder
/// (`../data/mob.yaml`). An applied line of one of these explains a change anywhere.
pub const OPERATIONS_WRITING_ANYWHERE: &[&str] = &["file_edit", "files_replace", "data_set"];

/// **The files an agent changed that no operation-log line wrote**: it went around the editor's tools,
/// through its shell or a file tool, and the editor never heard of the change.
///
/// The log names an operation, not the files it wrote, so the answer is a bound and not a trace: a changed
/// file is **explained** when the log holds a line the agent's operations appended after the setup *and*
/// the file sits where the editor's operations write ([`OPERATION_WRITE_ROOTS`], [`OPERATION_WRITE_FILES`]),
/// or that line is one of [`OPERATIONS_WRITING_ANYWHERE`]. Every other changed file is behind the editor's
/// back, whichever way it got there:
///
/// - **no applied line after the setup** explains nothing: a map that changed in a run that applied no
///   operation was written by something else;
/// - **a file outside the named folders** is never an operation's output.
///
/// What this cannot see is an agent that applied an operation *and also* hand-edited a second file inside
/// those folders; that is counted as the operation's. The bound errs toward the agent.
pub fn edits_behind_the_back(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let applied: Vec<&LogLine> =
        after.oplog.iter().skip(before.oplog.len()).filter(|line| line.applied()).collect();
    let anywhere = applied.iter().any(|line| OPERATIONS_WRITING_ANYWHERE.contains(&line.op.as_str()));
    let written_by_an_operation = |file: &str| {
        OPERATION_WRITE_ROOTS.iter().any(|root| file.starts_with(&format!("{root}/")))
            || OPERATION_WRITE_FILES.contains(&file)
    };
    before
        .differences(after)
        .into_iter()
        .filter(|file| !(!applied.is_empty() && (anywhere || written_by_an_operation(file))))
        .collect()
}
