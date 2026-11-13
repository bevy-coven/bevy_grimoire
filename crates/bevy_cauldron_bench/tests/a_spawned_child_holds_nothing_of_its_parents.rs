//! **A child this harness starts holds nothing of its parent's** (BM-02, #412).
//!
//! Measured on big, 2026-11-13: a test ran under `flock -s /tmp/fleet/big-heavy.lock cargo nextest ...` and
//! left an `Xvfb` behind. That `Xvfb` **inherited the lock's file descriptor** and held the host lock for
//! thirteen minutes with no trial alive, blocking the push gate and every other agent's build. `flock(1)` keeps
//! the lock for as long as any process holds the descriptor, so a harness whose long-lived children inherit
//! its descriptors lets one orphan hold what its caller took.
//!
//! These tests run [`detach`] on the children the way `Display`, the editor, the MCP server and `omp` are
//! started, with no editor and no `Xvfb`, so the gate holds them.
#![cfg(target_os = "linux")]

use std::process::{Command, Stdio};

use bevy_cauldron_bench::run::detach;

/// **A descriptor number far above anything a test opens**, so the listing of the child's own
/// `/proc/self/fd` (it opens the directory at the lowest free number) cannot be mistaken for it.
const HIGH_FD: libc::c_int = 100;

/// **A descriptor this process holds and does not mark close-on-exec**  -  the lock's descriptor
/// `flock(1)` leaves open in every process it starts.
fn leaked_descriptor() -> libc::c_int {
    // SAFETY: `open` and `fcntl` on a path this test names and a descriptor it just opened.
    unsafe {
        let file = libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY);
        assert!(file >= 0, "/dev/null opens: {}", std::io::Error::last_os_error());
        // `F_DUPFD` (not `F_DUPFD_CLOEXEC`) gives the copy no close-on-exec flag, as `flock(1)`'s is.
        let leaked = libc::fcntl(file, libc::F_DUPFD, HIGH_FD);
        assert!(leaked >= HIGH_FD, "the descriptor is duplicated above {HIGH_FD}: {}", std::io::Error::last_os_error());
        libc::close(file);
        leaked
    }
}

/// **The descriptors a child holds**, as `ls /proc/self/fd` prints them from inside it.
fn descriptors_of(command: &mut Command) -> Vec<String> {
    let out = command
        .args(["-c", "ls /proc/self/fd"])
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("sh did not start: {e}"));
    assert!(out.status.success(), "ls ran: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).split_whitespace().map(str::to_owned).collect()
}

/// **A detached child does not inherit a descriptor, and an ordinary child does**  -  the control that
/// shows the leak this test is about is real on this host.
#[test]
fn a_detached_child_does_not_inherit_a_descriptor_and_an_ordinary_one_does() {
    let leaked = leaked_descriptor().to_string();

    let ordinary = descriptors_of(&mut Command::new("sh"));
    assert!(
        ordinary.contains(&leaked),
        "the control: an ordinary child inherits descriptor {leaked}, which is the leak (it holds {ordinary:?})"
    );

    let mut command = Command::new("sh");
    detach(&mut command);
    let detached = descriptors_of(&mut command);
    assert!(
        !detached.contains(&leaked),
        "a detached child inherited descriptor {leaked}, so it would hold a host lock its parent took: {detached:?}"
    );
}

/// **A detached child is its own process group**, so a signal to the harness's group is not delivered to
/// a child the harness is stopping itself.
#[test]
fn a_detached_child_leads_its_own_process_group() {
    let mut command = Command::new("sleep");
    command.arg("30");
    let mut child = detach(&mut command).spawn().unwrap_or_else(|e| panic!("sleep did not start: {e}"));
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", child.id())).unwrap_or_default();
    // `pid (comm) state ppid pgrp ...`: the fields after the closing parenthesis.
    let after = stat.rsplit_once(')').map(|(_, rest)| rest.to_owned()).unwrap_or_default();
    let pgrp = after.split_whitespace().nth(2).and_then(|f| f.parse::<u32>().ok());
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(pgrp, Some(child.id()), "the child leads a process group of its own (stat: {stat})");
}

/// **A child that cannot be executed is still an error to the parent**  -  closing descriptors in the
/// child must not close the pipe `std` reports a failed `exec` through, or a missing `Xvfb` would read as a
/// display that started.
#[test]
fn a_detached_child_that_cannot_exec_is_still_a_spawn_error() {
    let mut command = Command::new("/nonexistent/cauldron-bench-no-such-binary");
    let failed = detach(&mut command).spawn();
    assert!(failed.is_err(), "a binary that is not there is a spawn error, not a child that started");
}
