//! **A trial stopped at its tool-call budget says so, and the report counts them** (#433, item 3c).
//!
//! The budget stop used to be recorded nowhere: a trial that was cut off looked like one that ran out of ideas. It is
//! now a field of `trial.json` (`call_budget_exhausted`), a per-task count in the report and a section of the markdown.
//! Built from recorded `trial.json` fixtures: no model, no editor.

use std::path::PathBuf;

use bevy_cauldron_bench::report::{Header, Report, RunData, REPORT_VERSION, RUN_FILE};
use serde_json::{json, Value};

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-budget-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

fn header() -> Header {
    Header {
        version: REPORT_VERSION,
        label: "budget".to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        guide_sha256: "ab".repeat(32),
        agent: "omp".to_owned(),
        agent_version: "omp/18.18.9".to_owned(),
        model: Some("anthropic/claude-sonnet-5-5".to_owned()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: 2,
        host: "big".to_owned(),
        date: "2026-11-14".to_owned(),
        price: None,
    }
}

/// **A run of two trials of one task**, the first with `exhausted` as `call_budget_exhausted` (or the field absent for `None`).
fn report_with(test: &str, exhausted: Option<bool>) -> Report {
    let dir = scratch(test);
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header()).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    for n in 1..=2u32 {
        let d = dir.join("raise-a-hill").join(format!("trial-{n}"));
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
        let mut value = json!({
            "task": "raise-a-hill", "trial": n, "title": "t", "split": "dev", "tags": ["place"], "outcome": "fail", "reason": "",
            "checks": [], "tool_calls": [], "tool_log": [], "refusals": 0, "refusals_by_tool": {}, "undos": 0,
            "edits_behind": [], "wall_s": 20.0, "wishes": [],
        });
        if let (Some(flag), 1, Some(object)) = (exhausted, n, value.as_object_mut()) {
            object.insert("call_budget_exhausted".to_owned(), json!(flag));
        }
        std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&value).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    }
    let report = Report::build(&RunData::read(&dir).unwrap_or_else(|e| panic!("the run reads: {e}")));
    let _ = std::fs::remove_dir_all(&dir);
    report
}

/// **The trial that hit its budget is counted and named.**
#[test]
fn a_budget_stop_is_in_the_report() {
    let report = report_with("stopped", Some(true));
    let json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    let task = json.pointer("/tasks/0").cloned().unwrap_or_default();
    assert_eq!(task.get("budget_exhausted").and_then(Value::as_u64), Some(1), "one of two trials was stopped: {task}");
    let md = report.markdown();
    assert!(md.contains("## Tool-call budget"), "a section says so:\n{md}");
    assert!(md.contains("`raise-a-hill` trial 1"), "and names the trial:\n{md}");
}

/// **A run with no budget stop has no such section**, and a trial.json from before the field reads as not stopped.
#[test]
fn no_stop_no_section() {
    for (name, flag) in [("false", Some(false)), ("absent", None)] {
        let report = report_with(name, flag);
        assert!(!report.markdown().contains("## Tool-call budget"), "{name}:\n{}", report.markdown());
        assert_eq!(report.tasks.first().map(|t| t.budget_exhausted), Some(0), "{name}");
    }
}
