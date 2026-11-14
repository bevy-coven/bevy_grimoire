//! **SIGINT and SIGTERM stop a run through its guards** (#433, item 3b).
//!
//! Rust's default SIGINT and SIGTERM end the process without running `Drop`, so a Ctrl-C left every credential guard
//! (#432) undone. The runner now installs a handler that only sets a flag: workers take no further trial, a waiting
//! agent's process group is killed, and the run unwinds normally, so every guard runs. One test, because the flag is
//! the process's and latches.
#![cfg(unix)]

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy_cauldron::ops;
use bevy_cauldron_bench::run::{
    detach, install_interrupt_handler, interrupted, preflight_turn, run_work, start_editor, wait_within, write_reports, Agent, Options,
};
use bevy_cauldron_bench::{Outcome, Task, TrialRecord, TASKS_DIR};

fn task() -> Task {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("{e}"));
    let known: Vec<&str> = ops::registry().iter().map(|op| op.name()).collect();
    Task::load(&repo.join(TASKS_DIR).join("refuse-a-piece-the-project-lacks"), &repo, &known).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn record(task: &str, trial: u32, reason: &str) -> TrialRecord {
    TrialRecord {
        task: task.to_owned(),
        trial,
        agent: "omp".to_owned(),
        model: None,
        variant: None,
        outcome: Outcome::Pass,
        reason: reason.to_owned(),
        checks: Vec::new(),
        tool_calls: Vec::new(),
        tool_log: Vec::new(),
        tokens: None,
        cost_reported_usd: None,
        edits_behind: Vec::new(),
        wall_s: 0.0,
        tools_offered: Vec::new(),
        wishes: Vec::new(),
        call_budget_exhausted: false,
        dir: std::path::PathBuf::new(),
        held_out: false,
    }
}

#[test]
fn a_signal_stops_the_run_and_a_waiting_agent_without_skipping_a_guard() {
    assert!(!interrupted(), "nothing has interrupted this process yet");
    install_interrupt_handler();
    let task = task();
    let work: Vec<_> = (1..=6u32).map(|n| (task.clone(), task.id.clone(), n)).collect();
    let started = AtomicUsize::new(0);

    // A SIGTERM arrives in the middle of the third trial.
    let records = run_work(
        &work,
        1,
        &|t, _, n| {
            started.fetch_add(1, Ordering::SeqCst);
            if n == 3 {
                // SAFETY: raising a signal in this process, whose handler only stores a flag.
                unsafe { libc::raise(libc::SIGTERM) };
            }
            record(&t.id, n, "")
        },
        &|t, _, n, why| {
            let mut r = record(&t.id, n, &why);
            r.outcome = Outcome::HarnessError;
            r
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(interrupted(), "the handler recorded the signal and the process is still here");
    assert_eq!(started.load(Ordering::SeqCst), 3, "no trial starts after the signal");
    assert_eq!(records.len(), 6, "every trial has a record");
    let skipped: Vec<_> = records.iter().filter(|r| r.outcome == Outcome::HarnessError).collect();
    assert_eq!(skipped.len(), 3);
    assert!(skipped.iter().all(|r| r.reason.contains("interrupted")), "and says why: {:?}", skipped.iter().map(|r| &r.reason).collect::<Vec<_>>());

    // And a waiting agent is stopped, as an error naming the interrupt, with its group dead.
    let mut command = Command::new("sleep");
    command.arg("300").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let child = detach(&mut command).spawn().unwrap_or_else(|e| panic!("{e}"));
    let pid = child.id();
    let said = wait_within(child, 600, "stand-in", None, || None).expect_err("an interrupted wait is an error");
    assert!(said.contains("interrupted"), "{said}");
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists() || std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| s.contains(") Z")) , "the agent is stopped");

    // **The handler fired once and is gone**: a second Ctrl-C must kill a run that is stuck, so the disposition is back to
    // the default (`signal` answers the one it replaced).
    // SAFETY: setting a disposition in this process.
    let replaced = unsafe { libc::signal(libc::SIGTERM, libc::SIG_DFL) };
    assert_eq!(replaced, libc::SIG_DFL, "the handler reset itself after the first signal");

    // **An interrupt in the pre-flight turn does not wait out its timeout**: the stand-in `omp` would sleep five minutes.
    let dir = std::env::temp_dir().join(format!("grimoire-bench-interrupt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap_or_else(|e| panic!("{e}"));
    let omp = dir.join("bin/omp");
    std::fs::write(
        &omp,
        "#!/bin/sh\ncase \"$1 $2\" in\n \"auth export\") echo '{\"version\":1,\"credentials\":[{\"provider\":\"anthropic\",\"credential\":{\"k\":\"x\"}}]}'; exit 0;;\n \"auth import\") exit 0;;\nesac\nexec sleep 300\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::set_permissions(&omp, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    let started = std::time::Instant::now();
    let said = preflight_turn(&omp.to_string_lossy(), "anthropic/claude-sonnet-5-5", &dir.join("run/preflight")).expect_err("interrupted");
    assert!(said.contains("interrupted"), "{said}");
    assert!(started.elapsed().as_secs() < 30, "the pre-flight turn was not waited out: {:?}", started.elapsed());

    // **Nor does an editor start**: no display and no editor are started once the run is interrupted.
    let said = start_editor(&dir.join("no-such-editor"), &dir, "m", &dir).err().expect("interrupted");
    assert!(said.contains("interrupted"), "{said}");

    // **An interrupted run publishes nothing**: its unstarted trials are harness errors, so a report of them would still
    // read as fit (10% tolerated); no report is written, and so none under `--label` either.
    let out = dir.join("out");
    let bench = dir.join("docs-bench");
    std::fs::create_dir_all(&out).unwrap_or_else(|e| panic!("{e}"));
    let options = Options {
        root: dir.clone(),
        tasks: dir.clone(),
        agent: Agent::Oracle,
        trials: 1,
        jobs: 1,
        model: String::new(),
        variant: None,
        out: out.clone(),
        editor: dir.clone(),
        server: dir.clone(),
        repo: dir.clone(),
        label: "interrupted".to_owned(),
        publish: Some(bench.clone()),
        only: Vec::new(),
    };
    let said = write_reports(&options).err().expect("an interrupted run writes no report");
    assert!(said.contains("interrupted"), "{said}");
    assert!(!out.join("report.md").exists() && !out.join("report.json").exists() && !bench.exists(), "nothing was written anywhere");
    let _ = std::fs::remove_dir_all(&dir);
}
