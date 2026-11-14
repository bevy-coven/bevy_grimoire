//! **Claude Code as the benchmarked agent**, beside `omp` (BM-06, #416).
//!
//! The runner's second harness: `claude -p --output-format stream-json --verbose --mcp-config <the config the
//! editor handed out> --strict-mcp-config`. Everything the trial measures comes from what the harness
//! itself wrote, which is the stream-json transcript this module reads ([`read_stream`]): the tools it was
//! offered (`system/init`), the calls it made (`tool_use`) and whether each was refused (`tool_result`),
//! and the tokens and cost it reported (`result`).
//!
//! # Isolated the same way as `omp`
//!
//! A benchmarked agent knows only what the Dark Factory hands it, so Claude Code runs with:
//!
//! - **`CLAUDE_CONFIG_DIR` set to a scratch directory of the trial's own**, so it reads no user
//!   `CLAUDE.md`, no memory, no user settings and no session store;
//! - **`HOME` set to the trial's own**, for the same reason `omp` is;
//! - **`--strict-mcp-config`**, so the only MCP server is the Dark Factory's;
//! - **`--disable-slash-commands`**, so no skill is loaded;
//! - **the credential carried into that scratch directory** (the file `claude` logged in with, or
//!   `ANTHROPIC_API_KEY` from the environment), and nothing else.
//!
//! **A transcript that shows anything else is a harness error**: a tool outside the editor's and the
//! harness's own, or an MCP server other than the Dark Factory's, is a profile that was not isolated and
//! says nothing about the editor.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::run::{handback_of, is_refusal, AgentRun, CallLog, Editor, Tokens};

/// **The model Claude Code runs unless `--model` says otherwise**: an alias the CLI resolves itself.
pub const DEFAULT_MODEL: &str = "sonnet";

/// **Claude Code's own tools**, which an agent is offered beside the editor's and which are not a leak.
///
/// Measured on this host (claude 2.1.288, 2026-11-13) from a `system/init` event, plus the names its
/// documentation gives. **Any other name is a tool the Dark Factory did not hand out**, so a trial that
/// offers one is a harness error naming it, and a new Claude Code that adds a built-in is a refusal that
/// says which rather than a silently wider agent.
pub const BUILTIN_TOOLS: &[&str] = &[
    "Task", "Bash", "BashOutput", "KillShell", "CronCreate", "CronDelete", "CronList", "DesignSync", "Edit",
    "EnterWorktree", "ExitWorktree", "ExitPlanMode", "Glob", "Grep", "ListAgents", "ListMcpResourcesTool",
    "LSP", "Monitor", "NotebookEdit", "PushNotification", "Read", "ReadMcpResourceDirTool",
    "ReadMcpResourceTool", "ReportFindings", "ScheduleWakeup", "SendMessage", "Skill", "SlashCommand",
    "TaskCreate", "TaskGet", "TaskList", "TaskStop", "TaskUpdate", "TodoWrite", "ToolSearch", "WebFetch",
    "WebSearch", "Workflow", "Write", "AskUserQuestion",
];

/// **The built-ins a trial lets the agent use**, the ones omp's own set also has: a shell and the file tools.
///
/// Everything else Claude Code could do (a web fetch, a subagent) is not allowed, so a trial cannot reach
/// outside the project through it. A shell is allowed because a person's agent has one, and what it does
/// with it is measured (`edits_behind`).
pub const ALLOWED_BUILTINS: &[&str] = &["Bash", "Read", "Write", "Edit", "Glob", "Grep"];

/// **The credential file `claude` logs in with**, relative to its config directory.
pub const CREDENTIALS_FILE: &str = ".credentials.json";

/// **Give a trial's own config directory the credential the user's holds**, and nothing else.
///
/// `ANTHROPIC_API_KEY` in the environment needs nothing carried. Otherwise the login file is copied from the
/// user's own config directory (`CLAUDE_CONFIG_DIR`, or `~/.claude`). A host with neither is a harness error
/// naming what the user has to do: a trial whose harness cannot reach a model measures the harness.
pub fn seed_credentials(config: &Path) -> Result<(), String> {
    std::fs::create_dir_all(config).map_err(|e| format!("{}: {e}", config.display()))?;
    if std::env::var_os("ANTHROPIC_API_KEY").is_some_and(|k| !k.is_empty()) {
        return Ok(());
    }
    let user = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".claude")))
        .ok_or("there is no HOME, so Claude Code's login could not be found")?;
    let from = user.join(CREDENTIALS_FILE);
    if !from.is_file() {
        return Err(format!(
            "this host has no Claude Code credential ({} is not a file and ANTHROPIC_API_KEY is not set), so \
             a trial's harness could not reach a model. Log in with `claude` once, or set ANTHROPIC_API_KEY.",
            from.display()
        ));
    }
    std::fs::copy(&from, config.join(CREDENTIALS_FILE))
        .map(|_| ())
        .map_err(|e| format!("{} could not be carried into the trial: {e}", from.display()))
}

/// **Run Claude Code on the config the editor handed out**, in the trial's directory.
///
/// Stopped at the task's budget (Claude Code has no flag for either): its wall clock, and the calls it was
/// given, counted from the stream it writes ([`crate::run::ToolBudget`]). The editor's death ends the
/// trial at once. Answers what the transcript says the agent did.
pub fn run(
    model: &str,
    prompt: &str,
    budget: crate::task::Budget,
    config: &Value,
    project: &Path,
    editor: &mut Editor,
    dir: &Path,
) -> Result<AgentRun, String> {
    let server = config
        .get("mcpServers")
        .and_then(Value::as_object)
        .and_then(|servers| servers.keys().next().cloned())
        .ok_or("the editor's MCP config names no server")?;
    let mcp = dir.join("mcp.json");
    let text = serde_json::to_string_pretty(config).map_err(|e| format!("{e}"))? + "\n";
    bevy_grimoire_core::atomic_file::write(&mcp, text).map_err(|e| format!("{}: {e}", mcp.display()))?;
    let claude_config = dir.join("claude-config");
    // **The login copied into the trial and the HOME beside it are deleted with the trial's agent**, on every
    // path out of this function, as `omp`'s are: a copy of the user's credential file does not outlive its use (#432, D6).
    let _secrets = crate::run::TrialSecrets::new([claude_config.clone(), dir.join("home")]);
    seed_credentials(&claude_config)?;
    // **What the trial's agent can read of the credential**, to be scrubbed from the files the trial keeps (#433): the
    // login file's strings and an `ANTHROPIC_API_KEY` in the environment it inherits.
    let mut secrets: Vec<String> = std::fs::read_to_string(claude_config.join(CREDENTIALS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|login| crate::run::secret_strings(&login))
        .unwrap_or_default();
    secrets.extend(std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty()));
    let transcript = dir.join("transcript.jsonl");
    let stdout = std::fs::File::create(&transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
    let stderr = std::fs::File::create(dir.join("agent.err")).map_err(|e| format!("{e}"))?;
    let allowed = ALLOWED_BUILTINS
        .iter()
        .map(|t| (*t).to_owned())
        .chain(std::iter::once(format!("mcp__{server}")))
        .collect::<Vec<_>>()
        .join(",");
    let mut claude = Command::new("claude");
    claude
        .args([
            "-p",
            prompt,
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            model,
            "--mcp-config",
            &mcp.to_string_lossy(),
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--no-session-persistence",
            "--allowedTools",
            &allowed,
        ])
        .current_dir(project)
        .env("BEVY_BRP_PORT", editor.port.to_string())
        .env("HOME", dir.join("home"))
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    let child = crate::run::detach(&mut claude)
        .spawn()
        .map_err(|e| format!("claude did not start: {e}"))?;
    let mut calls = crate::run::ToolBudget::new(transcript.clone(), crate::run::claude_calls, budget.tool_calls);
    let waited = crate::run::wait_for_agent(child, editor, budget.wall_s, "claude", Some(&mut calls));
    crate::run::scrub_secrets(&[transcript.clone(), dir.join("agent.err")], &secrets);
    let end = waited?;
    let mut run = read_stream(&transcript, dir, &server)?;
    run.call_budget_exhausted = end == crate::run::WaitEnd::CallBudget;
    Ok(run)
}

/// **What a stream-json transcript says the agent did.**
///
/// - **`system/init`** names the tools the agent was offered and the MCP servers it connected: a server
///   other than the Dark Factory's, a tool outside [`BUILTIN_TOOLS`] and the editor's, and a server that did
///   not connect are each a harness error naming what was seen.
/// - **`tool_use` and `tool_result`** are a call and its answer; a call to one of the editor's tools is
///   logged with its arguments and whether the editor refused it ([`is_refusal`]).
/// - **`result`** carries the tokens and cost the harness reported, cumulative over the run. A run killed at
///   its budget has none, and then the last usage of each assistant message is summed instead.
/// - **A run that never reached the model** (`result.is_error` with nothing generated: an expired login, a
///   refused provider) is a harness error quoting the harness's own words.
pub fn read_stream(path: &Path, dir: &Path, server: &str) -> Result<AgentRun, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("the transcript at {} could not be read: {e}", path.display()))?;
    let prefix = format!("mcp__{server}__");
    let mut run = AgentRun::default();
    let mut pending: std::collections::BTreeMap<String, (String, Value)> = std::collections::BTreeMap::new();
    let mut per_message: std::collections::BTreeMap<String, Tokens> = std::collections::BTreeMap::new();
    let mut result_tokens: Option<Tokens> = None;
    let mut saw_init = false;
    let mut spoke = false;
    let mut failure: Option<String> = None;
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event.get("type").and_then(Value::as_str).unwrap_or_default() {
            "system" if event.get("subtype").and_then(Value::as_str) == Some("init") => {
                saw_init = true;
                run.tools_offered = init_tools(&event, &prefix, server)?;
            }
            "assistant" => {
                let Some(message) = event.get("message") else {
                    continue;
                };
                spoke |= message.get("model").and_then(Value::as_str) != Some("<synthetic>");
                if let (Some(id), Some(usage)) = (message.get("id").and_then(Value::as_str), message.get("usage")) {
                    per_message.insert(id.to_owned(), tokens_of(usage));
                }
                for block in message.get("content").and_then(Value::as_array).into_iter().flatten() {
                    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let (Some(id), Some(name)) =
                        (block.get("id").and_then(Value::as_str), block.get("name").and_then(Value::as_str))
                    else {
                        continue;
                    };
                    if !name.starts_with(&prefix) && !BUILTIN_TOOLS.contains(&name) {
                        return Err(format!(
                            "the transcript shows a tool `{name}` that is neither the editor's nor the \
                             harness's own set, so the profile was not isolated"
                        ));
                    }
                    pending.insert(
                        id.to_owned(),
                        (name.to_owned(), block.get("input").cloned().unwrap_or(Value::Null)),
                    );
                }
            }
            "user" => {
                let blocks = event
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let Some(id) = block.get("tool_use_id").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some((name, args)) = pending.remove(id) else {
                        continue;
                    };
                    // **Only the editor's tools are counted**, as for `omp`: the harness's own are not.
                    let Some(own) = name.strip_prefix(&prefix) else {
                        continue;
                    };
                    let answer = result_text(&block);
                    let refused =
                        is_refusal(block.get("is_error").and_then(Value::as_bool).unwrap_or(false), &answer);
                    run.tool_calls.push(own.to_owned());
                    run.tool_log.push(CallLog::new(own, &args, refused));
                    if own == "grimoire_present" {
                        run.handbacks.push(handback_of(&args));
                    }
                    if own == "grimoire_wish" {
                        run.wishes.push(args);
                    }
                }
            }
            "result" => {
                result_tokens = event.get("usage").map(tokens_of);
                run.cost_usd = event.get("total_cost_usd").and_then(Value::as_f64);
                if event.get("is_error").and_then(Value::as_bool) == Some(true) {
                    failure = Some(
                        event
                            .get("result")
                            .and_then(Value::as_str)
                            .unwrap_or("it gave no reason")
                            .lines()
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
            }
            _ => {}
        }
    }
    if let Some(why) = failure.filter(|_| !spoke) {
        return Err(format!(
            "claude never reached the model, so this trial says nothing about the editor: {why}"
        ));
    }
    if !saw_init {
        let stderr = std::fs::read_to_string(dir.join("agent.err")).unwrap_or_default();
        let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
        let from = lines.len().saturating_sub(4);
        return Err(format!(
            "claude wrote no system/init event, so what it was offered is unknown. Its stderr ends: {}",
            lines[from..].join(" | ")
        ));
    }
    if !spoke {
        return Err("claude never produced a model turn, so this trial says nothing about the editor".to_owned());
    }
    let usage = result_tokens.or_else(|| {
        let mut total = Tokens::default();
        for tokens in per_message.values() {
            total.add(*tokens);
        }
        (!per_message.is_empty()).then_some(total)
    });
    run.tokens = usage.map(|u| u.total);
    run.usage = usage;
    Ok(run)
}

/// **The tools a `system/init` offers**, as the bare names the scripted agents' server uses, or the reason
/// the profile was not isolated.
fn init_tools(event: &Value, prefix: &str, server: &str) -> Result<Vec<String>, String> {
    for entry in event.get("mcp_servers").and_then(Value::as_array).into_iter().flatten() {
        let name = entry.get("name").and_then(Value::as_str).unwrap_or_default();
        let status = entry.get("status").and_then(Value::as_str).unwrap_or_default();
        if name != server {
            return Err(format!(
                "the agent connected an MCP server `{name}` that the Dark Factory did not hand out, so the \
                 profile was not isolated"
            ));
        }
        if status != "connected" {
            return Err(format!("the editor's MCP server `{server}` did not connect: its status is `{status}`"));
        }
    }
    let mut names = Vec::new();
    let mut editor_tools = 0usize;
    let mut outside = Vec::new();
    for tool in event.get("tools").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if let Some(own) = tool.strip_prefix(prefix) {
            editor_tools += 1;
            names.push(own.to_owned());
        } else if BUILTIN_TOOLS.contains(&tool) {
            names.push(tool.to_owned());
        } else {
            outside.push(tool.to_owned());
        }
    }
    if !outside.is_empty() {
        return Err(format!(
            "the agent was offered {} that the Dark Factory did not hand out, so the profile was not isolated",
            outside.join(", ")
        ));
    }
    if editor_tools == 0 {
        return Err(format!(
            "the agent was offered none of the editor's tools (`{prefix}*`): the MCP server never connected. \
             It was offered: {}",
            names.join(", ")
        ));
    }
    Ok(names)
}

/// **One usage object as tokens**: Claude Code's four counts, and their sum.
fn tokens_of(usage: &Value) -> Tokens {
    let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let (input, output) = (count("input_tokens"), count("output_tokens"));
    let (cache_read, cache_write) = (count("cache_read_input_tokens"), count("cache_creation_input_tokens"));
    Tokens { input, output, cache_read, cache_write, total: input + output + cache_read + cache_write }
}

/// **A `tool_result`'s text**, whether its content is a string or a list of blocks.
fn result_text(block: &Value) -> String {
    match block.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}
