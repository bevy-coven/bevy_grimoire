//! **The token column counts what the trial moved, cache included** (#432, D8).
//!
//! The by-task table's "tokens in/out" showed fresh input and output only (`28/3851`) for a trial that moved
//! 693,759 tokens (614,716 read from the cache, 75,164 written to it): the cost beside it was right, because it
//! prices the cache, and the column was about a hundredth of the truth. The cache read and write are shown too.
//! Built from a recorded `trial.json`, as the report tests are: no model, no editor.

use std::path::PathBuf;

use bevy_cauldron_bench::report::{Header, Report, RunData, REPORT_VERSION, RUN_FILE};
use serde_json::{json, Value};

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-cache-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

fn header() -> Header {
    Header {
        version: REPORT_VERSION,
        label: "cache".to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        guide_sha256: "ab".repeat(32),
        agent: "omp".to_owned(),
        agent_version: "omp/18.18.9".to_owned(),
        model: Some("anthropic/claude-sonnet-5-5".to_owned()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: 1,
        host: "big".to_owned(),
        date: "2026-11-14".to_owned(),
        price: None,
    }
}

/// **A report of one passing trial with these token counts** `(in, out, cache read, cache write)`.
fn report_with(test: &str, tokens: Option<(u64, u64, u64, u64)>) -> Report {
    let dir = scratch(test);
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header()).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let d = dir.join("raise-a-hill").join("trial-1");
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
    let mut value = json!({
        "task": "raise-a-hill", "trial": 1, "title": "t", "split": "dev", "tags": ["place"], "outcome": "pass", "reason": "",
        "checks": [], "tool_calls": [], "tool_log": [], "refusals": 0, "refusals_by_tool": {}, "undos": 0,
        "edits_behind": [], "wall_s": 58.0, "wishes": [],
    });
    if let (Some((i, o, r, w)), Some(object)) = (tokens, value.as_object_mut()) {
        object.insert("tokens_in".to_owned(), json!(i));
        object.insert("tokens_out".to_owned(), json!(o));
        object.insert("tokens_cache_read".to_owned(), json!(r));
        object.insert("tokens_cache_write".to_owned(), json!(w));
    }
    std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&value).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let report = Report::build(&RunData::read(&dir).unwrap_or_else(|e| panic!("the run reads: {e}")));
    let _ = std::fs::remove_dir_all(&dir);
    report
}

/// **The task's row shows fresh in/out and the cache read/write**, with the numbers the smoke run measured.
#[test]
fn the_token_column_shows_the_cache() {
    let report = report_with("shown", Some((28, 3851, 614_716, 75_164)));
    let md = report.markdown();
    let row = md.lines().find(|l| l.starts_with("| `raise-a-hill`")).unwrap_or_default().to_owned();
    assert!(row.contains("28/3851"), "fresh input and output stay: {row}");
    assert!(row.contains("614716") && row.contains("75164"), "and the cache read and written are in the row: {row}");
    let header = md.lines().find(|l| l.starts_with("| task |")).unwrap_or_default().to_owned();
    assert!(header.contains("cache"), "the column says what it counts: {header}");
    let json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    let task = json.pointer("/tasks/0").cloned().unwrap_or_default();
    assert_eq!(task.get("tokens_cache_read").and_then(Value::as_u64), Some(614_716), "report.json carries it: {task}");
    assert_eq!(task.get("tokens_cache_write").and_then(Value::as_u64), Some(75_164), "{task}");
}

/// **A trial that reported no tokens has unknown ones, in the cache columns too**: never `0`.
#[test]
fn an_unreported_cache_is_unknown_not_zero() {
    let report = report_with("unknown", None);
    let md = report.markdown();
    let row = md.lines().find(|l| l.starts_with("| `raise-a-hill`")).unwrap_or_default().to_owned();
    assert!(row.contains("unknown"), "no tokens reported is unknown: {row}");
    assert!(!row.contains("0/0"), "and not zero: {row}");
}
