//! **`budget.tool_calls` is enforced: an agent that has used its calls is stopped** (#432, D3).
//!
//! Every task declares `budget: {wall_s: 600, tool_calls: 60}`, and until now only `wall_s` was read: a task's
//! `tool_calls` was parsed, defaulted and then ignored by the runner, the grader, Claude Code's reader and the
//! report. (Deleting the field is not open: the held-out tasks outside this repository carry it, and a file with an
//! unknown field is refused.) The runner now counts the calls in the transcript the harness is writing, and stops
//! the agent's process group once it has made as many as the task gave it.
//!
//! The agent here is a shell loop that appends the events omp and Claude Code write, one every few
//! milliseconds: no model, no editor.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use bevy_cauldron_bench::run::{claude_calls, detach, omp_calls, wait_within, ToolBudget, WaitEnd};
use serde_json::json;

/// **A scratch directory this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-budget-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **An agent that makes `calls` tool calls, one every 50 ms**, each a `tool_execution_end` line in `transcript`, with
/// a `message_update` between them (which is not a call), then exits.
fn agent(transcript: &Path, calls: u32) -> std::process::Child {
    let script = format!(
        "i=0; while [ $i -lt {calls} ]; do \
           echo '{{\"type\":\"message_update\"}}' >> '{t}'; \
           echo '{{\"type\":\"tool_execution_end\",\"result\":{{}}}}' >> '{t}'; \
           i=$((i+1)); sleep 0.05; \
         done",
        t = transcript.display()
    );
    let mut command = Command::new("sh");
    command.args(["-c", &script]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    detach(&mut command).spawn().unwrap_or_else(|e| panic!("sh did not start: {e}"))
}

/// **How many calls the transcript holds**, counted the way the runner counts them.
fn written(transcript: &Path) -> usize {
    std::fs::read_to_string(transcript).unwrap_or_default().lines().filter(|l| l.contains("tool_execution_end")).count()
}

/// **An agent that has used its calls is stopped**, long before its wall budget and before it finishes.
#[test]
fn an_agent_is_stopped_at_its_tool_call_budget() {
    let dir = scratch("stops");
    let transcript = dir.join("transcript.jsonl");
    let child = agent(&transcript, 400); // would run 20 s
    let mut budget = ToolBudget::new(transcript.clone(), omp_calls, 5);
    let started = std::time::Instant::now();
    let end = wait_within(child, 120, "stand-in", Some(&mut budget), || None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(end, WaitEnd::CallBudget, "the wait says it ended at the budget, so the trial can be recorded as stopped");
    assert!(started.elapsed().as_secs() < 10, "it was stopped, not waited out: {:?}", started.elapsed());
    let made = written(&transcript);
    assert!((6..400).contains(&made), "it made its 5 calls and the one beyond them, and nowhere near its 400: {made}");
    assert!(made < 40, "and it was stopped soon after the fifth (the poll is a quarter of a second): {made}");
    // **Nothing is still writing**: the process group is dead, not only the shell.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = written(&transcript);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(written(&transcript), after, "the agent writes no more once it is stopped");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **An agent within its budget is left alone**, and so is one with no meter at all.
#[test]
fn an_agent_within_its_budget_runs_to_the_end() {
    let dir = scratch("within");
    let transcript = dir.join("transcript.jsonl");
    let child = agent(&transcript, 6);
    let mut budget = ToolBudget::new(transcript.clone(), omp_calls, 60);
    wait_within(child, 120, "stand-in", Some(&mut budget), || None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(written(&transcript), 6, "all six calls were made: it was never stopped");
    let _ = std::fs::remove_dir_all(&dir);
    let dir = scratch("unmetered");
    let transcript = dir.join("transcript.jsonl");
    wait_within(agent(&transcript, 6), 120, "stand-in", None, || None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(written(&transcript), 6, "with no meter the agent finishes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **An agent that uses exactly its budget is not cut off**, and its wait says it finished (#433).
#[test]
fn an_agent_that_uses_exactly_its_budget_is_not_stopped() {
    let dir = scratch("exact");
    let transcript = dir.join("transcript.jsonl");
    let mut budget = ToolBudget::new(transcript.clone(), omp_calls, 6);
    let end = wait_within(agent(&transcript, 6), 120, "stand-in", Some(&mut budget), || None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(end, WaitEnd::Finished, "six calls of a budget of six: it ran to its own end");
    assert_eq!(written(&transcript), 6, "every call was made, the last one included");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The editor's death still ends the wait**, with its reason, budget or none.
#[test]
fn the_editors_death_still_ends_the_wait() {
    let dir = scratch("editor");
    let transcript = dir.join("transcript.jsonl");
    let child = agent(&transcript, 400);
    let mut budget = ToolBudget::new(transcript.clone(), omp_calls, 100);
    let said = wait_within(child, 120, "stand-in", Some(&mut budget), || Some("the editor exited".to_owned()))
        .expect_err("a dead editor is the trial's reason");
    assert_eq!(said, "the editor exited");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **What counts as a call**: omp's `tool_execution_end`, Claude Code's `tool_use` blocks; a half-written line counts when it is whole.
#[test]
fn only_a_finished_call_is_counted() {
    let dir = scratch("counts");
    let transcript = dir.join("transcript.jsonl");
    let mut meter = ToolBudget::new(transcript.clone(), omp_calls, 3);
    assert_eq!(meter.calls(), 0, "no transcript yet is no calls");
    let end = json!({"type": "tool_execution_end"}).to_string();
    let append = |text: &str| {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&transcript).unwrap_or_else(|e| panic!("{e}"));
        std::io::Write::write_all(&mut file, text.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    };
    append(&format!("{{\"type\":\"message_update\"}}\n{end}\n{{\"type\":\"tool_exec"));
    assert_eq!(meter.calls(), 1, "one whole call, and a line still being written is not one yet");
    append("ution_end\"}\n");
    assert_eq!(meter.calls(), 2, "the half line, once whole, is counted once: {}", meter.calls());
    assert!(!meter.spent(), "two of three is not spent");
    append(&format!("{end}\n"));
    assert!(!meter.spent(), "the third call uses a budget of three exactly: it is not spent until a call goes beyond it (#433)");
    append(&format!("{end}\n"));
    assert!(meter.spent(), "the fourth call is beyond a budget of three");
    // Claude Code: each `tool_use` block of an assistant message is a call, whatever else the message holds.
    let assistant = json!({"type": "assistant", "message": {"content": [
        {"type": "text", "text": "x"}, {"type": "tool_use", "id": "a", "name": "Read"}, {"type": "tool_use", "id": "b", "name": "Bash"}]}});
    assert_eq!(claude_calls(&assistant), 2);
    assert_eq!(claude_calls(&json!({"type": "user"})), 0);
    assert_eq!(omp_calls(&json!({"type": "tool_execution_end"})), 1);
    assert_eq!(omp_calls(&json!({"type": "tool_execution_start"})), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
