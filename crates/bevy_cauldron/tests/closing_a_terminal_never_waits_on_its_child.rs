//! **Closing a terminal, or quitting with one open, never waits for ever on its child** (#337).
//!
//! `alacritty_terminal`'s `Pty` hangs the child up and then blocks in `Child::wait` with no limit. A
//! shell leaves at once, but an agent tab (`omp`) may ignore `SIGHUP`, and the editor's main thread then
//! sat in that wait after its window was gone: an update that quit the app left it alive and window-less
//! until it was killed by hand. Dropping a session now gives the child [`HANGUP_GRACE_MS`] to leave and
//! then kills it.
#![cfg(unix)]

use std::time::{Duration, Instant};

use bevy_cauldron::terminal::{HANGUP_GRACE_MS, ShellChoice, TerminalSession};
use rustix::process::{Pid, test_kill_process};

/// How long the stubborn child lives if nothing ends it, seconds: far past any bound below.
const CHILD_LIFE_S: u64 = 20;

/// The most a drop may take: the grace, and slack for a loaded machine, milliseconds. Well under
/// [`CHILD_LIFE_S`], so a drop that waited for the child fails.
const MOST_A_DROP_MAY_TAKE_MS: u64 = HANGUP_GRACE_MS + 6_000;

/// How many 50 ms looks the test gives the child to say it is ready.
const TRIES: usize = 200;

/// A session whose child runs `script` under `/bin/sh -c`.
fn session_running(script: &str) -> TerminalSession {
    let shell = ShellChoice { program: "/bin/sh".to_owned(), args: vec!["-c".to_owned(), script.to_owned()] };
    TerminalSession::spawn_with(80, 24, &std::env::temp_dir(), &[], Some(&shell)).unwrap_or_else(|e| panic!("{e}"))
}

/// Waits until the child has printed `ready`, which it does after it has installed what the test needs.
fn wait_until_ready(session: &mut TerminalSession) {
    let mut seen = Vec::new();
    for _ in 0..TRIES {
        session.drain(&mut seen).unwrap_or_else(|e| panic!("{e}"));
        if String::from_utf8_lossy(&seen).contains("ready") {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the child never said ready: {:?}", String::from_utf8_lossy(&seen));
}

fn gone(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else { return true };
    test_kill_process(pid).is_err()
}

#[test]
fn a_child_that_ignores_the_hangup_does_not_hold_the_drop() {
    let mut session = session_running(&format!("trap '' HUP; echo ready; exec sleep {CHILD_LIFE_S}"));
    wait_until_ready(&mut session);
    let pid = session.pid();

    let started = Instant::now();
    drop(session);
    let took = started.elapsed();

    assert!(
        took < Duration::from_millis(MOST_A_DROP_MAY_TAKE_MS),
        "closing the session took {took:?}: it waited on a child that ignores the hangup"
    );
    // The child was given its grace before it was ended, not killed at once.
    assert!(
        took >= Duration::from_millis(HANGUP_GRACE_MS * 9 / 10),
        "closing the session took {took:?}: the child was killed without being given {HANGUP_GRACE_MS} ms to leave"
    );
    assert!(gone(pid), "the child {pid} is still there after the session closed");
}

#[test]
fn a_shell_that_leaves_on_the_hangup_is_not_made_to_wait_out_the_grace() {
    // The default disposition: `sleep` dies on `SIGHUP` at once.
    let mut session = session_running(&format!("echo ready; exec sleep {CHILD_LIFE_S}"));
    wait_until_ready(&mut session);
    let pid = session.pid();

    let started = Instant::now();
    drop(session);
    let took = started.elapsed();

    assert!(
        took < Duration::from_millis(HANGUP_GRACE_MS),
        "closing the session took {took:?}, as long as the grace a stubborn child is given"
    );
    assert!(gone(pid), "the child {pid} is still there after the session closed");
}
