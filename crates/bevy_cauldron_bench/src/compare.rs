//! **What changed between two runs, and whether it is more than noise** (BM-03, #413).
//!
//! `cauldron-bench compare <a> <b>` answers one question about the suite: is the mean success of `b`
//! better than `a`'s, worse, or **not measurably different**. The answer is a verdict and the interval it
//! rests on, never a bare difference, because five trials of a task are a small sample and a difference
//! the interval includes zero in is a difference the runs cannot tell from luck.
//!
//! # Paired, because the tasks are the same
//!
//! Both runs attempted the same tasks, so the comparison resamples **tasks once and applies the same
//! draw to both runs** ([`stats::paired_bootstrap`]). A task both runs find hard cancels out of the
//! difference instead of widening it, which is what lets a real change show through on twenty tasks.
//!
//! # What it refuses
//!
//! - **A run that is unfit** ([`Report::unfit`]): more than [`HARNESS_ERROR_MAX_FRACTION`] of its trials
//!   were the harness failing, and a comparison would measure the harness.
//! - **Two runs of different suites, agents or trial counts**, unless `--across` is given, which says which
//!   of those differ. A difference in success between two agents is a fact about two agents; the flag is
//!   the person saying that is what they meant to ask.
//!
//! **A single task's move is listed and never called significant**: one task's five trials cannot carry
//! a verdict, and the list is there so a person can read what moved, not so it can be cited as a result.
//!
//! **Held-out tasks are a total on each side and are not paired**: naming them to pair them is what a
//! held-out set exists to prevent.

use serde::{Deserialize, Serialize};

use crate::report::{pct, with_interval, Report, HARNESS_ERROR_MAX_FRACTION};
use crate::stats::{self, Interval, BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED};

/// **The one verdict a comparison gives.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// `b` succeeds more than `a`, and the interval is wholly above zero.
    Better,
    /// `b` succeeds less than `a`, and the interval is wholly below zero.
    Worse,
    /// **The interval includes zero.**
    NoMeasurableChange,
}

impl Change {
    /// **The words a report writes.**
    pub fn words(self) -> &'static str {
        match self {
            Change::Better => "better",
            Change::Worse => "worse",
            Change::NoMeasurableChange => "no measurable change",
        }
    }

    /// **The verdict an interval of the difference `b − a` gives.**
    pub fn of(interval: &Interval) -> Change {
        if interval.lo > 0.0 {
            Change::Better
        } else if interval.hi < 0.0 {
            Change::Worse
        } else {
            Change::NoMeasurableChange
        }
    }
}

/// **One task whose success moved**, with both counts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMove {
    /// The task's id.
    pub task: String,
    /// `a`'s successes.
    pub a_successes: u32,
    /// `a`'s scored trials.
    pub a_trials: u32,
    /// `b`'s successes.
    pub b_successes: u32,
    /// `b`'s scored trials.
    pub b_trials: u32,
}

/// **The comparison of two runs.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// What `a` is called: its label and its agent.
    pub a: String,
    /// What `b` is called.
    pub b: String,
    /// **What differs between the two runs** (suite, agent, trials), which `--across` accepted. Empty
    /// when they are alike.
    pub across: Vec<String>,
    /// How many dev tasks both runs scored.
    pub tasks_paired: u32,
    /// `b`'s mean success less `a`'s, over the paired tasks.
    pub difference: f64,
    /// **The paired 95% bootstrap interval of that difference.**
    pub interval: Interval,
    /// **The verdict.**
    pub change: Change,
    /// **Every task whose success moved**, each a single task and not significant on its own.
    pub moved: Vec<TaskMove>,
    /// `a`'s held-out mean success, as a total.
    pub a_held_out: Option<f64>,
    /// `b`'s held-out mean success, as a total.
    pub b_held_out: Option<f64>,
}

/// **What differs between two reports' suites, agents and trial counts.**
pub fn differences(a: &Report, b: &Report) -> Vec<String> {
    let mut out = Vec::new();
    let ids = |r: &Report| r.tasks.iter().map(|t| t.task.clone()).collect::<Vec<_>>();
    if a.header.suite != b.header.suite || ids(a) != ids(b) {
        out.push(format!(
            "the suites differ ({} task(s) in `{}` against {} in `{}`)",
            a.tasks.len(),
            a.header.suite,
            b.tasks.len(),
            b.header.suite
        ));
    }
    if a.header.pair() != b.header.pair() {
        out.push(format!("the agents differ (`{}` against `{}`)", a.header.pair(), b.header.pair()));
    }
    if a.header.trials != b.header.trials {
        out.push(format!("the trials per task differ ({} against {})", a.header.trials, b.header.trials));
    }
    out
}

/// **Compare `b` against `a`.**
///
/// `across` accepts a difference of suite, agent or trial count. An unfit run is refused either way.
pub fn compare(a: &Report, b: &Report, across: bool) -> Result<Comparison, String> {
    for (name, report) in [("first", a), ("second", b)] {
        if let Some(why) = &report.unfit {
            return Err(format!(
                "the {name} run (`{}`) is unfit for comparison and will not be compared: {why}. Rerun it; a \
                 run is fit when at most {:.0}% of its trials are harness errors",
                report.header.label,
                HARNESS_ERROR_MAX_FRACTION * 100.0
            ));
        }
    }
    let differ = differences(a, b);
    if !differ.is_empty() && !across {
        return Err(format!(
            "these runs are not alike: {}. Compare them anyway with --across, which says so in the answer",
            differ.join("; ")
        ));
    }
    let mut pairs = Vec::new();
    let mut moved = Vec::new();
    for ta in &a.tasks {
        let Some(tb) = b.tasks.iter().find(|t| t.task == ta.task) else {
            continue;
        };
        let (va, vb) = (ta.trial_values(), tb.trial_values());
        if va.is_empty() || vb.is_empty() {
            continue;
        }
        if ta.successes != tb.successes || ta.trials != tb.trials {
            moved.push(TaskMove {
                task: ta.task.clone(),
                a_successes: ta.successes,
                a_trials: ta.trials,
                b_successes: tb.successes,
                b_trials: tb.trials,
            });
        }
        pairs.push((va, vb));
    }
    let Some((difference, interval)) = stats::paired_bootstrap(&pairs, BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED)
    else {
        return Err("the two runs scored no task in common, so there is nothing to pair".to_owned());
    };
    Ok(Comparison {
        a: format!("{} ({})", a.header.label, a.header.pair()),
        b: format!("{} ({})", b.header.label, b.header.pair()),
        across: differ,
        tasks_paired: pairs.len() as u32,
        difference,
        interval,
        change: Change::of(&interval),
        moved,
        a_held_out: a.held_out.as_ref().and_then(|h| h.mean_success),
        b_held_out: b.held_out.as_ref().and_then(|h| h.mean_success),
    })
}

impl Comparison {
    /// **The answer a person reads.**
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# {} against {}\n\n", self.a, self.b));
        if !self.across.is_empty() {
            out.push_str(&format!("> **Compared across**: {}.\n\n", self.across.join("; ")));
        }
        out.push_str(&format!(
            "**{}.** Over {} task(s) both runs scored, `b`'s mean success is {:+.1} points against `a`'s, \
             with a paired 95% bootstrap interval of [{:+.1}, {:+.1}] points.\n\n",
            self.change.words(),
            self.tasks_paired,
            self.difference * 100.0,
            self.interval.lo * 100.0,
            self.interval.hi * 100.0
        ));
        if self.moved.is_empty() {
            out.push_str("No task's success count moved.\n");
        } else {
            out.push_str(
                "Tasks whose success moved. **Each is a single task and is not significant on its own**; \
                 the verdict above is about the suite.\n\n| task | a | b |\n|---|---|---|\n",
            );
            for m in &self.moved {
                out.push_str(&format!(
                    "| `{}` | {}/{} | {}/{} |\n",
                    m.task, m.a_successes, m.a_trials, m.b_successes, m.b_trials
                ));
            }
        }
        if self.a_held_out.is_some() || self.b_held_out.is_some() {
            out.push_str(&format!(
                "\nHeld-out tasks, as a total on each side and not paired: `a` {}, `b` {}.\n",
                pct(self.a_held_out),
                pct(self.b_held_out)
            ));
        }
        out
    }
}

/// **Both runs' headline, for a line a matrix or an index can reuse.**
pub fn headline(report: &Report) -> String {
    format!(
        "{} over {} task(s)",
        with_interval(report.dev.mean_success, report.dev.interval),
        report.dev.tasks
    )
}
