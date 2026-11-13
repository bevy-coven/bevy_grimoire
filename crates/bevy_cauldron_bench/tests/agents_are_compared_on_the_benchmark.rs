//! **Agents and models compared on the benchmark: Claude Code beside `omp`, success against cost**
//! (BM-06, #416).
//!
//! Like `a_report_says_what_a_run_did`, these tests need no model: a matrix is built from recorded
//! reports, and Claude Code's transcript is read from recorded stream-json events. The one test that
//! runs the harness end to end ([`a_claude_code_run_writes_trials_with_tokens_and_calls_read_from_the_transcript`])
//! stands a recording in for `claude` on the `PATH`, because a trial must be provable on a host with no
//! login; it starts an editor and so is `#[ignore]`d like the rest of `cauldron-bench run`'s proofs.

use std::path::{Path, PathBuf};

use bevy_cauldron_bench::claude::read_stream;
use bevy_cauldron_bench::index;
use bevy_cauldron_bench::matrix::{Frontier, Matrix, Pair};
use bevy_cauldron_bench::report::{Header, Price, Report, RunData, REPORT_VERSION, RUN_FILE};
use bevy_cauldron_bench::run::Agent;
use serde_json::{json, Value};

// -------------------------------------------------------------------------------------------------
// Fixtures.
// -------------------------------------------------------------------------------------------------

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-matrix-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn header(agent: &str, model: &str, price: Option<Price>) -> Header {
    Header {
        version: REPORT_VERSION,
        label: format!("matrix-{agent}"),
        commit: "feedfacefeedfacefeedfacefeedfacefeedface".to_owned(),
        guide_sha256: "cd".repeat(32),
        agent: agent.to_owned(),
        agent_version: "1.0".to_owned(),
        model: Some(model.to_owned()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: 2,
        host: "big".to_owned(),
        date: "2026-11-13".to_owned(),
        price,
    }
}

/// **One pair's run**: `tasks` tasks of two trials, `passing` of them passing both; every trial reports
/// `tokens` of input and the same output, and `refused` names a tool the editor refused on each trial.
fn run_of(dir: &Path, h: &Header, passing: usize, tokens: u64, refused: &[&str]) -> Report {
    std::fs::create_dir_all(dir).expect("run dir");
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string(h).expect("header")).expect("run.json");
    for task in 0..4usize {
        for n in 1..=2u32 {
            let d = dir.join(format!("task-{task}")).join(format!("trial-{n}"));
            std::fs::create_dir_all(&d).expect("trial dir");
            let log: Vec<Value> = refused.iter().map(|t| json!({"tool": t, "args": "{}", "refused": true})).collect();
            let by_tool: serde_json::Map<String, Value> = refused.iter().map(|t| ((*t).to_owned(), json!(1))).collect();
            let trial = json!({
                "task": format!("task-{task}"), "trial": n, "title": "t", "split": "dev", "tags": ["place"],
                "outcome": if task < passing { "pass" } else { "fail" }, "checks": [],
                "tool_calls": ["grimoire_state"], "tool_log": log, "refusals": refused.len(),
                "refusals_by_tool": by_tool, "tokens_in": tokens, "tokens_out": 0, "wall_s": 12.0,
            });
            std::fs::write(d.join("trial.json"), trial.to_string()).expect("trial.json");
        }
    }
    Report::build(&RunData::read(dir).expect("reads"))
}

// -------------------------------------------------------------------------------------------------
// The matrix (#416, acceptance 2).
// -------------------------------------------------------------------------------------------------

#[test]
fn a_matrix_of_pairs_writes_one_report_with_every_row_and_the_frontier_marked() {
    let dir = scratch("frontier");
    // $1 per million input tokens. 1 M tokens per trial is $1 a trial.
    let price = |input: f64| Some(Price { input, output: 0.0, cache_read: 0.0, cache_write: 0.0 });
    let strong = run_of(&dir.join("strong"), &header("claude-code", "sonnet", price(1.0)), 4, 1_000_000, &[]);
    let cheap = run_of(&dir.join("cheap"), &header("omp", "ollama-cloud/deepseek-v4.1-flash:1017", price(0.1)), 3, 1_000_000, &[]);
    let wasteful = run_of(&dir.join("wasteful"), &header("omp", "other/expensive", price(2.0)), 2, 1_000_000, &[]);
    let matrix = Matrix::build("frontier", &[strong, cheap, wasteful]).expect("three alike runs make a matrix");

    assert_eq!(matrix.rows.len(), 3, "a row per pair");
    let frontier = |pair: &str| matrix.rows.iter().find(|r| r.pair == pair).expect("a row").frontier;
    // sonnet: 100% at $1/trial. deepseek: 75% at $0.10/trial. expensive: 50% at $2/trial.
    assert_eq!(frontier("claude-code:sonnet"), Frontier::On, "the most successful pair is on the frontier");
    assert_eq!(frontier("omp:ollama-cloud/deepseek-v4.1-flash:1017"), Frontier::On, "the cheapest pair is too");
    assert_eq!(frontier("omp:other/expensive"), Frontier::Dominated, "lower success at higher cost than the cheap pair");

    let md = matrix.markdown();
    for needle in ["claude-code:sonnet", "omp:other/expensive", "on the frontier", "dominated", "same editor commit"] {
        assert!(md.contains(needle), "the matrix names `{needle}`:\n{md}");
    }
    // One report, published as a pair of files, and read back.
    let bench = dir.join("docs/bench");
    let (md_path, json_path) = index::publish_matrix(&bench, &matrix).expect("publishes");
    assert!(md_path.ends_with("matrices/2026-11-13-frontier.md") && json_path.is_file());
    let back: Matrix = serde_json::from_str(&std::fs::read_to_string(&json_path).expect("json")).expect("parses");
    assert_eq!(back, matrix);
    assert!(
        std::fs::read_to_string(bench.join("README.md")).expect("README").contains("2026-11-13-frontier"),
        "the README lists the matrix"
    );
}

#[test]
fn a_matrix_refuses_pairs_run_at_different_commits_guides_or_suites() {
    let dir = scratch("refuse");
    let a = run_of(&dir.join("a"), &header("omp", "m1", None), 2, 0, &[]);
    let mut later = header("claude-code", "sonnet", None);
    later.commit = "0000000000000000000000000000000000000000".to_owned();
    let b = run_of(&dir.join("b"), &later, 2, 0, &[]);
    let refused = Matrix::build("x", &[a.clone(), b]).expect_err("a different commit is refused");
    assert!(refused.contains("editor commit"), "{refused}");
    assert!(refused.contains("same tasks, trials and editor commit"), "it says why: {refused}");

    let mut guide = header("claude-code", "sonnet", None);
    guide.guide_sha256 = "ef".repeat(32);
    let c = run_of(&dir.join("c"), &guide, 2, 0, &[]);
    assert!(Matrix::build("x", &[a.clone(), c]).expect_err("a different guide").contains("shipped guide"));

    let mut trials = header("claude-code", "sonnet", None);
    trials.trials = 5;
    let d = run_of(&dir.join("d"), &trials, 2, 0, &[]);
    assert!(Matrix::build("x", &[a.clone(), d]).expect_err("different trial counts").contains("trials per task"));

    assert!(Matrix::build("x", std::slice::from_ref(&a)).is_err(), "one run is a report, not a matrix");
    assert!(Matrix::build("x", &[a.clone(), a]).expect_err("the same pair twice").contains("twice"));
}

#[test]
fn a_model_with_no_price_is_unknown_and_not_placed_and_a_local_price_of_zero_is_zero() {
    let dir = scratch("unknown-price");
    let local_free = Some(Price { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 });
    let known = run_of(&dir.join("known"), &header("omp", "llama.cpp/priced-zero", local_free), 2, 1_000_000, &[]);
    let unpriced = run_of(&dir.join("unpriced"), &header("omp", "llama.cpp/bonsai-2-27b", None), 4, 1_000_000, &[]);
    let matrix = Matrix::build("unknown", &[known, unpriced]).expect("builds");
    let row = |pair: &str| matrix.rows.iter().find(|r| r.pair == pair).expect("row");
    assert_eq!(row("omp:llama.cpp/priced-zero").cost_per_trial_usd, Some(0.0), "a listed price of 0 is 0");
    assert_eq!(row("omp:llama.cpp/bonsai-2-27b").cost_per_trial_usd, None, "no listed price is not 0");
    assert_eq!(row("omp:llama.cpp/bonsai-2-27b").frontier, Frontier::Unplaced, "and is not placed on the frontier");
    assert_eq!(row("omp:llama.cpp/priced-zero").frontier, Frontier::On);
    let md = matrix.markdown();
    assert!(md.contains("unknown"), "the unknown is said, not hidden:\n{md}");
    assert!(md.contains("cost unknown, not placed"), "{md}");
}

#[test]
fn the_matrix_says_whether_a_hard_spot_is_the_factorys_or_one_agents() {
    let dir = scratch("stumbles");
    let a = run_of(&dir.join("a"), &header("omp", "m1", None), 2, 0, &["grimoire_apply_operation", "grimoire_lesson"]);
    let b = run_of(&dir.join("b"), &header("claude-code", "sonnet", None), 2, 0, &["grimoire_apply_operation"]);
    let matrix = Matrix::build("stumbles", &[a, b]).expect("builds");
    assert_eq!(matrix.refused_by_every_pair, vec!["grimoire_apply_operation".to_owned()], "every pair stumbles here");
    assert_eq!(
        matrix.refused_by_one_pair.get("omp:m1"),
        Some(&vec!["grimoire_lesson".to_owned()]),
        "only omp was refused the lesson"
    );
    let md = matrix.markdown();
    assert!(md.contains("refused to every pair") && md.contains("refused only to `omp:m1`"), "{md}");
    assert!(md.contains("tools called most") && md.contains("refusals by tool"), "each row lists its stumbles:\n{md}");
}

#[test]
fn pairs_are_read_as_agent_and_model() {
    let pairs = Pair::parse_all(
        "omp:ollama-cloud/deepseek-v4.1-flash:1017, omp:llama.cpp/bonsai-2-27b, claude-code:sonnet",
    )
    .expect("the three pairs of the issue");
    assert_eq!(pairs.len(), 3);
    assert_eq!(pairs[0].agent, Agent::Omp);
    assert_eq!(pairs[0].model, "ollama-cloud/deepseek-v4.1-flash:1017", "a model keeps its own colons");
    assert_eq!(pairs[2].agent, Agent::ClaudeCode);
    assert_eq!(pairs[2].model, "sonnet");
    assert_eq!(pairs[2].slug(), "claude-code-sonnet");
    assert!(Pair::parse_all("omp:a,omp:a").expect_err("a pair twice").contains("twice"));
    assert!(Pair::parse_all("omp:a").is_err(), "one pair is not a matrix");
    assert!(Pair::parse("gpt:x").is_err(), "an agent this build does not have");
    assert!(Pair::parse("oracle:x").is_err(), "a scripted agent runs no model");
    assert_eq!(Pair::parse("claude-code").expect("a bare harness").model, "sonnet", "its own default");
    assert_eq!(Agent::parse("claude-code"), Ok(Agent::ClaudeCode));
}

// -------------------------------------------------------------------------------------------------
// Claude Code's transcript (#416, acceptance 1, without a login).
// -------------------------------------------------------------------------------------------------

/// **A stream-json transcript as `claude -p --output-format stream-json --verbose` writes it**, shaped as
/// a real `system/init` and `result` event are (measured on this host, claude 2.1.288), with two calls to
/// the editor's tools between them, one of which the editor refuses.
fn stream(init_tools: &[&str], servers: &[(&str, &str)], extra_call: Option<&str>) -> String {
    let mut events = vec![json!({
        "type": "system", "subtype": "init", "session_id": "s", "tools": init_tools,
        "mcp_servers": servers.iter().map(|(n, s)| json!({"name": n, "status": s})).collect::<Vec<_>>(),
    })];
    let usage = |i: u64, o: u64, r: u64, w: u64| {
        json!({"input_tokens": i, "output_tokens": o, "cache_read_input_tokens": r, "cache_creation_input_tokens": w})
    };
    events.push(json!({"type": "assistant", "message": {"id": "m1", "model": "claude-sonnet", "role": "assistant",
        "usage": usage(10, 5, 100, 1_000),
        "content": [{"type": "tool_use", "id": "t1", "name": "mcp__cauldron__grimoire_state", "input": {}}]}}));
    events.push(json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": "t1", "content": [{"type": "text", "text": "{\"placements\":3}"}], "is_error": false}]}}));
    events.push(json!({"type": "assistant", "message": {"id": "m2", "model": "claude-sonnet", "role": "assistant",
        "usage": usage(20, 15, 200, 0),
        "content": [{"type": "tool_use", "id": "t2", "name": "mcp__cauldron__grimoire_apply_operation",
                     "input": {"operation": "placement_remove", "parameters": {"id": "piano"}}}]}}));
    events.push(json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": "t2", "content": "{\"refused\":\"no placement is called piano\"}", "is_error": false}]}}));
    events.push(json!({"type": "assistant", "message": {"id": "m3", "model": "claude-sonnet", "role": "assistant",
        "usage": usage(30, 25, 300, 0),
        "content": [{"type": "tool_use", "id": "t3", "name": "mcp__cauldron__grimoire_present",
                     "input": {"title": "No piano", "summary": "There is no piano.", "specified": ["remove the piano"]}}]}}));
    events.push(json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": "t3", "content": "ok", "is_error": false}]}}));
    if let Some(name) = extra_call {
        events.push(json!({"type": "assistant", "message": {"id": "m4", "model": "claude-sonnet", "role": "assistant",
            "usage": usage(1, 1, 1, 1), "content": [{"type": "tool_use", "id": "t4", "name": name, "input": {}}]}}));
    }
    // The `result` event's usage is cumulative over the run, and is what the report uses.
    events.push(json!({"type": "result", "subtype": "success", "is_error": false, "total_cost_usd": 0.0421,
        "usage": usage(60, 45, 600, 1_000)}));
    events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
}

fn read(test: &str, text: &str) -> Result<bevy_cauldron_bench::run::AgentRun, String> {
    let dir = scratch(test);
    std::fs::write(dir.join("transcript.jsonl"), text).expect("transcript");
    read_stream(&dir.join("transcript.jsonl"), &dir, "cauldron")
}

const EDITOR_TOOLS: &[&str] = &[
    "mcp__cauldron__grimoire_state",
    "mcp__cauldron__grimoire_apply_operation",
    "mcp__cauldron__grimoire_present",
];

fn offered(extra: &[&str]) -> Vec<&'static str> {
    let mut tools: Vec<&'static str> = vec!["Bash", "Read", "Write"];
    tools.extend(EDITOR_TOOLS);
    let _ = extra;
    tools
}

#[test]
fn a_stream_json_transcript_gives_tokens_calls_refusals_and_cost() {
    let run = read("stream-ok", &stream(&offered(&[]), &[("cauldron", "connected")], None)).expect("reads");
    assert_eq!(
        run.tool_calls,
        vec!["grimoire_state", "grimoire_apply_operation", "grimoire_present"],
        "the editor's tools by their own names, not Claude Code's `mcp__cauldron__` spelling"
    );
    assert_eq!(
        run.tool_log.iter().map(|c| c.refused).collect::<Vec<_>>(),
        vec![false, true, false],
        "a payload that says `refused` is a refusal even when the envelope says it is not an error"
    );
    let tokens = run.usage.expect("tokens");
    assert_eq!(
        (tokens.input, tokens.output, tokens.cache_read, tokens.cache_write),
        (60, 45, 600, 1_000),
        "the result event's cumulative usage, not a sum that counts a turn twice"
    );
    assert_eq!(run.cost_usd, Some(0.0421), "the harness's own cost is kept for a model with no listed price");
    assert_eq!(run.handbacks.len(), 1, "the hand-back is read from the call");
    assert!(run.tools_offered.contains(&"grimoire_state".to_owned()), "offered tools carry the bare name");
    assert!(run.tools_offered.contains(&"Bash".to_owned()));
}

#[test]
fn a_run_killed_before_its_result_sums_the_last_usage_of_each_message() {
    let text = stream(&offered(&[]), &[("cauldron", "connected")], None);
    let without_result: String = text.lines().filter(|l| !l.contains("\"type\":\"result\"")).collect::<Vec<_>>().join("\n");
    let run = read("stream-killed", &without_result).expect("reads");
    let tokens = run.usage.expect("tokens");
    // m1 (10, 5, 100, 1000) + m2 (20, 15, 200, 0) + m3 (30, 25, 300, 0).
    assert_eq!((tokens.input, tokens.output, tokens.cache_read, tokens.cache_write), (60, 45, 600, 1_000));
    assert_eq!(run.cost_usd, None);
}

#[test]
fn a_profile_that_was_not_isolated_is_a_harness_error_naming_what_leaked() {
    let leaked_server = read(
        "stream-server",
        &stream(&offered(&[]), &[("cauldron", "connected"), ("home-still", "connected")], None),
    )
    .expect_err("another MCP server is a leak");
    assert!(leaked_server.contains("home-still") && leaked_server.contains("not isolated"), "{leaked_server}");

    let mut tools = offered(&[]);
    tools.push("mcp__golem__generate_image");
    let leaked_tool = read("stream-tool", &stream(&tools, &[("cauldron", "connected")], None)).expect_err("a foreign tool");
    assert!(leaked_tool.contains("mcp__golem__generate_image"), "{leaked_tool}");

    let called = read(
        "stream-called",
        &stream(&offered(&[]), &[("cauldron", "connected")], Some("mcp__elsewhere__do_it")),
    )
    .expect_err("a call outside the set");
    assert!(called.contains("mcp__elsewhere__do_it"), "{called}");

    let down = read("stream-down", &stream(&offered(&[]), &[("cauldron", "failed")], None)).expect_err("a server that never connected");
    assert!(down.contains("did not connect"), "{down}");

    let none = read("stream-none", &stream(&["Bash", "Read"], &[("cauldron", "connected")], None))
        .expect_err("none of the editor's tools offered");
    assert!(none.contains("none of the editor's tools"), "{none}");
}

#[test]
fn a_login_that_failed_is_a_harness_error_and_not_the_agents_failure() {
    let events = [
        json!({"type": "system", "subtype": "init", "tools": offered(&[]), "mcp_servers": [{"name": "cauldron", "status": "connected"}]}),
        json!({"type": "assistant", "message": {"id": "x", "model": "<synthetic>", "role": "assistant",
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "content": [{"type": "text", "text": "Failed to authenticate: OAuth session expired"}]}}),
        json!({"type": "result", "subtype": "success", "is_error": true, "result": "Failed to authenticate: OAuth session expired",
               "usage": {"input_tokens": 0, "output_tokens": 0}, "total_cost_usd": 0}),
    ];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let why = read("stream-login", &text).expect_err("a harness that never reached the model");
    assert!(why.contains("never reached the model") && why.contains("OAuth session expired"), "{why}");
}

// -------------------------------------------------------------------------------------------------
// The whole pipe, with a recording standing in for `claude` (#416, acceptance 1).
// -------------------------------------------------------------------------------------------------

/// **`cauldron-bench run --agent claude-code` against a real editor, with a recorded stream-json transcript
/// on the `PATH` where `claude` would be.**
///
/// The recording is what a real `claude` wrote for these calls, so every field a trial reads from it is
/// read from a transcript and not from a fixture the harness made up; what the stand-in cannot do is change
/// the map, so the trial fails its checks and the test asserts only what `trial.json` says about the
/// transcript: its tokens, its tool calls and its outcome being a graded `fail` and not a harness error.
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn a_claude_code_run_writes_trials_with_tokens_and_calls_read_from_the_transcript() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repo");
    let bench = PathBuf::from(env!("CARGO_BIN_EXE_cauldron-bench"));
    let dir = scratch("claude-run");
    let shim_dir = dir.join("bin");
    std::fs::create_dir_all(&shim_dir).expect("shim dir");
    // The recording, with the server's name the editor's own config gives (`cauldron`), played back by a
    // shell script named `claude` that ignores its arguments, as a recorded session does.
    let recording = dir.join("recording.jsonl");
    let calls = offered(&[]);
    std::fs::write(&recording, stream(&calls, &[("cauldron", "connected")], None)).expect("recording");
    let shim = shim_dir.join("claude");
    std::fs::write(&shim, format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo '2.1.288 (Claude Code)'; exit 0; fi\ncat '{}'\n", recording.display()))
        .expect("shim");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let credentials = dir.join("login");
    std::fs::create_dir_all(&credentials).expect("login dir");
    let built = std::process::Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "bevy_cauldron", "-p", "bevy_cauldron_mcp"])
        .current_dir(&repo)
        .output()
        .expect("cargo");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let beside = bench.parent().expect("dir").to_path_buf();
    let path = format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default());
    let out = std::process::Command::new(&bench)
        .args(["run", "--agent", "claude-code", "--model", "sonnet", "--run", "claude-stand-in", "--trials", "1"])
        .args(["--task", "count-what-the-bedroom-holds"])
        .arg("--editor")
        .arg(beside.join("bevy_cauldron"))
        .arg("--server")
        .arg(beside.join("bevy_cauldron_mcp"))
        .current_dir(&repo)
        .env("HOME", &dir)
        .env("PATH", path)
        .env("ANTHROPIC_API_KEY", "stand-in")
        .output()
        .expect("cauldron-bench");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let trial_json = dir.join(".local/state/cauldron-bench/claude-stand-in/count-what-the-bedroom-holds/trial-1/trial.json");
    let record: Value = serde_json::from_str(&std::fs::read_to_string(&trial_json).unwrap_or_else(|e| panic!("{e}:\n{said}")))
        .expect("trial.json is JSON");
    assert_eq!(record["agent"], "claude-code", "{said}");
    assert_ne!(record["outcome"], "harness_error", "the transcript read cleanly: {}", record["reason"]);
    assert_eq!(record["tool_calls"], json!(["grimoire_state", "grimoire_apply_operation", "grimoire_present"]));
    assert_eq!(record["tokens_in"], 60);
    assert_eq!(record["tokens_out"], 45);
    assert_eq!(record["tokens_cache_read"], 600);
    assert_eq!(record["refusals"], 1);
}
