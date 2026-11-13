//! **A run ends with a report, and `compare` says what changed between two** (BM-03, #413).
//!
//! Every test here builds its reports from **recorded `trial.json` fixtures** written into a scratch
//! directory: no model, no editor, no network. That is the point of #413's first acceptance item. A report
//! is a function of the files a run left, so its arithmetic can be checked against values worked out by
//! hand, and a comparison can be given two runs whose difference the test chose.
//!
//! The values asserted are written out where they are used, with the sum that makes them, so a reader can
//! check the test's own arithmetic without trusting the code under test.

use std::path::{Path, PathBuf};

use bevy_cauldron_bench::compare::{compare, Change};
use bevy_cauldron_bench::grade::{edits_behind_the_back, Snapshot};
use bevy_cauldron_bench::index;
use bevy_cauldron_bench::probe::civil_date;
use bevy_cauldron_bench::report::{
    Header, Price, Report, RunData, HARNESS_ERROR_MAX_FRACTION, LAST_CALLS_SHOWN, REPORT_VERSION, RUN_FILE,
};
use bevy_cauldron_bench::run::{is_refusal, read_transcript};
use bevy_cauldron_bench::stats::{bootstrap_mean, pass_hat_k};
use serde_json::{json, Value};

// -------------------------------------------------------------------------------------------------
// Fixtures.
// -------------------------------------------------------------------------------------------------

/// **A scratch directory that includes the test's name and this process**, removed first.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn header(label: &str) -> Header {
    Header {
        version: REPORT_VERSION,
        label: label.to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        guide_sha256: "ab".repeat(32),
        agent: "omp".to_owned(),
        agent_version: "omp/18.18.9".to_owned(),
        model: Some("ollama-cloud/deepseek-v4.1-flash:1017".to_owned()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: 5,
        host: "big".to_owned(),
        date: "2026-11-13".to_owned(),
        price: None,
    }
}

/// **One trial's `trial.json`**, in the shape `cauldron-bench run` writes.
struct Trial {
    task: String,
    n: u32,
    outcome: &'static str,
    split: &'static str,
    tags: Vec<&'static str>,
    wall_s: f64,
    calls: Vec<(&'static str, bool)>,
    undos: u32,
    edits_behind: Vec<&'static str>,
    tokens: Option<(u64, u64, u64)>,
    wishes: Vec<Value>,
    checks: Vec<Value>,
}

fn trial(task: &str, n: u32, outcome: &'static str) -> Trial {
    Trial {
        task: task.to_owned(),
        n,
        outcome,
        split: "dev",
        tags: vec!["place"],
        wall_s: 10.0,
        calls: vec![("grimoire_state", false), ("grimoire_present", false)],
        undos: 0,
        edits_behind: Vec::new(),
        tokens: None,
        wishes: Vec::new(),
        checks: Vec::new(),
    }
}

impl Trial {
    fn json(&self) -> Value {
        let log: Vec<Value> = self
            .calls
            .iter()
            .map(|(tool, refused)| json!({"tool": tool, "args": "{}", "refused": refused}))
            .collect();
        let mut refused_by_tool = serde_json::Map::new();
        for (tool, refused) in &self.calls {
            if *refused {
                let n = refused_by_tool.get(*tool).and_then(Value::as_u64).unwrap_or(0);
                refused_by_tool.insert((*tool).to_owned(), json!(n + 1));
            }
        }
        json!({
            "task": self.task,
            "trial": self.n,
            "title": format!("title of {}", self.task),
            "split": self.split,
            "tags": self.tags,
            "outcome": self.outcome,
            "reason": if self.outcome == "harness_error" { "the editor did not start" } else { "" },
            "checks": self.checks,
            "tool_calls": self.calls.iter().map(|(t, _)| t).collect::<Vec<_>>(),
            "tool_log": log,
            "refusals": self.calls.iter().filter(|(_, r)| *r).count(),
            "refusals_by_tool": refused_by_tool,
            "undos": self.undos,
            "edits_behind": self.edits_behind,
            "tokens_in": self.tokens.map(|t| t.0),
            "tokens_out": self.tokens.map(|t| t.1),
            "tokens_cache_read": self.tokens.map(|t| t.2),
            "wall_s": self.wall_s,
            "wishes": self.wishes,
            "prompt": format!("PROMPT-OF-{}", self.task),
        })
    }
}

/// **Write a run directory**: `run.json` and `<task>/trial-<n>/trial.json`.
fn write_run(dir: &Path, header: &Header, trials: &[Trial]) {
    std::fs::create_dir_all(dir).expect("run dir");
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(header).expect("header json")).expect("run.json");
    for t in trials {
        let d = dir.join(&t.task).join(format!("trial-{}", t.n));
        std::fs::create_dir_all(&d).expect("trial dir");
        std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&t.json()).expect("trial json"))
            .expect("trial.json");
    }
}

fn report_of(dir: &Path, header: &Header, trials: &[Trial]) -> Report {
    write_run(dir, header, trials);
    Report::build(&RunData::read(dir).expect("the run reads"))
}

/// **`successes` passes then fails, `n` trials in all**, for one task.
fn task_with(task: &str, n: u32, successes: u32) -> Vec<Trial> {
    (1..=n).map(|i| trial(task, i, if i <= successes { "pass" } else { "fail" })).collect()
}

/// **A suite of `tasks` tasks of 5 trials each**, task `i` passing `counts[i]` times.
fn suite_of(counts: &[u32]) -> Vec<Trial> {
    counts.iter().enumerate().flat_map(|(i, c)| task_with(&format!("task-{i:02}"), 5, *c)).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// -------------------------------------------------------------------------------------------------
// pass^k against hand-computed values (#413, acceptance 1).
// -------------------------------------------------------------------------------------------------

#[test]
fn pass_hat_k_is_the_hand_computed_value_for_c_0_3_and_5_of_n_5() {
    let dir = scratch("pass-hat");
    let mut trials = task_with("never", 5, 0);
    trials.extend(task_with("sometimes", 5, 3));
    trials.extend(task_with("always", 5, 5));
    let report = report_of(&dir, &header("pass-hat"), &trials);
    let by = |id: &str| report.tasks.iter().find(|t| t.task == id).expect("the task is reported").pass_hat.clone();

    // c = 0: no k trials can all have passed.
    assert_eq!(by("never"), vec![0.0; 5], "c = 0 of n = 5 is pass^k = 0 for every k");
    // c = 3, n = 5: C(3,k) / C(5,k) = 3/5, 3/10, 1/10, and 0 once k exceeds c.
    let sometimes = by("sometimes");
    for (k, want) in [(1, 3.0 / 5.0), (2, 3.0 / 10.0), (3, 1.0 / 10.0), (4, 0.0), (5, 0.0)] {
        assert!(
            near(sometimes[k - 1], want),
            "c = 3 of n = 5, k = {k}: the report says {} and C(3,{k})/C(5,{k}) is {want}",
            sometimes[k - 1]
        );
    }
    // c = 5, n = 5: C(5,k) / C(5,k) = 1.
    assert_eq!(by("always"), vec![1.0; 5], "c = 5 of n = 5 is pass^k = 1 for every k");
    // The function the report calls says the same, so the table above is not the report agreeing with
    // itself.
    assert!(near(pass_hat_k(5, 3, 2), 0.3));
    assert!(near(pass_hat_k(5, 3, 3), 0.1));
}

// -------------------------------------------------------------------------------------------------
// The report's header and per-task numbers.
// -------------------------------------------------------------------------------------------------

#[test]
fn the_header_names_everything_a_reader_needs_to_reproduce_the_run() {
    let dir = scratch("header");
    let mut h = header("repro");
    h.variant = Some("no-frame-view".to_owned());
    let report = report_of(&dir, &h, &task_with("only", 5, 5));
    let md = report.markdown();
    for (what, needle) in [
        ("the editor and MCP server commit", h.commit.as_str()),
        ("the shipped guide's SHA-256", h.guide_sha256.as_str()),
        ("the agent", "omp"),
        ("the agent's version", "omp/18.18.9"),
        ("the model", "ollama-cloud/deepseek-v4.1-flash:1017"),
        ("the variant", "no-frame-view"),
        ("the host", "big"),
        ("the date", "2026-11-13"),
        ("the trials per task", "| Trials per task | 5 |"),
    ] {
        assert!(md.contains(needle), "the report's header must name {what} (`{needle}`); it says:\n{md}");
    }
}

#[test]
fn a_task_row_states_time_calls_refusals_undos_edits_tokens_wishes_and_cost() {
    let dir = scratch("task-row");
    let mut h = header("task-row");
    // $1 per million fresh input, $2 output, $0.5 cache read.
    h.price = Some(Price { input: 1.0, output: 2.0, cache_read: 0.5, cache_write: 0.0 });
    let walls = [10.0, 20.0, 30.0, 40.0, 50.0];
    let mut trials = Vec::new();
    for (i, wall) in walls.iter().enumerate() {
        let mut t = trial("measured", i as u32 + 1, if i < 4 { "pass" } else { "fail" });
        t.wall_s = *wall;
        t.calls = vec![("grimoire_state", false), ("grimoire_apply_operation", true), ("grimoire_present", false)];
        t.tokens = Some((1_000_000, 500_000, 2_000_000));
        if i == 0 {
            t.undos = 2;
            t.edits_behind = vec!["notes.txt", "assets/x.txt"];
            t.wishes = vec![json!({"tool": "grimoire_x", "task": "wish"})];
        }
        trials.push(t);
    }
    let report = report_of(&dir, &h, &trials);
    let t = &report.tasks[0];

    assert_eq!((t.trials, t.successes), (5, 4), "n and c");
    assert!(near(t.success_rate.expect("rate"), 0.8), "c/n is 4/5");
    // Walls 10, 20, 30, 40, 50: the median is the 3rd (30); p90 sits 0.9 of the way along 5 values, at
    // position 0.9 × 4 = 3.6, which is 40 + 0.6 × (50 − 40) = 46.
    assert!(near(t.wall_median_s.expect("median"), 30.0), "median wall");
    assert!(near(t.wall_p90_s.expect("p90"), 46.0), "p90 wall");
    assert!(near(t.tool_calls_mean.expect("calls"), 3.0), "three calls in every trial");
    assert_eq!(t.refusals, 5, "one refused call in each of five trials");
    assert_eq!(t.undos, 2, "undos");
    assert_eq!((t.edits_behind, t.trials_with_edits_behind), (2, 1), "two files behind the back, in one trial");
    assert_eq!((t.tokens_in, t.tokens_out), (Some(5_000_000), Some(2_500_000)), "tokens, summed");
    assert_eq!(t.wishes, 1, "the wishes made");
    // Per trial: 1 M × $1 + 0.5 M × $2 + 2 M × $0.5 = 1 + 1 + 1 = $3; five trials = $15.
    assert!(near(t.cost_usd.expect("a cost"), 15.0), "cost is the listed price times the tokens");
    assert_eq!(report.refusals_by_tool.get("grimoire_apply_operation"), Some(&5), "refusals by tool");
    assert_eq!(report.wishes_by_tool.get("grimoire_x"), Some(&1), "wishes by tool");
}

#[test]
fn a_model_with_no_listed_price_costs_unknown_and_never_zero() {
    let dir = scratch("no-price");
    let mut trials = task_with("free-looking", 5, 5);
    for t in &mut trials {
        t.tokens = Some((1_000, 1_000, 0));
    }
    let report = report_of(&dir, &header("no-price"), &trials);
    assert_eq!(report.tasks[0].cost_usd, None, "no price listed: the cost is not known");
    assert_eq!(report.dev.cost_usd, None);
    assert_eq!(report.dev.cost_per_success_usd, None);
    let md = report.markdown();
    assert!(md.contains("unknown"), "the report says unknown; it says:\n{md}");
    assert!(!md.contains("$0.0000"), "and never a zero it did not measure:\n{md}");
}

#[test]
fn a_price_is_read_from_omp_models_and_unpriced_status_is_not_a_price() {
    let listing = r#"{"models":[
        {"selector":"a/priced","cost":{"input":0.15,"output":0.6,"cacheRead":0.003,"cacheWrite":0},"pricingStatus":"fixed"},
        {"selector":"a/unpriced","cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"pricingStatus":"unknown"}]}"#;
    let priced = Price::from_models(listing, "a/priced").expect("a fixed price is read");
    assert_eq!((priced.input, priced.output, priced.cache_read), (0.15, 0.6, 0.003));
    assert_eq!(Price::from_models(listing, "a/unpriced"), None, "a listed 0 with unknown status is not a free model");
    assert_eq!(Price::from_models(listing, "a/missing"), None);
}

#[test]
fn a_failed_trial_names_its_first_failing_check_and_the_last_five_calls() {
    let dir = scratch("failure");
    let mut t = trial("hard", 1, "fail");
    t.calls = vec![
        ("grimoire_guide", false),
        ("grimoire_state", false),
        ("grimoire_list_pieces", false),
        ("grimoire_capture_still", false),
        ("grimoire_apply_operation", true),
        ("grimoire_apply_operation", false),
        ("grimoire_present", false),
    ];
    t.checks = vec![
        json!({"check": "placement_exists", "asks": "1 × a `lamp` on the map", "measured": "1 found", "passed": true}),
        json!({"check": "placement_near_wall", "asks": "`bed` within 0.05 m of the north wall", "measured": "0.80 m from it", "passed": false}),
        json!({"check": "no_problem", "asks": "no overlap", "measured": "1 overlap", "passed": false}),
    ];
    let report = report_of(&dir, &header("failure"), &[t]);
    let f = &report.tasks[0].failures[0];
    assert_eq!(f.check, "placement_near_wall", "the FIRST failing check, not a later one");
    assert_eq!(f.wanted, "`bed` within 0.05 m of the north wall", "what it wanted");
    assert_eq!(f.measured, "0.80 m from it", "what it measured");
    assert_eq!(LAST_CALLS_SHOWN, 5);
    assert_eq!(f.last_calls.len(), 5, "the last five calls of seven");
    assert!(f.last_calls[0].starts_with("grimoire_list_pieces"), "oldest of the five is the third call");
    assert!(f.last_calls[2].contains("(refused)"), "a refused call says so");
    assert!(f.last_calls[4].starts_with("grimoire_present"), "the last call is last");
    let md = report.markdown();
    assert!(md.contains("placement_near_wall") && md.contains("0.80 m from it"), "the markdown says it:\n{md}");
}

#[test]
fn a_harness_error_is_counted_beside_the_trials_and_never_scored() {
    let dir = scratch("harness");
    let mut trials = task_with("t", 3, 2);
    trials.push(trial("t", 4, "harness_error"));
    let report = report_of(&dir, &header("harness"), &trials);
    let t = &report.tasks[0];
    assert_eq!((t.trials, t.successes, t.harness_errors), (3, 2, 1), "n counts the scored trials only");
    assert_eq!(t.pass_hat.len(), 3, "pass^k runs to the scored n");
    assert_eq!(report.dev.harness_errors, 1);
}

// -------------------------------------------------------------------------------------------------
// Suites, tags and the bootstrap.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_suite_and_a_tag_state_mean_success_with_a_seeded_bootstrap_interval() {
    let dir = scratch("aggregate");
    let mut trials = suite_of(&[5, 5, 5, 5, 0, 0, 0, 0]);
    for t in &mut trials {
        t.tags = if t.task.as_str() < "task-04" { vec!["place", "easy"] } else { vec!["place", "hard"] };
    }
    let report = report_of(&dir, &header("aggregate"), &trials);
    // Four tasks at 1.0 and four at 0.0: the mean of the task means is 0.5.
    assert!(near(report.dev.mean_success.expect("mean"), 0.5));
    let interval = report.dev.interval.expect("an interval");
    assert!(interval.lo < 0.5 && 0.5 < interval.hi, "the interval {interval:?} surrounds the mean");
    assert!(interval.lo > 0.0 && interval.hi < 1.0, "and resampling tasks cannot reach the extremes at 8 tasks");
    let tag = |name: &str| report.tags.iter().find(|t| t.name == name).expect("the tag").clone();
    assert!(near(tag("easy").mean_success.expect("easy"), 1.0));
    assert!(near(tag("hard").mean_success.expect("hard"), 0.0));
    assert!(near(tag("place").mean_success.expect("place"), 0.5));
    // Seeded: the same trials give the same interval on every build.
    let again = report_of(&scratch("aggregate-again"), &header("aggregate"), &trials);
    assert_eq!(again.dev.interval, report.dev.interval);
    // And the bootstrap function says the same when handed the tasks directly.
    let tasks: Vec<Vec<f64>> = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0].iter().map(|v| vec![*v; 5]).collect();
    let (mean, i) = bootstrap_mean(&tasks, 10_000, bevy_cauldron_bench::stats::BOOTSTRAP_SEED).expect("a mean");
    assert!(near(mean, 0.5));
    assert_eq!(Some(i), report.dev.interval);
}

#[test]
fn cost_per_success_divides_the_total_by_the_successes() {
    let dir = scratch("cost-per-success");
    let mut h = header("cost-per-success");
    h.price = Some(Price { input: 1.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 });
    let mut trials = task_with("a", 4, 2);
    for t in &mut trials {
        t.tokens = Some((1_000_000, 0, 0)); // $1 each
    }
    let report = report_of(&dir, &h, &trials);
    assert!(near(report.dev.cost_usd.expect("cost"), 4.0), "four trials at $1");
    assert!(near(report.dev.cost_per_success_usd.expect("per success"), 2.0), "$4 over two successes");
}

// -------------------------------------------------------------------------------------------------
// Held-out tasks are a total and nothing else.
// -------------------------------------------------------------------------------------------------

#[test]
fn held_out_tasks_are_reported_only_as_a_total() {
    let dir = scratch("held-out");
    let mut trials = task_with("a-dev-task", 5, 5);
    for n in 1..=5 {
        let mut t = trial("a-secret-held-out-task", n, if n <= 2 { "pass" } else { "fail" });
        t.split = "held_out";
        t.tags = vec!["place", "a-secret-tag"];
        t.checks = vec![json!({"check": "placement_at", "asks": "SECRET-WANTED", "measured": "SECRET-MEASURED", "passed": false})];
        trials.push(t);
    }
    let report = report_of(&dir, &header("held-out"), &trials);
    let total = report.held_out.as_ref().expect("a held-out total is stated");
    assert_eq!((total.trials, total.successes), (5, 2));
    assert_eq!(report.tasks.len(), 1, "only the dev task is listed per task");
    for (what, text) in [("the markdown", report.markdown()), ("the JSON", report.json().expect("json"))] {
        for secret in ["a-secret-held-out-task", "SECRET-WANTED", "SECRET-MEASURED", "a-secret-tag", "PROMPT-OF-"] {
            assert!(!text.contains(secret), "{what} leaks `{secret}` from a held-out task:\n{text}");
        }
    }
}

// -------------------------------------------------------------------------------------------------
// A run is unfit when too many trials are harness errors (acceptance 3).
// -------------------------------------------------------------------------------------------------

#[test]
fn a_run_with_two_harness_errors_in_ten_trials_is_unfit_and_compare_refuses_it() {
    assert!((HARNESS_ERROR_MAX_FRACTION - 0.10).abs() < 1e-12);
    let mut shaky = suite_of(&[5, 5]);
    shaky.truncate(8);
    shaky.push(trial("task-01", 9, "harness_error"));
    shaky.push(trial("task-01", 10, "harness_error"));
    let unfit = report_of(&scratch("unfit"), &header("shaky"), &shaky);
    assert_eq!((unfit.harness_errors, unfit.trials_total), (2, 10));
    let why = unfit.unfit.clone().expect("2 of 10 is more than 10%: the run is unfit");
    assert!(why.contains("2 of 10"), "the reason counts them: {why}");
    assert!(unfit.markdown().contains("Unfit for comparison"), "and the report says so at the top");

    let fit = report_of(&scratch("fit"), &header("steady"), &suite_of(&[5, 5]));
    let refused = compare(&fit, &unfit, true).expect_err("an unfit run is never compared, with or without --across");
    assert!(refused.contains("unfit"), "{refused}");
    let refused = compare(&unfit, &fit, false).expect_err("in either position");
    assert!(refused.contains("unfit"), "{refused}");

    // One harness error in ten is exactly the tolerance, not over it.
    let mut edge = suite_of(&[5, 5]);
    edge.truncate(9);
    edge.push(trial("task-01", 10, "harness_error"));
    assert!(report_of(&scratch("edge"), &header("edge"), &edge).unfit.is_none(), "1 of 10 is fit");
}

// -------------------------------------------------------------------------------------------------
// The comparison (acceptance 2).
// -------------------------------------------------------------------------------------------------

#[test]
fn two_runs_that_differ_by_one_task_show_no_measurable_change() {
    let a = report_of(&scratch("one-a"), &header("a"), &suite_of(&[5, 5, 5, 5, 5, 5, 5, 5, 5, 0]));
    let b = report_of(&scratch("one-b"), &header("b"), &suite_of(&[5, 5, 5, 5, 5, 5, 5, 5, 5, 5]));
    let c = compare(&a, &b, false).expect("two alike runs compare");
    assert_eq!(c.change, Change::NoMeasurableChange, "one task of ten moving is not a measurable change: {c:?}");
    assert!(c.interval.contains(0.0), "the interval includes zero: {:?}", c.interval);
    assert!(near(c.difference, 0.1), "but the observed difference is one task in ten");
    assert_eq!(c.moved.len(), 1, "the one task that moved is listed");
    assert_eq!(c.moved[0].task, "task-09");
    assert_eq!((c.moved[0].a_successes, c.moved[0].b_successes, c.moved[0].a_trials), (0, 5, 5));
    assert!(c.markdown().contains("no measurable change"));
    assert!(c.markdown().contains("not significant on its own"), "a single task is marked as such");
}

#[test]
fn two_runs_that_differ_on_most_tasks_say_better_or_worse_with_the_interval() {
    let low = report_of(&scratch("many-low"), &header("low"), &suite_of(&[0, 0, 0, 0, 0, 0, 0, 0, 5, 5]));
    let high = report_of(&scratch("many-high"), &header("high"), &suite_of(&[5, 5, 5, 5, 5, 5, 5, 5, 5, 5]));
    let up = compare(&low, &high, false).expect("compares");
    assert_eq!(up.change, Change::Better, "{up:?}");
    assert!(up.interval.lo > 0.0, "the whole interval is above zero: {:?}", up.interval);
    assert!(near(up.difference, 0.8), "eight of ten tasks moved by 100 points");
    assert_eq!(up.moved.len(), 8);
    let down = compare(&high, &low, false).expect("compares");
    assert_eq!(down.change, Change::Worse, "{down:?}");
    assert!(down.interval.hi < 0.0, "the whole interval is below zero: {:?}", down.interval);
    let md = up.markdown();
    assert!(md.contains("better") && md.contains("95%"), "the markdown states the verdict and the interval:\n{md}");
}

#[test]
fn runs_of_different_suites_agents_or_trial_counts_compare_only_across() {
    let a = report_of(&scratch("across-a"), &header("a"), &suite_of(&[5, 5, 5]));
    let mut other = header("b");
    other.agent = "claude-code".to_owned();
    other.model = Some("sonnet".to_owned());
    other.trials = 3;
    let b = report_of(&scratch("across-b"), &other, &suite_of(&[5, 5, 5, 5]));
    let refused = compare(&a, &b, false).expect_err("different suites, agents and trial counts are refused");
    for named in ["suites differ", "agents differ", "trials per task differ", "--across"] {
        assert!(refused.contains(named), "the refusal names `{named}`: {refused}");
    }
    let allowed = compare(&a, &b, true).expect("--across accepts it");
    assert_eq!(allowed.across.len(), 3, "and says which three differ: {:?}", allowed.across);
    assert!(allowed.markdown().contains("Compared across"));
    assert_eq!(allowed.tasks_paired, 3, "only the tasks both runs scored are paired");
}

#[test]
fn held_out_tasks_are_never_named_in_a_comparison() {
    let mut a_trials = suite_of(&[5, 5]);
    let mut b_trials = suite_of(&[5, 5]);
    for trials in [&mut a_trials, &mut b_trials] {
        for n in 1..=5 {
            let mut t = trial("a-secret-held-out-task", n, "pass");
            t.split = "held_out";
            trials.push(t);
        }
    }
    let a = report_of(&scratch("cmp-held-a"), &header("a"), &a_trials);
    let b = report_of(&scratch("cmp-held-b"), &header("b"), &b_trials);
    let c = compare(&a, &b, false).expect("compares");
    assert_eq!(c.a_held_out, Some(1.0));
    assert!(!c.markdown().contains("a-secret-held-out-task"));
    assert_eq!(c.tasks_paired, 2, "the held-out task is not paired");
}

// -------------------------------------------------------------------------------------------------
// The committed reports and their README.
// -------------------------------------------------------------------------------------------------

#[test]
fn published_reports_are_listed_newest_first_and_read_back() {
    let bench = scratch("publish").join("docs/bench");
    let mut older = header("baseline");
    older.date = "2026-11-01".to_owned();
    let mut newer = header("after-the-fix");
    newer.date = "2026-11-13".to_owned();
    let old_report = report_of(&scratch("pub-old"), &older, &suite_of(&[5, 0]));
    let new_report = report_of(&scratch("pub-new"), &newer, &suite_of(&[5, 5]));
    // Published oldest first, so the README's order is the sort and not the order of writing.
    let (md, json) = index::publish(&bench, &old_report).expect("publishes");
    index::publish(&bench, &new_report).expect("publishes");
    assert!(md.ends_with("runs/2026-11-01-baseline.md") && json.ends_with("runs/2026-11-01-baseline.json"));
    let readme = std::fs::read_to_string(bench.join("README.md")).expect("a README");
    let (newest, oldest) = (
        readme.find("2026-11-13-after-the-fix").expect("the newer run is listed"),
        readme.find("2026-11-01-baseline").expect("the older run is listed"),
    );
    assert!(newest < oldest, "newest first:\n{readme}");
    assert!(readme.contains("50%") && readme.contains("100%"), "each row carries its headline number:\n{readme}");
    // A committed `.json` is what `compare` reads, so the pair round-trips.
    let back = Report::load(&json).expect("the json loads");
    assert_eq!(back, old_report);
    let c = compare(&back, &Report::load(&bench.join("runs/2026-11-13-after-the-fix.json")).expect("loads"), false)
        .expect("two committed reports compare");
    assert_eq!(c.moved.len(), 1);
}

#[test]
fn a_date_is_read_from_a_day_count() {
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(19_000), "2022-01-08");
    assert_eq!(civil_date(20_000), "2024-10-04");
}

// -------------------------------------------------------------------------------------------------
// What a trial records: refusals, tokens and edits behind the editor's back.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_refusal_is_the_envelope_or_the_payload_saying_so() {
    assert!(is_refusal(true, "anything"));
    assert!(is_refusal(false, r#"{"refused":"grimoire/lesson: no guide"}"#));
    assert!(is_refusal(false, r#"{"outcome":"refused","reason":"locked"}"#));
    assert!(!is_refusal(false, r#"{"outcome":"applied"}"#));
    assert!(!is_refusal(false, "plain text that is not JSON"));
}

#[test]
fn a_transcript_gives_calls_refusals_and_each_kind_of_token() {
    let dir = scratch("transcript");
    let tool = |name: &str, is_error: bool, text: &str| {
        json!({"type": "tool_execution_end", "toolName": "write", "isError": false, "result": {"details": {"xdev": {
            "tool": format!("mcp__cauldron_{name}"), "args": {"a": 1},
            "inner": {"mcpToolName": name, "isError": is_error, "rawContent": [{"type": "text", "text": text}]}}}}})
    };
    let turn = |input: u64, output: u64, cache_read: u64, cache_write: u64| {
        json!({"type": "message_end", "message": {"role": "assistant", "stopReason": "stop", "usage": {
            "input": input, "output": output, "cacheRead": cache_read, "cacheWrite": cache_write,
            "totalTokens": input + output + cache_read + cache_write}}})
    };
    let events = [
        turn(10, 20, 30, 40),
        tool("grimoire_state", false, r#"{"ok":true}"#),
        tool("grimoire_apply_operation", false, r#"{"refused":"that piece is locked"}"#),
        tool("grimoire_undo_operation", false, r#"{"outcome":"applied"}"#),
        tool("grimoire_lesson", true, "no guide"),
        turn(1, 2, 3, 4),
    ];
    std::fs::write(
        dir.join("transcript.jsonl"),
        events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"),
    )
    .expect("transcript");
    std::fs::write(
        dir.join("tools-offered.jsonl"),
        r#"{"why":"before_agent_start","tools":["mcp__cauldron_grimoire_state","read"]}"#,
    )
    .expect("sidecar");
    let run = read_transcript(&dir.join("transcript.jsonl"), &dir, "cauldron").expect("reads");
    let refused: Vec<(&str, bool)> = run.tool_log.iter().map(|c| (c.tool.as_str(), c.refused)).collect();
    assert_eq!(
        refused,
        vec![
            ("grimoire_state", false),
            ("grimoire_apply_operation", true),
            ("grimoire_undo_operation", false),
            ("grimoire_lesson", true)
        ],
        "a payload's `refused` and an envelope's `isError` are both refusals"
    );
    let tokens = run.usage.expect("tokens by kind");
    assert_eq!((tokens.input, tokens.output, tokens.cache_read, tokens.cache_write), (11, 22, 33, 44));
    assert_eq!(tokens.total, 110, "the harness's own total is kept");
    assert_eq!(run.tokens, Some(110), "and `tokens` is still that total");
}

/// **A project directory** with a map file, one other file and an operation log of `lines` lines.
fn project(dir: &Path, map_text: &str, other: &str, applied_lines: usize) {
    let grimoire = dir.join("assets/grimoire");
    std::fs::create_dir_all(grimoire.join("maps")).expect("maps dir");
    std::fs::write(grimoire.join("maps/m.map.yaml"), map_text).expect("map");
    std::fs::write(dir.join("notes.txt"), other).expect("other");
    let line = r#"{"op":"placement_add","outcome":"applied","source":"agent"}"#;
    std::fs::write(grimoire.join("oplog.jsonl"), vec![line; applied_lines].join("\n")).expect("oplog");
}

#[test]
fn a_file_no_operation_wrote_is_an_edit_behind_the_editors_back() {
    let snapshot = |dir: &Path| Snapshot::read(dir, "m", Vec::new(), Default::default());

    // The map changed and the log has a new applied line: the editor did it.
    let (before, through_tools) = (scratch("behind-before"), scratch("behind-tools"));
    project(&before, "v1", "n1", 0);
    project(&through_tools, "v2", "n1", 1);
    assert!(
        edits_behind_the_back(&snapshot(&before), &snapshot(&through_tools)).is_empty(),
        "a map change with an applied log line behind it is the editor's own"
    );

    // The map changed and the log did not: the agent wrote the file itself.
    let around = scratch("behind-around");
    project(&around, "v2", "n1", 0);
    assert_eq!(
        edits_behind_the_back(&snapshot(&before), &snapshot(&around)),
        vec!["assets/grimoire/maps/m.map.yaml".to_owned()],
        "no log line, so the change went around the tools"
    );

    // A file outside the editor's data directory is never an operation's output, log or not.
    let stray = scratch("behind-stray");
    project(&stray, "v2", "n2", 1);
    assert_eq!(
        edits_behind_the_back(&snapshot(&before), &snapshot(&stray)),
        vec!["notes.txt".to_owned()],
        "a stray file is behind the back even when operations ran"
    );
}

/// **A file an operation wrote under `design/` is the editor's own** (review of #413, M1): `board_set` writes a
/// board's `board.yaml` and copies its pictures into `design/<place>/board/`, and `tree_set` writes the style
/// tree. A run that applied one is not "behind the editor's back" for it; a change under `design/` with no
/// applied line is.
#[test]
fn a_board_an_operation_wrote_is_not_behind_the_editors_back_and_a_hand_written_one_is() {
    let snapshot = |dir: &Path| Snapshot::read(dir, "m", Vec::new(), Default::default());
    let board = |dir: &Path, yaml: &str, picture: &str| {
        let place = dir.join("design/levels/m/board");
        std::fs::create_dir_all(&place).expect("board dir");
        std::fs::write(place.join("board.yaml"), yaml).expect("board.yaml");
        std::fs::write(place.join("shot.png"), picture).expect("picture");
    };

    let before = scratch("design-before");
    project(&before, "v1", "n1", 0);
    board(&before, "pictures: []", "png-1");

    let through_tools = scratch("design-tools");
    project(&through_tools, "v1", "n1", 1);
    board(&through_tools, "pictures: [shot.png]", "png-2");
    assert!(
        edits_behind_the_back(&snapshot(&before), &snapshot(&through_tools)).is_empty(),
        "a board and a picture changed under an applied log line are the operation's"
    );

    let by_hand = scratch("design-hand");
    project(&by_hand, "v1", "n1", 0);
    board(&by_hand, "pictures: [shot.png]", "png-2");
    assert_eq!(
        edits_behind_the_back(&snapshot(&before), &snapshot(&by_hand)),
        vec!["design/levels/m/board/board.yaml".to_owned(), "design/levels/m/board/shot.png".to_owned()],
        "the same files with no applied line behind them went around the tools"
    );
}
