//! **A quit has a deadline and names what it waits on, and a second press of the quit key ends it at once** (#427).
//!
//! The owner, on 0.5.3: Cmd+Q "doesn't exit gracefully, it gets stuck", and "if it gets stuck in the graceful exit,
//! repeatedly hitting the exit key combo, with some debounce, then it force kills the application". `a_quit_that_does_not_finish_is_ended`
//! (#337) holds the deadline itself; this holds what #427 adds: the waits an exit can be stuck in are named, the first past its
//! bound is said, a second press after the debounce ends the process, and the record says which waits it cut short.
//!
//! The binary's own action kills the process, so these tests give the watchdog one that counts. The stuck wait is a real child
//! process that ignores `SIGHUP`, waited on the way a terminal's shell is when its pane goes (`wait` after a hangup).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::exit_watchdog::{Cut, ExitWatchdog, ExitWatchdogPlugin, SecondPress, arm_exit, last_cut, overdue_waits, quit_chord_is_down, waiting, waits_now};

/// The deadline the tests give a quit: short enough to wait for, long enough to tell "not yet" from "done".
const DEADLINE: Duration = Duration::from_millis(900);

/// How far past its deadline a quit may be ended in a test, for a loaded machine.
const SLACK: Duration = Duration::from_secs(3);

/// The most a test waits for the watchdog to act.
const PATIENCE: Duration = Duration::from_secs(10);

/// A debounce short enough to wait out in a test.
const DEBOUNCE: Duration = Duration::from_millis(400);

fn wait_for(count: &AtomicUsize) -> Duration {
    let started = Instant::now();
    while count.load(Ordering::SeqCst) < 1 {
        assert!(started.elapsed() < PATIENCE, "the watchdog never acted");
        std::thread::sleep(Duration::from_millis(10));
    }
    started.elapsed()
}

static SHELL_QUIT_ENDED: AtomicUsize = AtomicUsize::new(0);

fn shell_quit_ended() {
    SHELL_QUIT_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
#[cfg(unix)]
fn a_quit_stuck_on_a_shell_that_ignores_hangup_is_ended_inside_its_deadline_and_the_wait_is_named() {
    use rustix::process::{Pid, Signal, kill_process};

    // A shell that ignores the hangup and keeps running: what a pane's shell can be when it is closed.
    let mut shell = std::process::Command::new("sh")
        .args(["-c", "trap '' HUP; echo ready; while :; do sleep 1; done"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("a shell to hold: {e}"));
    let pid = Pid::from_raw(i32::try_from(shell.id()).unwrap()).expect("a pid");
    // The hangup is sent only once the shell has said its trap is in place, or it could die of it before ignoring it.
    let mut ready = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(shell.stdout.take().expect("a piped stdout")), &mut ready).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ready.trim(), "ready", "the shell ignores the hangup before it is sent");

    // The teardown: hang up, then wait for the child, as dropping a pane's session does. It listed its wait first.
    let teardown = std::thread::spawn(move || {
        let _wait = waiting("a shell that ignores SIGHUP", Duration::from_millis(100));
        let _ = kill_process(pid, Signal::HUP);
        let _ = shell.wait();
    });

    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ExitWatchdogPlugin { watchdog: ExitWatchdog { grace: DEADLINE, then: shell_quit_ended } });
    app.world_mut().write_message(AppExit::Success);
    let asked = Instant::now();
    app.update();
    wait_for(&SHELL_QUIT_ENDED);
    let took = asked.elapsed();
    assert!(took >= DEADLINE * 9 / 10, "it was ended after {took:?}, before the deadline of {DEADLINE:?}");
    assert!(took <= DEADLINE + SLACK, "the process outlived its quit by {took:?}: the deadline is {DEADLINE:?}");

    let waits = waits_now();
    assert!(waits.iter().any(|w| w.starts_with("a shell that ignores SIGHUP")), "the stuck wait is on the list: {waits:?}");
    assert!(overdue_waits().contains(&"a shell that ignores SIGHUP"), "and it is said to be past its bound, by name");
    let cut = last_cut().expect("the watchdog recorded what it ended the process in");
    assert_eq!(cut.why, Cut::Deadline);
    assert!(cut.waits.iter().any(|w| w.starts_with("a shell that ignores SIGHUP")), "the record names the wait it cut short: {:?}", cut.waits);

    // The test's own cleanup: the process the watchdog would have ended is this one, so the shell is ended by hand.
    let _ = kill_process(pid, Signal::KILL);
    teardown.join().unwrap_or_else(|_| panic!("the teardown thread panicked"));
    assert!(!waits_now().iter().any(|w| w.starts_with("a shell that ignores SIGHUP")), "a wait that finishes leaves the list");
}

static CHORD: AtomicBool = AtomicBool::new(false);
static SECOND_PRESS_ENDED: AtomicUsize = AtomicUsize::new(0);

fn chord_is_down() -> bool {
    CHORD.load(Ordering::SeqCst)
}

fn second_press_ended() {
    SECOND_PRESS_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn a_second_press_after_the_debounce_ends_a_quit_that_is_stuck_and_names_what_it_cut_short() {
    // The first press is still down when the quit begins, as a finger is.
    CHORD.store(true, Ordering::SeqCst);
    let _stuck = waiting("a wait that never ends", Duration::from_secs(60));
    // A deadline far past the test: only a second press can end this quit.
    let second = SecondPress { debounce: DEBOUNCE, poll: Duration::from_millis(10), down: chord_is_down };
    arm_exit(Duration::from_secs(60), Some(second), second_press_ended).unwrap_or_else(|e| panic!("{e}"));
    let began = Instant::now();

    // Held: not a second press.
    std::thread::sleep(Duration::from_millis(100));
    // Let go, and press again inside the debounce: the first press bouncing, not a second one.
    CHORD.store(false, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(60));
    CHORD.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(60));
    CHORD.store(false, Ordering::SeqCst);
    std::thread::sleep(DEBOUNCE.saturating_sub(began.elapsed()) + Duration::from_millis(100));
    assert_eq!(SECOND_PRESS_ENDED.load(Ordering::SeqCst), 0, "a held key and a press inside the debounce are not a second press");

    // Pressed again, after the debounce: the person has waited and means it.
    let pressed = Instant::now();
    CHORD.store(true, Ordering::SeqCst);
    let took = wait_for(&SECOND_PRESS_ENDED);
    CHORD.store(false, Ordering::SeqCst);
    assert!(pressed.elapsed() < Duration::from_secs(2), "the second press ended the process after {took:?}: at once, not at a deadline");
    assert!(began.elapsed() > DEBOUNCE, "and it was after the debounce");

    let cut = last_cut().expect("the watchdog recorded what it ended the process in");
    assert_eq!(cut.why, Cut::SecondPress);
    assert!(cut.waits.iter().any(|w| w.starts_with("a wait that never ends")), "the record names the wait it cut short: {:?}", cut.waits);
}

#[test]
fn a_wait_is_listed_while_it_is_held_and_is_past_its_bound_only_after_it() {
    let wait = waiting("a short wait for the list", Duration::from_millis(80));
    assert!(waits_now().iter().any(|w| w.starts_with("a short wait for the list")), "listed while held");
    assert!(!overdue_waits().contains(&"a short wait for the list"), "not past its bound at once");
    std::thread::sleep(Duration::from_millis(120));
    assert!(overdue_waits().contains(&"a short wait for the list"), "past its bound after it");
    drop(wait);
    assert!(!waits_now().iter().any(|w| w.starts_with("a short wait for the list")), "gone when released");
}

#[test]
fn the_machines_quit_chord_probe_answers_without_a_key_down() {
    // The probe links and answers; nobody is holding Cmd+Q in a test run.
    assert!(!quit_chord_is_down());
}
