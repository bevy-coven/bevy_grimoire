//! **A report says WHY a trial was a harness error** (#432, D1).
//!
//! The first real-agent run's all-harness-error report said "Unfit for comparison" and "harness errors 3" and
//! nothing about the cause (HTTP 429, a spent quota): `TrialData.reason` was read from every `trial.json` and
//! never used. The cause was only in the run's stdout and in each trial's own file, so a committed report of a
//! run that measured the provider could not say so. Built from recorded `trial.json` fixtures, as the report
//! tests are: no model, no editor.

use std::path::{Path, PathBuf};

use bevy_cauldron_bench::report::{Header, Report, RunData, REPORT_VERSION, RUN_FILE};
use serde_json::{json, Value};

/// What a spent quota says, as a trial's reason quotes it.
const QUOTA_REASON: &str = "the model provider refused a turn, so this trial measures the provider and not the agent: \
                            HTTP 429: you have reached your weekly usage limit";

/// **A scratch directory this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-why-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

fn header() -> Header {
    Header {
        version: REPORT_VERSION,
        label: "why".to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        guide_sha256: "ab".repeat(32),
        agent: "omp".to_owned(),
        agent_version: "omp/18.18.9".to_owned(),
        model: Some("ollama-cloud/deepseek-v4.1-flash:1017".to_owned()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: 1,
        host: "big".to_owned(),
        date: "2026-11-14".to_owned(),
        price: None,
    }
}

/// **Write a run of these trials** `(task, trial, outcome, reason)`, in the shape `cauldron-bench run` writes.
fn write_run(dir: &Path, trials: &[(&str, u32, &str, &str)]) {
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header()).unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}"));
    for (task, n, outcome, reason) in trials {
        let d = dir.join(task).join(format!("trial-{n}"));
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
        let value = json!({
            "task": task, "trial": n, "title": format!("title of {task}"), "split": "dev", "tags": ["place"],
            "outcome": outcome, "reason": reason, "checks": [], "tool_calls": [], "tool_log": [],
            "refusals": 0, "refusals_by_tool": {}, "undos": 0, "edits_behind": [], "wall_s": 0.0, "wishes": [],
        });
        std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&value).unwrap_or_default())
            .unwrap_or_else(|e| panic!("{e}"));
    }
}

fn report_of(dir: &Path, trials: &[(&str, u32, &str, &str)]) -> Report {
    write_run(dir, trials);
    Report::build(&RunData::read(dir).unwrap_or_else(|e| panic!("the run reads: {e}")))
}

/// **An all-harness-error run's report carries the cause**, in the markdown and in the JSON.
#[test]
fn an_unfit_report_says_why() {
    let dir = scratch("unfit");
    let report = report_of(
        &dir,
        &[
            ("raise-a-hill", 1, "harness_error", QUOTA_REASON),
            ("refuse-a-piece", 1, "harness_error", QUOTA_REASON),
            ("remove-crates", 1, "harness_error", "the editor exited with exit status: 1 during the trial"),
        ],
    );
    assert!(report.unfit.is_some(), "three harness errors in three is unfit");
    let md = report.markdown();
    assert!(md.contains("## Harness errors"), "a section names the errors:\n{md}");
    assert!(md.contains("HTTP 429: you have reached your weekly usage limit"), "it quotes the provider:\n{md}");
    assert!(md.contains("the editor exited with exit status: 1"), "and every distinct reason:\n{md}");
    for who in ["raise-a-hill", "refuse-a-piece", "remove-crates"] {
        assert!(md.contains(who), "and which trials had it ({who}):\n{md}");
    }
    let json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    let rows = json.get("harness_error_reasons").and_then(Value::as_array).cloned().unwrap_or_default();
    assert_eq!(rows.len(), 3, "report.json carries one row per harness-error trial: {json}");
    let row = rows.iter().find(|r| r.get("task").and_then(Value::as_str) == Some("refuse-a-piece")).cloned().unwrap_or_default();
    assert_eq!(row.get("trial").and_then(Value::as_u64), Some(1));
    assert_eq!(row.get("reason").and_then(Value::as_str), Some(QUOTA_REASON));
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A run with no harness error has no such section**, and a report from before the field still reads.
#[test]
fn a_clean_run_has_no_harness_error_section_and_an_old_report_still_reads() {
    let dir = scratch("clean");
    let report = report_of(&dir, &[("a", 1, "pass", ""), ("a", 2, "fail", "")]);
    assert!(!report.markdown().contains("## Harness errors"), "nothing to explain:\n{}", report.markdown());
    let mut json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    if let Some(object) = json.as_object_mut() {
        object.remove("harness_error_reasons");
    }
    let old = Report::from_json(&json.to_string()).unwrap_or_else(|e| panic!("an older report reads: {e}"));
    assert!(old.harness_error_reasons.is_empty(), "a report with no such field has none");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Many trials with one cause are one row**, not a hundred lines of the same sentence.
#[test]
fn trials_with_one_cause_share_a_row() {
    let dir = scratch("grouped");
    let trials: Vec<(String, u32)> = (1..=40).map(|n| ("t".to_owned(), n)).collect();
    let rows: Vec<(&str, u32, &str, &str)> = trials.iter().map(|(t, n)| (t.as_str(), *n, "harness_error", QUOTA_REASON)).collect();
    let report = report_of(&dir, &rows);
    let md = report.markdown();
    let mentions = md.matches("HTTP 429").count();
    assert_eq!(mentions, 1, "forty trials, one cause, one row ({mentions} mentions):\n{md}");
    assert!(md.contains("40"), "the row says how many trials had it:\n{md}");
    assert_eq!(report.harness_error_reasons.len(), 40, "while the JSON keeps every trial");
    let _ = std::fs::remove_dir_all(&dir);
}
