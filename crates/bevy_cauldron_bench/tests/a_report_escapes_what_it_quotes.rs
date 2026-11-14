//! **A report's quoted text cannot become markup** (#433, item 3d).
//!
//! `cell()` escaped only `|` and newlines, so a provider's error body with backticks, a link or inline HTML rendered in
//! the committed report. Text the report quotes (a harness error's reason, the agent's version, the model and variant
//! names) is now escaped: HTML characters become entities and markdown's own punctuation is backslash-escaped. The
//! report's own markup (the code spans around task ids and the commit) is untouched. Built from recorded fixtures.

use std::path::PathBuf;

use bevy_cauldron_bench::report::{Header, Report, RunData, REPORT_VERSION, RUN_FILE};
use serde_json::json;

/// A provider body with every kind of markup a report must not render.
const HOSTILE: &str = "429 <script>alert(1)</script> `code` [click](https://evil.example/x) ![img](https://evil.example/i.png) <b>bold</b> *em* & done";

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-escape-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

fn report() -> Report {
    let dir = scratch("hostile");
    let header = Header {
        version: REPORT_VERSION,
        label: "escape".to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        guide_sha256: "ab".repeat(32),
        agent: "omp".to_owned(),
        agent_version: "omp/<i>18</i>".to_owned(),
        model: Some("anthropic/claude-sonnet-5-5".to_owned()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: 1,
        host: "big".to_owned(),
        date: "2026-11-14".to_owned(),
        price: None,
    };
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let d = dir.join("raise-a-hill").join("trial-1");
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
    let value = json!({
        "task": "raise-a-hill", "trial": 1, "title": "t", "split": "dev", "tags": ["place"], "outcome": "harness_error",
        "reason": HOSTILE, "checks": [], "tool_calls": [], "tool_log": [], "refusals": 0, "refusals_by_tool": {},
        "undos": 0, "edits_behind": [], "wall_s": 0.0, "wishes": [],
    });
    std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&value).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let report = Report::build(&RunData::read(&dir).unwrap_or_else(|e| panic!("the run reads: {e}")));
    let _ = std::fs::remove_dir_all(&dir);
    report
}

/// **No raw tag, link or code span survives in the quoted reason.**
#[test]
fn a_quoted_reason_renders_as_text() {
    let md = report().markdown();
    let row = md.lines().find(|l| l.contains("429")).unwrap_or_default().to_owned();
    assert!(!row.contains("<script>") && !row.contains("<b>"), "no raw HTML tag: {row}");
    assert!(row.contains("&lt;script&gt;"), "an HTML character is its entity: {row}");
    assert!(!row.contains("](https"), "a link's `](` is broken: {row}");
    assert!(row.contains("\\]") && row.contains("\\["), "brackets are escaped: {row}");
    assert!(!row.contains(" `code` "), "a backtick is escaped: {row}");
    assert!(row.contains("\\`code\\`"), "{row}");
    assert!(row.contains("\\*em\\*"), "{row}");
    assert!(row.contains("&amp; done"), "an ampersand is an entity: {row}");
}

/// **The agent's version a harness printed is escaped, and the report's own code spans are not.**
#[test]
fn the_header_is_escaped_but_the_reports_own_markup_is_kept() {
    let md = report().markdown();
    assert!(!md.contains("<i>18</i>"), "the agent version cannot inject a tag:\n{md}");
    assert!(md.contains("omp/&lt;i&gt;18&lt;/i&gt;"), "{md}");
    assert!(md.contains("`0123456789abcdef0123456789abcdef01234567`"), "the commit stays a code span:\n{md}");
    assert!(md.contains("`raise-a-hill` trial 1"), "so does a task's id:\n{md}");
}
