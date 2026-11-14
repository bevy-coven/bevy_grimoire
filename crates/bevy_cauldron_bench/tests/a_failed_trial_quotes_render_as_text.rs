//! **What a failed trial quotes is escaped too** (#433 review).
//!
//! The "Failed trials" section prints the first failing check with what it wanted and what it MEASURED, and a
//! check measures the agent's own hand-back (`the hand-back "..." does not state ...`): an agent that wrote a tag, a
//! link or markdown into its hand-back would render it in a committed `report.md`. `wanted` and `measured` go through
//! [`quoted`] like every other quoted text, and the check's name cannot break out of its code span.
//! Built from a recorded `trial.json`: no model, no editor.

use bevy_cauldron_bench::report::{Header, Report, RunData, REPORT_VERSION, RUN_FILE};
use serde_json::json;

/// A hand-back's words with a tag, an image, a link, a backtick and a pipe in them.
const HOSTILE: &str = "the hand-back <img src=x onerror=1> says [click](https://evil.example/x) and `code` | **bold**";

fn report() -> Report {
    let dir = std::env::temp_dir().join(format!("bench-report-failed-quotes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let header = Header {
        version: REPORT_VERSION,
        label: "failed".to_owned(),
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
    };
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let d = dir.join("refuse").join("trial-1");
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
    let value = json!({
        "task": "refuse", "trial": 1, "title": "t", "split": "dev", "tags": ["refusal"], "outcome": "fail", "reason": "",
        "checks": [{"check": "hand`back", "asks": "a hand-back stating <b>armchair</b>", "measured": HOSTILE, "passed": false}],
        "tool_calls": [], "tool_log": [], "refusals": 0, "refusals_by_tool": {}, "undos": 0, "edits_behind": [],
        "wall_s": 1.0, "wishes": [],
    });
    std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&value).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let report = Report::build(&RunData::read(&dir).unwrap_or_else(|e| panic!("the run reads: {e}")));
    let _ = std::fs::remove_dir_all(&dir);
    report
}

/// **The measured hand-back and the wanted text render as text**, and the check's name stays inside its span.
#[test]
fn a_failed_trials_quotes_are_inert() {
    let md = report().markdown();
    let line = md.lines().find(|l| l.contains("first failing check")).unwrap_or_default().to_owned();
    assert!(!line.contains("<img") && !line.contains("<b>"), "no raw tag: {line}");
    assert!(line.contains("&lt;img src=x onerror=1&gt;"), "a tag is its entity: {line}");
    assert!(!line.contains("](https"), "a link is broken: {line}");
    assert!(line.contains("\\[click\\]\\(https://evil.example/x\\)"), "its brackets and parentheses are escaped: {line}");
    assert!(line.contains("\\`code\\`"), "a backtick is escaped: {line}");
    assert!(line.contains("\\*\\*bold\\*\\*"), "emphasis is escaped: {line}");
    assert!(line.contains("&lt;b&gt;armchair&lt;/b&gt;"), "what the check wanted is escaped too: {line}");
    assert!(line.contains("`hand'back`"), "a backtick in the check's name cannot close its code span: {line}");
}
