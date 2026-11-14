//! **A trial HOME a killed run left behind is swept at the next run's start** (#433, item 3b).
//!
//! The credential guard is a `Drop`, which `kill -KILL` and the default SIGINT never run. A later run sweeps the
//! directories a dead run leaves: `<state>/<run>/<task>/trial-<n>/home` and `<state>/<run>/preflight`. It removes only
//! those older than the longest a trial can run, so a run in progress on the same host (another `--run`) keeps its own.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy_cauldron_bench::run::sweep_stale_secrets;

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-sweep-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **A directory holding a file, last modified `age` ago.**
fn dir_aged(path: &Path, age: Duration) {
    std::fs::create_dir_all(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    std::fs::write(path.join("agent.db"), "secret").unwrap_or_else(|e| panic!("{e}"));
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{e}"));
    file.set_modified(SystemTime::now() - age).unwrap_or_else(|e| panic!("{e}"));
}

/// **Old homes go, fresh ones and the trial's record stay.**
#[test]
fn only_stale_homes_are_swept() {
    let state = scratch("stale");
    let old = state.join("dead-run/some-task/trial-1");
    let fresh = state.join("live-run/some-task/trial-1");
    dir_aged(&old.join("home"), Duration::from_secs(10_000));
    dir_aged(&state.join("dead-run/preflight"), Duration::from_secs(10_000));
    dir_aged(&fresh.join("home"), Duration::from_secs(5));
    std::fs::write(old.join("trial.json"), "{}").unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(old.join("project")).unwrap_or_else(|e| panic!("{e}"));

    let swept = sweep_stale_secrets(&state, Duration::from_secs(3600));

    assert_eq!(swept, 2, "the old home and the old pre-flight directory");
    assert!(!old.join("home").exists(), "a dead run's home is gone");
    assert!(!state.join("dead-run/preflight").exists(), "so is its pre-flight directory");
    assert!(fresh.join("home").join("agent.db").is_file(), "a live run's home is left alone");
    assert!(old.join("trial.json").is_file() && old.join("project").is_dir(), "nothing else of a trial is touched");
    let _ = std::fs::remove_dir_all(&state);
}

/// **With no age, everything of this kind goes** (a run's own output directory, swept as it starts), and a missing root is nothing.
#[test]
fn no_age_sweeps_all_and_a_missing_root_is_nothing() {
    let state = scratch("all");
    dir_aged(&state.join("r/t/trial-1/home"), Duration::from_secs(1));
    dir_aged(&state.join("r/preflight"), Duration::from_secs(1));
    assert_eq!(sweep_stale_secrets(&state, Duration::ZERO), 2);
    assert_eq!(sweep_stale_secrets(&state.join("nowhere"), Duration::ZERO), 0);
    let _ = std::fs::remove_dir_all(&state);
}
