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

/// **The directory under a run's output where the pre-flight turn's isolated HOME is made**, and deleted again.
pub const PREFLIGHT_DIR: &str = "preflight";

/// **What the pre-flight turn says**: a few words, so the model's answer costs next to nothing.
pub const PREFLIGHT_PROMPT: &str = "reply ok";

/// **How long omp itself is given for the pre-flight turn** (`--max-time`), seconds. A trial's own turn took ~35 s
/// to be refused on a spent quota (#432), and an answering model needs a few: this is several times either.
pub const PREFLIGHT_WALL_S: u64 = 90;

/// **How long past [`PREFLIGHT_WALL_S`] the harness waits before killing an omp that ignored its own flag**, seconds.
pub const PREFLIGHT_GRACE_S: u64 = 15;

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

/// **The refusal an operation's answer carries**, or `None` when it did not refuse (#433).
///
/// The editor answers an operation call with `Ok` even when the operation was refused: the answer says
/// `outcome: "refused"` and gives `refusal` (or `problem.what`). A setup step that was refused changed
/// nothing, and the trial that follows would measure a project the task never described.
pub fn refused_answer(answer: &Value) -> Option<String> {
    if !is_refusal(false, &answer.to_string()) {
        return None;
    }
    let why = answer
        .get("refusal")
        .and_then(Value::as_str)
        .or_else(|| answer.get("problem").and_then(|p| p.get("what")).and_then(Value::as_str))
        .map_or_else(|| answer.to_string(), str::to_owned);
    Some(format!("`{}` was refused: {why}", answer.get("operation").and_then(Value::as_str).unwrap_or("an operation")))
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
    /// **Whether the task is held out**, so nothing that prints the trial names it (see [`summary_line`]).
    pub held_out: bool,
    /// **Whether the agent was stopped because it made a call beyond its task's `budget.tool_calls`** (#433): such a trial
    /// was cut off, and what it left was graded as it stood.
    pub call_budget_exhausted: bool,
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
            "call_budget_exhausted": self.call_budget_exhausted,
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
    Ok(state_root()?.join(run))
}

/// **The directory every run's directory is under**: `~/.local/state/cauldron-bench`.
pub fn state_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("there is no HOME, so a run has nowhere to write")?;
    Ok(PathBuf::from(home).join(".local/state/cauldron-bench"))
}

/// **A trial's one line in a report**, as a person reads it.
///
/// **A held-out trial is one line that names no task**: its id and the checks it failed are held-out
/// content, and the run's console output is a log.
pub fn summary_line(record: &TrialRecord) -> String {
    if record.held_out {
        return format!(
            "(a held-out task) trial {}: {}",
            record.trial,
            record.outcome.name().to_uppercase()
        );
    }

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
    if interrupted() {
        return Err(INTERRUPTED_SENTENCE.to_owned());
    }
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
        if interrupted() {
            return Err(INTERRUPTED_SENTENCE.to_owned());
        }
        if brp_call(port, "grimoire/state", json!({})).is_ok() && map_is_open(port) {
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

/// **The operation a started editor is probed with**: a dry run of `note_remove` for a note that does not exist.
///
/// It changes nothing, and its refusal says whether a map is open: "no map is open, ..." before the editor has opened
/// the map it was pointed at, and "no note ... on this map" after. **Pinned to the editor's wording** by
/// `tests/a_map_probe_matches_the_editor.rs`, which reads `bevy_cauldron`'s source for both.
pub const MAP_PROBE_OP: &str = "note_remove";

/// **What a refusal says when no map is open yet** (the start of the editor's own sentence; see [`MAP_PROBE_OP`]).
pub const NO_MAP_OPEN: &str = "no map is open";

/// **Whether the editor has opened its map**, which `grimoire/state` cannot say: it names the map from the moment the
/// editor is pointed at it (#433). Under load the BRP server answers many frames before the map is open, and a setup
/// step or an agent's first call in that gap is refused with "no map is open": `work-the-open-note` failed 8 of 24 trials
/// run 8 at a time that way, and passed alone.
fn map_is_open(port: u16) -> bool {
    let answer = brp_call(
        port,
        "grimoire/operation_apply",
        json!({"operation": MAP_PROBE_OP, "parameters": {"id": "bench-probe"}, "dry_run": true}),
    );
    match answer {
        Ok(value) => !refused_answer(&value).is_some_and(|why| why.contains(NO_MAP_OPEN)),
        Err(_) => false,
    }
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
///
/// `held_out` is **where the held-out tasks are**, outside every repository (`--held-out <dir>`); `None` is a
/// dev-only run. It is an argument and not a field of [`Options`] because it is not a property of a run's
/// settings: it is a place on this host, which no report or header may carry.
pub fn run(options: &Options, held_out: Option<&Path>) -> Result<Vec<TrialRecord>, String> {
    if options.agent == Agent::Omp {
        check_model_form(&options.model)?;
    }
    let known = operations(&options.editor)?;
    let known_refs: Vec<&str> = known.iter().map(String::as_str).collect();
    let suite = Suite::load(&options.root, &options.tasks, &known_refs)
        .map_err(|p| format!("the suite was refused: {}", p.line()))?;
    // **The held-out tasks, when a directory names them**: read from outside the repository, refused when
    // the directory is inside one, and selected by id like any other (an id none of the two holds is
    // refused, naming only the dev ids, since a held-out id is not to reach a log).
    let held: Vec<Task> = match held_out {
        Some(dir) => {
            Suite::load_held_out(&options.root, dir, &known_refs)
                .map_err(|_| held_out_refusal(&options.root, dir, &known_refs))?
                .tasks
        }
        None => Vec::new(),
    };
    let selected: Vec<Task> = if options.only.is_empty() {
        suite.tasks.iter().chain(held.iter()).cloned().collect()
    } else {
        let mut chosen = Vec::with_capacity(options.only.len());
        for id in &options.only {
            match held.iter().find(|t| &t.id == id) {
                Some(task) => chosen.push(task.clone()),
                // **The dev suite answers, and refuses in its own words**, which name only its own ids.
                None => chosen.extend(suite.select(std::slice::from_ref(id))?.into_iter().cloned()),
            }
        }
        chosen
    };
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
        Agent::Omp => check_model(OMP_PROGRAM, &options.model)?,
        Agent::ClaudeCode => check_claude()?,
        Agent::Oracle | Agent::Nothing => {}
    }
    std::fs::create_dir_all(&options.out)
        .map_err(|e| format!("{}: {e}", options.out.display()))?;
    // **A killed run's credential directories are swept before this one starts** (#433): the guards are `Drop`s, which
    // `kill -KILL` never runs. Only directories the runner made (a run directory carries its `run.json`) and only those
    // older than any trial can run, so a run in progress under another `--run` keeps its own. This run's own directory is
    // not special: nothing here is swept at age zero while another process might share its name.
    if let Some(state) = options.out.parent() {
        sweep_stale_secrets(state, Duration::from_secs(STALE_SECRETS_AGE_S));
    }
    // **The header, before the pre-flight turn and any trial**, so a run killed halfway still says how it was made and
    // its directory carries the marker the sweep looks for.
    let header = crate::probe::header(options, &options.label);
    let header_text = serde_json::to_string_pretty(&header).map_err(|e| format!("{e}"))? + "\n";
    bevy_grimoire_core::atomic_file::write(&options.out.join(crate::report::RUN_FILE), header_text)
        .map_err(|e| format!("{}: {e}", options.out.display()))?;
    // **One real turn before any trial** (#432, D2): a model the host lists can still have a provider with no
    // quota left or a key that is refused, and finding that out trial by trial costs 35 s each, for every
    // trial of the run. The turn runs in the same kind of isolated HOME a trial uses, with the same credential.
    if options.agent == Agent::Omp {
        // **Said once per run**: which of the runner's environment variables the omp agent is NOT given (names only), so the
        // filter's effect is visible to whoever reads the run's output (#434).
        eprintln!("{}", withheld_line(&credential_variables(std::env::vars_os().map(|(name, _)| name))));
        let usage = preflight_turn(OMP_PROGRAM, &options.model, &options.out.join(PREFLIGHT_DIR))?;
        // **The pre-flight turn is a paid turn no trial counts**: its tokens are kept beside the run so the report's total has it.
        bevy_grimoire_core::atomic_file::write(&options.out.join(crate::report::PREFLIGHT_FILE), preflight_usage_json(usage).to_string() + "\n")
            .map_err(|e| format!("{}: {e}", options.out.display()))?;
    }
    // **A model is only scored on held-out tasks that are valid** (BM-04): the scripted agents are the proof
    // themselves, so they skip it.
    let held_selected: Vec<&Task> = selected.iter().filter(|t| t.split == crate::task::Split::HeldOut).collect();
    if !held_selected.is_empty() && !matches!(options.agent, Agent::Oracle | Agent::Nothing) {
        let owned: Vec<Task> = held_selected.into_iter().cloned().collect();
        validate_held_out(options, &owned)?;
    }
    // (the header was written above)

    // **One work list, one worker per `jobs`**, so a `--jobs` of 1 is the same code as a `--jobs` of 4.
    // **A held-out task's trials are filed by ordinal**, never by id: the trial's directory is an ancestor of
    // the agent's working directory, and its name is the one thing in the path an agent reads without looking.
    let mut work: Vec<(Task, String, u32)> = Vec::new();
    let mut held_seen = 0usize;
    for task in &selected {
        let label = if task.split == crate::task::Split::HeldOut {
            held_seen += 1;
            held_out_label(held_seen)
        } else {
            task.id.clone()
        };
        for trial in 1..=options.trials {
            work.push((task.clone(), label.clone(), trial));
        }
    }
    let records = run_work(
        &work,
        options.jobs,
        &|task, label, trial| one_trial(options, task, label, trial, variant.as_ref()),
        &|task, label, trial, reason| not_run(options, task, label, trial, variant.as_ref(), reason),
    )?;
    Ok(records)
}

/// **One unit of a run's work**: a task, the name its trials are filed under, and which trial of it.
pub type Work = (Task, String, u32);

/// **How many provider refusals in a row stop a run** (#432, D2). Three in a row is not a flaky network and not
/// the agent's doing: it is a provider that is not answering, and every later trial would say the same thing.
pub const PROVIDER_REFUSALS_MAX: u32 = 3;

/// **How a trial's reason begins when the model's provider refused a turn** (an HTTP 429 for a spent quota, a
/// refused key), which is what the run's circuit breaker counts.
pub const PROVIDER_REFUSAL: &str = "the model provider refused a turn";

/// **Whether a trial ended because the provider refused**, and for no other reason.
pub fn is_provider_refusal(record: &TrialRecord) -> bool {
    record.outcome == Outcome::HarnessError && record.reason.starts_with(PROVIDER_REFUSAL)
}

/// **Run a work list on `jobs` workers, and stop it when the provider has stopped answering.**
///
/// `run_one` makes one trial. After [`PROVIDER_REFUSALS_MAX`] provider refusals **in a row** (a success, a
/// failure or any other harness error resets the count) no worker takes another trial: those in flight finish,
/// and every trial that was not started gets a record from `not_run` carrying the reason, so the run's report
/// counts all of them and says why they are harness errors rather than leaving them out.
pub fn run_work(
    work: &[Work],
    jobs: u32,
    run_one: &(dyn Fn(&Task, &str, u32) -> TrialRecord + Sync),
    not_run: &(dyn Fn(&Task, &str, u32, String) -> TrialRecord + Sync),
) -> Result<Vec<TrialRecord>, String> {
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::SeqCst};
    let next = AtomicUsize::new(0);
    let consecutive = AtomicU32::new(0);
    let stopped = AtomicBool::new(false);
    let last_refusal: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    let finished: std::sync::Mutex<Vec<(usize, TrialRecord)>> = std::sync::Mutex::new(Vec::new());
    let panicked = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..jobs.max(1))
            .map(|_| {
                scope.spawn(|| {
                    loop {
                        if stopped.load(SeqCst) || interrupted() {
                            return;
                        }
                        let i = next.fetch_add(1, SeqCst);
                        let Some((task, label, trial)) = work.get(i) else {
                            return;
                        };
                        let record = run_one(task, label, *trial);
                        if is_provider_refusal(&record) {
                            if let Ok(mut last) = last_refusal.lock() {
                                *last = Some(record.reason.clone());
                            }
                            if consecutive.fetch_add(1, SeqCst) + 1 >= PROVIDER_REFUSALS_MAX {
                                stopped.store(true, SeqCst);
                            }
                        } else {
                            consecutive.store(0, SeqCst);
                        }
                        if let Ok(mut finished) = finished.lock() {
                            finished.push((i, record));
                        }
                    }
                })
            })
            .collect();
        let joined: Vec<bool> = handles.into_iter().map(|h| h.join().is_err()).collect();
        joined.contains(&true)
    });
    if panicked {
        return Err("a trial's worker panicked".to_owned());
    }
    let mut finished = finished.into_inner().map_err(|_| "a trial's records were poisoned".to_owned())?;
    let was_interrupted = interrupted();
    if stopped.load(SeqCst) || was_interrupted {
        let why = last_refusal
            .into_inner()
            .map_err(|_| "a trial's records were poisoned".to_owned())?
            .unwrap_or_default();
        let started: std::collections::BTreeSet<usize> = finished.iter().map(|(i, _)| *i).collect();
        for (i, (task, label, trial)) in work.iter().enumerate() {
            if !started.contains(&i) {
                let reason = if was_interrupted {
                    "not run: the run was interrupted (SIGINT or SIGTERM), so this trial was never started".to_owned()
                } else {
                    format!(
                        "not run: the run was stopped after {PROVIDER_REFUSALS_MAX} consecutive provider refusals, so this \
                         trial was never started. The last refusal: {why}"
                    )
                };
                finished.push((i, not_run(task, label, *trial, reason)));
            }
        }
    }
    let mut records: Vec<TrialRecord> = finished.into_iter().map(|(_, record)| record).collect();
    records.sort_by(|a, b| a.task.cmp(&b.task).then(a.trial.cmp(&b.trial)));
    Ok(records)
}

/// **Prove the held-out tasks valid before any trial scores one** (BM-04, #414).
///
/// BM-01's two runs, applied to the running editor: an agent that connects and stops (`nothing`) must fail
/// the task, and the task's own `solution` replayed through the real MCP server (`oracle`) must pass it. A
/// held-out task that fails either way is a number about the task and not about the agent, and it would sit in
/// the held-out total for as long as nobody looked. They are not in the repository, so the push gate cannot
/// prove them (`every_task_is_valid` does that for the dev tasks); this is the same proof, made where they are.
///
/// **A failure names the task's place in the directory, never its id or its checks**, because a held-out
/// task's content is not to reach a log. The trial's own `trial.json`, under the state directory, says more.
pub fn validate_held_out(options: &Options, tasks: &[Task]) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    // **The proof's files are deleted before any model trial starts.** An oracle trial's record holds the task's
    // own solution (every call it made, with its arguments) and its project holds the solved scene; they are the
    // answer key. They are written under a scratch directory of this process, never under the run's output
    // directory, which is an ancestor of every model trial's working directory.
    let scratch = std::env::temp_dir().join(format!("cauldron-bench-validity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    for (n, task) in tasks.iter().enumerate() {
        for agent in [Agent::Nothing, Agent::Oracle] {
            let mut proof = options.clone();
            proof.agent = agent;
            proof.out = scratch.join(agent.name());
            let record = one_trial(&proof, task, &held_out_label(n + 1), 1, None);
            let wanted = if agent == Agent::Nothing { Outcome::Fail } else { Outcome::Pass };
            if record.outcome != wanted {
                problems.push(format!(
                    "held-out task {} of {} under `{}` ended `{}` and should end `{}`; the proof's files were deleted with the rest, so read the task in the held-out directory",
                    n + 1,
                    tasks.len(),
                    agent.name(),
                    record.outcome.name(),
                    wanted.name(),
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("the held-out tasks are not all valid:\n  {}", problems.join("\n  ")))
    }
}

/// **Build a finished run's report, write it beside its trials, and publish it** when the run was named.
///
/// Answers the report and the paths written. A run whose `trial.json` files do not read is an error naming
/// the file: a report of fewer trials than the run made would be a worse number with a better look.
pub fn write_reports(options: &Options) -> Result<(crate::report::Report, Vec<PathBuf>), String> {
    // **An interrupted run publishes nothing** (#433 review): the trials it never started are only harness errors, so a
    // report of them would still read as fit (10% are tolerated) and could be committed under `--label` as a result.
    if interrupted() {
        return Err(format!("{INTERRUPTED_SENTENCE}, so no report was written"));
    }
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

/// **The calls an agent has made so far, counted from the transcript its harness is writing**, against the
/// number its task gave it (`budget.tool_calls`, #432 D3).
///
/// The transcript is read as it grows: each poll reads only the bytes appended since the last, keeps a line
/// that is not finished yet until it is, and adds up what `count` says each whole event is. omp counts a
/// `tool_execution_end` ([`omp_calls`]) and Claude Code the `tool_use` blocks of an assistant message
/// ([`claude_calls`]); every tool counts, the harness's own as well as the editor's.
pub struct ToolBudget {
    path: PathBuf,
    count: fn(&Value) -> u32,
    limit: u32,
    offset: u64,
    pending: Vec<u8>,
    calls: u32,
}

impl ToolBudget {
    /// **A meter over the transcript at `path`**, spent after `limit` calls.
    pub fn new(path: PathBuf, count: fn(&Value) -> u32, limit: u32) -> ToolBudget {
        ToolBudget { path, count, limit, offset: 0, pending: Vec::new(), calls: 0 }
    }

    /// **How many calls the transcript holds**, whole lines only. A transcript that is not there yet holds none.
    pub fn calls(&mut self) -> u32 {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(&self.path) else {
            return self.calls;
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return self.calls;
        }
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() {
            return self.calls;
        }
        self.offset += bytes.len() as u64;
        self.pending.extend_from_slice(&bytes);
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            if let Ok(event) = serde_json::from_slice::<Value>(&line[..line.len() - 1]) {
                self.calls += (self.count)(&event);
            }
        }
        self.calls
    }

    /// **Whether the agent has made a call BEYOND the number it was given** (#433): an agent that uses exactly its budget
    /// is not stopped, and so is not cut before its closing message. The call that went over has already finished by
    /// the time it is counted, so it is the one the stop is about.
    pub fn spent(&mut self) -> bool {
        self.calls() > self.limit
    }
}

/// **omp's call count for one event**: a `tool_execution_end` is a call that finished.
pub fn omp_calls(event: &Value) -> u32 {
    // **A call made through omp's `eval` tool is each editor tool it called** (`result.details.cells[*].statusEvents[*].op`,
    // #434), not the one `eval`: three calls in a cell are three against a task's budget. An `eval` that called no editor
    // tool is one call, as any tool is.
    if event.get("type").and_then(Value::as_str) != Some("tool_execution_end") {
        return 0;
    }
    match eval_ops(event) {
        Some(ops) => (ops.len() as u32).max(1),
        None => 1,
    }
}

/// **Claude Code's call count for one stream-json event**: every `tool_use` block of an assistant message.
pub fn claude_calls(event: &Value) -> u32 {
    if event.get("type").and_then(Value::as_str) != Some("assistant") {
        return 0;
    }
    event
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .map_or(0, |blocks| {
            blocks.iter().filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use")).count() as u32
        })
}

/// **Wait for an agent to finish, or stop it at its wall budget** (`wall_s` seconds) or its call budget, and stop it for good.
///
/// A dead editor ends the trial now, not at the budget: the agent is talking to nothing, and the editor's own
/// exit is the reason to report. **The whole process group is killed**, not only the child: a harness runs
/// its tools as children of its own, and a `bash` left running after its parent was killed is a process
/// holding the trial's project (the follow-up #412's review named). The agent leads its own group
/// ([`detach`]), so the group is the agent's and nothing else's.
///
/// **An agent that has made a call beyond what its task gave it is stopped** as one that ran out of time is
/// (`budget`, [`ToolBudget`]), and what it left is graded: the call budget is a bound on the run, not a failure of it.
/// The answer says which way the wait ended, so a trial stopped at its budget can be recorded as such.
pub fn wait_for_agent(
    child: Child,
    editor: &mut Editor,
    wall_s: u32,
    name: &str,
    budget: Option<&mut ToolBudget>,
) -> Result<WaitEnd, String> {
    wait_within(child, wall_s, name, budget, || editor_exit(editor))
}

/// **How the wait for an agent ended**, when it ended without an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitEnd {
    /// The agent exited by itself.
    Finished,
    /// The wall budget ran out and the agent was stopped.
    WallTime,
    /// The agent made a call beyond its tool-call budget and was stopped.
    CallBudget,
}

/// **[`wait_for_agent`] with the editor's death passed as a question**, so a stand-in agent needs no editor.
pub fn wait_within(
    mut child: Child,
    wall_s: u32,
    name: &str,
    mut budget: Option<&mut ToolBudget>,
    mut editor_gone: impl FnMut() -> Option<String>,
) -> Result<WaitEnd, String> {
    let deadline = Instant::now() + Duration::from_secs(u64::from(wall_s));
    let mut end = WaitEnd::WallTime;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => {
                end = WaitEnd::Finished;
                break;
            }
            Ok(None) => {
                if interrupted() {
                    stop_group(&mut child);
                    return Err("the run was interrupted (SIGINT or SIGTERM), so this trial was stopped".to_owned());
                }
                if let Some(reason) = editor_gone() {
                    stop_group(&mut child);
                    return Err(reason);
                }
                if budget.as_mut().is_some_and(|b| b.spent()) {
                    end = WaitEnd::CallBudget;
                    break;
                }
                std::thread::sleep(Duration::from_millis(AGENT_POLL_INTERVAL_MS));
            }
            Err(e) => return Err(format!("{name} could not be waited on: {e}")),
        }
    }
    stop_group(&mut child);
    Ok(end)
}

/// **Kill an agent's process group while its leader still runs, then the agent itself, and wait for it.**
///
/// Answers whether the group was signalled. **A leader that was already reaped is not group-killed** (#432, D4):
/// `kill -KILL -- -<pid>` on a number nothing holds prints `No such process` for every normal trial, and once the
/// number is reused it can hit an unrelated group. A process still running keeps its group's number, so that is the
/// only time the number is safe to signal.
pub fn stop_group(child: &mut Child) -> bool {
    let running = matches!(child.try_wait(), Ok(None));
    if running {
        let group = format!("-{}", child.id());
        let _ = Command::new("kill")
            .args(["-KILL", "--", &group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = child.kill();
    }
    let _ = child.wait();
    running
}

/// **Refuse a model this host does not list**, before any trial runs.
///
/// Listing proves the name and nothing about the provider behind it; [`preflight_turn`] asks the provider.
pub fn check_model(omp: &str, model: &str) -> Result<(), String> {
    check_model_form(model)?;
    let out = Command::new(omp)
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

/// **One real turn of the model, before any trial** (#432, D2): `omp -p --model <m> "reply ok"`.
///
/// It runs the way a trial does, in an isolated HOME made under `scratch` that holds the model's provider's
/// credential and is deleted before this returns, so it also proves the credential a trial would carry. A
/// provider with no quota left (HTTP 429), a refused key or a model that does not answer is an error naming
/// the model and quoting the provider; a run that would have found it out 35 s at a time, trial by trial,
/// finds it out here, once.
pub fn preflight_turn(omp: &str, model: &str, scratch: &Path) -> Result<Tokens, String> {
    let home = scratch.join("home");
    // A pre-flight directory a killed run left is this one's to replace.
    let _ = std::fs::remove_dir_all(scratch);
    let _secrets = TrialSecrets::new([scratch.to_path_buf()]);
    seed_credentials(omp, &home, model)?;
    let wall = format!("{PREFLIGHT_WALL_S}s");
    let mut command = Command::new(omp);
    command
        .args([
            "-p",
            "--mode",
            "json",
            "--model",
            model,
            "--no-session",
            "--no-context-files",
            "--no-skills",
            "--no-rules",
            "--no-extensions",
            "--max-time",
            &wall,
            "--cwd",
        ])
        .arg(scratch)
        .arg(PREFLIGHT_PROMPT)
        .env("HOME", &home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    withhold_credentials(&mut command);
    let child = detach(&mut command)
        .spawn()
        .map_err(|e| format!("omp did not start for the pre-flight turn: {e}"))?;
    let (stdout, stderr) = output_within(child, Duration::from_secs(PREFLIGHT_WALL_S + PREFLIGHT_GRACE_S))?;
    turn_verdict(&stdout, &stderr).map_err(|why| {
        format!(
            "`{model}` is listed, but a one-turn pre-flight (`{PREFLIGHT_PROMPT}`) got no answer, so no trial of \
             this run could: {why}"
        )
    })
}

/// **What a one-turn run printed, read as omp's JSON events**: `Ok` when an assistant message came back and the
/// provider did not refuse it (a refusal omp retried and recovered from, `auto_retry_end{success: true}`, is not
/// one), and otherwise the reason, in the provider's own words when it gave them.
fn turn_verdict(stdout: &str, stderr: &str) -> Result<Tokens, String> {
    let mut tokens = Tokens::default();
    let mut spoke = false;
    let mut standing_refusal: Option<String> = None;
    for line in stdout.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event.get("type").and_then(Value::as_str);
        if kind == Some("auto_retry_end") && event.get("success").and_then(Value::as_bool) == Some(true) {
            standing_refusal = None;
        }
        if kind != Some("message_end") {
            continue;
        }
        let Some(message) = event.get("message").filter(|m| m.get("role").and_then(Value::as_str) == Some("assistant")) else {
            continue;
        };
        match provider_words(message) {
            Some(why) => standing_refusal = Some(why),
            None => {
                spoke = true;
                if let Some(usage) = message.get("usage") {
                    let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
                    tokens.add(Tokens {
                        input: count("input"),
                        output: count("output"),
                        cache_read: count("cacheRead"),
                        cache_write: count("cacheWrite"),
                        total: count("totalTokens"),
                    });
                }
            }
        }
    }
    if let Some(why) = standing_refusal {
        return Err(format!("the provider refused the turn: {why}"));
    }
    if spoke {
        return Ok(tokens);
    }
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(4);
    Err(format!("omp never reached the model. Its stderr ends: {}", lines[from..].join(" | ")))
}

/// **The provider's own words for an assistant message it refused**, or `None` for a message it answered.
///
/// An assistant message whose `stopReason` is `error` (an HTTP 429 for a spent usage limit, a dropped
/// connection) holds no content and no tokens.
fn provider_words(message: &Value) -> Option<String> {
    if message.get("stopReason").and_then(Value::as_str) != Some("error") {
        return None;
    }
    let why = message.get("errorMessage").and_then(Value::as_str).unwrap_or("it gave no reason");
    Some(why.lines().collect::<Vec<_>>().join(" "))
}

/// **A child's stdout and stderr, read whole, or an error when it is still running after `within`** (and killed).
fn output_within(mut child: Child, within: Duration) -> Result<(String, String), String> {
    use std::io::Read;
    fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let mut bytes = Vec::new();
                let _ = pipe.read_to_end(&mut bytes);
                text = String::from_utf8_lossy(&bytes).into_owned();
            }
            text
        })
    }
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = Instant::now() + within;
    loop {
        if interrupted() {
            stop_group(&mut child);
            return Err(INTERRUPTED_SENTENCE.to_owned());
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(AGENT_POLL_INTERVAL_MS)),
            Ok(None) => {
                stop_group(&mut child);
                return Err(format!("it was still running after {} s, and was stopped", within.as_secs()));
            }
            Err(e) => return Err(format!("it could not be waited on: {e}")),
        }
    }
    let stdout = out.join().map_err(|_| "its output could not be read".to_owned())?;
    let stderr = err.join().map_err(|_| "its error output could not be read".to_owned())?;
    Ok((stdout, stderr))
}

/// **What a held-out record puts where a call, a file or a wish was**: a record keeps how many there were, not what they said.
const HELD_OUT_BLANK: &str = "held-out";

/// **What a held-out task is called in a path and in a record**: its place among the held-out tasks of the run.
pub fn held_out_label(ordinal: usize) -> String {
    format!("held-out-{ordinal}")
}

/// **Why the held-out directory was refused, without naming a task.** The loader's own sentence names the task (its id, a path under the
/// held-out directory), and the run's output is a log. This finds the first task that does not load and says its place in the directory,
/// which is also what a record calls it; a refusal that is about the directory, not one task, says so.
pub fn held_out_refusal(root: &Path, dir: &Path, known: &[&str]) -> String {
    if let Ok(dirs) = Suite::task_dirs(dir) {
        for (n, task_dir) in dirs.iter().enumerate() {
            if Task::load(task_dir, root, known).is_err() {
                return format!(
                    "the held-out tasks were refused: task {} of {} in the held-out directory does not load; the loader's sentence names the task, so read it there",
                    n + 1,
                    dirs.len()
                );
            }
        }
    }
    "the held-out tasks were refused: the directory is inside a repository, cannot be read, holds a task that does not say `split: held_out`, \
     or holds two tasks with one id; the loader's sentence names the task, so run it where the directory is"
        .to_owned()
}

/// **Where one trial is filed** under a run's output directory.
fn trial_dir(options: &Options, label: &str, trial: u32) -> PathBuf {
    options.out.join(label).join(format!("trial-{trial}"))
}

/// **A trial's record before anything has been measured**: no checks, no calls, no tokens.
fn blank_record(
    options: &Options,
    task: &Task,
    label: &str,
    trial: u32,
    variant: Option<&Variant>,
    outcome: Outcome,
    reason: String,
) -> TrialRecord {
    TrialRecord {
        task: label.to_owned(),
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
        dir: trial_dir(options, label, trial),
        held_out: task.split == crate::task::Split::HeldOut,
        call_budget_exhausted: false,
    }
}

/// **A trial the run never started**, filed as the harness error it is, with `reason` (the run's own stop).
fn not_run(options: &Options, task: &Task, label: &str, trial: u32, variant: Option<&Variant>, reason: String) -> TrialRecord {
    let record = blank_record(options, task, label, trial, variant, Outcome::HarnessError, reason.clone());
    let _ = std::fs::remove_dir_all(&record.dir);
    if std::fs::create_dir_all(&record.dir).is_ok() {
        write_record(&record, task, &reason);
    }
    record
}

/// **One trial, from the copy to its `trial.json`**, and never a `pass` for a harness that failed.
///
/// `label` is what the trial is filed and recorded under: the task's id for a dev task, and
/// [`held_out_label`] for a held-out one, whose id is not to reach a path an agent can read.
fn one_trial(options: &Options, task: &Task, label: &str, trial: u32, variant: Option<&Variant>) -> TrialRecord {
    let dir = trial_dir(options, label, trial);
    let _ = std::fs::remove_dir_all(&dir);
    let record = |outcome: Outcome, reason: String| blank_record(options, task, label, trial, variant, outcome, reason);
    let held_out = task.split == crate::task::Split::HeldOut;
    let harness = |reason: String| {
        let record = record(Outcome::HarnessError, reason.clone());
        write_record(&record, task, &reason);
        record
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return harness(format!("{}: {e}", dir.display()));
    }
    let project = dir.join("project");
    if let Err(e) = fixture::lay_out_task(&options.root, &task.dir, &project) {
        // **A held-out task's own files are not named**: the error is a path under the held-out directory, which ends in the task's id.
        return harness(if held_out {
            "the fixture was not laid out (the task's own files; read the task in the held-out directory)".to_owned()
        } else {
            format!("the fixture was not laid out: {e}")
        });
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
        // **A refused answer is a failed setup, not a success** (#433): the BRP call is `Ok` when the editor
        // answered, and the answer says `outcome: refused` when the operation did not happen.
        let answer = answer.and_then(|value| match refused_answer(&value) {
            Some(why) => Err(why),
            None => Ok(value),
        });
        if let Err(e) = answer {
            // **The refusal quotes the step's operation and arguments**, which for a held-out task are its setup.
            return harness(if held_out {
                "the task's setup was refused (read the task in the held-out directory)".to_owned()
            } else {
                format!("the task's setup was refused: {e}")
            });
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
            task.budget,
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
    record.call_budget_exhausted = run.call_budget_exhausted;
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
    /// **The editor tools this agent called through a route whose arguments the transcript does not show** (omp's `eval`,
    /// #434), by the tool's own name, in order. A call listed here is counted and logged, but what it was given is not
    /// known to the runner: [`reconcile_with_editor`] takes a hand-back's words from the editor instead.
    pub blind_calls: Vec<String>,
    /// **Whether a hand-back was taken from the editor's state** because the transcript showed none (#434).
    pub handbacks_from_editor: bool,
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
    /// **Whether the agent was stopped at its tool-call budget** (#433).
    pub call_budget_exhausted: bool,
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
    // export` names credentials, so no MCP server, skill or memory can ride along. **Only the model's own
    // provider's** is carried, and the HOME (which also holds omp's 128 MB of natives) is deleted when the
    // trial's agent is done, on every path out of this function (#432, D6 and D7). The guard is made first, so
    // a failed import leaves nothing behind either.
    let _secrets = TrialSecrets::new([dir.join("home")]);
    let secrets = seed_credentials(OMP_PROGRAM, &dir.join("home"), &options.model)?;
    let transcript = dir.join("transcript.jsonl");
    let stdout = std::fs::File::create(&transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
    let stderr = std::fs::File::create(dir.join("agent.err")).map_err(|e| format!("{e}"))?;
    // **The budget in the harness's own words**, so it ends its turn itself rather than being killed in the
    // middle of one: `--max-time` is omp's own stop, and a trial that kills a write leaves a project no agent
    // wrote. The wait below is the outer bound for a harness that ignores it.
    let wall = format!("{}s", task.budget.wall_s);
    let mut omp = Command::new(OMP_PROGRAM);
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
    withhold_credentials(&mut omp);
    let child = detach(&mut omp)
        .spawn()
        .map_err(|e| format!("omp did not start: {e}"))?;
    // **Stopped at the budget**: the wall clock is the one bound a model's turns cannot be argued with.
    // `--max-time` is what ends the agent's own turn; the deadline is the outer bound for a harness that
    // ignores its own flag, and it is the same number so a trial is one budget and not two. **The calls it
    // was given are the other bound** (#432, D3): the transcript is counted while the agent runs.
    let mut calls = ToolBudget::new(transcript.clone(), omp_calls, task.budget.tool_calls);
    let waited = wait_for_agent(child, editor, task.budget.wall_s, "omp", Some(&mut calls));
    // **The credential strings the trial was given are scrubbed from the files it keeps** (#433), whichever way the wait
    // ended: an agent that read its own `agent.db` printed it into the transcript.
    scrub_secrets(&[transcript.clone(), dir.join("agent.err")], &secrets);
    let end = waited?;
    let server = config
        .get("mcpServers")
        .and_then(Value::as_object)
        .and_then(|servers| servers.keys().next().cloned())
        .ok_or("the editor's MCP config names no server")?;
    let mut run = read_transcript(&transcript, dir, &server)?;
    run.call_budget_exhausted = end == WaitEnd::CallBudget;
    // **Ask the editor what it was handed** (#434): a hand-back the transcript did not show (an `eval` call shows no
    // arguments) is taken from the editor's own state, and a trial in which the runner cannot tell is a harness error,
    // never "no hand-back was made".
    reconcile_with_editor(&mut run, brp_call(editor.port, "grimoire/state", json!({})))?;
    Ok(run)
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

/// **The program a trial runs for the `omp` harness**, found on `PATH`.
///
/// A parameter of [`check_model`] and [`seed_credentials`] and not only a literal, so a test drives them
/// with a stand-in script that holds no credential and calls no model.
pub const OMP_PROGRAM: &str = "omp";

/// **The provider a model runs on**: the part of `provider/model` before its first `/`, or `None` for a
/// bare model name that names none.
pub fn model_provider(model: &str) -> Option<&str> {
    model.split_once('/').map(|(provider, _)| provider).filter(|provider| !provider.is_empty())
}

/// **An example of a model name that names its provider**, for a refusal to quote.
pub const EXAMPLE_MODEL: &str = "anthropic/claude-sonnet-5-5";

/// **The sentence for a model name that names no provider.**
fn no_provider_sentence(model: &str) -> String {
    format!(
        "`{model}` names no provider, and a trial carries only its model's provider's credential: name the model as \
         `provider/model`, for example `--model {EXAMPLE_MODEL}`"
    )
}

/// **Refuse a model name with no `provider/` in front of it** (#433), before omp is asked anything.
///
/// The provider is what the credential is narrowed by ([`credentials_for`]); a bare name used to fall back to
/// carrying every stored credential into the trial.
pub fn check_model_form(model: &str) -> Result<(), String> {
    if model_provider(model).is_some() { Ok(()) } else { Err(no_provider_sentence(model)) }
}

/// **What a trial keeps from `omp auth export`: the credentials of one provider**, in the export's own shape.
///
/// A trial runs one model, so it needs one provider's login and no other: copying every stored credential
/// into every trial's `HOME` put the host's whole login (an OAuth refresh token included, which omp itself
/// warns signs the source machine out once a copy refreshes it) in a directory that outlived the trial (#432,
/// D6). **A bare model name names no provider and is refused** (#433): nothing could be narrowed, and carrying every
/// stored credential into a trial is what the narrowing is for.
///
/// An export that holds nothing for the provider is an error naming the providers it does hold (a provider's
/// name is not a secret) and the one to log in to.
pub fn credentials_for(carried: &Value, provider: Option<&str>, model: &str) -> Result<Value, String> {
    let all = carried.get("credentials").and_then(Value::as_array).cloned().unwrap_or_default();
    if all.is_empty() {
        return Err(
            "this host has no stored omp credentials, so a trial's harness could not reach a model. \
             Log in with `omp` once."
                .to_owned(),
        );
    }
    let Some(provider) = provider else {
        return Err(no_provider_sentence(model));
    };
    let own: Vec<Value> = all
        .iter()
        .filter(|c| c.get("provider").and_then(Value::as_str) == Some(provider))
        .cloned()
        .collect();
    if own.is_empty() {
        let held: Vec<&str> = all.iter().filter_map(|c| c.get("provider").and_then(Value::as_str)).collect();
        return Err(format!(
            "this host has no stored omp credential for `{provider}`, the provider `{model}` runs on, so a trial's \
             harness could not reach it. It holds credentials for: {}. Log in to `{provider}` with `omp` once.",
            held.join(", ")
        ));
    }
    let mut narrowed = carried.clone();
    if let Some(object) = narrowed.as_object_mut() {
        object.insert("credentials".to_owned(), Value::Array(own));
    }
    Ok(narrowed)
}

/// **Directories and files a trial made that hold a credential, deleted when this is dropped.**
///
/// A trial's isolated `HOME` holds the imported login (`agent.db`, mode 600) and, because omp extracts its
/// natives there, about 200 MB (a 128 MB `.node` and a 10 MB `models.db`): kept for every trial of a 140-trial
/// run that is 28 GB and 140 copies of a secret on a disk nobody reads again (#432, D6 and D7). Nothing in
/// them is needed once the trial is graded, so they go with the guard: **on every path out of the trial**, an
/// error and a panic included, because the guard is made *before* anything is written.
pub struct TrialSecrets {
    paths: Vec<PathBuf>,
}

impl TrialSecrets {
    /// **A guard over these paths**, which need not exist yet.
    pub fn new(paths: impl IntoIterator<Item = PathBuf>) -> TrialSecrets {
        TrialSecrets { paths: paths.into_iter().collect() }
    }
}

impl Drop for TrialSecrets {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_dir_all(path);
            let _ = std::fs::remove_file(path);
        }
    }
}

/// **The file the credentials travel in** between `omp auth export` and `omp auth import`, inside the trial's `HOME`.
const AUTH_CARRIED_FILE: &str = "auth-carried.json";

/// **Give a trial's isolated `HOME` the credential of its model's provider**, and nothing else.
///
/// `omp auth export` prints what this host is logged in to; `omp auth import` writes it into the `HOME`
/// this is pointed at. A failure here is a harness error, because a trial whose harness cannot reach a
/// model measures the harness: **the reason is what the user has to fix**, and it is named.
///
/// **The carrying file is removed on every path**, a failed import included: it holds the credentials in
/// plain text. The caller holds a [`TrialSecrets`] over `home`, which removes what the import wrote.
pub fn seed_credentials(omp: &str, home: &Path, model: &str) -> Result<Vec<String>, String> {
    let exported = Command::new(omp)
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
    let carried = credentials_for(&carried, model_provider(model), model)?;
    let secrets = credential_strings(&carried);
    let file = home.join(AUTH_CARRIED_FILE);
    std::fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
    let imported = import_credentials(omp, home, &file, &carried);
    // **Removed whether or not the import worked**: the file is the secret in plain text.
    let _ = std::fs::remove_file(&file);
    imported.map(|()| secrets)
}

/// **What marks the place a secret was in a file the trial keeps.**
pub const REDACTED: &str = "[redacted]";

/// **The shortest string treated as a secret**, characters. A shorter value (`oauth`, `api_key`, a provider's name) is a
/// word the files say elsewhere, and scrubbing it would damage them.
pub const MIN_SECRET_LEN: usize = 8;

/// **Every string value in `value`, however deep, that is long enough to be a secret.**
pub fn secret_strings(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![value];
    while let Some(next) = stack.pop() {
        match next {
            Value::String(s) if s.chars().count() >= MIN_SECRET_LEN => found.push(s.clone()),
            Value::Array(items) => stack.extend(items),
            Value::Object(map) => stack.extend(map.values()),
            _ => {}
        }
    }
    found.sort();
    found.dedup();
    found
}

/// **The secret strings of an `omp auth export`**: every string value under each entry's `credential` (an access
/// token, a refresh token, an API key), which is what the carried HOME holds and an agent in it can read.
pub fn credential_strings(export: &Value) -> Vec<String> {
    let mut found: Vec<String> = export
        .get("credentials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("credential"))
        .flat_map(secret_strings)
        .collect();
    found.sort();
    found.dedup();
    found
}

/// **Replace every secret in these files with [`REDACTED`]**, in place, and answer how many times (#433).
///
/// Each secret is looked for as it is and as it reads inside a JSON string (`"` and `\` escaped), because the
/// transcript is JSON lines and an agent's output is a string in it. A file that is not there or is not UTF-8 is
/// skipped. The retained transcript and `agent.err` are what this is for; the credential in HOME is deleted (#432).
pub fn scrub_secrets(files: &[PathBuf], secrets: &[String]) -> usize {
    let mut forms: Vec<String> = Vec::new();
    for secret in secrets.iter().filter(|s| s.chars().count() >= MIN_SECRET_LEN) {
        forms.push(secret.clone());
        if let Ok(escaped) = serde_json::to_string(secret) {
            forms.push(escaped.trim_matches('"').to_owned());
        }
    }
    forms.sort();
    forms.dedup();
    let mut replaced = 0;
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let mut scrubbed = text.clone();
        for form in &forms {
            let hits = scrubbed.matches(form.as_str()).count();
            if hits > 0 {
                scrubbed = scrubbed.replace(form.as_str(), REDACTED);
                replaced += hits;
            }
        }
        if scrubbed != text {
            let _ = bevy_grimoire_core::atomic_file::write(file, scrubbed);
        }
    }
    replaced
}

/// **Write the carrying file and run `omp auth import` on it** into `home`.
fn import_credentials(omp: &str, home: &Path, file: &Path, carried: &Value) -> Result<(), String> {
    bevy_grimoire_core::atomic_file::write(file, serde_json::to_vec(carried).map_err(|e| format!("{e}"))?)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let imported = Command::new(omp)
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
    // **The provider's last refusal that nothing has recovered from** (#432, D9): omp retries a transient 429 or
    // 5xx itself, and says so with `auto_retry_end{success: true}` once an answer came. A refusal is held
    // against the trial only if the stream ends with it still standing.
    let mut standing_refusal: Option<String> = None;
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
        if kind == "auto_retry_end" && event.get("success").and_then(Value::as_bool) == Some(true) {
            standing_refusal = None;
        }
        // **A call through omp's `eval` tool** (#434): a `tool_execution_end` whose `result.details.cells[*].statusEvents[*].op`
        // names each editor tool the cell called (`mcp__<server>_<tool>`), and which carries no arguments. Each is a call in
        // the list and the log; the transcript cannot say what it was given or what it answered, so a hand-back's words and
        // a wish's content are not here (see `reconcile_with_editor`) and a refusal is not known.
        if kind == "tool_execution_end"
            && let Some(ops) = eval_ops(&event)
        {
            let prefix = format!("mcp__{server}_");
            for op in ops {
                let name = op.strip_prefix(&prefix).unwrap_or(&op).to_owned();
                run.tool_calls.push(name.clone());
                run.tool_log.push(CallLog::new(&name, &Value::Null, false));
                if name == "grimoire_wish" {
                    run.wishes.push(json!({"tool": EVAL_WISH_TOOL}));
                }
                run.blind_calls.push(name);
            }
            continue;
        }
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
            if let Some(why) = provider_words(message) {
                standing_refusal = Some(why);
                continue;
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
    if let Some(why) = standing_refusal {
        // **Starts with [`PROVIDER_REFUSAL`]**, which is what the run's circuit breaker counts.
        return Err(format!("{PROVIDER_REFUSAL}, so this trial measures the provider and not the agent: {why}"));
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
    let text = match serde_json::to_string_pretty(&record_json(record, task, sentence)) {
        Ok(text) => text + "\n",
        Err(_) => return,
    };
    let _ = bevy_grimoire_core::atomic_file::write(&path, text);
}

/// **What `trial.json` says of a trial.**
///
/// A dev trial's record names its task, quotes its prompt and states every check with what it measured, so a
/// report is built from the trials alone. **A held-out trial's record is a count and nothing else of the
/// task**: no id (its label is an ordinal), no prompt, title or tags, no sentence, each check is only
/// whether it held, and **the agent's calls are only how many there were**: for `--agent oracle` the tool log
/// *is* the task's solution, with its arguments, and the wishes and the files changed behind the tools are the
/// agent's account of the task. The record sits in a directory that is an ancestor of every later trial's working
/// directory, and a model that reads `../trial-1/trial.json` must learn what a score is and nothing about the
/// task it was a score of.
pub fn record_json(record: &TrialRecord, task: &Task, sentence: &str) -> Value {
    if record.held_out {
        let mut shown = record.clone();
        shown.reason = if record.outcome == Outcome::HarnessError { record.reason.clone() } else { String::new() };
        shown.checks = record
            .checks
            .iter()
            .map(|c| crate::Verdict { check: "held-out".to_owned(), asks: String::new(), passed: c.passed, measured: String::new() })
            .collect();
        // **The counts the JSON derives from the calls** (`tool_call_count`, `refusals`, `undos`) are written from the full record; the
        // lists they come from are then blanked to as many placeholders, so a report that counts a list's length still counts, and
        // nothing that was in one is left.
        let (calls, behind, wishes) = (record.tool_calls.len(), record.edits_behind.len(), record.wishes.len());
        let mut value = shown.to_json();
        if let Some(object) = value.as_object_mut() {
            object.insert("split".to_owned(), json!(task.split.name()));
            object.insert("tool_calls".to_owned(), json!(vec![HELD_OUT_BLANK; calls]));
            object.insert("tool_log".to_owned(), json!([]));
            object.insert("edits_behind".to_owned(), json!(vec![HELD_OUT_BLANK; behind]));
            object.insert("wishes".to_owned(), json!(vec![json!({ "tool": HELD_OUT_BLANK }); wishes]));
            object.insert("refusals_by_tool".to_owned(), json!({}));
            // The tools offered are the server's own list, which a held-out task does not choose, and are kept.
        }
        return value;
    }
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
    value
}

/// **How old a trial's credential directory must be before another run sweeps it**, seconds: longer than any trial runs
/// (a task's wall budget is minutes), so a run in progress under another `--run` keeps its own.
pub const STALE_SECRETS_AGE_S: u64 = 3600;

/// **Delete the credential directories a dead run left**, `<state>/<run>/preflight` and
/// `<state>/<run>/<task>/trial-<n>/{home,claude-config}`, that were last modified `older_than` ago or earlier, and
/// answer how many were removed (#433). `kill -KILL` never runs the [`TrialSecrets`] guards; the next run does this.
pub fn sweep_stale_secrets(state: &Path, older_than: Duration) -> usize {
    stale_secret_dirs(state, older_than).iter().filter(|path| std::fs::remove_dir_all(path).is_ok()).count()
}

/// **The credential directories under a state directory that [`sweep_stale_secrets`] would remove**, without removing
/// them: every run directory the runner made (it carries `run.json`), its `preflight` directory and each trial's `home`
/// and `claude-config`, last modified `older_than` ago or earlier (any age for a zero `older_than`).
pub fn stale_secret_dirs(state: &Path, older_than: Duration) -> Vec<PathBuf> {
    let Ok(runs) = std::fs::read_dir(state) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = runs.flatten().filter(|run| run.path().is_dir()).flat_map(|run| stale_in_run(&run.path(), older_than)).collect();
    found.sort();
    found
}

/// **[`stale_secret_dirs`] for one run's directory.**
fn stale_in_run(run: &Path, older_than: Duration) -> Vec<PathBuf> {
    // **Only a directory the runner made**: it writes `run.json` before any trial or pre-flight turn.
    if !run.join(crate::report::RUN_FILE).is_file() {
        return Vec::new();
    }
    let stale = |path: &Path| {
        path.is_dir()
            && (older_than.is_zero()
                || std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age >= older_than)))
    };
    let mut candidates = vec![run.join(PREFLIGHT_DIR)];
    for task in std::fs::read_dir(run).into_iter().flatten().flatten().filter(|t| t.path().is_dir()) {
        for trial in std::fs::read_dir(task.path()).into_iter().flatten().flatten() {
            if trial.file_name().to_string_lossy().starts_with("trial-") {
                candidates.push(trial.path().join("home"));
                candidates.push(trial.path().join("claude-config"));
            }
        }
    }
    candidates.into_iter().filter(|path| stale(path)).collect()
}

/// **`cauldron-bench clean`: remove the stale credential directories under a state directory** (#434), or only list them.
///
/// Older runs kept their trials' `home` trees, which hold the provider keys they imported (the guard protects new runs
/// only). Answers what was removed, or with `dry_run` what would be. Marker-checked like the sweep: only inside run
/// directories the runner made.
pub fn clean_state(state: &Path, older_than: Duration, dry_run: bool) -> Vec<PathBuf> {
    let found = stale_secret_dirs(state, older_than);
    if dry_run {
        return found;
    }
    found.into_iter().filter(|path| std::fs::remove_dir_all(path).is_ok()).collect()
}

/// **Whether SIGINT or SIGTERM has arrived** since [`install_interrupt_handler`] (#433).
static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// **Has the run been told to stop?** Workers take no further trial, a waiting agent's process group is killed, and the
/// run unwinds through its guards, so no credential directory is left by a Ctrl-C.
pub fn interrupted() -> bool {
    INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
}

/// **The handler: it only sets the flag**, which is all a signal handler may safely do.
#[cfg(unix)]
extern "C" fn note_interrupt(signal: libc::c_int) {
    INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
    // SAFETY: `signal` is async-signal-safe. **The default is restored after the first signal**, so a second Ctrl-C
    // kills a run that is stuck instead of being swallowed.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
    }
}

/// **Make SIGINT and SIGTERM stop the run through its guards instead of ending the process** (#433).
///
/// Rust's default for both ends the process without running `Drop`, so a Ctrl-C used to leave every trial's
/// credential directory behind. `kill -KILL` cannot be caught; [`sweep_stale_secrets`] cleans up after that.
#[cfg(unix)]
pub fn install_interrupt_handler() {
    let handler = note_interrupt as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SAFETY: `signal` with a handler that only stores to an atomic, which is async-signal-safe.
    unsafe {
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

/// **No signal handling off Unix**: the default stands.
#[cfg(not(unix))]
pub fn install_interrupt_handler() {}

/// **What an interrupted wait, start or report says.**
const INTERRUPTED_SENTENCE: &str = "the run was interrupted (SIGINT or SIGTERM)";

/// **The names in an environment that look like a credential** (#433 review): a key, token, secret, password or
/// credential, an `AWS_` variable, an ssh agent's socket. The omp agent's shell can run `env`, and what it prints lands
/// in the kept transcript; the runner only scrubs the credential it imported, so the rest is not handed over at all.
pub fn credential_variables(names: impl IntoIterator<Item = std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    names
        .into_iter()
        .filter(|name| {
            let upper = name.to_string_lossy().to_uppercase();
            ["_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_PASSWD", "_AUTH"].iter().any(|s| upper.ends_with(s))
                || ["SECRET", "TOKEN", "PASSWORD", "CREDENTIAL", "AUTH_SOCK"].iter().any(|s| upper.contains(s))
                || upper.starts_with("AWS_")
        })
        .collect()
}

/// **Remove these variables from a command's environment.**
pub fn withhold_from(command: &mut Command, names: impl IntoIterator<Item = std::ffi::OsString>) {
    for name in names {
        command.env_remove(name);
    }
}

/// **Start an omp agent without the runner's credential-looking variables** ([`credential_variables`]). Everything else
/// is inherited: `PATH`, the editor's port, and a test's marker. (Claude Code's `ANTHROPIC_API_KEY` is its credential
/// and is carried and scrubbed instead.)
pub fn withhold_credentials(command: &mut Command) {
    withhold_from(command, credential_variables(std::env::vars_os().map(|(name, _)| name)));
}

/// **What a wish made through omp's `eval` is recorded as**: its arguments are not in the transcript, so the tool it asked
/// for is unknown, and a report counts it under this name.
pub const EVAL_WISH_TOOL: &str = "(a wish made through eval: its tool is not in the transcript)";

/// **The editor tools one omp `eval` call used**, as `mcp__<server>_<tool>` names, or `None` for any other tool end.
///
/// omp's `eval` ends with `result.details.cells[*].statusEvents[*]`, each `{op, chars}`; `op` is the name of a tool the
/// cell called, and nothing in it carries an argument or an answer. Only the editor's (`mcp__`) are listed: omp's own
/// tools a cell used are not editor calls.
pub fn eval_ops(event: &Value) -> Option<Vec<String>> {
    let cells = event.get("result")?.get("details")?.get("cells")?.as_array()?;
    Some(
        cells
            .iter()
            .flat_map(|cell| cell.get("statusEvents").and_then(Value::as_array).into_iter().flatten())
            .filter_map(|status| status.get("op").and_then(Value::as_str))
            .filter(|op| op.starts_with("mcp__"))
            .map(str::to_owned)
            .collect(),
    )
}

/// **Settle what the runner could not read from the transcript with what the editor holds** (#434).
///
/// `state` is the editor's `grimoire/state` answer, or why it could not be had.
///
/// - A hand-back the transcript showed is left alone.
/// - None shown and the editor's `presentation` holds one: it is the agent's hand-back (the call is counted if the
///   transcript did not), graded from the editor's words.
/// - None shown, none held, and a `grimoire_present` was made through a route that shows no arguments
///   ([`AgentRun::blind_calls`]): the runner cannot tell what was said, so the trial is a harness error naming
///   "tool calls the transcript did not show". It is never scored "no hand-back was made".
/// - Otherwise the agent made no hand-back, which is its answer.
pub fn reconcile_with_editor(run: &mut AgentRun, state: Result<Value, String>) -> Result<(), String> {
    if !run.handbacks.is_empty() {
        return Ok(());
    }
    let held = state.as_ref().ok().and_then(|s| s.get("presentation")).filter(|p| p.is_object());
    if let Some(presentation) = held {
        run.handbacks.push(handback_from_state(presentation));
        run.handbacks_from_editor = true;
        if !run.tool_calls.iter().any(|c| c == "grimoire_present") {
            run.tool_calls.push("grimoire_present".to_owned());
            run.tool_log.push(CallLog::new("grimoire_present", &Value::Null, false));
        }
        return Ok(());
    }
    if run.blind_calls.iter().any(|c| c == "grimoire_present") {
        let why = match &state {
            Ok(_) => "the editor holds no hand-back".to_owned(),
            Err(e) => format!("the editor could not be asked: {e}"),
        };
        return Err(format!(
            "tool calls the transcript did not show: the agent called `grimoire_present` through a route that shows no \
             arguments, and {why}, so what it handed back is unknown and this trial says nothing about the agent"
        ));
    }
    Ok(())
}

/// **A hand-back as the editor's state holds it**: the title, the summary and the three lists.
fn handback_from_state(presentation: &Value) -> Handback {
    let text = |key: &str| presentation.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let list = |key: &str| -> Vec<String> {
        presentation
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    };
    Handback {
        id: text("id"),
        title: text("title"),
        summary: text("summary"),
        specified: list("specified"),
        inferred: list("inferred"),
        check: list("check"),
    }
}

/// **The line a run says once about the environment variables the omp agent is not given**: names only, never values.
pub fn withheld_line(names: &[std::ffi::OsString]) -> String {
    if names.is_empty() {
        return "cauldron-bench: no environment variable is withheld from the omp agent (none of the runner's looks like a credential)".to_owned();
    }
    let mut listed: Vec<String> = names.iter().map(|n| n.to_string_lossy().into_owned()).collect();
    listed.sort();
    format!("cauldron-bench: withheld from the omp agent's environment ({}): {}", listed.len(), listed.join(", "))
}

/// **The pre-flight turn's tokens as `preflight-usage.json` keeps them.**
pub fn preflight_usage_json(tokens: Tokens) -> Value {
    json!({"input": tokens.input, "output": tokens.output, "cache_read": tokens.cache_read, "cache_write": tokens.cache_write, "total": tokens.total})
}
