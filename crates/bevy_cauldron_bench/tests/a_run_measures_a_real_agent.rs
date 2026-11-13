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
//! - **an outcome that is not the agent's**: an editor that was killed mid-trial must give `harness_error`
//!   with its reason, never `pass` and never `fail`;
//! - **a variant that hides nothing**: `--variant` with `hide_tools: [grimoire_frame_view]` must show the
//!   agent's `tools/list` without that tool, read from the trial's own transcript;
//! - **a transcript that claims more than it has**: every tool call a trial records must be one the editor
//!   or the harness's own fixed set serves.
//!
//! # Why they are `#[ignore]`d, and how to run them
//!
//! Each trial starts a Bevy editor, and the push gate runs the whole workspace suite on a swap-bound host
//! where a second editor beside 24 test threads is what `systemd-oomd` kills. So the proof that needs no
//! editor at all (`every_task_is_valid`) runs in the gate, and these, which are the shape #412 is accepted
//! by, run on demand:
//!
//! ```sh
//! cargo nextest run -p bevy_cauldron_bench -- --ignored --test-threads 1
//! cargo nextest run -p bevy_cauldron_bench -E 'test(a_run_measures_a_real_agent)' -- --ignored
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
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "grimoire-fixture-{}-benchrun-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
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
    let finished = command.output().unwrap_or_else(|e| panic!("cauldron-bench did not start: {e}"));
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

/// **The tool a task's own solution reaches the editor with**, by its task id.
///
/// Not discovered from the file: the three examples are the acceptance, and naming the call each one makes is
/// what makes "the oracle called the editor" a claim about the task rather than about any call at all.
fn suite_answer(task: &str) -> String {
    match task {
        // The placement goes through the tool an agent applies a change with.
        "bed-against-the-north-wall" => "grimoire_apply_operation".to_owned(),
        // Both others change nothing and answer in words.
        _ => "grimoire_present".to_owned(),
    }
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
        EXAMPLES.len(),
        "the oracle's run wrote one trial per example task, at {}:\n{}",
        oracle_dir.display(),
        oracle.quote()
    );
    for record in &passed {
        let task = field(record, "task").as_str().unwrap_or_default().to_owned();
        assert!(
            EXAMPLES.contains(&task.as_str()),
            "the oracle's run measured `{task}`, which is not one of the suite's examples"
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
        EXAMPLES.len(),
        "the do-nothing run wrote one trial per example task, at {}:\n{}",
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

/// **Killing the editor mid-trial gives `harness_error` with its reason**  -  #412's fourth acceptance item.
///
/// The editor under test is a **stand-in**: a shell that writes what it was asked, exits non-zero, and
/// names itself in its log. That is the honest shape of this case: what has to hold is that a trial whose
/// editor never answered `grimoire/state` is a harness error carrying the log's own last line, and a
/// stand-in that dies on purpose is the only way to make that happen at a chosen moment instead of a
/// race against a real editor.
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn an_editor_killed_before_it_answers_is_a_harness_error() {
    let (_, server) = binaries();
    let scratch = scratch("an-editor-killed-before-it-answers");
    std::fs::create_dir_all(&scratch).unwrap_or_else(|e| panic!("{}: {e}", scratch.display()));
    let dying = scratch.join("dying-editor");
    // **It answers `--print-operations`, because that is not the editor starting**: a run asks the binary
    // which operations it has before any trial, and a stand-in that refused that question would fail the
    // run at its door and write no `trial.json` at all — which is a different case from the one here.
    let text = "#!/bin/sh\n\
         case \"$*\" in\n\
         *--print-operations*)\n\
         \x20 printf 'placement_add\\n'\n\
         \x20 exit 0;;\n\
         esac\n\
         # Stands in for the editor under test: it never answers `grimoire/state`, and says why in its own\n\
         # log, which is what a harness error has to quote (BM-02, #412).\n\
         echo 'dying-editor: exiting on purpose, so a trial has no editor' >&2\n\
         exit 7\n";
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

/// **The agent is offered the editor's tools and the harness's own, and nothing from outside**  -  #412's
/// rule that the agent knows only what the Dark Factory gives it, read from the trial's own record.
///
/// The two scripted agents reach the server the way an agent does, so their offered list **is** the
/// server's `tools/list`. Every name must be one of the editor's (`grimoire_*`), because a name from
/// anywhere else is a tool the benchmark did not hand out  -  the failure that made an isolated `HOME`
/// necessary (`docs`: a developer's own `home-still` server was loaded into every trial before it).
#[test]
#[ignore = "starts a Bevy editor per trial; run on demand, not in the gate"]
fn the_agent_is_offered_the_editors_tools_and_nothing_else() {
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
