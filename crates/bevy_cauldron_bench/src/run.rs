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
    /// **Claude Code** (`claude -p --output-format stream-json`), the second real harness (BM-06, #416).
    ClaudeCode,
    /// **The task's own `solution`, called through the real MCP server**: no model, and it must pass every
    /// task. It proves the editor, the server, the config and the grader are one pipe.
    Oracle,
    /// **It connects and stops**, which must pass no task: it is the do-nothing loophole measured against
    /// the running editor rather than against a task file.
    Nothing,
}

impl Agent {
    /// Every agent, in the order `--agent`'s refusal lists them.
    pub const ALL: [Agent; 4] = [Agent::Omp, Agent::ClaudeCode, Agent::Oracle, Agent::Nothing];

    /// As `--agent` spells it.
    pub fn name(self) -> &'static str {
        match self {
            Agent::Omp => "omp",
            Agent::ClaudeCode => "claude-code",
            Agent::Oracle => "oracle",
            Agent::Nothing => "nothing",
        }
    }

    /// **Whether the harness runs a model**, which a report names and prices.
    pub fn has_model(self) -> bool {
        matches!(self, Agent::Omp | Agent::ClaudeCode)
    }

    /// **The model a harness runs when `--model` is not given**; empty for a scripted agent.
    pub fn default_model(self) -> &'static str {
        match self {
            Agent::Omp => DEFAULT_MODEL,
            Agent::ClaudeCode => crate::claude::DEFAULT_MODEL,
            Agent::Oracle | Agent::Nothing => "",
        }
    }

    /// The agent a command line names.
    pub fn parse(name: &str) -> Result<Agent, String> {
        for agent in Agent::ALL {
            if agent.name() == name {
                return Ok(agent);
            }
        }
        Err(format!(
            "`{name}` is not an agent. The agents are: {}.",
            Agent::ALL.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")
        ))
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

/// **How long a new `Xvfb` is given to take its display**, seconds: its socket exists and its lock file names
/// it. A live server on the number refuses by exiting inside this, so it bounds only a server that hangs.
pub const DISPLAY_START_S: u64 = 5;

/// **How long a display is given to stop after `SIGTERM`**, seconds, before it is killed.
pub const DISPLAY_STOP_S: u64 = 5;

/// **How often a wait looks at the process it waits for**, milliseconds.
pub const POLL_INTERVAL_MS: u64 = 50;

/// **How often a running agent's editor is looked at**, milliseconds: the editor's death is a harness error
/// found while the agent runs, not minutes later when the budget ends.
pub const AGENT_POLL_INTERVAL_MS: u64 = 250;

/// **The harness's own tools**, which an agent is offered beside the editor's and which are not a leak.
///
/// Measured on this host (omp 18.18.9, 2026-11-13) from the registry of an isolated trial: twenty names.
/// The rest are names the model has been seen to call. **Any other name is a tool the Dark Factory did not
/// hand out**, so a trial that offers one is a harness error naming it, and a new omp that adds a built-in
/// is a refusal that says which, not a silently wider agent.
pub const BUILTIN_TOOLS: &[&str] = &[
    "ast_edit", "bash", "debug", "edit", "eval", "find", "glob", "goal", "grep", "init_experiment",
    "log_experiment", "lsp", "read", "run_experiment", "task", "todo", "update_notes", "wait", "web_search",
    "write", "learn", "manage_skill", "ask", "browser", "archive",
];

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
    /// **The repository this binary was built in**, whose commit and shipped guide a run's report names so a
    /// reader can reproduce it. It is not `root`: a game's own suite runs against a game, and the editor and
    /// the guide under test are this repository's.
    pub repo: PathBuf,
    /// **What the run is called in its report's file name**: `docs/bench/runs/<date>-<label>.md`.
    pub label: String,
    /// **Where to publish the report**, when the person named a `--label`: the project's `docs/bench`.
    /// A run that was not named is not committed, so a dozen oracle runs leave the working tree alone.
    pub publish: Option<PathBuf>,
    /// Only these tasks, by id.
    pub only: Vec<String>,
}

/// **What a harness reported a model's turns used**, summed over the trial.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    /// Fresh input tokens.
    pub input: u64,
    /// Output tokens, reasoning included as the harness counts it.
    pub output: u64,
    /// Input tokens served from the provider's cache, which a model prices apart from fresh input.
    pub cache_read: u64,
    /// Input tokens written to the provider's cache, which a model prices apart again (often dearest).
    pub cache_write: u64,
    /// The harness's own total.
    pub total: u64,
}

impl Tokens {
    /// **Add one turn's usage.**
    pub fn add(&mut self, other: Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.total += other.total;
    }
}

/// **How many characters of a call's arguments a trial's log keeps**: enough to read what the agent asked
/// for, short enough that the log of a long trial stays a page.
pub const CALL_ARGS_SHOWN_CHARS: usize = 160;

/// **One tool call an agent made**, as a trial's log keeps it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallLog {
    /// The tool's own name (`grimoire_apply_operation`), whichever harness spelled it otherwise.
    pub tool: String,
    /// Its arguments as compact JSON, cut at [`CALL_ARGS_SHOWN_CHARS`].
    pub args: String,
    /// **Whether the editor refused it**: the tool answered an error, or its payload says `refused`.
    pub refused: bool,
}

impl CallLog {
    /// **A call, with its arguments shortened.**
    pub fn new(tool: &str, args: &Value, refused: bool) -> CallLog {
        let text = args.to_string();
        let args = if text.chars().count() > CALL_ARGS_SHOWN_CHARS {
            let cut: String = text.chars().take(CALL_ARGS_SHOWN_CHARS).collect();
            format!("{cut}…")
        } else {
            text
        };
        CallLog { tool: tool.to_owned(), args, refused }
    }
}

/// **Whether a tool's answer is the editor refusing.**
///
/// An MCP tool reports a refusal two ways: the envelope's own `isError`, or a payload that says so in
/// its own object (`{"refused": …}`, or an operation's `"outcome": "refused"`). A reader that took only the
/// first would count a refused change as a success, which is how the oracle's placement was once dropped.
pub fn is_refusal(is_error: bool, text: &str) -> bool {
    if is_error {
        return true;
    }
    let Ok(value) = serde_json::from_str::<Value>(text.trim()) else {
        return false;
    };
    value.get("refused").is_some() || value.get("outcome").and_then(Value::as_str) == Some("refused")
}

/// **The tool an `undo` is**, as the editor's MCP server names it.
pub const UNDO_TOOL: &str = "grimoire_undo_operation";

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
    /// **Every one of them with its arguments and whether it was refused**, so a report can show the last
    /// five an agent made before it stopped, and count the refusals it received.
    pub tool_log: Vec<CallLog>,
    /// What the agent's harness reported for tokens, when it reported any.
    pub tokens: Option<Tokens>,
    /// **The cost the harness itself reported**, dollars, when it reports one (Claude Code's `total_cost_usd`).
    /// A report prices a model from its listed price and uses this only when none is listed.
    pub cost_reported_usd: Option<f64>,
    /// **Files the agent changed that no operation-log line wrote**: it went around the tools, through its
    /// shell. See [`crate::grade::edits_behind_the_back`].
    pub edits_behind: Vec<String>,
    /// How long the agent ran, seconds.
    pub wall_s: f64,
    /// **The tools the agent was offered**, which is what a `hide_tools` arm is judged by: the server's own
    /// `tools/list` for the scripted agents, and the harness's registry for `omp`, whose transcript carries no
    /// tool list at all ([`tools_offered_path`]).
    pub tools_offered: Vec<String>,
    /// The wishes the agent made, as `{tool, task}`.
    pub wishes: Vec<Value>,
    /// The directory this trial's project and transcript are under.
    pub dir: PathBuf,
}

impl TrialRecord {
    /// **How many tool calls the editor refused.**
    pub fn refusals(&self) -> usize {
        self.tool_log.iter().filter(|c| c.refused).count()
    }

    /// **How many times the agent undid a change**: an `undo` it called and the editor did not refuse.
    pub fn undos(&self) -> usize {
        self.tool_log.iter().filter(|c| c.tool == UNDO_TOOL && !c.refused).count()
    }

    /// **The JSON a `trial.json` holds.**
    pub fn to_json(&self) -> Value {
        let mut refused_by_tool = std::collections::BTreeMap::<&str, u32>::new();
        for call in self.tool_log.iter().filter(|c| c.refused) {
            *refused_by_tool.entry(call.tool.as_str()).or_default() += 1;
        }
        let tokens = self.tokens.unwrap_or_default();
        let reported = self.tokens.is_some();
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
            "tool_log": self.tool_log,
            "refusals": self.refusals(),
            "refusals_by_tool": refused_by_tool,
            "undos": self.undos(),
            "edits_behind": self.edits_behind,
            "tokens": reported.then_some(tokens.total),
            "tokens_in": reported.then_some(tokens.input),
            "tokens_out": reported.then_some(tokens.output),
            "tokens_cache_read": reported.then_some(tokens.cache_read),
            "tokens_cache_write": reported.then_some(tokens.cache_write),
            "wall_s": self.wall_s,
            "cost_reported_usd": self.cost_reported_usd,
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
    /// **A display that was already running is never adopted.** The number is the trial's only when the
    /// lock file names the child this call started; a number a live server holds (another run's, another
    /// agent's) is skipped, and no number is ever shared. The child is detached ([`detach`]) so it holds no
    /// descriptor of the caller's, and `Drop` stops it on every exit that unwinds, an early `?` included.
    ///
    /// **Linux only.** A macOS run uses the desktop's own display, which is what a person's editor is on.
    pub fn start() -> Result<Display, String> {
        // **A socket file is not evidence of a server.** `Xvfb` killed with the signal this harness sends
        // leaves `/tmp/.X11-unix/X<n>` behind, and a ladder that skipped every existing socket therefore
        // ran out of displays on a host where nobody was drawing at all: measured on big, 2026-11-13, 42
        // stale sockets from killed servers and three live ones left the whole `:120..160` range refused.
        // So every number is **tried**, and a server that is really there answers by refusing: its child
        // exits inside the wait below and the next number is taken.
        for offset in 0..DISPLAY_TRIES {
            let number = DISPLAY_BASE + offset;
            let mut xvfb = Command::new("Xvfb");
            xvfb.arg(format!(":{number}"))
                .args(["-screen", "0", "1600x1000x24", "-nolisten", "tcp"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let child = detach(&mut xvfb)
                .spawn()
                .map_err(|e| format!("Xvfb could not be started: {e}"))?;
            // **The display is ours when its lock file names our child**, and not before. A socket file proves
            // only that *some* server made one: a live Xvfb of another run, or of another agent, has its
            // socket at once, and a harness that returned on seeing it shared that screen and lost it when the
            // owner stopped. `Xvfb` writes its own pid into `/tmp/.X<n>-lock`, so the pid is the proof; a
            // server that holds the number refuses ours by exiting, and the next number is tried.
            let socket = format!("/tmp/.X11-unix/X{number}");
            let lock = format!("/tmp/.X{number}-lock");
            let deadline = Instant::now() + Duration::from_secs(DISPLAY_START_S);
            let mut running = child;
            while Instant::now() < deadline {
                if Path::new(&socket).exists() && lock_pid(&lock) == Some(running.id()) {
                    return Ok(Display { child: running, number });
                }
                if let Ok(Some(_)) = running.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            let _ = running.kill();
            let _ = running.wait();
        }
        Err(format!(
            "no free display in :{DISPLAY_BASE}..{}, so this trial has no screen to draw on",
            DISPLAY_BASE + DISPLAY_TRIES
        ))
    }
}

/// **`CLOSE_RANGE_CLOEXEC`** (`close_range(2)`, Linux 5.11): mark the range close-on-exec instead of closing it.
/// Closing in the child would also close the pipe `std` reports a failed `exec` through, and a binary that
/// is not there would then read as a child that started.
#[cfg(target_os = "linux")]
const CLOSE_RANGE_CLOEXEC: libc::c_uint = 1 << 2;

/// **The first descriptor a child must not inherit**: 0, 1 and 2 are its stdio and everything above is the
/// caller's.
#[cfg(target_os = "linux")]
const FIRST_INHERITED_FD: libc::c_uint = 3;

/// **Start a child that holds nothing of its parent's, and does not outlive it.**
///
/// - **No descriptor above stderr is inherited.** Measured on big, 2026-11-13: a test ran under
///   `flock -s /tmp/fleet/big-heavy.lock cargo nextest ...`, left an `Xvfb` running, and that `Xvfb`
///   **inherited the lock's descriptor and held the host lock for 13 minutes** with no trial alive, blocking
///   the push gate and every other agent's build. A child of this harness is a long-lived process that can
///   outlive its parent (a display, an editor, a harness), and a descriptor it inherits is held until it
///   exits.
/// - **Its own process group**, so a signal to the harness's group (a terminal's `Ctrl-C`) is not delivered
///   to a child the harness is stopping itself.
/// - **`SIGTERM` when the parent dies** (`PR_SET_PDEATHSIG`), so a harness that is killed outright leaves no
///   `Xvfb` or editor running: [`Display`]'s `Drop` covers every exit that unwinds, and this covers the one
///   that does not. The parent is the thread that spawned the child, which a trial keeps until it is done.
///
/// **Linux only for the last two.** Elsewhere the child is only given its own process group, and there is no
/// host lock to hold.
pub fn detach(command: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    #[cfg(target_os = "linux")]
    {
        // SAFETY: the closure runs between `fork` and `exec` and calls only `close_range` and `prctl`,
        // both async-signal-safe system calls; it allocates nothing and takes no lock.
        unsafe {
            command.pre_exec(|| {
                let marked = libc::syscall(
                    libc::SYS_close_range,
                    FIRST_INHERITED_FD,
                    libc::c_uint::MAX,
                    CLOSE_RANGE_CLOEXEC,
                );
                if marked != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command
}

/// **The pid an X server's lock file names**, or `None` when there is no file or it holds no number.
fn lock_pid(path: &str) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

impl Drop for Display {
    fn drop(&mut self) {
        // **Asked, then made to**: `SIGTERM` is what lets `Xvfb` remove its own socket and lock file, and
        // `SIGKILL` (what `Child::kill` sends) is what left 42 dead sockets behind on big. The kill below
        // is for a server that ignored the request, and `-nolisten tcp` has nothing to flush.
        let pid = self.child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", &pid]).status();
        let deadline = Instant::now() + Duration::from_secs(DISPLAY_STOP_S);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        }
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

/// **Why the editor is gone, when it is**: its exit, and the end of its own log.
///
/// The editor is a child this harness started, and it can die at any time: the start-up wait sees only its
/// first seconds. A trial asks again while the agent runs and once more before it grades, because an agent
/// that ran on a dead editor measured nothing about the editor, and a grade of the files it left would
/// score the harness's loss as the agent's work.
pub fn editor_exit(editor: &mut Editor) -> Option<String> {
    match editor.child.try_wait() {
        Ok(Some(status)) => Some(format!(
            "the editor exited with {status} during the trial. {}",
            tail(&editor.log)
        )),
        Ok(None) => None,
        Err(e) => Some(format!("the editor could not be waited on: {e}")),
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
    let child = detach(&mut command)
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
        let mut server_command = Command::new(command);
        server_command
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
            .stderr(Stdio::null());
        let mut child = detach(&mut server_command)
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
    ///
    /// **A tool that refused is an error here**, and it is `isError` that says so rather than the
    /// JSON-RPC envelope: an MCP tool reports a refusal as a *result* whose text carries the reason, so a
    /// caller that only checked the envelope would read "the editor refused this change" as a success  -
    /// which is exactly how the oracle's placement was silently dropped the first time this ran.
    pub fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let answer = self.request("tools/call", json!({"name": name, "arguments": arguments}))?;
        if answer.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(format!("{name} refused: {}", tool_text(&answer)));
        }
        let structured = answer.get("structuredContent").cloned().unwrap_or(Value::Null);
        // **A tool that answers "refused" in its own object** says so in the payload, not in the envelope:
        // `grimoire_apply_operation` reports a refused operation that way, so the oracle stops on one
        // rather than carrying on to a hand-back about a change that never happened.
        if structured.get("refused").is_some()
            || structured.get("outcome").and_then(Value::as_str) == Some("refused")
        {
            return Err(format!("{name} refused: {}", tool_text(&answer)));
        }
        Ok(answer)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// **What a tool's answer says, as text**: its `content` blocks joined, for a refusal's sentence.
pub fn tool_text(answer: &Value) -> String {
    answer
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| answer.to_string())
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
    match options.agent {
        Agent::Omp => check_model(&options.model)?,
        Agent::ClaudeCode => check_claude()?,
        Agent::Oracle | Agent::Nothing => {}
    }
    std::fs::create_dir_all(&options.out)
        .map_err(|e| format!("{}: {e}", options.out.display()))?;
    // **The header, before any trial**, so a run killed halfway still says how it was made.
    let header = crate::probe::header(options, &options.label);
    let header_text = serde_json::to_string_pretty(&header).map_err(|e| format!("{e}"))? + "\n";
    bevy_grimoire_core::atomic_file::write(&options.out.join(crate::report::RUN_FILE), header_text)
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

/// **Build a finished run's report, write it beside its trials, and publish it** when the run was named.
///
/// Answers the report and the paths written. A run whose `trial.json` files do not read is an error naming
/// the file: a report of fewer trials than the run made would be a worse number with a better look.
pub fn write_reports(options: &Options) -> Result<(crate::report::Report, Vec<PathBuf>), String> {
    let data = crate::report::RunData::read(&options.out)?;
    let report = crate::report::Report::build(&data);
    let md = options.out.join("report.md");
    let json = options.out.join("report.json");
    bevy_grimoire_core::atomic_file::write(&md, report.markdown()).map_err(|e| format!("{}: {e}", md.display()))?;
    bevy_grimoire_core::atomic_file::write(&json, report.json()?).map_err(|e| format!("{}: {e}", json.display()))?;
    let mut written = vec![md, json];
    if let Some(bench) = &options.publish {
        let (md, json) = crate::index::publish(bench, &report)?;
        written.push(md);
        written.push(json);
    }
    Ok((report, written))
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

/// **Refuse a Claude Code this host does not have**, before any trial runs: a `claude` that cannot print its
/// version would make every trial a harness error that says nothing about the editor.
fn check_claude() -> Result<(), String> {
    Command::new("claude")
        .arg("--version")
        .output()
        .map_err(|e| format!("`claude --version` could not be run, so a claude-code trial could not start: {e}"))
        .and_then(|out| {
            if out.status.success() {
                Ok(())
            } else {
                Err(format!("`claude --version` failed ({}), so a claude-code trial could not start", out.status))
            }
        })
}

/// **Wait for an agent to finish, or stop it at its wall budget** (`wall_s` seconds), and stop it for good.
///
/// A dead editor ends the trial now, not at the budget: the agent is talking to nothing, and the editor's own
/// exit is the reason to report. **The whole process group is killed**, not only the child: a harness runs
/// its tools as children of its own, and a `bash` left running after its parent was killed is a process
/// holding the trial's project (the follow-up #412's review named). The agent leads its own group
/// ([`detach`]), so the group is the agent's and nothing else's.
pub fn wait_for_agent(mut child: Child, editor: &mut Editor, wall_s: u32, name: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(u64::from(wall_s));
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if let Some(reason) = editor_exit(editor) {
                    stop_group(&mut child);
                    return Err(reason);
                }
                std::thread::sleep(Duration::from_millis(AGENT_POLL_INTERVAL_MS));
            }
            Err(e) => return Err(format!("{name} could not be waited on: {e}")),
        }
    }
    stop_group(&mut child);
    Ok(())
}

/// **Kill an agent's process group, then the agent itself, and wait for it.**
fn stop_group(child: &mut Child) {
    let group = format!("-{}", child.id());
    let _ = Command::new("kill").args(["-KILL", "--", &group]).status();
    let _ = child.kill();
    let _ = child.wait();
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
        model: options.agent.has_model().then(|| options.model.clone()),
        variant: variant.map(|v| v.name.clone()),
        outcome,
        reason,
        checks: Vec::new(),
        tool_calls: Vec::new(),
        tool_log: Vec::new(),
        tokens: None,
        cost_reported_usd: None,
        edits_behind: Vec::new(),
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
    let mut editor = match start_editor(&options.editor, &project, &task.map, &dir) {
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
        Agent::Oracle => run_oracle(task, &config),
        Agent::Omp => run_omp(options, task, &config, &project, &mut editor, &dir),
        Agent::ClaudeCode => crate::claude::run(
            &options.model,
            &task.prompt,
            task.budget.wall_s,
            &config,
            &project,
            &mut editor,
            &dir,
        ),
    };
    let wall_s = started.elapsed().as_secs_f64();
    // **The editor's death outranks whatever the agent said about it.** An agent whose editor was killed
    // reports a refused call or a dropped connection, and that is the symptom: the reason is the exit.
    if let Some(reason) = editor_exit(&mut editor) {
        drop(editor);
        return harness(reason);
    }
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
    record.tool_log = run.tool_log.clone();
    record.tokens = run.usage;
    record.cost_reported_usd = run.cost_usd;
    record.edits_behind = crate::grade::edits_behind_the_back(&before, &left);
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
    /// **The same calls with their arguments and whether each was refused.**
    pub tool_log: Vec<CallLog>,
    /// The hand-backs it made, oldest first.
    pub handbacks: Vec<Handback>,
    /// The wishes it made.
    pub wishes: Vec<Value>,
    /// What its harness reported for tokens, in total.
    pub tokens: Option<u64>,
    /// **The same, by kind** (fresh input, output, cached read and written), which is what a price is applied to.
    pub usage: Option<Tokens>,
    /// **A cost the harness itself reported**, dollars (Claude Code's `total_cost_usd`).
    pub cost_usd: Option<f64>,
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
fn run_oracle(task: &Task, config: &Value) -> Result<AgentRun, String> {
    let mut client = McpClient::from_config(config)?;
    let mut run = AgentRun {
        tools_offered: client.tool_names()?,
        ..AgentRun::default()
    };
    for step in &task.solution {
        if step.op == crate::PRESENT_STEP {
            run.tool_calls.push("grimoire_present".to_owned());
            run.tool_log.push(CallLog::new("grimoire_present", &step.params, false));
            client.call("grimoire_present", step.params.clone())?;
            run.handbacks.push(handback_of(&step.params));
            continue;
        }
        run.tool_calls.push("grimoire_apply_operation".to_owned());
        // **The parameters are the tool's own**, which is `operation`, `parameters`, `dry_run` and an
        // optional `agent`: the tool derives who did it from the launch it was started by, so a token
        // here is a field it does not read.
        let params = json!({
            "operation": step.op,
            "parameters": step.params,
            "dry_run": false,
        });
        run.tool_log.push(CallLog::new("grimoire_apply_operation", &params, false));
        client.call("grimoire_apply_operation", params)?;
    }
    Ok(run)
}

/// **A real model harness**, launched on the config the editor handed out, in the trial's directory.
///
/// **The agent knows only what the Dark Factory gives it**, and that takes three things:
///
/// - `--no-context-files --no-skills --no-rules --no-extensions -e <folder>`, so no `AGENTS.md`, no skill,
///   no rule and no extension discovery beyond the one folder this writes;
/// - **`HOME` set to the trial's own** (`<trial>/home`), so the harness reads no user
///   `~/.omp/agent/mcp.json`, no memory and no session store. Measured on this host, 2026-11-13: without it
///   omp loaded the developer's own `home-still` server into every trial  -  a tool from outside the set
///   BM-02 allows  -  and the agent spent its turn on `bash` and `write` instead of the editor;
/// - **the credential imported into that `HOME`**, because an isolated `HOME` also has no login: a trial
///   with neither refuses at "No API key found", which is a harness error that says nothing about the
///   editor. `omp auth export` from the user's own `HOME` and `omp auth import` into the trial's is the
///   documented, supported way to carry it, and it carries credentials only  -  never an MCP server.
///
/// **Not `--profile`**, though isolation is what a profile is for: a profile is isolated in exactly the same
/// way and has the same missing credential, so it buys nothing this does not and adds a directory outside
/// the trial. Measured, 2026-11-13.
fn run_omp(
    options: &Options,
    task: &Task,
    config: &Value,
    project: &Path,
    editor: &mut Editor,
    dir: &Path,
) -> Result<AgentRun, String> {
    let extension = dir.join("mcp");
    std::fs::create_dir_all(&extension).map_err(|e| format!("{}: {e}", extension.display()))?;
    // **The config the editor gave, written where omp reads it**: `-e <dir>` discovers `.mcp.json` beside
    // the extension entry, which is exactly how the terminal's agent tab hands omp its server.
    let text = serde_json::to_string_pretty(config).map_err(|e| format!("{e}"))? + "\n";
    bevy_grimoire_core::atomic_file::write(&extension.join(".mcp.json"), text)
        .map_err(|e| format!("{}: {e}", extension.display()))?;
    // **The tool list, from the harness's own registry.** A `hide_tools` arm is only proved if the list the
    // *agent* was offered is read back, and `omp --mode json`'s stream does not carry one: measured on this
    // host, 2026-11-13, a whole 810-event transcript holds no `tools` key at all. So the extension omp
    // already loads to find `.mcp.json` writes what `pi.getAllTools()` answers at the start of the first
    // agent turn, into a sidecar the record reads. Measured the same day: 45 tools with the editor's server
    // connected, and `grimoire_frame_view` absent under `--hide-tools grimoire_frame_view`.
    //
    // **A valid omp extension, because omp refuses one that does not export a factory** and then does not
    // read the `.mcp.json` beside it.
    let offered = tools_offered_path(dir);
    bevy_grimoire_core::atomic_file::write(
        &extension.join("index.ts"),
        format!(
            "// Written by cauldron-bench. An omp extension folder whose .mcp.json beside it names the\n\
             // editor's MCP server exactly as the terminal's agent tab does, and which records the tool\n\
             // list the agent was offered so a `hide_tools` arm is judged by what the agent saw.\n\
             import {{ appendFileSync }} from \"node:fs\";\n\
             export default function (pi: any) {{\n\
             \x20 const record = (why: string) => {{\n\
             \x20   let line: any;\n\
             \x20   try {{ line = {{ why, tools: (pi.getAllTools?.() ?? []).map((t: any) => t.name) }}; }}\n\
             \x20   catch (e) {{ line = {{ why, error: String(e) }}; }}\n\
             \x20   appendFileSync({offered:?}, JSON.stringify(line) + \"\\n\");\n\
             \x20 }};\n\
             \x20 pi.on(\"before_agent_start\", async () => record(\"before_agent_start\"));\n\
             }}\n",
            offered = offered.to_string_lossy()
        ),
    )
    .map_err(|e| format!("{}: {e}", extension.display()))?;
    // **The credential the trial's HOME needs**, carried from the user's own and nothing else: `omp auth
    // export` names credentials, so no MCP server, skill or memory can ride along.
    seed_credentials(&dir.join("home"))?;
    let transcript = dir.join("transcript.jsonl");
    let stdout = std::fs::File::create(&transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
    let stderr = std::fs::File::create(dir.join("agent.err")).map_err(|e| format!("{e}"))?;
    // **The budget in the harness's own words**, so it ends its turn itself rather than being killed in the
    // middle of one: `--max-time` is omp's own stop, and a trial that kills a write leaves a project no agent
    // wrote. The wait below is the outer bound for a harness that ignores it.
    let wall = format!("{}s", task.budget.wall_s);
    let mut omp = Command::new("omp");
    omp.args([
            "-p",
            "--mode",
            "json",
            "--model",
            &options.model,
            "--no-context-files",
            "--no-skills",
            "--no-rules",
            "--no-extensions",
            // **No session file**, so a trial leaves a transcript and its project and nothing a later trial
            // could resume by accident.
            "--no-session",
            "--max-time",
            &wall,
            "-e",
            &extension.to_string_lossy(),
            "--cwd",
            &project.to_string_lossy(),
            &task.prompt,
        ])
        .env("BEVY_BRP_PORT", editor.port.to_string())
        // **The trial's own HOME**, so the harness reads no user `mcp.json`, no memory and no session
        // store: see this function's own note.
        .env("HOME", dir.join("home"))
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    let child = detach(&mut omp)
        .spawn()
        .map_err(|e| format!("omp did not start: {e}"))?;
    // **Stopped at the budget**: the wall clock is the one bound a model's turns cannot be argued with.
    // `--max-time` is what ends the agent's own turn; the deadline is the outer bound for a harness that
    // ignores its own flag, and it is the same number so a trial is one budget and not two.
    wait_for_agent(child, editor, task.budget.wall_s, "omp")?;
    let server = config
        .get("mcpServers")
        .and_then(Value::as_object)
        .and_then(|servers| servers.keys().next().cloned())
        .ok_or("the editor's MCP config names no server")?;
    read_transcript(&transcript, dir, &server)
}

/// **Where a trial's harness records the tool list it was offered**, beside the transcript.
///
/// `omp --mode json`'s stream carries no `tools` key (measured, 2026-11-13), so the extension the trial
/// already loads writes one line here per agent turn: `{why, tools}`. It is a separate file because it is
/// a fact about the harness that the harness's own stream does not state.
pub fn tools_offered_path(dir: &Path) -> PathBuf {
    dir.join("tools-offered.jsonl")
}

/// **The tools the agent was offered**, from the sidecar the extension wrote, in the names the scripted
/// agents' server uses, or the reason the trial cannot be judged.
///
/// - **Names are normalised.** omp spells the editor's tool `mcp__<server>_grimoire_frame_view`; the server's
///   own `tools/list` says `grimoire_frame_view`. A check of the second against the first is vacuously
///   "absent" with no `hide_tools` at all, so the prefix is stripped here, the way `mcpToolName` is for a call.
/// - **An empty answer is an error, never `[]`.** A missing file, a registry that threw, a list with no
///   editor tool (the MCP server never connected) and a name from outside the Dark Factory's set are each a
///   harness error naming what was seen. `tools_offered: []` would read as "offered nothing" and pass a
///   `hide_tools` arm that hid nothing.
/// - The last line wins: each is the registry at the start of a turn.
pub fn parse_tools_offered(text: &str, server: &str) -> Result<Vec<String>, String> {
    let prefix = format!("mcp__{server}_");
    let mut last: Option<Vec<String>> = None;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let event: Value = serde_json::from_str(line)
            .map_err(|e| format!("a line of the tool list the harness recorded is not JSON ({e}): {line}"))?;
        if let Some(error) = event.get("error").and_then(Value::as_str) {
            return Err(format!("the harness could not list its own tools: {error}"));
        }
        if let Some(tools) = event.get("tools").and_then(Value::as_array) {
            last = Some(tools.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect());
        }
    }
    let Some(raw) = last else {
        return Err(
            "the harness recorded no tool list, so what the agent was offered is unknown: its first turn \
             never started, or the extension that records it did not load"
                .to_owned(),
        );
    };
    let mut names = Vec::with_capacity(raw.len());
    let mut editor_tools = 0usize;
    let mut outside: Vec<String> = Vec::new();
    for tool in raw {
        if let Some(own) = tool.strip_prefix(&prefix) {
            editor_tools += 1;
            names.push(own.to_owned());
        } else if BUILTIN_TOOLS.contains(&tool.as_str()) {
            names.push(tool);
        } else {
            outside.push(tool);
        }
    }
    if !outside.is_empty() {
        return Err(format!(
            "the agent was offered {} that the Dark Factory did not hand out, so the profile was not \
             isolated",
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

/// **Give a trial's isolated `HOME` the credentials the user's own holds**, and nothing else.
///
/// `omp auth export` prints what this host is logged in to; `omp auth import` writes it into the `HOME`
/// this is pointed at. A failure here is a harness error, because a trial whose harness cannot reach a
/// model measures the harness: **the reason is what the user has to fix**, and it is named.
fn seed_credentials(home: &Path) -> Result<(), String> {
    let exported = Command::new("omp")
        .args(["auth", "export"])
        .output()
        .map_err(|e| format!("`omp auth export` could not be run: {e}"))?;
    if !exported.status.success() {
        return Err(format!(
            "`omp auth export` failed ({}): {}",
            exported.status,
            String::from_utf8_lossy(&exported.stderr).lines().next().unwrap_or("nothing")
        ));
    }
    let carried = serde_json::from_slice::<Value>(&exported.stdout)
        .map_err(|e| format!("`omp auth export` did not print JSON: {e}"))?;
    let count = carried
        .get("credentials")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if count == 0 {
        return Err(
            "this host has no stored omp credentials, so a trial's harness could not reach a model. \
             Log in with `omp` once."
                .to_owned(),
        );
    }
    let file = home.join("auth-carried.json");
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    bevy_grimoire_core::atomic_file::write(
        &file,
        serde_json::to_vec(&carried).map_err(|e| format!("{e}"))?,
    )
    .map_err(|e| format!("{}: {e}", file.display()))?;
    let imported = Command::new("omp")
        .args(["auth", "import", &file.to_string_lossy()])
        .env("HOME", home)
        .output()
        .map_err(|e| format!("`omp auth import` could not be run: {e}"))?;
    if !imported.status.success() {
        return Err(format!(
            "`omp auth import` into the trial's HOME failed ({}): {}",
            imported.status,
            String::from_utf8_lossy(&imported.stderr).lines().next().unwrap_or("nothing")
        ));
    }
    let _ = std::fs::remove_file(&file);
    Ok(())
}

/// **What a transcript says the agent did**, read from `omp --mode json`'s own events.
///
/// **A harness that never reached the model is a harness error**, and it is the *model's own* refusal that
/// says so: omp writes "No API key found" or "No such model" to its stderr, and a run whose transcript
/// holds no assistant message at all is a run of nothing. Scoring that as an agent failure would be a
/// result about the harness with the wrong name on it  -  measured on this host, 2026-11-13.
pub fn read_transcript(path: &Path, dir: &Path, server: &str) -> Result<AgentRun, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("the transcript at {} could not be read: {e}", path.display()))?;
    let mut run = AgentRun::default();
    let mut spoke = false;
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
        // **`tool_execution_end` is where omp reports a tool finishing**, and `details.xdev` names the
        // editor tool it ran.
        //
        // **This harness reaches an MCP tool through omp's device namespace**, not as a bare call: the
        // transcript shows `write xd://mcp__cauldron_grimoire_present {"content": "{…}"}`, and the answer
        // carries `details.xdev = {tool: "mcp__cauldron_grimoire_present", mode: "execute", args: {…}}`.
        // Measured on this host, 2026-11-13. So the tool's own name is `details.xdev.inner.mcpToolName`
        // (the plain `grimoire_guide`, where `details.xdev.tool` carries the `mcp__cauldron_` prefix), its
        // arguments are `details.xdev.args`, and its result is the text content — and a reader that looked
        // only for `toolcall_end`'s `name` saw `write` and counted nothing.
        if kind == "tool_execution_end"
            && let Some(details) = event.get("result").and_then(|r| r.get("details"))
            && let Some(xdev) = details.get("xdev")
        {
            // **The tool's own name**, which is `inner.mcpToolName` (`grimoire_guide`): the
            // `mcp__<server>_` prefix names the server the tool came from, and a record that kept it
            // would spell one call two ways depending on which agent made it.
            let Some(name) = xdev
                .get("inner")
                .and_then(|inner| inner.get("mcpToolName"))
                .and_then(Value::as_str)
                .map(str::to_owned)
            else {
                continue;
            };
            let name = name.as_str();
            let args = xdev.get("args").cloned().unwrap_or(Value::Null);
            // **Refused**, by the envelope's `isError` or by the payload's own word for it.
            let inner = xdev.get("inner");
            let answer: String = inner
                .and_then(|i| i.get("rawContent"))
                .and_then(Value::as_array)
                .map(|blocks| {
                    blocks.iter().filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join(" ")
                })
                .unwrap_or_default();
            let refused = is_refusal(
                inner.and_then(|i| i.get("isError")).and_then(Value::as_bool).unwrap_or(false),
                &answer,
            );
            run.tool_calls.push(name.to_owned());
            run.tool_log.push(CallLog::new(name, &args, refused));
            if name == "grimoire_present" {
                run.handbacks.push(handback_of(&args));
            }
            if name == "grimoire_wish" {
                run.wishes.push(args);
            }
        }
        // **A `toolcall_end` inside a `message_update`** is a tool the model called by name, and a name
        // that is not one of the harness's own is a tool the benchmark did not hand out  -  which means
        // something outside the isolated set was loaded.
        if kind == "message_update"
            && let Some(inner) = event.get("assistantMessageEvent")
            && inner.get("type").and_then(Value::as_str) == Some("toolcall_end")
            && let Some(call) = inner.get("toolCall")
            && let Some(name) = call.get("name").and_then(Value::as_str)
        {
            // **`read`/`write` against `xd://mcp__…` are the device path**, counted above from their
            // execution; the built-ins are the harness's own.
            let is_device = call
                .get("arguments")
                .and_then(|a| a.get("path"))
                .and_then(Value::as_str)
                .is_some_and(|p| p.starts_with("xd://mcp__"));
            if !is_device && !BUILTIN_TOOLS.contains(&name) {
                return Err(format!(
                    "the transcript shows a tool `{name}` that is neither the editor's nor the harness's \
                     own set, so the profile was not isolated"
                ));
            }
        }
        // **A model turn**, and the tokens it cost.
        if kind == "message_end"
            && let Some(message) = event.get("message")
            && message.get("role").and_then(Value::as_str) == Some("assistant")
        {
            // **A turn the provider refused is not the agent's failure.** An assistant message whose
            // `stopReason` is `error` (an HTTP 429 for a spent usage limit, a dropped connection) holds no
            // content and no tokens, and the agent that follows is graded on a project it never touched:
            // measured on this host, 2026-11-13, a spent weekly limit scored a trial `fail` with 0 calls and
            // 0 tokens. That is the provider's answer with the agent's name on it, so it is a harness error
            // quoting the provider's own words.
            if message.get("stopReason").and_then(Value::as_str) == Some("error") {
                let why = message
                    .get("errorMessage")
                    .and_then(Value::as_str)
                    .unwrap_or("it gave no reason");
                return Err(format!(
                    "the model provider refused a turn, so this trial measures the provider and not the \
                     agent: {}",
                    why.lines().collect::<Vec<_>>().join(" ")
                ));
            }
            spoke = true;
            if let Some(usage) = message.get("usage") {
                let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
                let mut tokens = run.usage.unwrap_or_default();
                let turn = Tokens {
                    input: count("input"),
                    output: count("output"),
                    cache_read: count("cacheRead"),
                    cache_write: count("cacheWrite"),
                    total: count("totalTokens"),
                };
                tokens.add(turn);
                run.usage = Some(tokens);
                // **`tokens` is the harness's own total, as it always was**: a turn that reports none adds none.
                if usage.get("totalTokens").and_then(Value::as_u64).is_some() {
                    run.tokens = Some(run.tokens.unwrap_or(0) + turn.total);
                }
            }
        }
    }
    if !spoke {
        let stderr = std::fs::read_to_string(dir.join("agent.err")).unwrap_or_default();
        let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
        let from = lines.len().saturating_sub(4);
        return Err(format!(
            "omp never reached the model, so this trial says nothing about the editor. Its stderr ends: {}",
            lines[from..].join(" | ")
        ));
    }
    // **What the agent was offered, not what it called.** The list is the sidecar the trial's extension
    // writes (`tools_offered_path`), because a `hide_tools` arm is a fact about the list and calling
    // nothing would otherwise read as offering nothing. It is read after the check above: a harness that
    // never reached the model has no turn to record, and that is the better reason to give.
    let sidecar = tools_offered_path(dir);
    let text = std::fs::read_to_string(&sidecar)
        .map_err(|e| format!("the tool list the harness records could not be read at {}: {e}", sidecar.display()))?;
    run.tools_offered = parse_tools_offered(&text, server)?;
    Ok(run)
}

/// **The hand-back a `grimoire_present` call makes**, from its parameters.
///
/// `deny_unknown_fields` is not applied here: a hand-back's extra fields are the harness's or the editor's,
/// and a check reads the words rather than the shape.
pub fn handback_of(params: &Value) -> Handback {
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
        // **What a report groups by**, so a report is built from the trials alone and not from a suite
        // that may have moved since.
        object.insert("title".to_owned(), json!(task.title));
        object.insert("split".to_owned(), json!(task.split.name()));
        object.insert("tags".to_owned(), json!(task.tags));
    }
    let text = match serde_json::to_string_pretty(&value) {
        Ok(text) => text + "\n",
        Err(_) => return,
    };
    let _ = bevy_grimoire_core::atomic_file::write(&path, text);
}
