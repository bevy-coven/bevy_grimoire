//! **`cauldron-bench run` measures a real agent against a real editor, and says which failure is whose**
//! (BM-02, #412).
//!
//! The one shape #412 asks for is a run of a **real editor** and a **real MCP server**, with no model in
//! the loop: the `oracle` agent calls the task's own `solution` through the server a person's agent uses,
//! and the `nothing` agent connects and stops. The oracle must pass every example task and `nothing` must
//! pass none, because a suite where either is untrue is a harness measuring its own plumbing rather than
//! the editor.
//!
//! # Why these tests run the binaries and not the crate's functions
//!
//! `run` owns a process and a socket: it lays a project out, starts `bevy_cauldron` under its own `Xvfb`
//! display, asks `grimoire/agent_setup` for the config a person's agent is handed, drives
//! `bevy_cauldron_mcp` over stdio, stops, and grades what is on disk. A test that called `one_trial` would
//! still be the same code proving itself. These tests run **`cauldron-bench run`** and read the
//! `trial.json` files it writes, which is exactly what a person does at a shell.
//!
//! # The three things a harness can get wrong, and what a test of each looks like
//!
//! - **an outcome that is not the agent's**: an editor that is gone  -  at start-up, or killed while an agent
//!   runs  -  must give `harness_error` with its reason, never `pass` and never `fail`;
//! - **a screen that is not the trial's**: a display held by a live server of another run is never handed
//!   to this one;
//! - **a variant that hides nothing**: `--variant` with `hide_tools: [grimoire_frame_view]` must show the
//!   agent's `tools/list` without that tool, read from the trial's own transcript;
//! - **a tool list that claims more than it has**: what the agent is offered is read from the server, and the
//!   omp spelling and the empty-list cases are proved without a model in
//!   `the_agents_tool_list_is_read_whole.rs`.
//!
//! # Why they are `#[ignore]`d, and how to run them
//!
//! Each trial starts a Bevy editor, and the push gate runs the whole workspace suite on a swap-bound host
//! where a second editor beside 24 test threads is what `systemd-oomd` kills. So the proof that needs no
//! editor at all (`every_task_is_valid`) runs in the gate, and these, which are the shape #412 is accepted
//! by, run on demand:
//!
//! ```sh
//! cargo nextest run -p bevy_cauldron_bench --run-ignored all -E 'binary(a_run_measures_a_real_agent)'
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// **The example tasks the suite ships**, in the order the suite reads them. Spelled out for the reason
/// BM-01's own test spells them out: the acceptance is about these three shapes.
const EXAMPLES: &[&str] = &[
    "bed-against-the-north-wall",
    "count-what-the-bedroom-holds",
    "refuse-a-piece-the-project-lacks",
];

/// **The arm `bench/tasks/variants.yaml` states**, and the tool #412 asks the agent not to be offered.
const ARM: &str = "no-frame-view";
const HIDDEN: &str = "grimoire_frame_view";

/// **A scratch directory this test owns**, named with the test's name and the process id so a rerun and a
/// parallel test never share one.
///
/// **Under cargo's `CARGO_TARGET_TMPDIR`, on disk, and never `/tmp`.** Each trial copies a ~200 MB fixture
/// project, and on the hosts that run this `/tmp` is a RAM-backed 16 GB tmpfs shared with every agent:
/// measured on big, 2026-11-13, this suite's own leftovers (22 directories, 3 GB) filled it, and every
/// later trial was a harness error "No space left on device".
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "grimoire-fixture-{}-benchrun-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// **The environment variable that marks every process of one run**, set on `cauldron-bench` and inherited
/// by the `Xvfb`, the editor, the MCP server and the agent it starts, so what is left of a run can be found
/// by what it carries rather than by a name another test's process may share.
const MARK_VAR: &str = "CAULDRON_BENCH_TEST_MARK";

/// **The processes still alive that carry `mark`**, as `pid comm`.
///
/// Read from `/proc/<pid>/environ`, which this user may read for its own processes. A zombie has no
/// environment and is not counted: it is a child nobody has waited for yet, not one still running.
fn lingering(mark: &str) -> Vec<String> {
    let needle = format!("{MARK_VAR}={mark}");
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return found;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(environ) = std::fs::read(entry.path().join("environ")) else {
            continue;
        };
        if environ.split(|b| *b == 0).any(|var| var == needle.as_bytes()) {
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            found.push(format!("{pid} {}", comm.trim()));
        }
    }
    found
}

/// **The operations the suite's own task files use**, read from `bench/tasks/*/task.yaml` (every `op:` but the
/// hand-back), sorted and without repeats. A stand-in editor answers `--print-operations` with these, so the list is
/// never a second copy that goes stale when a task starts using another operation (#433).
fn suite_operations() -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    let tasks = repo().join("bench/tasks");
    for entry in std::fs::read_dir(&tasks).unwrap_or_else(|e| panic!("{}: {e}", tasks.display())).flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("task.yaml")) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim_start().trim_start_matches("- ");
            if let Some(op) = line.strip_prefix("op:") {
                let op = op.trim().trim_matches('"').trim_matches('\'');
                if !op.is_empty() && op != "grimoire_present" {
                    found.insert(op.to_owned());
                }
            }
        }
    }
    assert!(found.contains("placement_remove"), "the suite's task files name `placement_remove`, so the scan found them: {found:?}");
    found.into_iter().collect()
}

/// **The repository root**, which is what a run measures by default.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **The `cauldron-bench` binary under test.**
fn bench_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cauldron-bench"))
}

/// **The editor and the MCP server beside it**, which is where `cauldron-bench` looks for them by default.
///
/// **They are built here, because nothing else builds them for this crate.** `bevy_cauldron` is a
/// dev-dependency of this crate, and a dev-dependency gives its *library*: `cargo nextest run -p
/// bevy_cauldron_bench` never links `bevy_cauldron`'s or `bevy_cauldron_mcp`'s binary, so a run from a
/// warm test binary would find nothing beside it. The build is idempotent (a warm tree answers "Finished"
/// without compiling) and it names the two packages a person would type, so the harness runs the binaries
/// of the commit under test rather than whatever a previous build left in the target directory.
fn binaries() -> (PathBuf, PathBuf) {
    let dir = bench_bin().parent().expect("the bench binary has a directory").to_path_buf();
    let built = Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "bevy_cauldron", "-p", "bevy_cauldron_mcp"])
        .current_dir(repo())
        .output()
        .unwrap_or_else(|e| panic!("cargo could not be run: {e}"));
    assert!(
        built.status.success(),
        "the editor and the server did not build, and a run needs both beside {}:\n{}{}",
        bench_bin().display(),
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let editor = dir.join("bevy_cauldron");
    let server = dir.join("bevy_cauldron_mcp");
    for path in [&editor, &server] {
        assert!(
            path.is_file(),
            "{} is not beside {} after a build of both, which is where `cauldron-bench` looks",
            path.display(),
            bench_bin().display()
        );
    }
    (editor, server)
}

/// **One `cauldron-bench run`, and what it printed.**
struct Ran {
    code: Option<i32>,
    said: String,
}

impl Ran {
    /// The text a failure quotes: both streams, so a refusal is never only "it failed".
    fn quote(&self) -> String {
        let lines: Vec<&str> = self.said.lines().filter(|l| !l.trim().is_empty()).collect();
        let from = lines.len().saturating_sub(12);
        lines[from..].join("\n")
    }
}

/// **Run every task of the shipped suite, once each, on the named agent**, writing under `run`.
fn run(agent: &str, run: &str, extra: &[&str]) -> (Ran, PathBuf) {
    let (editor, server) = binaries();
    let out = scratch(run);
    let mut command = Command::new(bench_bin());
    command
        .args(["run", "--agent", agent, "--run", run, "--trials", "1"])
        .arg("--editor")
        .arg(&editor)
        .arg("--server")
        .arg(&server)
        .args(extra)
        .current_dir(repo());
    // **The run writes under the state directory this test's own output is under**, so a person reading a
    // failure finds the trials beside the scratch directory rather than in their real state.
    command.env("HOME", &out);
    // **Everything this run starts carries the mark**, so the check below finds an `Xvfb`, an editor, a
    // server or an agent that outlived its trial, and finds nothing of any other test's.
    let mark = format!("{run}-{}", std::process::id());
    command.env(MARK_VAR, &mark);
    let finished = command.output().unwrap_or_else(|e| panic!("cauldron-bench did not start: {e}"));
    let left = lingering(&mark);
    assert!(
        left.is_empty(),
        "`run` ended and left processes of its own running (an Xvfb here is what held the host lock for 13 \
         minutes): {left:?}"
    );
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&finished.stdout),
        String::from_utf8_lossy(&finished.stderr)
    );
    let ran = Ran { code: finished.status.code(), said };
    (ran, out.join(".local/state/cauldron-bench").join(run))
}

/// **Every trial record a run wrote**, by task id and trial number.
fn records(dir: &Path) -> Vec<Value> {
    let mut found = Vec::new();
    let Ok(tasks) = std::fs::read_dir(dir) else {
        return found;
    };
    for task in tasks.flatten() {
        let Ok(trials) = std::fs::read_dir(task.path()) else {
            continue;
        };
        for trial in trials.flatten() {
            let file = trial.path().join("trial.json");
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let value: Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{} is not JSON: {e}", file.display()));
            found.push(value);
        }
    }
    found.sort_by(|a, b| {
        let key = |v: &Value| {
            (
                v.get("task").and_then(Value::as_str).unwrap_or_default().to_owned(),
                v.get("trial").and_then(Value::as_u64).unwrap_or_default(),
            )
        };
        key(a).cmp(&key(b))
    });
    found
}

/// **What the server listed for one task's trial**, from the record the run wrote.
fn offered(dir: &Path, ran: &Ran, task: &str) -> Vec<String> {
    let written = records(dir);
    let record = written
        .iter()
        .find(|r| r.get("task").and_then(Value::as_str) == Some(task))
        .unwrap_or_else(|| {
            panic!(
                "the run wrote no `{task}` trial at {}:\n{}",
                dir.display(),
                ran.quote()
            )
        });
    let list: Vec<String> = field(record, "tools_offered")
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.as_str().map(str::to_owned))
        .collect();
    assert!(
        !list.is_empty(),
        "the `{task}` trial was offered no tools at all, so a missing tool would prove nothing:\n{}",
        ran.quote()
    );
    list
}

/// **The tool a task's own solution reaches the editor with**: `grimoire_apply_operation` when the solution applies an
/// operation, `grimoire_present` when it only answers in words. Read from the task's `solution:` block (#433): the
/// suite grew from the three examples to twenty tasks, and a list of names written here went stale with it.
fn suite_answer(task: &str) -> String {
    let path = repo().join("bench/tasks").join(task).join("task.yaml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut in_solution = false;
    for line in text.lines() {
        if line.starts_with("solution:") {
            in_solution = true;
        } else if in_solution && !line.starts_with(' ') && !line.starts_with('-') && !line.starts_with('#') && !line.trim().is_empty() {
            break;
        } else if in_solution && line.trim_start().trim_start_matches("- ").starts_with("op:") {
            let op = line.trim_start().trim_start_matches("- ").trim_start_matches("op:").trim();
            if op != "grimoire_present" {
                return "grimoire_apply_operation".to_owned();
            }
        }
    }
    "grimoire_present".to_owned()
}

/// **Every dev task of the shipped suite**, by id (the directory's name), sorted: what the oracle must pass and
/// `nothing` must fail.
fn suite_tasks() -> Vec<String> {
    let tasks = repo().join("bench/tasks");
    let mut ids: Vec<String> = std::fs::read_dir(&tasks)
        .unwrap_or_else(|e| panic!("{}: {e}", tasks.display()))
        .flatten()
        .filter(|e| std::fs::read_to_string(e.path().join("task.yaml")).is_ok_and(|t| t.lines().any(|l| l.trim() == "split: dev")))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

/// **One field of a record**, with the record quoted when it is missing: a `trial.json` that lost a field
/// is the failure this whole slice is about.
fn field<'a>(record: &'a Value, name: &str) -> &'a Value {
    record.get(name).unwrap_or_else(|| {
        panic!(
            "a trial record has no `{name}`: {}",
            serde_json::to_string_pretty(record).unwrap_or_default()
        )
    })
}

/// **The word a record gives for its outcome.**
fn outcome(record: &Value) -> &str {
    field(record, "outcome").as_str().unwrap_or("(not a string)")
}

/// **The oracle passes every example task, and `nothing` passes none, each through a real editor and the
/// real MCP server**  -  #412's first acceptance item.
///
/// This is the one shape the slice is accepted by, and it needs no model: the oracle calls the task's own
/// `solution` through the server an agent's client speaks to, and `nothing` connects, reads `tools/list`
/// and stops. A run where the oracle fails is a pipe that does not carry an agent's call to the editor; a
/// run where `nothing` passes is a suite that scores doing nothing as success.
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn the_oracle_passes_every_task_and_nothing_passes_none() {
    let (oracle, oracle_dir) = run("oracle", "the-oracle-passes-every-task", &[]);
    assert_eq!(
        oracle.code,
        Some(0),
        "`run --agent oracle` must exit 0, because every one of its checks holds:\n{}",
        oracle.quote()
    );
    let passed = records(&oracle_dir);
    assert_eq!(
        passed.len(),
        suite_tasks().len(),
        "the oracle's run wrote one trial per dev task of the suite, at {}:\n{}",
        oracle_dir.display(),
        oracle.quote()
    );
    for record in &passed {
        let task = field(record, "task").as_str().unwrap_or_default().to_owned();
        assert!(
            suite_tasks().contains(&task),
            "the oracle's run measured `{task}`, which is not one of the suite's dev tasks"
        );
        assert_eq!(
            outcome(record),
            "pass",
            "the oracle must pass `{task}`: its own solution, called through the real server, has to \
             satisfy every check. The record says:\n{}",
            serde_json::to_string_pretty(record).unwrap_or_default()
        );
        // **The pipe, not only the answer**: every example task has at least one step, so a record with no
        // call at all is a grader that passed whatever was already on disk.
        let calls: Vec<String> = field(record, "tool_calls")
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c.as_str().map(str::to_owned))
            .collect();
        assert!(
            !calls.is_empty(),
            "the oracle's `{task}` trial records no tool call, so nothing was asked of the editor"
        );
        // **And the step the task names is one of them**: a record that counted something else would pass
        // this while the task's own answer never reached the editor.
        let answer = suite_answer(&task);
        assert!(
            calls.iter().any(|c| c == &answer),
            "the oracle's `{task}` trial records {calls:?}, which does not hold `{}`  -  the call the task's \
             own solution makes",
            answer
        );
        // **A real editor**: the server answered `tools/list`, which it can only do by reaching the editor's
        // own config, and the hand-back is in the record the grader read.
        let offered = field(record, "tools_offered").as_array().cloned().unwrap_or_default();
        assert!(
            offered.len() > 1,
            "the oracle's `{task}` trial was offered {} tool(s), so the server never listed them",
            offered.len()
        );
    }

    let (nothing, nothing_dir) = run("nothing", "nothing-passes-none", &[]);
    assert_ne!(
        nothing.code,
        Some(0),
        "`run --agent nothing` must exit non-zero, because a trial that connects and stops passes no task:\n{}",
        nothing.quote()
    );
    let failed = records(&nothing_dir);
    assert_eq!(
        failed.len(),
        suite_tasks().len(),
        "the do-nothing run wrote one trial per dev task of the suite, at {}:\n{}",
        nothing_dir.display(),
        nothing.quote()
    );
    for record in &failed {
        let task = field(record, "task").as_str().unwrap_or_default().to_owned();
        assert_eq!(
            outcome(record),
            "fail",
            "`nothing` must FAIL `{task}` rather than pass it or call the harness wrong: an agent that \
             connects and stops is the do-nothing loophole, and a task it passes measures nothing. The \
             record says:\n{}",
            serde_json::to_string_pretty(record).unwrap_or_default()
        );
        let checks = field(record, "checks").as_array().cloned().unwrap_or_default();
        let unmet: Vec<String> = checks
            .iter()
            .filter(|c| c.get("passed").and_then(Value::as_bool) != Some(true))
            .map(|c| c.get("check").and_then(Value::as_str).unwrap_or("(unnamed)").to_owned())
            .collect();
        assert!(
            !unmet.is_empty(),
            "`nothing` FAILED `{task}` without naming a check that did not hold, so the outcome is not \
             the checks' answer"
        );
    }
}

/// **An editor that exits before it answers `grimoire/state` is a `harness_error` with its reason.**
///
/// This is the start-up half of #412's fourth acceptance item; the mid-trial kill is
/// `a_real_editor_killed_while_an_agent_runs_is_a_harness_error`. The editor under test is a **stand-in**: a shell that writes what it was asked, exits non-zero, and
/// names itself in its log. That is the honest shape of this case: what has to hold is that a trial whose
/// editor never answered `grimoire/state` is a harness error carrying the log's own last line, and a
/// stand-in that dies on purpose is the only way to make that happen at a chosen moment instead of a
/// race against a real editor.
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn an_editor_that_exits_at_start_up_is_a_harness_error() {
    let (_, server) = binaries();
    let scratch = scratch("an-editor-that-exits-at-start-up");
    std::fs::create_dir_all(&scratch).unwrap_or_else(|e| panic!("{}: {e}", scratch.display()));
    let dying = scratch.join("dying-editor");
    // **It answers `--print-operations`, because that is not the editor starting**: a run asks the binary
    // which operations it has before any trial, and a stand-in that refused that question would fail the
    // run at its door and write no `trial.json` at all — which is a different case from the one here.
    // **The operations are the ones the suite's own task files use** ([`suite_operations`]), never a list
    // written here: a hard-coded `placement_add` went stale the day a task needed `placement_remove`, and the
    // run was refused at its door (#433).
    let operations = scratch.join("operations.txt");
    std::fs::write(&operations, suite_operations().join("\n") + "\n").unwrap_or_else(|e| panic!("{}: {e}", operations.display()));
    let text = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
         *--print-operations*)\n\
         \x20 cat '{}'\n\
         \x20 exit 0;;\n\
         esac\n\
         # Stands in for the editor under test: it never answers `grimoire/state`, and says why in its own\n\
         # log, which is what a harness error has to quote (BM-02, #412).\n\
         echo 'dying-editor: exiting on purpose, so a trial has no editor' >&2\n\
         exit 7\n",
        operations.display()
    );
    std::fs::write(&dying, text).unwrap_or_else(|e| panic!("{}: {e}", dying.display()));
    let made = Command::new("chmod")
        .arg("+x")
        .arg(&dying)
        .status()
        .unwrap_or_else(|e| panic!("chmod: {e}"));
    assert!(made.success(), "the stand-in editor is made executable");

    let out = scratch.join("home");
    let finished = Command::new(bench_bin())
        .args(["run", "--agent", "oracle", "--run", "the-dying-editor", "--trials", "1"])
        .args(["--task", EXAMPLES[0]])
        .arg("--editor")
        .arg(&dying)
        .arg("--server")
        .arg(&server)
        .current_dir(repo())
        .env("HOME", &out)
        .output()
        .unwrap_or_else(|e| panic!("cauldron-bench did not start: {e}"));
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&finished.stdout),
        String::from_utf8_lossy(&finished.stderr)
    );
    assert_ne!(
        finished.status.code(),
        Some(0),
        "a run of one trial whose editor died must exit non-zero:\n{said}"
    );
    let dir = out.join(".local/state/cauldron-bench/the-dying-editor");
    let written = records(&dir);
    assert_eq!(
        written.len(),
        1,
        "the dying editor's run still wrote its trial's record, at {}:\n{said}",
        dir.display()
    );
    let record = &written[0];
    assert_eq!(
        outcome(record),
        "harness_error",
        "an editor that exited before `grimoire/state` answered is a HARNESS ERROR  -  never `pass`, and \
         never `fail`, because neither is a fact about the agent. The record says:\n{}",
        serde_json::to_string_pretty(record).unwrap_or_default()
    );
    let reason = field(record, "reason").as_str().unwrap_or_default();
    assert!(
        reason.contains("exited"),
        "the harness error names what happened: the editor exited.\nThe reason was: {reason}"
    );
    assert!(
        reason.contains("dying-editor"),
        "the harness error quotes the editor's own log, which is the fix a person needs.\nThe reason was: \
         {reason}"
    );
}

/// **The MCP server serves the agent only the editor's tools.**
///
/// A scripted agent reaches the server the way a harness does, so its offered list **is** the server's
/// `tools/list`, and every name must be one of the editor's (`grimoire_*`). **This does not test omp's
/// isolated `HOME`**  -  no model runs here  -  which is enforced where the omp list is read:
/// `parse_tools_offered` refuses a name outside the editor's and the harness's own set, proved in
/// `the_agents_tool_list_is_read_whole.rs`.
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn the_server_serves_the_agent_only_the_editors_tools() {
    let (ran, dir) = run("oracle", "the-tools-it-is-offered", &["--task", EXAMPLES[0]]);
    let written = records(&dir);
    assert_eq!(
        written.len(),
        1,
        "the run wrote one trial, at {}:\n{}",
        dir.display(),
        ran.quote()
    );
    let offered: Vec<String> = field(&written[0], "tools_offered")
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.as_str().map(str::to_owned))
        .collect();
    assert!(
        !offered.is_empty(),
        "the trial was offered no tools, so this would say nothing about isolation:\n{}",
        ran.quote()
    );
    let outside: Vec<&String> = offered.iter().filter(|t| !t.starts_with("grimoire_")).collect();
    assert!(
        outside.is_empty(),
        "the agent was offered {outside:?}, which the editor does not serve: a tool from outside the \
         Dark Factory's set is a harness error rather than a result about the agent. The whole list was: {}",
        offered.join(", ")
    );
}

/// **`--variant` with `hide_tools` leaves that tool, and only that tool, out of the agent's
/// `tools/list`**  -  #412's third acceptance item.
///
/// **The evidence is the server's own `tools/list`, asked twice.** The scripted agent's `tools_offered` is
/// what the MCP server answered, so the measurement is the same task and the same agent run twice: once
/// bare and once under the arm. The arm's list must be the bare list **minus exactly one name**, which is
/// what makes this a test of the arm rather than of a tool that happened not to be listed either way  -  a
/// bare list and an arm list that both lacked `grimoire_frame_view` would pass a test that only looked for
/// its absence.
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn a_variant_that_hides_a_tool_removes_it_from_the_tool_list() {
    let without = run("nothing", "the-tool-list-bare", &["--task", EXAMPLES[1]]);
    let with = run(
        "nothing",
        "the-hidden-tool",
        &["--variant", ARM, "--task", EXAMPLES[1]],
    );
    let bare = offered(&without.1, &without.0, EXAMPLES[1]);
    assert!(
        bare.iter().any(|t| t == HIDDEN),
        "the bare run's `tools/list` holds `{HIDDEN}`, so its absence under the arm means something. \
         The bare list was: {}",
        bare.join(", ")
    );
    let armed = records(&with.1);
    assert_eq!(
        armed.len(),
        1,
        "the arm's run wrote one trial, at {}:\n{}",
        with.1.display(),
        with.0.quote()
    );
    assert_eq!(
        field(&armed[0], "variant").as_str(),
        Some(ARM),
        "the record names the arm it was run as"
    );
    let arm = offered(&with.1, &with.0, EXAMPLES[1]);
    assert!(
        !arm.iter().any(|t| t == HIDDEN),
        "the agent's `tools/list` still holds `{HIDDEN}` under `--variant {ARM}`, so the arm hid \
         nothing. The list was: {}",
        arm.join(", ")
    );
    // **Exactly one name gone**, which is the difference between an arm and a different server.
    let expected: Vec<&String> = bare.iter().filter(|t| t.as_str() != HIDDEN).collect();
    let got: Vec<&String> = arm.iter().collect();
    assert_eq!(
        got, expected,
        "the arm's list is the bare list less `{HIDDEN}` and nothing else: `--hide-tools` removes one \
         tool, so any other difference is a second change wearing the arm's name"
    );
}

/// **A server this test starts and always stops**: a panic in the middle of the test unwinds through this and
/// kills the `Xvfb`, which a bare `Child` does not. The first version of
/// `a_display_a_live_server_holds_is_never_handed_to_another_trial` panicked on the old code with its foreign
/// `Xvfb` still running, and that orphan held the host lock for 13 minutes.
struct Foreign(std::process::Child);

impl Drop for Foreign {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// **Whether a process is alive**, by `/proc`.
fn alive_pid(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// **The pid an X server's lock file names**, when there is one.
fn lock_pid(number: u32) -> Option<u32> {
    std::fs::read_to_string(format!("/tmp/.X{number}-lock")).ok()?.trim().parse().ok()
}

/// **A display a live server holds is never handed to a second trial**  -  the screen of a concurrent run,
/// or of another agent's `Xvfb` inside the ladder  -  **and every display this test started is stopped.**
///
/// The ladder used to return on seeing the socket file, and a live server's socket exists at once: two runs
/// took one screen, and the first to stop took it from the other. Here one display is held, a foreign `Xvfb`
/// (not started by this crate) holds the next free number, and a third display must be neither, with both
/// still alive afterwards. When the displays are dropped their servers are gone, waited for, not left to
/// hold what the caller holds.
#[test]
#[ignore = "starts Xvfb servers; run on demand, not in the gate"]
fn a_display_a_live_server_holds_is_never_handed_to_another_trial() {
    use bevy_cauldron_bench::run::{detach, Display, DISPLAY_BASE, DISPLAY_TRIES};
    let first = Display::start().unwrap_or_else(|e| panic!("a first display: {e}"));
    let first_number = first.number;
    let first_pid = lock_pid(first_number).unwrap_or_else(|| panic!("the first display :{first_number} has no lock"));
    assert!(alive_pid(first_pid), "the first display's server is running");

    // **The foreign server**: a plain `Xvfb`, on the first number above ours that it can take. It is held in
    // a guard, so a failure below cannot leave it running.
    let mut foreign: Option<(u32, Foreign)> = None;
    for number in (first_number + 1)..(DISPLAY_BASE + DISPLAY_TRIES) {
        let mut xvfb = Command::new("Xvfb");
        xvfb.arg(format!(":{number}"))
            .args(["-screen", "0", "800x600x24", "-nolisten", "tcp"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut child =
            Foreign(detach(&mut xvfb).spawn().unwrap_or_else(|e| panic!("Xvfb could not be started: {e}")));
        // Held when its lock file names it, which is the proof the ladder uses.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut held = false;
        while std::time::Instant::now() < deadline {
            if lock_pid(number) == Some(child.0.id()) && Path::new(&format!("/tmp/.X11-unix/X{number}")).exists() {
                held = true;
                break;
            }
            if matches!(child.0.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if held {
            foreign = Some((number, child));
            break;
        }
    }
    let (foreign_number, foreign_server) =
        foreign.unwrap_or_else(|| panic!("no number above :{first_number} could be taken by a foreign Xvfb"));
    let foreign_pid = foreign_server.0.id();

    let third = Display::start().unwrap_or_else(|e| panic!("a third display: {e}"));
    let third_number = third.number;
    let third_pid = lock_pid(third_number).unwrap_or_else(|| panic!("the third display :{third_number} has no lock"));
    assert_ne!(
        third_number, first_number,
        "the second trial was handed :{first_number}, which a live server of the first holds"
    );
    assert_ne!(
        third_number, foreign_number,
        "the second trial was handed :{foreign_number}, which a foreign live Xvfb holds"
    );
    assert!(alive_pid(first_pid), "the first display's server still runs");
    assert!(alive_pid(foreign_pid), "the foreign server still runs");
    assert!(alive_pid(third_pid), "the third display's server runs");

    // **Stopped on drop**: both servers this crate started are gone and waited for, and the foreign one,
    // which this crate did not start, is untouched.
    drop(first);
    drop(third);
    assert!(!alive_pid(first_pid), "the first display's Xvfb (pid {first_pid}) outlived its Display");
    assert!(!alive_pid(third_pid), "the third display's Xvfb (pid {third_pid}) outlived its Display");
    assert!(alive_pid(foreign_pid), "a display this crate did not start is left alone");
    drop(foreign_server);
    assert!(!alive_pid(foreign_pid), "the test's own foreign server is stopped");
}

/// **The pid of the process whose argv starts `program project`**, found by reading `/proc`, so a test
/// kills exactly the editor of one trial and never a process whose arguments merely mention its path.
fn editor_pid(program: &Path, project: &Path) -> Option<u32> {
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let mut argv = raw.split(|b| *b == 0).map(|a| String::from_utf8_lossy(a).into_owned());
        if argv.next().as_deref() == Some(&program.to_string_lossy())
            && argv.next().as_deref() == Some(&project.to_string_lossy())
        {
            return Some(pid);
        }
    }
    None
}

/// **How long the run is given to report a killed editor**, seconds. The task's budget is 600 s, so a run
/// that waited for the budget would take ten times this.
const REPORT_KILLED_EDITOR_S: u64 = 30;

/// **How long the editor is given to start and the stand-in agent to be launched**, seconds.
const AGENT_LAUNCH_S: u64 = 300;

/// **How long a harness's children are given to go once the harness is killed outright**, seconds.
const ORPHANS_GO_S: u64 = 20;

/// **A `cauldron-bench run` on a REAL editor whose agent is a stand-in `omp` that waits for the test**, and
/// the means to act on it once the editor has answered.
struct Standing {
    child: std::process::Child,
    home: PathBuf,
    editor: PathBuf,
    mark: String,
    run_name: String,
}

impl Standing {
    /// The trial's project, which an editor's argv names.
    fn project(&self) -> PathBuf {
        self.home
            .join(".local/state/cauldron-bench")
            .join(&self.run_name)
            .join(EXAMPLES[0])
            .join("trial-1/project")
    }

    /// **The real editor of the trial**, by `/proc`.
    fn editor_pid(&self) -> u32 {
        let project = self.project();
        editor_pid(&self.editor, &project).unwrap_or_else(|| {
            panic!("no running editor was found for {}: is `bevy_cauldron` at {}?", project.display(), self.editor.display())
        })
    }
}

impl Drop for Standing {
    /// **A test that fails still stops the run it started**, and everything of its own that it left.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for entry in lingering(&self.mark) {
            if let Some(pid) = entry.split_whitespace().next() {
                let _ = Command::new("kill").args(["-KILL", pid]).status();
            }
        }
    }
}

/// **Start the run and wait until the stand-in agent is launched**  -  which is after the real editor's
/// `grimoire/state` answered and its `grimoire/agent_setup` was served, so the editor is up and the trial is
/// in the middle of the agent's turn.
///
/// The stand-in `omp` answers the harness's questions (`models find`, `auth export`, `auth import`), drops
/// a marker when it is launched and then **becomes** `sleep` (`exec`), so the one process the harness
/// started is the one that waits, and nothing is left behind if it is stopped.
fn run_with_stand_in_agent(test: &str, run_name: &str) -> Standing {
    let (editor, server) = binaries();
    let scratch = scratch(test);
    let bin = scratch.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("{}: {e}", bin.display()));
    let marker = scratch.join("agent-launched");
    let omp = bin.join("omp");
    let script = format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
         --version) echo 'omp/0.0.0-stand-in'; exit 0;;\n\
         models) echo \"$3\"; exit 0;;\n\
         auth)\n\
         \x20 case \"$2\" in\n\
         \x20 export) echo '{{\"version\":1,\"credentials\":[{{\"provider\":\"stand-in\"}}]}}'; exit 0;;\n\
         \x20 import) exit 0;;\n\
         \x20 esac;;\n\
         esac\n\
         # The pre-flight turn (one word, no extension): the provider answers.\n\
         case \"$*\" in\n\
         *'reply ok'*) echo '{{\"type\":\"message_end\",\"message\":{{\"role\":\"assistant\",\"content\":[{{\"type\":\"text\",\"text\":\"ok\"}}],\"stopReason\":\"stop\"}}}}'; exit 0;;\n\
         esac\n\
         # The agent run: launched once the editor answered, so the editor is up. It waits for the test.\n\
         : > '{}'\n\
         exec sleep 600\n",
        marker.display()
    );
    std::fs::write(&omp, script).unwrap_or_else(|e| panic!("{}: {e}", omp.display()));
    let made = Command::new("chmod").arg("+x").arg(&omp).status().unwrap_or_else(|e| panic!("chmod: {e}"));
    assert!(made.success(), "the stand-in agent is made executable");

    let home = scratch.join("home");
    let mark = format!("{test}-{}", std::process::id());
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let child = Command::new(bench_bin())
        .args(["run", "--agent", "omp", "--model", "stand-in/model", "--run", run_name, "--trials", "1", "--task", EXAMPLES[0]])
        .arg("--editor")
        .arg(&editor)
        .arg("--server")
        .arg(&server)
        .current_dir(repo())
        .env("HOME", &home)
        .env("PATH", path)
        .env(MARK_VAR, &mark)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cauldron-bench did not start: {e}"));
    let mut standing = Standing { child, home, editor, mark, run_name: run_name.to_owned() };

    let launched = std::time::Instant::now();
    while !marker.exists() {
        assert!(
            launched.elapsed() < std::time::Duration::from_secs(AGENT_LAUNCH_S),
            "the stand-in agent was not launched within {AGENT_LAUNCH_S} s, so the editor never answered"
        );
        if let Ok(Some(status)) = standing.child.try_wait() {
            let mut said = String::new();
            for stream in [
                standing.child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read>),
                standing.child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read>),
            ]
            .into_iter()
            .flatten()
            {
                let mut stream = stream;
                let _ = std::io::Read::read_to_string(&mut stream, &mut said);
            }
            panic!("cauldron-bench ended ({status}) before the agent was launched:\n{said}");
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    standing
}

/// **A REAL editor killed after its state answered, while an agent runs, is a `harness_error` with the
/// reason, and nothing of the run is left running**  -  #412's fourth acceptance item, and the lifecycle
/// rule that the display is stopped on this exit path too.
///
/// The test kills the real editor with `SIGKILL` once the stand-in agent is launched. The run must say so
/// within [`REPORT_KILLED_EDITOR_S`] seconds rather than at the task's 600 s budget, must not grade the
/// files the dead editor left, and must leave no `Xvfb`, editor, server or agent running.
#[test]
#[ignore = "starts a Bevy editor; run on demand, not in the gate"]
fn a_real_editor_killed_while_an_agent_runs_is_a_harness_error() {
    let mut run = run_with_stand_in_agent("a-real-editor-killed-while-an-agent-runs", "the-killed-editor");
    let pid = run.editor_pid();
    let killed = Command::new("kill").args(["-KILL", &pid.to_string()]).status().unwrap_or_else(|e| panic!("kill: {e}"));
    assert!(killed.success(), "the real editor (pid {pid}) is killed");
    let killed_at = std::time::Instant::now();

    let finished = loop {
        if let Ok(Some(status)) = run.child.try_wait() {
            break status;
        }
        assert!(
            killed_at.elapsed() < std::time::Duration::from_secs(REPORT_KILLED_EDITOR_S),
            "the run did not report the killed editor within {REPORT_KILLED_EDITOR_S} s: it waits for the \
             agent's budget while the editor is dead"
        );
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    assert_ne!(finished.code(), Some(0), "a run whose editor was killed exits non-zero");
    let written = records(&run.home.join(".local/state/cauldron-bench/the-killed-editor"));
    assert_eq!(written.len(), 1, "the killed trial still wrote its record");
    let record = &written[0];
    assert_eq!(
        outcome(record),
        "harness_error",
        "an editor killed while the agent ran is a HARNESS ERROR, never pass and never fail. The record says:\n{}",
        serde_json::to_string_pretty(record).unwrap_or_default()
    );
    let reason = field(record, "reason").as_str().unwrap_or_default();
    assert!(
        reason.contains("the editor exited with") && reason.contains("during the trial"),
        "the reason names the editor's exit: {reason}"
    );
    assert!(reason.contains("Its log ends"), "the reason quotes the editor's own log: {reason}");
    let checks = field(record, "checks").as_array().cloned().unwrap_or_default();
    assert!(checks.is_empty(), "a trial with no editor is not graded, but it records {} check(s)", checks.len());
    let left = lingering(&run.mark);
    assert!(
        left.is_empty(),
        "the run reported the dead editor and left processes of its own running (the Xvfb here held the \
         host lock for 13 minutes): {left:?}"
    );
}

/// **A harness killed outright leaves no child running**  -  the exit `Drop` cannot cover.
///
/// `cauldron-bench` is `SIGKILL`ed in the middle of a trial, with a real editor, its `Xvfb` and the agent
/// up. Nothing it owns may outlive it: each child asked the kernel for `SIGTERM` when its parent dies
/// (`detach`), so none is left holding the host lock or a display.
#[test]
#[ignore = "starts a Bevy editor; run on demand, not in the gate"]
fn a_harness_killed_outright_leaves_no_child_running() {
    let mut run = run_with_stand_in_agent("a-harness-killed-outright", "the-killed-harness");
    let before = lingering(&run.mark);
    assert!(
        before.iter().any(|p| p.ends_with("Xvfb")) && before.iter().any(|p| p.ends_with("bevy_cauldron")),
        "the trial is up: an Xvfb and an editor carry the run's mark (found {before:?})"
    );
    let harness = run.child.id().to_string();
    let killed = Command::new("kill").args(["-KILL", &harness]).status().unwrap_or_else(|e| panic!("kill: {e}"));
    assert!(killed.success(), "the harness (pid {harness}) is killed");
    let _ = run.child.wait();

    let waited = std::time::Instant::now();
    let left = loop {
        let left = lingering(&run.mark);
        if left.is_empty() || waited.elapsed() >= std::time::Duration::from_secs(ORPHANS_GO_S) {
            break left;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    assert!(
        left.is_empty(),
        "{ORPHANS_GO_S} s after its harness was killed, these of the run were still running (an Xvfb here \
         holds a host lock it inherited): {left:?}"
    );
}
