//! **The pre-flight turn's cost is in the report's total** (#434).
//!
//! The runner makes one paid model turn before any trial (#432) and no trial's cost holds it, so a baseline's cost was
//! understated by one turn per run. The runner keeps that turn's tokens in `preflight-usage.json` beside the trials; the report
//! prices them like a trial's and states a total that includes them. Built from recorded files: no model, no editor.

use bevy_cauldron_bench::report::{Header, Price, Report, RunData, PREFLIGHT_FILE, REPORT_VERSION, RUN_FILE};
use serde_json::{json, Value};

fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-preflight-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// $1 per million fresh input, $2 output, $0.5 cache read, $0 cache write.
fn header() -> Header {
    Header {
        version: REPORT_VERSION,
        label: "preflight".to_owned(),
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
        price: Some(Price { input: 1.0, output: 2.0, cache_read: 0.5, cache_write: 0.0 }),
    }
}

/// **A run of one trial** (2,000,000 fresh in, 1,000,000 out, 1,000,000 cache read: $2 + $2 + $0.5 = $4.50), with a pre-flight turn of
/// 1,000,000 in, 500,000 out, 2,000,000 cache read ($1 + $1 + $1 = $3.00) when `preflight` is true.
fn report(test: &str, preflight: bool) -> Report {
    let dir = scratch(test);
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header()).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let d = dir.join("raise-a-hill").join("trial-1");
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
    let trial = json!({"task": "raise-a-hill", "trial": 1, "title": "t", "split": "dev", "tags": ["place"], "outcome": "pass", "reason": "",
        "checks": [], "tool_calls": [], "tool_log": [], "refusals": 0, "refusals_by_tool": {}, "undos": 0, "edits_behind": [], "wall_s": 1.0,
        "wishes": [], "tokens_in": 2_000_000, "tokens_out": 1_000_000, "tokens_cache_read": 1_000_000, "tokens_cache_write": 0});
    std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&trial).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    if preflight {
        std::fs::write(
            dir.join(PREFLIGHT_FILE),
            json!({"input": 1_000_000, "output": 500_000, "cache_read": 2_000_000, "cache_write": 0, "total": 3_500_000}).to_string(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    }
    let report = Report::build(&RunData::read(&dir).unwrap_or_else(|e| panic!("the run reads: {e}")));
    let _ = std::fs::remove_dir_all(&dir);
    report
}

fn near(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-9)
}

/// **The pre-flight turn is priced, and the total holds it and the trials.**
#[test]
fn the_total_includes_the_preflight_turn() {
    let report = report("with", true);
    let preflight = report.preflight.clone().unwrap_or_else(|| panic!("the report holds the pre-flight turn"));
    assert_eq!((preflight.tokens_in, preflight.tokens_out, preflight.tokens_cache_read), (1_000_000, 500_000, 2_000_000));
    assert!(near(preflight.cost_usd, 3.0), "1 + 2 x 0.5 + 0.5 x 2 = $3.00: {:?}", preflight.cost_usd);
    assert!(near(report.dev.cost_usd, 4.5), "the trials alone cost $4.50: {:?}", report.dev.cost_usd);
    assert!(near(report.cost_total_usd, 7.5), "the run costs the trials and the pre-flight turn: {:?}", report.cost_total_usd);
    let md = report.markdown();
    assert!(md.contains("Pre-flight turn") && md.contains("$3.0000"), "the markdown says so:\n{md}");
    assert!(md.contains("$7.5000"), "and states the total:\n{md}");
    let json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    assert_eq!(json["cost_total_usd"], 7.5, "{json}");
    assert_eq!(json["preflight"]["tokens_in"], 1_000_000);
}

/// **A run with no pre-flight turn (no `omp`) has none in its total**, and a report from before the field reads.
#[test]
fn no_preflight_no_line() {
    let report = report("without", false);
    assert!(report.preflight.is_none());
    assert!(near(report.cost_total_usd, 4.5), "{:?}", report.cost_total_usd);
    assert!(!report.markdown().contains("Pre-flight turn"));
    let mut json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    if let Some(object) = json.as_object_mut() {
        object.remove("preflight");
        object.remove("cost_total_usd");
    }
    let old = Report::from_json(&json.to_string()).unwrap_or_else(|e| panic!("an older report reads: {e}"));
    assert!(old.preflight.is_none() && old.cost_total_usd.is_none());
}
