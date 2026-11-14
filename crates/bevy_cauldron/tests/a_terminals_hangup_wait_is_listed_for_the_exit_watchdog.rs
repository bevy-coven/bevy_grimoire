//! **Closing a terminal lists its wait for the exit watchdog** (#337 x the exit watchdog).
//!
//! Dropping a session hangs the shell up and gives it `HANGUP_GRACE_MS` to leave before it is killed. That is a wait of the
//! graceful exit, and the watchdog names the waits in progress when it cuts a stuck quit short; a quit stuck in a shell that
//! ignores the hangup must say so, not nothing.
#![cfg(unix)]

use std::time::{Duration, Instant};

use bevy_cauldron::exit_watchdog;
use bevy_cauldron::terminal::{HANGUP_GRACE_MS, ShellChoice, TerminalSession};

/// How long the stubborn child lives if nothing ends it, seconds: far past the grace.
const CHILD_LIFE_S: u64 = 20;

/// What the wait is listed as.
const WAIT: &str = "a terminal's shell hanging up";

/// How many 50 ms looks the test gives the child to say it is ready.
const TRIES: usize = 200;

fn listed() -> Vec<String> {
    exit_watchdog::waits_now().into_iter().filter(|wait| wait.starts_with(WAIT)).collect()
}

#[test]
fn a_shell_that_ignores_the_hangup_is_listed_while_it_is_waited_on_and_not_after() {
    let script = format!("trap '' HUP; echo ready; exec sleep {CHILD_LIFE_S}");
    let shell = ShellChoice { program: "/bin/sh".to_owned(), args: vec!["-c".to_owned(), script] };
    let mut session = TerminalSession::spawn_with(80, 24, &std::env::temp_dir(), &[], Some(&shell)).unwrap_or_else(|e| panic!("{e}"));
    let mut seen = Vec::new();
    for _ in 0..TRIES {
        session.drain(&mut seen).unwrap_or_else(|e| panic!("{e}"));
        if String::from_utf8_lossy(&seen).contains("ready") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(String::from_utf8_lossy(&seen).contains("ready"), "the child said ready: {:?}", String::from_utf8_lossy(&seen));
    assert!(listed().is_empty(), "nothing is listed before the session is closed: {:?}", listed());

    // A second thread looks at the list while this one is inside the drop.
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let looking = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut said: Vec<String> = Vec::new();
            let mut overdue: Vec<&'static str> = Vec::new();
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let now = listed();
                if !now.is_empty() {
                    said = now;
                    overdue.extend(exit_watchdog::overdue_waits());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            (said, overdue)
        })
    };
    let started = Instant::now();
    drop(session);
    let took = started.elapsed();
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let (said, overdue) = looking.join().unwrap_or_else(|_| panic!("the looking thread panicked"));
    assert!(took >= Duration::from_millis(HANGUP_GRACE_MS * 9 / 10), "the drop gave the shell its grace ({took:?})");
    assert_eq!(said.len(), 1, "the wait for the shell to hang up is listed once while the drop waits on it: {said:?}");
    assert!(overdue.is_empty(), "the bound is a little over the grace the drop gives the shell, so the wait is not overdue: {overdue:?}");

    assert!(listed().is_empty(), "and it is taken off the list when the drop is over: {:?}", listed());
}
