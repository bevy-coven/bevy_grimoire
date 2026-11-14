//! **An agent that already exited is not group-killed, and one still running is** (#432, D4).
//!
//! `stop_group` ran `kill -KILL -- -<pid>` even after `try_wait` had reaped the child, so every normal trial
//! printed `kill: sending signal to -N failed: No such process` (three in a run, seen in all three logs). Beyond
//! the noise, signalling a process group whose leader was already reaped can hit an unrelated group once the
//! number is reused. The group is now killed only while its leader is still running.
//!
//! No model and no editor: the agents are `sh -c` scripts started the way the runner starts an agent (its own
//! process group, [`detach`]).
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use bevy_cauldron_bench::run::{detach, stop_group};

/// **A scratch directory this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-stopgroup-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **Start a script as its own process group**, silent.
fn start(script: &str) -> std::process::Child {
    let mut command = Command::new("sh");
    command.args(["-c", script]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    detach(&mut command).spawn().unwrap_or_else(|e| panic!("sh did not start: {e}"))
}

/// **An agent that has finished is reaped and not signalled**: nothing is sent to a group that has no leader.
#[test]
fn a_finished_agent_is_not_group_killed() {
    let mut child = start("exit 0");
    // The runner's own wait sees the exit first: `try_wait` reaps the child.
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap_or_else(|e| panic!("{e}")).is_none() {
        assert!(Instant::now() < deadline, "the stand-in agent exits");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!stop_group(&mut child), "a group whose leader was already reaped is not killed: its number may be someone else's now");
}

/// **An agent still running is killed with its whole group**, a child of its own included.
#[test]
fn a_running_agent_is_killed_with_its_group() {
    let dir = scratch("running");
    let pidfile = dir.join("tool.pid");
    let mut child = start(&format!("sleep 300 & echo $! > '{}'; wait", pidfile.display()));
    let deadline = Instant::now() + Duration::from_secs(10);
    let tool: u32 = loop {
        if let Some(pid) = std::fs::read_to_string(&pidfile).ok().and_then(|t| t.trim().parse().ok()) {
            break pid;
        }
        assert!(Instant::now() < deadline, "the agent starts a tool of its own");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(stop_group(&mut child), "a running agent's group is killed");
    let proc = PathBuf::from(format!("/proc/{tool}"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while proc.exists() {
        assert!(Instant::now() < deadline, "the agent's tool (pid {tool}) died with its group");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(child.try_wait().unwrap_or_else(|e| panic!("{e}")).is_some(), "and the agent itself is reaped");
    let _ = std::fs::remove_dir_all(&dir);
}
