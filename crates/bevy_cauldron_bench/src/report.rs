//! **The report a run ends with: what the agent did on every task, how often, at what cost, and how far
//! to trust the numbers** (BM-03, #413).
//!
//! A [`Report`] is built from the `trial.json` files a run wrote ([`TrialData`]) and the header that says
//! how to reproduce it ([`Header`]), with no model and no editor: a function of recorded files, which is
//! what lets a test check it against values worked out by hand. It renders as Markdown for a person
//! ([`Report::markdown`]) and as JSON for [`crate::compare`] ([`Report::json`]).
//!
//! # What a report states, and what it will not
//!
//! - **Success is `c / n` over the trials that were scored.** A harness error is never scored as either
//!   outcome ([`Outcome`]); it is counted beside, and a run with more than [`HARNESS_ERROR_MAX_FRACTION`]
//!   of them is **unfit for comparison** and says so at the top.
//! - **`pass^k`** (`stats::pass_hat_k`) is stated for `k = 1..n` per task, because a task passed three times
//!   in five is not a task passed every time.
//! - **Held-out tasks are a total and nothing more.** They are never named, never listed with a prompt
//!   and never broken down by tag, so a report cannot be used to tune against them (BM-04).
//! - **Cost is the model's listed price times the tokens the harness reported**
//!   (`omp models find --json`), never a guess: a model with no listed price has cost `unknown`, which is
//!   not `0` and is rendered as `unknown` (Kapoor et al., `10.48550/arXiv.2407.01502`). A harness that
//!   reports its own cost (Claude Code) is used only when no price is listed.
//! - **Edits behind the editor's back** are files that changed with no operation-log line to explain them
//!   ([`crate::grade::edits_behind_the_back`]): the agent went around the tools.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::check::Verdict;
use crate::grade::Outcome;
use crate::run::CallLog;
use crate::stats::{self, Interval, BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, WALL_SLOW_PERCENTILE};
use crate::task::Split;

/// **The version of the report's JSON**, so a report from a newer build is refused rather than misread.
pub const REPORT_VERSION: u32 = 1;

/// **The fraction of a run's trials that may be harness errors** before the run is unfit for comparison.
///
/// More than this and the numbers are about the harness: a comparison of two such runs would measure how
/// often each one's plumbing failed.
pub const HARNESS_ERROR_MAX_FRACTION: f64 = 0.10;

/// **How many of an agent's last tool calls a failed trial shows.**
pub const LAST_CALLS_SHOWN: usize = 5;

/// **How many trials a "Harness errors" row of the markdown names** before it says how many more there were.
pub const HARNESS_ERROR_TRIALS_SHOWN: usize = 10;

/// **How many tools a report lists as the ones an agent called most.**
pub const TOP_TOOLS_SHOWN: usize = 8;

/// **The file a run's header is written to**, beside its trials.
pub const RUN_FILE: &str = "run.json";

/// **The file the pre-flight turn's tokens are kept in**, beside a run's trials (#434). Written only for an `omp` run.
pub const PREFLIGHT_FILE: &str = "preflight-usage.json";

/// **What a model costs per million tokens**, in US dollars, as `omp models find --json` lists it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Price {
    /// Fresh input tokens.
    pub input: f64,
    /// Output tokens.
    pub output: f64,
    /// Input tokens served from the provider's cache.
    pub cache_read: f64,
    /// Input tokens written to the provider's cache.
    pub cache_write: f64,
}

/// **The number of tokens a price is quoted per.**
pub const TOKENS_PER_PRICE: f64 = 1_000_000.0;

impl Price {
    /// **The price of `model` in the text `omp models find --json` printed**, or `None` when the model is
    /// not listed or its price is not fixed (`pricingStatus` other than `fixed`).
    ///
    /// A listed `0` with a `pricingStatus` of `unknown` is a model nobody priced, not a free one, and it
    /// is why this reads the status and not only the numbers.
    pub fn from_models(text: &str, model: &str) -> Option<Price> {
        let value: Value = serde_json::from_str(text).ok()?;
        let entry = value.get("models")?.as_array()?.iter().find(|m| {
            m.get("selector").and_then(Value::as_str) == Some(model)
        })?;
        if entry.get("pricingStatus").and_then(Value::as_str) != Some("fixed") {
            return None;
        }
        let cost = entry.get("cost")?;
        let read = |key: &str| cost.get(key).and_then(Value::as_f64);
        Some(Price {
            input: read("input")?,
            output: read("output")?,
            cache_read: read("cacheRead").unwrap_or(0.0),
            cache_write: read("cacheWrite").unwrap_or(0.0),
        })
    }
}

/// **Everything a reader needs to reproduce a run**, written beside its trials as `run.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// The version of the shape.
    pub version: u32,
    /// What the run is called in its file name.
    pub label: String,
    /// **The commit the editor and the MCP server were built at**, with `+dirty` when the tree had
    /// uncommitted changes, or `unknown`.
    pub commit: String,
    /// **The SHA-256 of the shipped guide** (`guide.md`), which is part of what an agent is handed.
    pub guide_sha256: String,
    /// The agent: `omp`, `claude-code`, `oracle`, `nothing`.
    pub agent: String,
    /// The agent's own version, as it printed it.
    pub agent_version: String,
    /// The model, for a harness that has one.
    pub model: Option<String>,
    /// The arm of the suite, when one was named.
    pub variant: Option<String>,
    /// The suite's directory, as the run named it.
    pub suite: String,
    /// Trials of each task the run asked for.
    pub trials: u32,
    /// The machine it ran on.
    pub host: String,
    /// The day it ran, `YYYY-MM-DD` in UTC.
    pub date: String,
    /// What the model costs, when the host lists a fixed price for it.
    pub price: Option<Price>,
}

impl Header {
    /// **`agent` and its model as one name**, which is what a matrix row and a comparison call the pair.
    pub fn pair(&self) -> String {
        match &self.model {
            Some(model) => format!("{}:{model}", self.agent),
            None => self.agent.clone(),
        }
    }

    /// **The file stem a committed report has**: `<date>-<label>`.
    pub fn stem(&self) -> String {
        format!("{}-{}", self.date, self.label)
    }
}

/// **One trial, as `trial.json` states it.** Every field a build before this one did not write defaults,
/// so an older run still reads.
#[derive(Clone, Debug, Deserialize)]
pub struct TrialData {
    /// The task's id.
    pub task: String,
    /// Which trial of it.
    pub trial: u32,
    /// The task's title.
    #[serde(default)]
    pub title: String,
    /// Which half of the suite the task is in.
    #[serde(default = "dev_split")]
    pub split: Split,
    /// The task's tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// `pass`, `fail` or `harness_error`.
    pub outcome: Outcome,
    /// Why a harness error is one.
    #[serde(default)]
    pub reason: String,
    /// Every check with what it asked for and what it measured.
    #[serde(default)]
    pub checks: Vec<Verdict>,
    /// The agent's tool calls, by name.
    #[serde(default)]
    pub tool_calls: Vec<String>,
    /// The same calls with their arguments and whether each was refused.
    #[serde(default)]
    pub tool_log: Vec<CallLog>,
    /// Calls the editor refused.
    #[serde(default)]
    pub refusals: u32,
    /// Those refusals, by tool.
    #[serde(default)]
    pub refusals_by_tool: BTreeMap<String, u32>,
    /// Changes the agent took back.
    #[serde(default)]
    pub undos: u32,
    /// Files changed that no operation-log line explains.
    #[serde(default)]
    pub edits_behind: Vec<String>,
    /// Fresh input tokens.
    #[serde(default)]
    pub tokens_in: Option<u64>,
    /// Output tokens.
    #[serde(default)]
    pub tokens_out: Option<u64>,
    /// Cached input tokens read.
    #[serde(default)]
    pub tokens_cache_read: Option<u64>,
    /// Cached input tokens written.
    #[serde(default)]
    pub tokens_cache_write: Option<u64>,
    /// **A cost the harness itself reported**, in dollars, used only when the model has no listed price.
    #[serde(default)]
    pub cost_reported_usd: Option<f64>,
    /// How long the agent ran, seconds.
    #[serde(default)]
    pub wall_s: f64,
    /// The wishes the agent made, as `{tool, task}`.
    #[serde(default)]
    pub wishes: Vec<Value>,
    /// **Whether the agent was stopped at its tool-call budget** (#433); absent from a `trial.json` written before it was
    /// recorded, which reads as not stopped.
    #[serde(default)]
    pub call_budget_exhausted: bool,
}

fn dev_split() -> Split {
    Split::Dev
}

impl TrialData {
    /// **What this trial cost, in dollars**, or `None` when it cannot be said.
    ///
    /// The listed price times the tokens reported; the harness's own figure when no price is listed; and
    /// otherwise `None` — **never `0`**, because a trial nobody could price did not cost nothing.
    pub fn cost_usd(&self, price: Option<&Price>) -> Option<f64> {
        if let Some(price) = price {
            let (input, output) = (self.tokens_in?, self.tokens_out?);
            let cached = self.tokens_cache_read.unwrap_or(0);
            let written = self.tokens_cache_write.unwrap_or(0);
            return Some(
                (input as f64 * price.input
                    + output as f64 * price.output
                    + cached as f64 * price.cache_read
                    + written as f64 * price.cache_write)
                    / TOKENS_PER_PRICE,
            );
        }
        self.cost_reported_usd
    }

    /// Whether the harness, not the agent, failed.
    pub fn harness_error(&self) -> bool {
        self.outcome == Outcome::HarnessError
    }
}

/// **Every trial of a run and the header that says how it was made.**
#[derive(Clone, Debug)]
pub struct RunData {
    /// The header.
    pub header: Header,
    /// The trials, in task and trial order.
    pub trials: Vec<TrialData>,
    /// **The pre-flight turn's tokens**, when the run made one (#434): a paid turn no trial's cost holds.
    pub preflight: Option<PreflightUsage>,
}

impl RunData {
    /// **Read a run directory**: `run.json` and every `<task>/trial-<n>/trial.json` under it.
    ///
    /// A file that does not read is an error naming it. A report that skipped one would be a report of
    /// fewer trials than the run made.
    pub fn read(dir: &std::path::Path) -> Result<RunData, String> {
        let header_path = dir.join(RUN_FILE);
        let header: Header = serde_json::from_str(
            &std::fs::read_to_string(&header_path).map_err(|e| {
                format!(
                    "{} could not be read ({e}): a run directory holds the `{RUN_FILE}` that `cauldron-bench run` writes",
                    header_path.display()
                )
            })?,
        )
        .map_err(|e| format!("{}: {e}", header_path.display()))?;
        if header.version > REPORT_VERSION {
            return Err(format!(
                "{} is version {} but this build reads {REPORT_VERSION}",
                header_path.display(),
                header.version
            ));
        }
        let mut trials = Vec::new();
        let tasks = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for task in tasks.flatten() {
            if !task.path().is_dir() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(task.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path().join("trial.json");
                if !path.is_file() {
                    continue;
                }
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let trial: TrialData =
                    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                trials.push(trial);
            }
        }
        trials.sort_by(|a, b| a.task.cmp(&b.task).then(a.trial.cmp(&b.trial)));
        if trials.is_empty() {
            return Err(format!("{} holds no trial.json, so there is nothing to report", dir.display()));
        }
        let preflight = std::fs::read_to_string(dir.join(PREFLIGHT_FILE))
            .ok()
            .and_then(|text| serde_json::from_str::<PreflightUsage>(&text).ok());
        Ok(RunData { header, trials, preflight })
    }
}

/// **The first check that failed, with what it wanted and what it measured**, and the agent's last calls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    /// Which trial.
    pub trial: u32,
    /// The check's name.
    pub check: String,
    /// What it asked for.
    pub wanted: String,
    /// What it measured.
    pub measured: String,
    /// **The agent's last calls before it stopped**, oldest first, at most [`LAST_CALLS_SHOWN`].
    pub last_calls: Vec<String>,
}

/// **One task's numbers.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskReport {
    /// The task's id.
    pub task: String,
    /// Its title.
    pub title: String,
    /// Its tags.
    pub tags: Vec<String>,
    /// **Trials scored**: passes and fails, never a harness error.
    pub trials: u32,
    /// Trials that passed.
    pub successes: u32,
    /// Trials the harness failed.
    pub harness_errors: u32,
    /// Every trial's outcome, in trial order, which is what a paired comparison resamples.
    pub outcomes: Vec<Outcome>,
    /// `successes / trials`.
    pub success_rate: Option<f64>,
    /// **`pass^k` for `k = 1..trials`.**
    pub pass_hat: Vec<f64>,
    /// Median wall time of the scored trials, seconds.
    pub wall_median_s: Option<f64>,
    /// 90th-percentile wall time, seconds.
    pub wall_p90_s: Option<f64>,
    /// Mean tool calls per scored trial.
    pub tool_calls_mean: Option<f64>,
    /// Calls the editor refused, over the scored trials.
    pub refusals: u32,
    /// Changes the agent took back.
    pub undos: u32,
    /// Files changed behind the editor's back, summed over trials.
    pub edits_behind: u32,
    /// Trials that changed at least one.
    pub trials_with_edits_behind: u32,
    /// Input tokens, summed, or `None` when no scored trial reported any (never `0` for unknown).
    pub tokens_in: Option<u64>,
    /// Output tokens, summed, or `None` when no scored trial reported any.
    pub tokens_out: Option<u64>,
    /// **Input tokens served from the provider's cache, summed**, or `None` when no scored trial reported any (#432, D8).
    /// A report written before the field reads as unknown.
    #[serde(default)]
    pub tokens_cache_read: Option<u64>,
    /// **Input tokens written to the provider's cache, summed**, or `None` when no scored trial reported any.
    #[serde(default)]
    pub tokens_cache_write: Option<u64>,
    /// **Cost in dollars, or `None` for unknown** (a model with no listed price, or a trial that reported
    /// no tokens).
    pub cost_usd: Option<f64>,
    /// Wishes the agent made.
    pub wishes: u32,
    /// Each failed trial's first failing check and last calls.
    pub failures: Vec<Failure>,
    /// **Trials of this task the agent was stopped in at its tool-call budget** (#433).
    #[serde(default)]
    pub budget_exhausted: u32,
    /// Which trials those were, in order.
    #[serde(default)]
    pub budget_exhausted_trials: Vec<u32>,
}

/// **A group of tasks stated as one**: the suite, a tag, or the held-out total.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aggregate {
    /// `dev`, `held-out`, or a tag.
    pub name: String,
    /// Tasks with at least one scored trial.
    pub tasks: u32,
    /// Trials scored.
    pub trials: u32,
    /// Trials that passed.
    pub successes: u32,
    /// Trials the harness failed.
    pub harness_errors: u32,
    /// **Mean success over tasks** (each task's own rate, averaged).
    pub mean_success: Option<f64>,
    /// **Its 95% bootstrap interval**, tasks and trials within tasks resampled.
    pub interval: Option<Interval>,
    /// **Mean `pass^k`** for `k = 1..` the fewest scored trials any task has.
    pub mean_pass_hat: Vec<f64>,
    /// Total cost of the group's trials, or `None` for unknown.
    pub cost_usd: Option<f64>,
    /// **Cost per success**, or `None` when it cannot be said or nothing succeeded.
    pub cost_per_success_usd: Option<f64>,
    /// Mean wall time of the scored trials, seconds.
    pub wall_mean_s: Option<f64>,
    /// Mean tool calls of the scored trials.
    pub tool_calls_mean: Option<f64>,
    /// Files changed behind the editor's back, summed.
    pub edits_behind: u32,
}

/// **One trial the harness failed, and why** (#432, D1): what `trial.json` says in its `reason`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessErrorReason {
    /// The task's id (a held-out task is its ordinal, as in its `trial.json`).
    pub task: String,
    /// Which trial of it.
    pub trial: u32,
    /// **The cause, in the words the harness gave**: the provider's own for a refused turn.
    pub reason: String,
}

/// **A whole run's report.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The header.
    pub header: Header,
    /// **Why the run cannot be compared**, when it cannot.
    pub unfit: Option<String>,
    /// Trials the run made, harness errors included.
    pub trials_total: u32,
    /// Of those, the harness's own failures.
    pub harness_errors: u32,
    /// **The dev tasks as one**.
    pub dev: Aggregate,
    /// **The held-out tasks as one**, never by task: `None` when the run had none.
    pub held_out: Option<Aggregate>,
    /// The dev tasks by tag.
    pub tags: Vec<Aggregate>,
    /// Each dev task.
    pub tasks: Vec<TaskReport>,
    /// **How often each tool was called** over the dev trials.
    pub tool_counts: BTreeMap<String, u32>,
    /// **Refusals by tool** over the dev trials.
    pub refusals_by_tool: BTreeMap<String, u32>,
    /// **The wishes the agent made**, by the tool it wished for.
    pub wishes_by_tool: BTreeMap<String, u32>,
    /// **Every harness-error trial with its cause**, in task and trial order. Absent from a report written before
    /// the field existed, which then reads as none.
    #[serde(default)]
    pub harness_error_reasons: Vec<HarnessErrorReason>,
    /// **The pre-flight turn, priced** (#434), or `None` for a run that made none. Absent from an older report.
    #[serde(default)]
    pub preflight: Option<PreflightCost>,
    /// **What the whole run cost, in dollars**: every trial and the pre-flight turn, or `None` when any part is unknown
    /// (never `0` for unknown). Absent from an older report.
    #[serde(default)]
    pub cost_total_usd: Option<f64>,
}

/// **The sum of the counts that were reported**, or `None` when none was: a trial that reported no tokens
/// has unknown tokens, not zero.
fn sum_known(counts: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    counts.fold(None, |sum, count| match (sum, count) {
        (Some(a), Some(b)) => Some(a + b),
        (None, some) | (some, None) => some,
    })
}

/// **A token count for a table**, or `unknown`.
pub fn tokens_cell(count: Option<u64>) -> String {
    count.map_or_else(|| "unknown".to_owned(), |n| n.to_string())
}

/// **The sum of costs, or `None` when any is unknown.**
fn sum_costs(costs: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let mut total = 0.0;
    let mut any = false;
    for cost in costs {
        total += cost?;
        any = true;
    }
    any.then_some(total)
}

/// **What a wish names**: its `tool`, or the whole object when it names none.
fn wish_name(wish: &Value) -> String {
    wish.get("tool").and_then(Value::as_str).map_or_else(|| wish.to_string(), str::to_owned)
}

impl TaskReport {
    fn of(task: &str, trials: &[&TrialData], price: Option<&Price>) -> TaskReport {
        let scored: Vec<&&TrialData> = trials.iter().filter(|t| !t.harness_error()).collect();
        let n = scored.len() as u32;
        let c = scored.iter().filter(|t| t.outcome == Outcome::Pass).count() as u32;
        let first = trials.first();
        let walls: Vec<f64> = scored.iter().map(|t| t.wall_s).collect();
        let calls: Vec<f64> = scored.iter().map(|t| t.tool_calls.len() as f64).collect();
        let failures = scored
            .iter()
            .filter(|t| t.outcome == Outcome::Fail)
            .map(|t| failure_of(t))
            .collect();
        TaskReport {
            task: task.to_owned(),
            title: first.map(|t| t.title.clone()).unwrap_or_default(),
            tags: first.map(|t| t.tags.clone()).unwrap_or_default(),
            trials: n,
            successes: c,
            harness_errors: trials.len() as u32 - n,
            outcomes: trials.iter().map(|t| t.outcome).collect(),
            success_rate: (n > 0).then(|| f64::from(c) / f64::from(n)),
            pass_hat: stats::pass_hat_curve(n, c),
            wall_median_s: stats::median(&walls),
            wall_p90_s: stats::percentile(&walls, WALL_SLOW_PERCENTILE),
            tool_calls_mean: stats::mean(&calls),
            refusals: scored.iter().map(|t| t.refusals).sum(),
            undos: scored.iter().map(|t| t.undos).sum(),
            edits_behind: scored.iter().map(|t| t.edits_behind.len() as u32).sum(),
            trials_with_edits_behind: scored.iter().filter(|t| !t.edits_behind.is_empty()).count() as u32,
            tokens_in: sum_known(scored.iter().map(|t| t.tokens_in)),
            tokens_out: sum_known(scored.iter().map(|t| t.tokens_out)),
            tokens_cache_read: sum_known(scored.iter().map(|t| t.tokens_cache_read)),
            tokens_cache_write: sum_known(scored.iter().map(|t| t.tokens_cache_write)),
            cost_usd: if scored.is_empty() { None } else { sum_costs(scored.iter().map(|t| t.cost_usd(price))) },
            wishes: scored.iter().map(|t| t.wishes.len() as u32).sum(),
            failures,
            budget_exhausted: scored.iter().filter(|t| t.call_budget_exhausted).count() as u32,
            budget_exhausted_trials: scored.iter().filter(|t| t.call_budget_exhausted).map(|t| t.trial).collect(),
        }
    }

    /// **Each scored trial as `1.0` or `0.0`**, which is what a bootstrap resamples.
    pub fn trial_values(&self) -> Vec<f64> {
        self.outcomes
            .iter()
            .filter_map(|o| match o {
                Outcome::Pass => Some(1.0),
                Outcome::Fail => Some(0.0),
                Outcome::HarnessError => None,
            })
            .collect()
    }
}

/// **A failed trial's first failing check and the calls that led to it.**
fn failure_of(trial: &TrialData) -> Failure {
    let (check, wanted, measured) = trial
        .checks
        .iter()
        .find(|c| !c.passed)
        .map_or_else(
            || ("(none)".to_owned(), String::new(), "no check failed".to_owned()),
            |c| (c.check.clone(), c.asks.clone(), c.measured.clone()),
        );
    let from = trial.tool_log.len().saturating_sub(LAST_CALLS_SHOWN);
    let last_calls = trial.tool_log[from..]
        .iter()
        .map(|c| format!("{}{} {}", c.tool, if c.refused { " (refused)" } else { "" }, c.args))
        .collect();
    Failure { trial: trial.trial, check, wanted, measured, last_calls }
}

impl Aggregate {
    fn of(name: &str, tasks: &[&TaskReport], trials: &[&TrialData], price: Option<&Price>) -> Aggregate {
        let scored_tasks: Vec<&&TaskReport> = tasks.iter().filter(|t| t.trials > 0).collect();
        let values: Vec<Vec<f64>> = scored_tasks.iter().map(|t| t.trial_values()).collect();
        let boot = stats::bootstrap_mean(&values, BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED);
        let shortest = scored_tasks.iter().map(|t| t.trials).min().unwrap_or(0);
        let mean_pass_hat = (0..shortest as usize)
            .filter_map(|k| {
                stats::mean(&scored_tasks.iter().filter_map(|t| t.pass_hat.get(k).copied()).collect::<Vec<_>>())
            })
            .collect();
        let scored: Vec<&&TrialData> = trials.iter().filter(|t| !t.harness_error()).collect();
        let successes = scored.iter().filter(|t| t.outcome == Outcome::Pass).count() as u32;
        let cost = if scored.is_empty() { None } else { sum_costs(scored.iter().map(|t| t.cost_usd(price))) };
        Aggregate {
            name: name.to_owned(),
            tasks: scored_tasks.len() as u32,
            trials: scored.len() as u32,
            successes,
            harness_errors: (trials.len() - scored.len()) as u32,
            mean_success: boot.map(|(m, _)| m),
            interval: boot.map(|(_, i)| i),
            mean_pass_hat,
            cost_usd: cost,
            cost_per_success_usd: cost.filter(|_| successes > 0).map(|c| c / f64::from(successes)),
            wall_mean_s: stats::mean(&scored.iter().map(|t| t.wall_s).collect::<Vec<_>>()),
            tool_calls_mean: stats::mean(&scored.iter().map(|t| t.tool_calls.len() as f64).collect::<Vec<_>>()),
            edits_behind: scored.iter().map(|t| t.edits_behind.len() as u32).sum(),
        }
    }
}

/// **Group trials by task id**, in id order.
fn by_task<'a>(trials: &[&'a TrialData]) -> BTreeMap<&'a str, Vec<&'a TrialData>> {
    let mut out: BTreeMap<&str, Vec<&TrialData>> = BTreeMap::new();
    for trial in trials {
        out.entry(trial.task.as_str()).or_default().push(trial);
    }
    out
}

impl Report {
    /// **Build the report of a run.**
    pub fn build(run: &RunData) -> Report {
        let price = run.header.price.as_ref();
        let dev_trials: Vec<&TrialData> = run.trials.iter().filter(|t| t.split == Split::Dev).collect();
        let held_trials: Vec<&TrialData> = run.trials.iter().filter(|t| t.split == Split::HeldOut).collect();
        let dev_tasks: Vec<TaskReport> =
            by_task(&dev_trials).into_iter().map(|(id, ts)| TaskReport::of(id, &ts, price)).collect();
        let held_tasks: Vec<TaskReport> =
            by_task(&held_trials).into_iter().map(|(id, ts)| TaskReport::of(id, &ts, price)).collect();
        let dev_refs: Vec<&TaskReport> = dev_tasks.iter().collect();
        let held_refs: Vec<&TaskReport> = held_tasks.iter().collect();

        // **Per tag, over dev tasks only**: a tag's number over held-out tasks too would be a per-task
        // number for the one held-out task that carries the tag.
        let mut tag_names: Vec<&str> =
            dev_tasks.iter().flat_map(|t| t.tags.iter().map(String::as_str)).collect();
        tag_names.sort_unstable();
        tag_names.dedup();
        let tags = tag_names
            .into_iter()
            .map(|tag| {
                let members: Vec<&TaskReport> =
                    dev_tasks.iter().filter(|t| t.tags.iter().any(|x| x == tag)).collect();
                let their_trials: Vec<&TrialData> = dev_trials
                    .iter()
                    .copied()
                    .filter(|t| t.tags.iter().any(|x| x == tag))
                    .collect();
                Aggregate::of(tag, &members, &their_trials, price)
            })
            .collect();

        let mut tool_counts = BTreeMap::new();
        let mut refusals_by_tool: BTreeMap<String, u32> = BTreeMap::new();
        let mut wishes_by_tool = BTreeMap::new();
        for trial in dev_trials.iter().filter(|t| !t.harness_error()) {
            for call in &trial.tool_calls {
                *tool_counts.entry(call.clone()).or_insert(0u32) += 1;
            }
            for (tool, count) in &trial.refusals_by_tool {
                *refusals_by_tool.entry(tool.clone()).or_default() += count;
            }
            for wish in &trial.wishes {
                *wishes_by_tool.entry(wish_name(wish)).or_insert(0u32) += 1;
            }
        }

        let harness_errors = run.trials.iter().filter(|t| t.harness_error()).count() as u32;
        let trials_total = run.trials.len() as u32;
        let fraction = f64::from(harness_errors) / f64::from(trials_total.max(1));
        let unfit = (fraction > HARNESS_ERROR_MAX_FRACTION).then(|| {
            format!(
                "{harness_errors} of {trials_total} trials are harness errors ({:.0}%), over the {:.0}% a \
                 comparison tolerates: these numbers are about the harness, not the agent",
                fraction * 100.0,
                HARNESS_ERROR_MAX_FRACTION * 100.0
            )
        });
        let dev = Aggregate::of("dev", &dev_refs, &dev_trials, price);
        let held_out = (!held_trials.is_empty()).then(|| Aggregate::of("held-out", &held_refs, &held_trials, price));
        let preflight = run.preflight.map(|usage| PreflightCost {
            tokens_in: usage.input,
            tokens_out: usage.output,
            tokens_cache_read: usage.cache_read,
            tokens_cache_write: usage.cache_write,
            cost_usd: price.map(|p| {
                (usage.input as f64 * p.input
                    + usage.output as f64 * p.output
                    + usage.cache_read as f64 * p.cache_read
                    + usage.cache_write as f64 * p.cache_write)
                    / TOKENS_PER_PRICE
            }),
        });
        let cost_total_usd = sum_costs(
            std::iter::once(dev.cost_usd)
                .chain(held_out.as_ref().map(|h| h.cost_usd))
                .chain(preflight.as_ref().map(|p| p.cost_usd)),
        );
        Report {
            header: run.header.clone(),
            unfit,
            trials_total,
            harness_errors,
            dev,
            held_out,
            tags,
            tasks: dev_tasks,
            tool_counts,
            refusals_by_tool,
            wishes_by_tool,
            harness_error_reasons: run
                .trials
                .iter()
                .filter(|t| t.harness_error())
                .map(|t| HarnessErrorReason { task: t.task.clone(), trial: t.trial, reason: t.reason.clone() })
                .collect(),
            preflight,
            cost_total_usd,
        }
    }

    /// **The JSON a committed report's `.json` holds**, and what [`crate::compare`] reads.
    pub fn json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map(|text| text + "\n")
            .map_err(|e| format!("the report could not be written as JSON: {e}"))
    }

    /// **A report from a path**: a `.json` a run published, or a run directory the report is built from.
    pub fn load(path: &std::path::Path) -> Result<Report, String> {
        if path.is_dir() {
            return Ok(Report::build(&RunData::read(path)?));
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Report::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// **Read a report from the JSON [`Self::json`] wrote.**
    pub fn from_json(text: &str) -> Result<Report, String> {
        let report: Report = serde_json::from_str(text).map_err(|e| format!("a report did not read: {e}"))?;
        if report.header.version > REPORT_VERSION {
            return Err(format!(
                "a report is version {} but this build reads {REPORT_VERSION}",
                report.header.version
            ));
        }
        Ok(report)
    }
}

// -------------------------------------------------------------------------------------------------
// Markdown.
// -------------------------------------------------------------------------------------------------

/// **A fraction as a percentage**, or `-` for none.
pub fn pct(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |x| format!("{:.0}%", x * 100.0))
}

/// **Dollars**, or `unknown` — never `$0` for a cost nobody could state.
pub fn usd(x: Option<f64>) -> String {
    x.map_or_else(|| "unknown".to_owned(), |x| format!("${x:.4}"))
}

/// **Seconds**, or `-`.
pub fn secs(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |x| format!("{x:.0}"))
}

/// **A number with one decimal**, or `-`.
pub fn one(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |x| format!("{x:.1}"))
}

/// **`pass^k` values as `k=1 0.60, k=2 0.30`**.
pub fn curve(values: &[f64]) -> String {
    if values.is_empty() {
        return "-".to_owned();
    }
    values.iter().enumerate().map(|(i, v)| format!("{}:{v:.2}", i + 1)).collect::<Vec<_>>().join(" ")
}

/// **A success rate with its interval**: `60% [40%, 80%]`.
pub fn with_interval(mean: Option<f64>, interval: Option<Interval>) -> String {
    match (mean, interval) {
        (Some(m), Some(i)) => format!("{} [{}, {}]", pct(Some(m)), pct(Some(i.lo)), pct(Some(i.hi))),
        (Some(m), None) => pct(Some(m)),
        _ => "-".to_owned(),
    }
}

/// **A cell's text made safe for a table**: no pipe, no newline.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// **Text the report quotes, made inert** (#433): a provider's error body, an agent's version string, a model's name.
///
/// HTML's `&`, `<` and `>` become entities, and markdown's own punctuation (backslash, backtick, `*`, brackets and
/// parentheses, which make code, emphasis, links and images) is backslash-escaped, so what was quoted renders as the
/// text it is. The report's own markup is never passed through this. A pipe and a newline are [`cell`]'s.
pub fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\\' | '`' | '*' | '[' | ']' | '(' | ')' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

/// **The tools of a count map, most called first**, as `name ×n`.
pub fn top(counts: &BTreeMap<String, u32>, shown: usize) -> String {
    if counts.is_empty() {
        return "none".to_owned();
    }
    let mut entries: Vec<(&String, &u32)> = counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    entries.into_iter().take(shown).map(|(name, n)| format!("`{name}` ×{n}")).collect::<Vec<_>>().join(", ")
}

impl Report {
    /// **The "Tool-call budget" section: the trials whose agent was stopped for making a call beyond its task's budget** (#433).
    ///
    /// Empty when none was. Such a trial was graded as it stood, so its failure may be the cut-off and not the agent's idea.
    fn budget_markdown(&self) -> String {
        let stopped: Vec<String> = self
            .tasks
            .iter()
            .flat_map(|t| t.budget_exhausted_trials.iter().map(move |n| format!("`{}` trial {n}", t.task)))
            .collect();
        if stopped.is_empty() {
            return String::new();
        }
        format!(
            "## Tool-call budget\n\n{} trial(s) were stopped when the agent made a call beyond its task's `budget.tool_calls`, \
             and graded as they stood: {}.\n\n",
            stopped.len(),
            stopped.join(", ")
        )
    }

    /// **The "Harness errors" section: every distinct cause, how many trials had it, and which** (#432, D1).
    ///
    /// Empty for a run with none. Trials with the same reason are one row (a hundred refused turns are one
    /// sentence, not a hundred), naming the first [`HARNESS_ERROR_TRIALS_SHOWN`] of them; `report.json` keeps one row each.
    fn harness_errors_markdown(&self) -> String {
        if self.harness_error_reasons.is_empty() {
            return String::new();
        }
        let mut by_reason: Vec<(&str, Vec<&HarnessErrorReason>)> = Vec::new();
        for row in &self.harness_error_reasons {
            match by_reason.iter_mut().find(|(reason, _)| *reason == row.reason) {
                Some((_, rows)) => rows.push(row),
                None => by_reason.push((row.reason.as_str(), vec![row])),
            }
        }
        by_reason.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        let mut out = String::from("## Harness errors\n\nWhy the harness, and not the agent, failed these trials:\n\n");
        out.push_str("| trials | reason | which |\n|---|---|---|\n");
        for (reason, rows) in by_reason {
            let named: Vec<String> = rows
                .iter()
                .take(HARNESS_ERROR_TRIALS_SHOWN)
                .map(|r| format!("`{}` trial {}", r.task, r.trial))
                .collect();
            let more = rows.len().saturating_sub(HARNESS_ERROR_TRIALS_SHOWN);
            let which = if more == 0 { named.join(", ") } else { format!("{}, and {more} more", named.join(", ")) };
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                rows.len(),
                if reason.is_empty() { "no reason was recorded".to_owned() } else { cell(&quoted(reason)) },
                cell(&which)
            ));
        }
        out.push('\n');
        out
    }

    /// **The report a person reads**, as Markdown.
    pub fn markdown(&self) -> String {
        let h = &self.header;
        let mut out = String::new();
        out.push_str(&format!("# Benchmark run: {}\n\n", h.label));
        if let Some(why) = &self.unfit {
            out.push_str(&format!("> **Unfit for comparison.** {why}.\n\n"));
        }
        out.push_str("| | |\n|---|---|\n");
        let model = h.model.as_deref().map_or_else(|| "none (no model)".to_owned(), quoted);
        for (name, value) in [
            ("Editor and MCP server commit", format!("`{}`", h.commit)),
            ("Shipped guide SHA-256", format!("`{}`", h.guide_sha256)),
            ("Agent", format!("{} {}", quoted(&h.agent), quoted(&h.agent_version))),
            ("Model", model),
            ("Variant", quoted(&h.variant.clone().unwrap_or_else(|| "none".to_owned()))),
            ("Trials per task", h.trials.to_string()),
            ("Suite", quoted(&h.suite)),
            ("Host", quoted(&h.host)),
            ("Date", h.date.clone()),
            (
                "Price per million tokens",
                match &h.price {
                    Some(p) => format!(
                        "input ${}, output ${}, cache read ${}, cache write ${}",
                        p.input, p.output, p.cache_read, p.cache_write
                    ),
                    None => "none listed, so cost is unknown".to_owned(),
                },
            ),
        ] {
            out.push_str(&format!("| {name} | {} |\n", cell(&value)));
        }
        out.push_str(&format!(
            "\n{} trial(s), {} harness error(s). A harness error is never scored as a pass or a fail.\n\n",
            self.trials_total, self.harness_errors
        ));
        if let Some(p) = &self.preflight {
            out.push_str(&format!(
                "Pre-flight turn (one real model turn before any trial, in no trial's cost): {} fresh in, {} out, {} cache read, {} cache written, {}. \
                 **Cost of the run, trials and pre-flight turn: {}.**\n\n",
                p.tokens_in,
                p.tokens_out,
                p.tokens_cache_read,
                p.tokens_cache_write,
                usd(p.cost_usd),
                usd(self.cost_total_usd)
            ));
        }
        out.push_str(&self.budget_markdown());
        out.push_str(&self.harness_errors_markdown());

        out.push_str("## The suite\n\n");
        out.push_str("| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|\n");
        let aggregates: Vec<&Aggregate> = std::iter::once(&self.dev).chain(self.held_out.as_ref()).collect();
        for a in aggregates {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                if a.name == "dev" { "dev" } else { "held-out (total only)" },
                a.tasks,
                a.trials,
                a.successes,
                with_interval(a.mean_success, a.interval),
                curve(&a.mean_pass_hat),
                usd(a.cost_usd),
                usd(a.cost_per_success_usd),
                a.harness_errors
            ));
        }

        if !self.tags.is_empty() {
            out.push_str("\n## By tag (dev tasks)\n\n");
            out.push_str("| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |\n");
            out.push_str("|---|---|---|---|---|---|---|---|\n");
            for a in &self.tags {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                    a.name,
                    a.tasks,
                    a.trials,
                    with_interval(a.mean_success, a.interval),
                    curve(&a.mean_pass_hat),
                    usd(a.cost_usd),
                    usd(a.cost_per_success_usd),
                    a.harness_errors
                ));
            }
        }

        out.push_str("\n## By task (dev tasks)\n\n");
        out.push_str("| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        for t in &self.tasks {
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} (cache {}/{}) | {} | {} | {} |\n",
                t.task,
                t.trials,
                t.successes,
                pct(t.success_rate),
                curve(&t.pass_hat),
                secs(t.wall_median_s),
                secs(t.wall_p90_s),
                one(t.tool_calls_mean),
                t.refusals,
                t.undos,
                if t.edits_behind == 0 {
                    "0".to_owned()
                } else {
                    format!("{} file(s) in {} trial(s)", t.edits_behind, t.trials_with_edits_behind)
                },
                tokens_cell(t.tokens_in),
                tokens_cell(t.tokens_out),
                tokens_cell(t.tokens_cache_read),
                tokens_cell(t.tokens_cache_write),
                usd(t.cost_usd),
                t.wishes,
                t.harness_errors
            ));
        }

        let failed: Vec<&TaskReport> = self.tasks.iter().filter(|t| !t.failures.is_empty()).collect();
        if !failed.is_empty() {
            out.push_str("\n## Failed trials (dev tasks)\n");
            for t in failed {
                for f in &t.failures {
                    out.push_str(&format!("\n### `{}`, trial {}\n\n", t.task, f.trial));
                    out.push_str(&format!(
                        "- first failing check: `{}` — wanted {}; measured {}\n",
                        f.check.replace('`', "'"),
                        cell(&quoted(&f.wanted)),
                        cell(&quoted(&f.measured))
                    ));
                    if f.last_calls.is_empty() {
                        out.push_str("- the agent made no tool call\n");
                    } else {
                        out.push_str(&format!("- its last {} call(s), oldest first:\n", f.last_calls.len()));
                        for call in &f.last_calls {
                            out.push_str(&format!("  - `{}`\n", call.replace('`', "'")));
                        }
                    }
                }
            }
        }

        out.push_str("\n## Where the agent stumbles (dev tasks)\n\n");
        out.push_str(&format!("- tools called most: {}\n", top(&self.tool_counts, TOP_TOOLS_SHOWN)));
        out.push_str(&format!("- refusals by tool: {}\n", top(&self.refusals_by_tool, TOP_TOOLS_SHOWN)));
        out.push_str(&format!("- wishes, by the tool wished for: {}\n", top(&self.wishes_by_tool, TOP_TOOLS_SHOWN)));
        out
    }
}

/// **The pre-flight turn's tokens, as `preflight-usage.json` holds them** (#434).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct PreflightUsage {
    /// Fresh input tokens.
    #[serde(default)]
    pub input: u64,
    /// Output tokens.
    #[serde(default)]
    pub output: u64,
    /// Input tokens served from the provider's cache.
    #[serde(default)]
    pub cache_read: u64,
    /// Input tokens written to the provider's cache.
    #[serde(default)]
    pub cache_write: u64,
}

/// **The pre-flight turn in a report**: its tokens and what they cost at the model's listed price (#434).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreflightCost {
    /// Fresh input tokens.
    pub tokens_in: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// Input tokens read from the cache.
    pub tokens_cache_read: u64,
    /// Input tokens written to the cache.
    pub tokens_cache_write: u64,
    /// **Dollars**, or `None` when the model has no listed price (never `0` for unknown).
    pub cost_usd: Option<f64>,
}
