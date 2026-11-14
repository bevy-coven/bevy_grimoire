//! **`cauldron-bench clean` removes the credential directories older runs left** (#434).
//!
//! The #432 guard and the #433 sweep protect new runs; runs from before them kept every trial's `home` (the provider keys it
//! imported) under the state directory. `clean [--older-than DAYS] [--dry-run]` removes those, marker-checked like the sweep (only
//! inside a run directory the runner made, one carrying `run.json`), and `--dry-run` prints what it would remove.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use bevy_cauldron_bench::report::RUN_FILE;
use bevy_cauldron_bench::run::clean_state;

const DAY: u64 = 86_400;

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-clean-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **A directory holding a key, last modified `age` ago.**
fn home_aged(path: &Path, age: Duration) {
    std::fs::create_dir_all(path.join(".omp/agent")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(path.join(".omp/agent/agent.db"), "key").unwrap_or_else(|e| panic!("{e}"));
    std::fs::File::open(path).and_then(|f| f.set_modified(SystemTime::now() - age)).unwrap_or_else(|e| panic!("{e}"));
}

/// **A state directory with an old run, a recent run, and a directory that is not a run.** Answers it and the paths.
fn state(test: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let state = scratch(test);
    let old = state.join("old-run");
    let recent = state.join("recent-run");
    let other = state.join("not-a-run");
    std::fs::create_dir_all(&old).unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(&recent).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(old.join(RUN_FILE), "{}").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(recent.join(RUN_FILE), "{}").unwrap_or_else(|e| panic!("{e}"));
    home_aged(&old.join("task-a/trial-1/home"), Duration::from_secs(5 * DAY));
    home_aged(&old.join("task-a/trial-2/home"), Duration::from_secs(5 * DAY));
    home_aged(&old.join("preflight"), Duration::from_secs(5 * DAY));
    home_aged(&recent.join("task-a/trial-1/home"), Duration::from_secs(60));
    home_aged(&other.join("task-a/trial-1/home"), Duration::from_secs(5 * DAY));
    (state, old, recent, other)
}

/// **`--dry-run` lists what would go and removes nothing.**
#[test]
fn a_dry_run_lists_and_removes_nothing() {
    let (state, old, _, _) = state("dry");
    let listed = clean_state(&state, Duration::from_secs(DAY), true);
    let names: Vec<String> = listed.iter().map(|p| p.strip_prefix(&state).unwrap_or(p).display().to_string()).collect();
    assert_eq!(names, ["old-run/preflight", "old-run/task-a/trial-1/home", "old-run/task-a/trial-2/home"], "the old run's three directories");
    assert!(old.join("task-a/trial-1/home/.omp/agent/agent.db").is_file(), "nothing was removed");
    let _ = std::fs::remove_dir_all(&state);
}

/// **`clean` removes the old ones, keeps a recent run's and what is not a run's.**
#[test]
fn clean_removes_only_stale_homes_of_runs_the_runner_made() {
    let (state, old, recent, other) = state("clean");
    let removed = clean_state(&state, Duration::from_secs(DAY), false);
    assert_eq!(removed.len(), 3, "{removed:?}");
    assert!(!old.join("task-a/trial-1/home").exists() && !old.join("task-a/trial-2/home").exists() && !old.join("preflight").exists());
    assert!(recent.join("task-a/trial-1/home").is_dir(), "a recent run's home is a live run's until it is a day old");
    assert!(other.join("task-a/trial-1/home").is_dir(), "a directory with no run.json is not the runner's, whatever its age");
    assert!(old.join(RUN_FILE).is_file(), "the run's own files stay: only credential directories go");
    // And with no age at all, the recent run's goes too.
    let all = clean_state(&state, Duration::ZERO, false);
    assert_eq!(all.len(), 1, "{all:?}");
    let _ = std::fs::remove_dir_all(&state);
}

/// **The command line**: `clean --dry-run` prints what it would remove, `clean --older-than` honours its argument, `clean` removes.
#[test]
fn the_command_prints_and_removes() {
    let (state, old, recent, _) = state("command");
    // The state directory the command reads is `$HOME/.local/state/cauldron-bench`: lay the fixture there.
    let home = scratch("command-home");
    let root = home.join(".local/state");
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{e}"));
    std::fs::rename(&state, root.join("cauldron-bench")).unwrap_or_else(|e| panic!("{e}"));
    let bin = env!("CARGO_BIN_EXE_cauldron-bench");
    let run = |args: &[&str]| {
        let out = Command::new(bin).arg("clean").args(args).env("HOME", &home).output().unwrap_or_else(|e| panic!("{e}"));
        (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    };
    let live = root.join("cauldron-bench");
    let (ok, said) = run(&["--dry-run"]);
    assert!(ok, "{said}");
    assert!(said.contains("would remove") && said.contains("old-run/task-a/trial-1/home"), "it says what it would remove: {said}");
    assert!(!said.contains("recent-run"), "a recent run's home is not on the list at the default age: {said}");
    assert!(live.join("old-run/task-a/trial-1/home").is_dir(), "--dry-run removed nothing");
    let (ok, said) = run(&["--older-than", "10"]);
    assert!(ok && said.contains("nothing to remove"), "nothing is ten days old: {said}");
    let (ok, said) = run(&[]);
    assert!(ok && said.contains("removed 3"), "{said}");
    assert!(!live.join("old-run/task-a/trial-1/home").exists());
    assert!(live.join("recent-run/task-a/trial-1/home").is_dir());
    let (ok, said) = run(&["--older-than", "x"]);
    assert!(!ok && said.contains("--older-than"), "a bad number is refused: {said}");
    let _ = (old, recent);
    let _ = std::fs::remove_dir_all(&home);
}
