//! **A provider error that omp retried successfully does not void the trial** (#432, D9).
//!
//! `read_transcript` turned the FIRST assistant message with `stopReason: error` into a harness error for the
//! whole trial, although omp retries a transient 429 or 5xx itself and says so in the stream
//! (`auto_retry_start`, then `auto_retry_end` with `success: true`). A trial the agent then ran to the end was
//! discarded as "the provider refused a turn". The transcript is now read to its end before a refusal is
//! held against the trial: one that `auto_retry_end{success: true}` closes was recovered, and one nothing
//! closes (or that ended `success: false`) still is the provider's refusal, in its own words.
//!
//! The streams are written by hand in the shape omp 18.18.9 writes them (the event names and fields are read
//! from its own source): no model, no editor.

use std::path::{Path, PathBuf};

use bevy_cauldron_bench::run::{is_provider_refusal, preflight_turn, read_transcript};
use serde_json::{json, Value};

/// What a rate limit says.
const LIMIT_WORDS: &str = "HTTP 429: rate limit, try again shortly";

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-retry-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **A refused assistant turn**, as omp writes it: no content, `stopReason: error`.
fn refused() -> Value {
    json!({"type": "message_end", "message": {"role": "assistant", "content": [], "stopReason": "error", "errorMessage": LIMIT_WORDS}})
}

/// **An answered turn** with these fresh input and output tokens.
fn answered(input: u64, output: u64) -> Value {
    json!({"type": "message_end", "message": {"role": "assistant", "content": [{"type": "text", "text": "done"}], "stopReason": "stop",
        "usage": {"input": input, "output": output, "cacheRead": 0, "cacheWrite": 0, "totalTokens": input + output}}})
}

fn retry_start() -> Value {
    json!({"type": "auto_retry_start", "attempt": 1, "maxAttempts": 3, "delayMs": 2000, "errorMessage": LIMIT_WORDS})
}

fn retry_end(success: bool) -> Value {
    if success {
        json!({"type": "auto_retry_end", "success": true, "attempt": 1})
    } else {
        json!({"type": "auto_retry_end", "success": false, "attempt": 3, "finalError": LIMIT_WORDS})
    }
}

/// **Read a stream as a trial's transcript**, with the tool-list sidecar a real trial has.
fn read(test: &str, events: &[Value]) -> Result<bevy_cauldron_bench::run::AgentRun, String> {
    let dir = scratch(test);
    std::fs::write(dir.join("transcript.jsonl"), events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"))
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        dir.join("tools-offered.jsonl"),
        r#"{"why":"before_agent_start","tools":["mcp__cauldron_grimoire_state","read"]}"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let run = read_transcript(&dir.join("transcript.jsonl"), &dir, "cauldron");
    let _ = std::fs::remove_dir_all(&dir);
    run
}

/// **A refusal omp retried and recovered from is not the trial's end**: the agent's turns after it count.
#[test]
fn a_retried_refusal_that_recovered_does_not_void_the_trial() {
    let run = read("recovered", &[refused(), retry_start(), answered(10, 20), retry_end(true)])
        .unwrap_or_else(|e| panic!("a recovered 429 does not void the trial: {e}"));
    let tokens = run.usage.unwrap_or_default();
    assert_eq!((tokens.input, tokens.output), (10, 20), "the answered turn is what the trial cost");
}

/// **Several refusals inside one retry that then succeeded are one recovery.**
#[test]
fn several_refusals_closed_by_one_success_are_recovered() {
    read("saga", &[refused(), retry_start(), refused(), retry_start(), answered(1, 2), retry_end(true)])
        .unwrap_or_else(|e| panic!("two refusals and then success are a recovery: {e}"));
}

/// **A refusal omp gave up on is still the provider's**, quoting its words.
#[test]
fn a_refusal_omp_gave_up_on_is_still_a_harness_error() {
    let said = read("gave-up", &[refused(), retry_start(), refused(), retry_end(false)]).expect_err("retries that failed are a refusal");
    assert!(said.contains(LIMIT_WORDS), "the provider's words: {said}");
    assert!(said.starts_with("the model provider refused a turn"), "and the breaker's marker: {said}");
}

/// **A refusal with no retry at all** (a spent quota is not retried) **and one nothing closed** are refusals too.
#[test]
fn an_unretried_or_unclosed_refusal_is_a_harness_error() {
    let said = read("none", &[refused()]).expect_err("a refusal and nothing after it");
    assert!(said.contains(LIMIT_WORDS), "{said}");
    let said = read("unclosed", &[refused(), retry_start(), answered(1, 2)]).expect_err("an answer after a refusal that no retry closed");
    assert!(said.contains(LIMIT_WORDS), "a stream that never says the retry succeeded is not read as if it had: {said}");
    let record_reason = said;
    assert!(is_provider_refusal(&bevy_cauldron_bench::TrialRecord {
        task: "t".to_owned(),
        trial: 1,
        agent: "omp".to_owned(),
        model: None,
        variant: None,
        outcome: bevy_cauldron_bench::Outcome::HarnessError,
        reason: record_reason,
        checks: Vec::new(),
        tool_calls: Vec::new(),
        tool_log: Vec::new(),
        tokens: None,
        cost_reported_usd: None,
        edits_behind: Vec::new(),
        wall_s: 0.0,
        tools_offered: Vec::new(),
        wishes: Vec::new(),
        dir: PathBuf::new(),
        held_out: false,
    }));
}

/// **A stand-in `omp`** that exports and imports one credential and answers `-p` with the lines in `stream`, one per line.
#[cfg(unix)]
fn stand_in(dir: &Path, stream: &[Value]) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(bin.join("stream"), stream.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let path = bin.join("omp");
    let script = r#"#!/bin/sh
here="$(dirname "$0")"
case "$1 $2" in
  "auth export") echo '{"version":1,"credentials":[{"provider":"anthropic","credential":{"type":"api_key","key":"k"}}]}'; exit 0 ;;
  "auth import") mkdir -p "$HOME/.omp/agent"; cp "$3" "$HOME/.omp/agent/agent.db"; exit 0 ;;
esac
[ "$1" = "-p" ] && { cat "$here/stream"; exit 0; }
exit 2
"#;
    std::fs::write(&path, script).unwrap_or_else(|e| panic!("{e}"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    path
}

/// **The pre-flight turn reads the stream the same way**: a refusal omp retried and recovered from passes it, one it gave up on does not.
#[cfg(unix)]
#[test]
fn the_preflight_turn_forgives_a_recovered_refusal_too() {
    let dir = scratch("preflight");
    let model = "anthropic/claude-sonnet-5-5";
    let omp = stand_in(&dir, &[refused(), retry_start(), answered(3, 1), retry_end(true)]);
    preflight_turn(&omp.to_string_lossy(), model, &dir.join("run/p")).unwrap_or_else(|e| panic!("a recovered refusal passes: {e}"));
    let omp = stand_in(&dir, &[refused(), retry_start(), refused(), retry_end(false)]);
    let said = preflight_turn(&omp.to_string_lossy(), model, &dir.join("run/p")).expect_err("a refusal omp gave up on fails it");
    assert!(said.contains(LIMIT_WORDS), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}
