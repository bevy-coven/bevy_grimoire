//! **A call an omp agent makes through its `eval` tool is seen by the runner** (#434, BM-10).
//!
//! omp reaches an editor tool two ways: `write xd://mcp__<server>_<tool>` (a `tool_execution_end` whose
//! `result.details.xdev` names the tool and carries its arguments) and `eval` (`await tool.mcp__<server>_<tool>(...)`,
//! a `tool_execution_end` whose `result.details.cells[*].statusEvents[*].op` names each tool it called, with no arguments).
//! The runner read only the first, so a correct trial that answered through `eval` was scored "no hand-back was made" (the real
//! smoke's `refuse-a-piece-the-project-lacks` trial 2). The shape here is the one measured in that trial's transcript.
//!
//! What the runner cannot read from an `eval` call (its arguments) it takes from the editor (see
//! `an_unseen_hand_back_is_graded_from_the_editor.rs`).

use bevy_cauldron_bench::run::{omp_calls, read_transcript};
use serde_json::{json, Value};

const SERVER: &str = "cauldron";

fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-eval-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **An `eval` tool's end**, as omp writes it: the tool is `eval`, and each cell lists the tools it called.
fn eval_end(code: &str, ops: &[(&str, u64)]) -> Value {
    let events: Vec<Value> = ops.iter().map(|(op, chars)| json!({"op": op, "chars": chars})).collect();
    json!({"type": "tool_execution_end", "toolName": "eval", "isError": false,
        "result": {"details": {"cells": [{"code": code, "statusEvents": events}]}}})
}

/// **A `write xd://...` call's end**, the shape the runner already reads.
fn xdev_end(name: &str, args: Value) -> Value {
    json!({"type": "tool_execution_end", "toolName": "write", "isError": false, "result": {"details": {"xdev": {
        "tool": format!("mcp__{SERVER}_{name}"), "args": args,
        "inner": {"mcpToolName": name, "isError": false, "rawContent": [{"type": "text", "text": "{\"ok\":true}"}]}}}}})
}

fn turn() -> Value {
    json!({"type": "message_end", "message": {"role": "assistant", "stopReason": "stop",
        "usage": {"input": 5, "output": 7, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 12}}})
}

fn read(test: &str, events: &[Value]) -> bevy_cauldron_bench::run::AgentRun {
    let dir = scratch(test);
    std::fs::write(dir.join("transcript.jsonl"), events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        dir.join("tools-offered.jsonl"),
        format!("{{\"why\":\"before_agent_start\",\"tools\":[\"mcp__{SERVER}_grimoire_state\",\"eval\",\"read\"]}}"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let run = read_transcript(&dir.join("transcript.jsonl"), &dir, SERVER).unwrap_or_else(|e| panic!("the transcript reads: {e}"));
    let _ = std::fs::remove_dir_all(&dir);
    run
}

/// **The calls an `eval` made are in the call list**, by the tool's own name, in order, with the `write xd://` ones.
#[test]
fn eval_routed_calls_are_counted_like_xdev_calls() {
    let run = read(
        "counted",
        &[
            turn(),
            eval_end("print(await tool.mcp__cauldron_grimoire_guide({}));", &[("mcp__cauldron_grimoire_guide", 17688), ("mcp__cauldron_grimoire_state", 8756)]),
            xdev_end("grimoire_list_pieces", json!({})),
            eval_end("print(JSON.stringify(await tool.mcp__cauldron_grimoire_present({title:\"x\"})));", &[("mcp__cauldron_grimoire_present", 50)]),
        ],
    );
    assert_eq!(
        run.tool_calls,
        ["grimoire_guide", "grimoire_state", "grimoire_list_pieces", "grimoire_present"],
        "the names carry no `mcp__<server>_` prefix, in the order the agent made them"
    );
    assert_eq!(run.tool_log.len(), 4);
    assert!(run.tool_log.iter().all(|c| !c.refused), "a call the transcript shows no refusal for is not counted refused");
    assert_eq!(
        run.blind_calls,
        ["grimoire_guide", "grimoire_state", "grimoire_present"],
        "the three the eval made are the ones whose arguments the transcript did not show; the `write xd://` one is not"
    );
}

/// **A hand-back made through `eval` is known to have been made**, though its words are not in the transcript.
#[test]
fn an_eval_routed_hand_back_is_marked_unread() {
    let run = read("present", &[turn(), eval_end("await tool.mcp__cauldron_grimoire_present({})", &[("mcp__cauldron_grimoire_present", 50)])]);
    assert!(run.handbacks.is_empty(), "no words to read in the transcript: {:?}", run.handbacks);
    assert!(run.blind_calls.iter().any(|c| c == "grimoire_present"), "but the runner knows one was made");
}

/// **A wish made through `eval` is counted**, with the name it was made through and no content.
#[test]
fn an_eval_routed_wish_is_counted() {
    let run = read("wish", &[turn(), eval_end("await tool.mcp__cauldron_grimoire_wish({})", &[("mcp__cauldron_grimoire_wish", 40)])]);
    assert_eq!(run.wishes.len(), 1, "{:?}", run.wishes);
}

/// **The tool-call budget counts the calls an `eval` made**, not the one `eval` tool call.
#[test]
fn the_budget_counts_each_call_an_eval_made() {
    let three = eval_end("x", &[("mcp__cauldron_grimoire_guide", 1), ("mcp__cauldron_grimoire_state", 1), ("mcp__cauldron_grimoire_present", 1)]);
    assert_eq!(omp_calls(&three), 3, "three editor calls in one eval are three calls");
    assert_eq!(omp_calls(&eval_end("print(1)", &[])), 1, "an eval that called no editor tool is one call");
    assert_eq!(omp_calls(&xdev_end("grimoire_state", json!({}))), 1, "a `write xd://` call is one");
    assert_eq!(omp_calls(&json!({"type": "tool_execution_start"})), 0);
}
