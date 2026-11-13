//! **`cauldron-bench run`: a real agent against a real editor, on a fresh copy, n times** (BM-02, #412).
//!
//! One trial is:
//!
//! 1. the fixture laid out into a directory of its own, with the task's `setup` applied through the editor;
//! 2. the editor binary started on that directory, on a port and (on Linux) a display of its own;
//! 3. an agent launched **the way a person's is** — the MCP config comes from the editor's own
//!    `grimoire/agent_setup`, so the config a benchmark hands a harness is the config the terminal's agent
//!    tab writes, with its own fresh token;
//! 4. the agent given the task's prompt and nothing else;
//! 5. the agent stopped at `budget.wall_s`, the editor closed, and what the two left graded by
//!    [`bevy_cauldron_bench::Grading`];
//! 6. a `trial.json` under the state directory, holding the outcome, every check with its measured value,
//!    the agent's tool calls, tokens and wall time.
//!
//! # An outcome is one of three, and a harness failure is not an agent failure
//!
//! An editor that did not start, a port that did not open, a transcript that could not be read: the
//! outcome is [`bevy_cauldron_bench::Outcome::HarnessError`] with the reason, **never `pass` and never
//! `fail`**. A harness bug scored as an agent failure is a result about the harness, filed under the wrong
//! name, and it is the mistake that makes a benchmark's numbers move for no reason.
//!
//! # Scripted agents, so the pipe is proved without a model
//!
//! [`Agent::Oracle`] calls the task's own `solution` through the real MCP server and [`Agent::Nothing`]
//! connects and stops. On every suite the oracle must succeed on every task and `nothing` on none  -  the
//! same rule BM-01 applies to a task file, applied to the running editor. A suite where either is untrue is
//! a harness that measures its own plumbing.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::grade::Decision;
use crate::{fixture, Grading, Handback, Outcome, Snapshot, Suite, Task, Verdict, Variant};
use serde_json::{json, Value};

/// **Which agent a run uses.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    /// **A real model harness** (`omp`), which is what a benchmark is for.
    Omp,
    /// **The task's own `solution`, called through the real MCP server**: no model, and it must pass every
    /// task. It proves the editor, the server, the config and the grader are one pipe.
    Oracle,
    /// **It connects and stops**, which must pass no task: it is the do-nothing loophole measured against
    /// the running editor rather than against a task file.
    Nothing,
}

impl Agent {
    /// As `--agent` spells it.
    pub fn name(self) -> &'static str {
        match self {
            Agent::Omp => "omp",
            Agent::Oracle => "oracle",
            Agent::Nothing => "nothing",
        }
    }

    /// The agent a command line names.
    pub fn parse(name: &str) -> Result<Agent, String> {
        for agent in [Agent::Omp, Agent::Oracle, Agent::Nothing] {
            if agent.name() == name {
                return Ok(agent);
            }
        }
        Err(format!("`{name}` is not an agent. The agents are: omp, oracle, nothing."))
    }
}

/// **The model `omp` runs unless `--model` says otherwise**, which is the one this workstation was measured
/// on.
pub const DEFAULT_MODEL: &str = "ollama-cloud/deepseek-v4.1-flash:1017";

/// **How long the editor is given to answer `grimoire/state`**, seconds. A cold start on a loaded host is
/// the slow half (Bevy's plugin graph, the library read, the palette's thumbnails); the measured cost on
/// big is 20-40 s, so this is generous and still bounds a hang.
pub const EDITOR_START_S: u64 = 180;

/// **How long the MCP server is given to answer `tools/list`**, seconds.
pub const SERVER_START_S: u64 = 60;

/// **How many points down the display ladder a trial tries** before it calls the editor a harness error:
/// `Xvfb` refuses a display number a socket already holds, and a run of `--jobs 4` takes four.
pub const DISPLAY_TRIES: u32 = 40;

/// **The first display number a trial looks for**, chosen to sit clear of a developer's own `:0` and the
/// demo recorder's `:96`.
pub const DISPLAY_BASE: u32 = 120;

/// **How much a trial is run with.**
#[derive(Clone, Debug)]
pub struct Options {
    /// The project the tasks run against: the repository's own root, or a game's.
    pub root: PathBuf,
    /// The suite's task directory.
    pub tasks: PathBuf,
    /// Which agent.
    pub agent: Agent,
    /// How many trials of each task.
    pub trials: u32,
    /// How many trials run at once.
    pub jobs: u32,
    /// The model `omp` runs.
    pub model: String,
    /// Which arm, from `bench/variants.yaml`.
    pub variant: Option<String>,
    /// Where raw trials and their `trial.json` go: the user's state directory, never the repository.
    pub out: PathBuf,
    /// The editor binary under test.
    pub editor: PathBuf,
    /// The MCP server binary beside it.
    pub server: PathBuf,
    /// Only these tasks, by id.
    pub only: Vec<String>,
}

/// **What one trial left**, which is what `trial.json` holds.
#[derive(Clone, Debug)]
pub struct TrialRecord {
    /// The task's id.
    pub task: String,
    /// Which trial of that task this is, counted from 1.
    pub trial: u32,
    /// The agent's name.
    pub agent: String,
    /// The model, for a harness that has one.
    pub model: Option<String>,
    /// The arm, when one was named.
    pub variant: Option<String>,
    /// `pass`, `fail` or `harness_error`.
    pub outcome: Outcome,
    /// Why the outcome is a harness error, when it is.
    pub reason: String,
    /// Every check with the value it measured.
    pub checks: Vec<Verdict>,
    /// The agent's tool calls, by name, in the order it made them.
    pub tool_calls: Vec<String>,
    /// What the agent's harness reported for tokens, when it reported any.
    pub tokens: Option<u64>,
    /// How long the agent ran, seconds.
    pub wall_s: f64,
    /// **The tools the agent's `tools/list` held**, which is what a `hide_tools` arm is judged by.
    pub tools_offered: Vec<String>,
    /// The wishes the agent made, as `{tool, task}`.
    pub wishes: Vec<Value>,
    /// The directory this trial's project and transcript are under.
    pub dir: PathBuf,
}

impl TrialRecord {
    /// **The JSON a `trial.json` holds.**
    pub fn to_json(&self) -> Value {
        json!({
            "task": self.task,
            "trial": self.trial,
            "agent": self.agent,
            "model": self.model,
            "variant": self.variant,
            "outcome": self.outcome.name(),
            "reason": self.reason,
            "checks": self.checks,
            "score": {
                "passed": self.checks.iter().filter(|c| c.passed).count(),
                "total": self.checks.len(),
            },
            "tool_calls": self.tool_calls,
            "tool_call_count": self.tool_calls.len(),
            "tokens": self.tokens,
            "wall_s": self.wall_s,
            "tools_offered": self.tools_offered,
            "wishes": self.wishes,
        })
    }
}

// -------------------------------------------------------------------------------------------------
// The trial directories, and the state directory a run writes under.
// -------------------------------------------------------------------------------------------------

/// **Where a run's raw trials go**: `~/.local/state/cauldron-bench/<run>/`, never the repository.
///
/// A trial's directory holds an agent's whole project and its transcript, which are large and are not the
/// project under test. Putting them under the state directory is what keeps a benchmarked project's
/// `git status` empty.
pub fn state_dir(run: &str) -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("there is no HOME, so a run has nowhere to write")?;
    Ok(PathBuf::from(home).join(".local/state/cauldron-bench").join(run))
}

/// **A trial's one line in a report**, as a person reads it.
pub fn summary_line(record: &TrialRecord) -> String {
    let (passed, total) = (
        record.checks.iter().filter(|c| c.passed).count(),
        record.checks.len(),
    );
    let failed: Vec<&str> = record
        .checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.check.as_str())
        .collect();
    match record.outcome {
        Outcome::Pass => format!(
            "{} trial {}: PASS {passed}/{total} checks, {} call(s), {:.0} s",
            record.task,
            record.trial,
            record.tool_calls.len(),
            record.wall_s
        ),
        Outcome::Fail => format!(
            "{} trial {}: FAIL {passed}/{total} checks — {} failed",
            record.task,
            record.trial,
            failed.join(", ")
        ),
        Outcome::HarnessError => format!(
            "{} trial {}: HARNESS ERROR — {}",
            record.task, record.trial, record.reason
        ),
    }
}

// -------------------------------------------------------------------------------------------------
// The editor, and its display.
// -------------------------------------------------------------------------------------------------

/// **A display a trial owns**, killed when the trial ends.
pub struct Display {
    child: Child,
    /// The number, so a log can say which one.
    pub number: u32,
}

impl Display {
    /// **Start an `Xvfb` on the first free display from [`DISPLAY_BASE`] up.**
    ///
    /// **Linux only.** A macOS run uses the desktop's own display, which is what a person's editor is on.
    pub fn start() -> Result<Display, String> {
        // **The socket and the lock file both count as taken**: `Xvfb` refuses a number whose socket
        // exists, and a crashed server can leave one.
        for offset in 0..DISPLAY_TRIES {
            let number = DISPLAY_BASE + offset;
            if Path::new(&format!("/tmp/.X11-unix/X{number}")).exists() {
                continue;
            }
            let child = Command::new("Xvfb")
                .arg(format!(":{number}"))
                .args(["-screen", "0", "1600x1000x24", "-nolisten", "tcp"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Xvfb could not be started: {e}"))?;
            // **The server is up when its socket is**, and a child that exited means the number was taken
            // after all: the next one is tried rather than the trial failing on a race.
            let socket = format!("/tmp/.X11-unix/X{number}");
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if Path::new(&socket).exists() {
                    return Ok(Display { child, number });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Err(format!(
            "no free display in :{DISPLAY_BASE}..{}, so this trial has no screen to draw on",
            DISPLAY_BASE + DISPLAY_TRIES
        ))
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// **A running editor.**
pub struct Editor {
    /// The child, killed on drop so a trial that panics leaves no editor holding a port.
    pub child: Child,
    /// The BRP port its protocol answered on.
    pub port: u16,
    /// The display it drew on, held so it outlives the editor.
    pub display: Option<Display>,
    /// Its log, for a harness error to quote.
    pub log: PathBuf,
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// **A port nothing is listening on**, so two trials never bind one number.
pub fn free_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("no free port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("no free port: {e}"))?
        .port();
    drop(listener);
    Ok(port)
}

/// **Start the editor under test on `project`, with its own port and display**, and wait for
/// `grimoire/state` to answer.
pub fn start_editor(editor: &Path, project: &Path, map: &str, dir: &Path) -> Result<Editor, String> {
    let port = free_port()?;
    let display = match std::env::consts::OS {
        "linux" => Some(Display::start()?),
        _ => None,
    };
    let log = dir.join("editor.log");
    let stdout = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let stderr = stdout.try_clone().map_err(|e| format!("{}: {e}", log.display()))?;
    let mut command = Command::new(editor);
    command
        .arg(project)
        .arg(map)
        .env("BEVY_BRP_PORT", port.to_string())
        // **The project's own data, not the developer's**: a trial must not read or write the real
        // settings, recent list or trust store.
        .env("HOME", dir.join("home"))
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    if let Some(display) = &display {
        command.env("DISPLAY", format!(":{}", display.number));
    }
    let child = command
        .spawn()
        .map_err(|e| format!("the editor at {} did not start: {e}", editor.display()))?;
    let mut running = Editor { child, port, display, log: log.clone() };
    let deadline = Instant::now() + Duration::from_secs(EDITOR_START_S);
    while Instant::now() < deadline {
        if brp_call(port, "grimoire/state", json!({})).is_ok() {
            return Ok(running);
        }
        // **An editor that exited is a harness error now**, not in `EDITOR_START_S`: the reason is in its
        // log and waiting for the timeout would only add a minute to reading it.
        if let Ok(Some(status)) = running.child.try_wait() {
            return Err(format!(
                "the editor exited with {status} before its state answered. {}",
                tail(&log)
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(format!(
        "the editor did not answer grimoire/state on port {port} within {EDITOR_START_S} s. {}",
        tail(&log)
    ))
}

/// **The last few lines of a file**, for a harness error's reason.
pub fn tail(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return format!("({} could not be read)", path.display());
    };
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(6);
    format!("Its log ends: {}", lines[from..].join(" | "))
}

/// **One BRP call**, and its `result`.
pub fn brp_call(port: u16, method: &str, params: Value) -> Result<Value, String> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .build(),
    );
    let answer = agent
        .post(&format!("http://127.0.0.1:{port}"))
        .send_json(&body)
        .map_err(|e| format!("{method}: {e}"))?
        .into_body()
        .read_to_string()
        .map_err(|e| format!("{method}: {e}"))?;
    let value: Value = serde_json::from_str(&answer).map_err(|e| format!("{method}: {e}: {answer}"))?;
    if let Some(err) = value.get("error") {
        return Err(format!("{method}: {err}"));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

// -------------------------------------------------------------------------------------------------
// The MCP client a scripted agent drives.
// -------------------------------------------------------------------------------------------------

/// **A real MCP server, as a subprocess**, spoken to over stdio exactly as a harness speaks to it.
///
/// This is the same shape `bevy_cauldron_mcp/tests/an_agent_calls_the_server.rs` uses, and it is here for
/// the scripted agents and for reading a transcript: the oracle calls operations through the server rather
/// than through BRP, so what it proves is the pipe an agent actually uses.
pub struct McpClient {
    child: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    /// **Start the server from an MCP config**, which is the whole point: the config comes from the editor.
    pub fn from_config(config: &Value) -> Result<McpClient, String> {
        let server = config
            .get("mcpServers")
            .and_then(|m| m.as_object())
            .and_then(|m| m.values().next())
            .ok_or("the editor's MCP config names no server")?;
        let command = server
            .get("command")
            .and_then(Value::as_str)
            .ok_or("the editor's MCP config names no command")?;
        let args: Vec<String> = server
            .get("args")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        let mut child = Command::new(command)
            .args(&args)
            .envs(
                server
                    .get("env")
                    .and_then(Value::as_object)
                    .map(|e| {
                        e.iter()
                            .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("the MCP server at {command} did not start: {e}"))?;
        let stdin = child.stdin.take().ok_or("the MCP server has no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("the MCP server has no stdout")?);
        let mut client = McpClient { child, stdin, stdout, next_id: 0 };
        client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "cauldron-bench", "version": "0.1.0"},
            }),
        )?;
        client.notify("notifications/initialized", json!({}))?;
        Ok(client)
    }

    /// **One request, and its result.**
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .map_err(|e| format!("{method}: the server's answer could not be read: {e}"))?;
            if read == 0 {
                return Err(format!("{method}: the server closed its stdout"));
            }
            let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(err) = value.get("error") {
                return Err(format!("{method}: {err}"));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// **A notification**, which has no answer.
    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    fn write(&mut self, value: &Value) -> Result<(), String> {
        let text = serde_json::to_string(value).map_err(|e| format!("{e}"))?;
        writeln!(self.stdin, "{text}").map_err(|e| format!("the server's stdin: {e}"))?;
        self.stdin.flush().map_err(|e| format!("the server's stdin: {e}"))
    }

    /// **The tools the server offers**, which is what a `hide_tools` arm is measured by.
    pub fn tool_names(&mut self) -> Result<Vec<String>, String> {
        let answer = self.request("tools/list", json!({}))?;
        Ok(answer
            .get("tools")
            .and_then(Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// **Call one tool**, and answer with its result.
    pub fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// -------------------------------------------------------------------------------------------------
// The trial.
// -------------------------------------------------------------------------------------------------

/// **Run every task of a suite `trials` times** and write each trial's `trial.json`.
///
/// Trials run `jobs` at a time; each has its own port, its own display, and its own directory under
/// [`Options::out`].
pub fn run(options: &Options) -> Result<Vec<TrialRecord>, String> {
    let known = operations(&options.editor)?;
    let known_refs: Vec<&str> = known.iter().map(String::as_str).collect();
    let suite = Suite::load(&options.root, &options.tasks, &known_refs)
        .map_err(|p| format!("the suite was refused: {}", p.line()))?;
    let selected: Vec<Task> = suite.select(&options.only)?.into_iter().cloned().collect();
    let variants = Suite::variants(&options.root).map_err(|p| p.line())?;
    let variant = match &options.variant {
        Some(name) => Some(
            variants
                .iter()
                .find(|v| &v.name == name)
                .ok_or_else(|| {
                    format!(
                        "`{name}` is not an arm of this suite. The arms are: {}",
                        if variants.is_empty() {
                            "none — the suite states no variants.yaml".to_owned()
                        } else {
                            variants.iter().map(|v| v.name.as_str()).collect::<Vec<_>>().join(", ")
                        }
                    )
                })?
                .clone(),
        ),
        None => None,
    };
    if !options.editor.is_file() {
        return Err(format!("the editor under test is not at {}", options.editor.display()));
    }
    if !options.server.is_file() {
        return Err(format!("the MCP server is not at {}", options.server.display()));
    }
    // **The runner refuses a model this host does not have** (BM-02): a run whose harness could not start
    // would report every trial as a harness error and say nothing about the editor.
    if options.agent == Agent::Omp {
        check_model(&options.model)?;
    }
    std::fs::create_dir_all(&options.out)
        .map_err(|e| format!("{}: {e}", options.out.display()))?;

    // **One work list, one worker per `jobs`**, so a `--jobs` of 1 is the same code as a `--jobs` of 4.
    let mut work: Vec<(Task, u32)> = Vec::new();
    for task in &selected {
        for trial in 1..=options.trials {
            work.push((task.clone(), trial));
        }
    }
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let records = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    let workers = options.jobs.max(1) as usize;
    for _ in 0..workers {
        let next = next.clone();
        let records = records.clone();
        let work = work.clone();
        let options = options.clone();
        let variant = variant.clone();
        handles.push(std::thread::spawn(move || {
            loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some((task, trial)) = work.get(i) else {
                    return;
                };
                let record = one_trial(&options, task, *trial, variant.as_ref());
                if let Ok(mut records) = records.lock() {
                    records.push(record);
                }
            }
        }));
    }
    for handle in handles {
        if handle.join().is_err() {
            return Err("a trial's worker panicked".to_owned());
        }
    }
    let mut records = std::sync::Arc::try_unwrap(records)
        .map_err(|_| "a trial's records were still held".to_owned())?
        .into_inner()
        .map_err(|_| "a trial's records were poisoned".to_owned())?;
    records.sort_by(|a, b| a.task.cmp(&b.task).then(a.trial.cmp(&b.trial)));
    Ok(records)
}

/// **The operations the editor under test has**, asked of the editor itself.
///
/// The list is what a task's steps are checked against, and it cannot be baked in: a task naming an
/// operation a *newer* builder wrote must be refused when this build loads the suite.
fn operations(editor: &Path) -> Result<Vec<String>, String> {
    let out = Command::new(editor)
        .arg("--print-operations")
        .output()
        .map_err(|e| format!("{} could not be run: {e}", editor.display()))?;
    if !out.status.success() {
        return Err(format!(
            "the editor under test refused --print-operations ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("nothing")
        ));
    }
    let names: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    if names.is_empty() {
        return Err("the editor under test listed no operations".to_owned());
    }
    Ok(names)
}

/// **Refuse a model this host does not list**, before any trial runs.
fn check_model(model: &str) -> Result<(), String> {
    let out = Command::new("omp")
        .args(["models", "find", model])
        .output()
        .map_err(|e| format!("`omp models find` could not be run: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // **`omp models find` names the model it matched**, and exits 0 with a "requires a search substring"
    // message for no argument: so the test is that this model's own name came back.
    let last = model.rsplit('/').next().unwrap_or(model);
    if text.contains(last) {
        return Ok(());
    }
    Err(format!(
        "`{model}` is not a model this host lists, so an omp trial could not start. \
         `omp models find {last}` said: {}",
        text.lines().next().unwrap_or("nothing")
    ))
}

/// **One trial, from the copy to its `trial.json`**, and never a `pass` for a harness that failed.
fn one_trial(options: &Options, task: &Task, trial: u32, variant: Option<&Variant>) -> TrialRecord {
    let dir = options.out.join(&task.id).join(format!("trial-{trial}"));
    let _ = std::fs::remove_dir_all(&dir);
    let record = |outcome: Outcome, reason: String| TrialRecord {
        task: task.id.clone(),
        trial,
        agent: options.agent.name().to_owned(),
        model: (options.agent == Agent::Omp).then(|| options.model.clone()),
        variant: variant.map(|v| v.name.clone()),
        outcome,
        reason,
        checks: Vec::new(),
        tool_calls: Vec::new(),
        tokens: None,
        wall_s: 0.0,
        tools_offered: Vec::new(),
        wishes: Vec::new(),
        dir: dir.clone(),
    };
    let harness = |reason: String| {
        let record = record(Outcome::HarnessError, reason.clone());
        write_record(&record, task, &reason);
        record
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return harness(format!("{}: {e}", dir.display()));
    }
    let project = dir.join("project");
    if let Err(e) = fixture::lay_out(&options.root, &project) {
        return harness(format!("the fixture was not laid out: {e}"));
    }
    // **The arm's overlay**, laid over the copy after the fixture: a different `AGENTS.md` is the example.
    if let Some(variant) = variant {
        for rel in &variant.overlay {
            let from = options.root.join(rel);
            let to = project.join(rel);
            if let Some(parent) = to.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                return harness(format!("{}: {e}", parent.display()));
            }
            if let Err(e) = std::fs::copy(&from, &to) {
                return harness(format!("{} -> {}: {e}", from.display(), to.display()));
            }
        }
    }
    let editor = match start_editor(&options.editor, &project, &task.map, &dir) {
        Ok(editor) => editor,
        Err(e) => return harness(e),
    };
    // **The setup, through the editor**, exactly as a person's first actions are: the dispatcher, so each
    // one is in the operation log and undoable, and the task's `checks` were written against that state.
    for step in &task.setup {
        if step.op == crate::PRESENT_STEP {
            continue;
        }
        let answer = brp_call(
            editor.port,
            "grimoire/operation_apply",
            json!({"operation": step.op, "parameters": step.params, "dry_run": false}),
        );
        if let Err(e) = answer {
            return harness(format!("the task's setup was refused: {e}"));
        }
    }
    // **What the setup left**, which the byte comparisons are against.
    let before = Snapshot::read(&project, &task.map, Vec::new(), Default::default());
    // **The agent's config, from the editor itself**, with this arm's `--hide-tools` and a fresh token.
    let server_args: Vec<String> = match variant {
        Some(v) if !v.hide_tools.is_empty() => {
            vec!["--hide-tools".to_owned(), v.hide_tools.join(",")]
        }
        _ => Vec::new(),
    };
    let setup = match brp_call(
        editor.port,
        "grimoire/agent_setup",
        json!({"server_args": server_args}),
    ) {
        Ok(setup) => setup,
        Err(e) => return harness(format!("the editor could not hand an agent its tools: {e}")),
    };
    let config = setup.get("config").cloned().unwrap_or(Value::Null);
    let started = Instant::now();
    let run = match options.agent {
        Agent::Nothing => run_nothing(&config),
        Agent::Oracle => run_oracle(task, &config, &setup),
        Agent::Omp => run_omp(options, task, &config, &project, editor.port, &dir),
    };
    let wall_s = started.elapsed().as_secs_f64();
    let run = match run {
        Ok(run) => run,
        Err(e) => {
            drop(editor);
            return harness(e);
        }
    };
    // **The editor is closed before grading**, so nothing holds a file open and the map on disk is what the
    // agent left rather than what a running editor still has in memory.
    drop(editor);
    let left = Snapshot::read(&project, &task.map, run.handbacks.clone(), run.decisions.clone());
    let graded = Grading::of(task, &before, &left);
    let mut record = record(graded.outcome, String::new());
    record.checks = graded.verdicts.clone();
    record.tool_calls = run.tool_calls.clone();
    record.tokens = run.tokens;
    record.wall_s = wall_s;
    record.tools_offered = run.tools_offered.clone();
    record.wishes = run.wishes.clone();
    let reason = graded.sentence();
    write_record(&record, task, &reason);
    record
}

/// **What an agent did**, which is what the grader reads besides the files.
#[derive(Clone, Debug, Default)]
pub struct AgentRun {
    /// The tools it called, by name, in order.
    pub tool_calls: Vec<String>,
    /// The hand-backs it made, oldest first.
    pub handbacks: Vec<Handback>,
    /// The wishes it made.
    pub wishes: Vec<Value>,
    /// What its harness reported for tokens.
    pub tokens: Option<u64>,
    /// The tools its `tools/list` held.
    pub tools_offered: Vec<String>,
    /// The decisions the harness made, by the name a `decide` check asks for.
    pub decisions: std::collections::BTreeMap<String, Decision>,
}

/// **An agent that connects and stops**, which must pass no task.
fn run_nothing(config: &Value) -> Result<AgentRun, String> {
    let mut client = McpClient::from_config(config)?;
    Ok(AgentRun {
        tools_offered: client.tool_names()?,
        ..AgentRun::default()
    })
}

/// **The task's own `solution`, called through the real MCP server**, which must pass every task.
fn run_oracle(task: &Task, config: &Value, setup: &Value) -> Result<AgentRun, String> {
    let mut client = McpClient::from_config(config)?;
    let mut run = AgentRun {
        tools_offered: client.tool_names()?,
        ..AgentRun::default()
    };
    let token = setup.get("token").and_then(Value::as_str).unwrap_or_default().to_owned();
    for step in &task.solution {
        if step.op == crate::PRESENT_STEP {
            run.tool_calls.push("grimoire_present".to_owned());
            let answer = client.call("grimoire_present", step.params.clone())?;
            run.handbacks.push(handback_of(&step.params));
            let _ = answer;
            continue;
        }
        run.tool_calls.push("grimoire_apply_operation".to_owned());
        client.call(
            "grimoire_apply_operation",
            json!({
                "operation": step.op,
                "parameters": step.params,
                "dry_run": false,
                "token": token,
            }),
        )?;
    }
    Ok(run)
}

/// **A real model harness**, launched on the config the editor handed out, in the trial's directory.
///
/// **The agent knows only what the Dark Factory gives it**: an isolated `--profile`, no context files, no
/// skills, no rules, and the trial's directory as its working directory — which holds the project copy and
/// no benchmark file. A transcript showing a tool from outside that set is a harness error.
fn run_omp(
    options: &Options,
    task: &Task,
    config: &Value,
    project: &Path,
    port: u16,
    dir: &Path,
) -> Result<AgentRun, String> {
    let extension = dir.join("mcp");
    std::fs::create_dir_all(&extension).map_err(|e| format!("{}: {e}", extension.display()))?;
    // **The config the editor gave, written where omp reads it**: `-e <dir>` discovers `.mcp.json` beside
    // the extension entry, which is exactly how the terminal's agent tab hands omp its server.
    let text = serde_json::to_string_pretty(config).map_err(|e| format!("{e}"))? + "\n";
    bevy_grimoire_core::atomic_file::write(&extension.join(".mcp.json"), text)
        .map_err(|e| format!("{}: {e}", extension.display()))?;
    bevy_grimoire_core::atomic_file::write(
        &extension.join("index.ts"),
        "// Written by the benchmark. An omp extension folder whose .mcp.json beside it names the editor's\n\
         // MCP server exactly as the terminal's agent tab does.\nexport default {};\n",
    )
    .map_err(|e| format!("{}: {e}", extension.display()))?;
    let transcript = dir.join("transcript.jsonl");
    let stdout = std::fs::File::create(&transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
    let stderr = std::fs::File::create(dir.join("agent.err")).map_err(|e| format!("{e}"))?;
    let mut child = Command::new("omp")
        .args([
            "-p",
            "--mode",
            "json",
            "--model",
            &options.model,
            // **An isolated profile**: no user `AGENTS.md`, no skills, no memories, no other MCP servers.
            "--profile",
            "cauldron-bench",
            "--no-context-files",
            "--no-skills",
            "--no-rules",
            "--no-extensions",
            "-e",
            &extension.to_string_lossy(),
            "--cwd",
            &project.to_string_lossy(),
            &task.prompt,
        ])
        .env("BEVY_BRP_PORT", port.to_string())
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("omp did not start: {e}"))?;
    // **Stopped at the budget**, with a grace for the process to finish its turn.
    let deadline = Instant::now() + Duration::from_secs(u64::from(task.budget.wall_s));
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(e) => return Err(format!("omp could not be waited on: {e}")),
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    read_transcript(&transcript, config)
}

/// **What a transcript says the agent did**, read from `omp --mode json`'s own events.
///
/// **A tool from outside the set the harness was told about is a harness error**, because it means the
/// profile was not isolated: a result measured with a tool the benchmark did not hand out is not a result
/// about this benchmark.
fn read_transcript(path: &Path, config: &Value) -> Result<AgentRun, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("the transcript at {} could not be read: {e}", path.display()))?;
    let served: Vec<String> = config
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(|servers| servers.keys().cloned().collect())
        .unwrap_or_default();
    let mut run = AgentRun::default();
    let mut tools: Vec<String> = Vec::new();
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            // **A tool call**, whatever the harness calls the event: the name is the thing.
            "tool_use" | "tool_call" | "tool" => {
                if let Some(name) = event
                    .get("name")
                    .or_else(|| event.get("tool"))
                    .and_then(Value::as_str)
                {
                    let is_mcp = served
                        .iter()
                        .any(|server| name.starts_with(&format!("{server}_")) || name.starts_with(&format!("{server}.")));
                    let known_builtin = matches!(
                        name,
                        "bash" | "read" | "write" | "edit" | "glob" | "grep" | "find" | "todo" | "task"
                            | "eval" | "learn" | "manage_skill" | "ast_edit" | "debug" | "lsp" | "ask"
                            | "wait" | "web_search"
                    );
                    if !is_mcp && !known_builtin {
                        return Err(format!(
                            "the transcript shows a tool `{name}` that is neither the editor's nor the \
                             harness's own set, so the profile was not isolated"
                        ));
                    }
                    // **The editor's tools only**, since the built-ins are the harness's own account of
                    // itself and the benchmark grades the editor's work.
                    if is_mcp {
                        run.tool_calls.push(name.to_owned());
                        tools.push(name.to_owned());
                    }
                }
            }
            // **The tokens**, when the harness reports them once at the end.
            "usage" | "result" | "summary" => {
                if let Some(total) = event
                    .get("tokens")
                    .or_else(|| event.get("total_tokens"))
                    .and_then(Value::as_u64)
                {
                    run.tokens = Some(total);
                }
            }
            _ => {}
        }
        // **The hand-back, from the arguments themselves**: `grimoire_present`'s parameters are the words a
        // check reads, and they are in the transcript whether or not the editor recorded them.
        if let Some(call) = event.get("arguments").or_else(|| event.get("input"))
            && event
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| n.ends_with("grimoire_present"))
            && let Ok(params) = serde_json::from_value::<Value>(call.clone())
        {
            run.handbacks.push(handback_of(&params));
        }
        if let Some(wish) = event.get("wish") {
            run.wishes.push(wish.clone());
        }
    }
    run.tools_offered = tools;
    Ok(run)
}

/// **The hand-back a `grimoire_present` call makes**, from its parameters.
///
/// `deny_unknown_fields` is not applied here: a hand-back's extra fields are the harness's or the editor's,
/// and a check reads the words rather than the shape.
fn handback_of(params: &Value) -> Handback {
    let strings = |key: &str| -> Vec<String> {
        params
            .get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned).or_else(|| v.get("what").and_then(Value::as_str).map(str::to_owned)))
                    .collect()
            })
            .unwrap_or_default()
    };
    Handback {
        id: params.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
        title: params.get("title").and_then(Value::as_str).unwrap_or_default().to_owned(),
        summary: params.get("summary").and_then(Value::as_str).unwrap_or_default().to_owned(),
        specified: strings("specified"),
        inferred: strings("inferred"),
        check: strings("check"),
    }
}

/// **Write a trial's `trial.json`**, beside its project.
fn write_record(record: &TrialRecord, task: &Task, sentence: &str) {
    let path = record.dir.join("trial.json");
    let mut value = record.to_json();
    if let Some(object) = value.as_object_mut() {
        object.insert("prompt".to_owned(), json!(task.prompt));
        object.insert("sentence".to_owned(), json!(sentence));
    }
    let text = match serde_json::to_string_pretty(&value) {
        Ok(text) => text + "\n",
        Err(_) => return,
    };
    let _ = bevy_grimoire_core::atomic_file::write(&path, text);
}
