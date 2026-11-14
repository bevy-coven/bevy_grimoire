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
use bevy::window::WindowCloseRequested;
use bevy_cauldron::app_verbs::QuitRequested;
use bevy_cauldron::exit_watchdog::{
    Cut, ExitState, ExitWatchdog, ExitWatchdogPlugin, FROZEN_AFTER_MS, SecondPress, arm_exit, end_process_with, last_cut, overdue_waits, quit_chord_is_down, spawn_watch, waiting, waits_now,
};

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

/// An editor that watches for a quit with a stub chord, as the binary does with the machine's: its thread runs from the first frame.
fn watched_app(grace: Duration, then: fn(), down: fn() -> bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(ExitWatchdogPlugin { watchdog: ExitWatchdog { grace, then } })
        .insert_resource(SecondPress { debounce: DEBOUNCE, poll: Duration::from_millis(10), down });
    app.update();
    app
}

fn never_down() -> bool {
    false
}

static FROZEN_ENDED: AtomicUsize = AtomicUsize::new(0);

fn frozen_ended() {
    FROZEN_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn a_quit_request_on_an_app_that_then_stops_drawing_is_ended_by_the_deadline_though_no_exit_was_written() {
    let mut app = watched_app(DEADLINE, frozen_ended, never_down);
    // The chord, File -> Quit and the window's close button are each a request; this is the menu's and the chord's.
    app.world_mut().write_message(QuitRequested);
    app.update();
    let stopped = Instant::now();
    // The main thread is stuck: no frame, and no AppExit was ever written.
    wait_for(&FROZEN_ENDED);
    let took = stopped.elapsed();
    assert!(took >= DEADLINE * 8 / 10 && took <= DEADLINE + SLACK, "ended {took:?} after the app stopped drawing; the deadline is {DEADLINE:?}");
    assert_eq!(last_cut().expect("recorded").why, Cut::Deadline);
}

static CLOSE_ENDED: AtomicUsize = AtomicUsize::new(0);

fn close_ended() {
    CLOSE_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn a_window_close_request_arms_it_too() {
    let mut app = watched_app(DEADLINE, close_ended, never_down);
    app.world_mut().write_message(WindowCloseRequested { window: Entity::PLACEHOLDER });
    app.update();
    wait_for(&CLOSE_ENDED);
}

static ALIVE_ENDED: AtomicUsize = AtomicUsize::new(0);

fn alive_ended() {
    ALIVE_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn an_app_that_keeps_drawing_is_left_alone_while_a_question_waits_for_its_answer() {
    let mut app = watched_app(DEADLINE, alive_ended, never_down);
    app.world_mut().write_message(QuitRequested);
    // The unsaved-edits question is up: frames go on, nobody has answered, no AppExit is written.
    let started = Instant::now();
    while started.elapsed() < DEADLINE * 2 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(ALIVE_ENDED.load(Ordering::SeqCst), 0, "a request that is being answered is not a hang");
}

static FROZEN_CHORD: AtomicBool = AtomicBool::new(false);
static FROZEN_PRESS_ENDED: AtomicUsize = AtomicUsize::new(0);

fn frozen_chord_is_down() -> bool {
    FROZEN_CHORD.load(Ordering::SeqCst)
}

fn frozen_press_ended() {
    FROZEN_PRESS_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn two_presses_on_an_app_that_froze_before_it_could_read_the_first_end_it() {
    // A deadline far past the test: only the second press can end this.
    let app = watched_app(Duration::from_secs(60), frozen_press_ended, frozen_chord_is_down);
    // The main thread freezes: no more frames (the app is kept, not dropped, so nothing else changes).
    std::thread::sleep(Duration::from_millis(FROZEN_AFTER_MS + 200));
    FROZEN_CHORD.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(60));
    FROZEN_CHORD.store(false, Ordering::SeqCst);
    // The first press was a request the frozen app could not read; one inside the debounce does not end it.
    std::thread::sleep(Duration::from_millis(60));
    FROZEN_CHORD.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(60));
    FROZEN_CHORD.store(false, Ordering::SeqCst);
    assert_eq!(FROZEN_PRESS_ENDED.load(Ordering::SeqCst), 0, "a press inside the debounce is not the second");
    std::thread::sleep(DEBOUNCE);
    FROZEN_CHORD.store(true, Ordering::SeqCst);
    wait_for(&FROZEN_PRESS_ENDED);
    FROZEN_CHORD.store(false, Ordering::SeqCst);
    assert_eq!(last_cut().expect("recorded").why, Cut::SecondPress);
    drop(app);
}

static ORDER: std::sync::Mutex<Vec<&str>> = std::sync::Mutex::new(Vec::new());

fn sampled() {
    ORDER.lock().unwrap().push("sample");
}

fn killed() {
    ORDER.lock().unwrap().push("kill");
}

#[test]
fn a_deadline_records_where_it_is_stuck_before_it_ends_the_process_and_a_second_press_does_not_wait_for_it() {
    end_process_with(Some(Cut::Deadline), sampled, killed);
    end_process_with(Some(Cut::SecondPress), sampled, killed);
    assert_eq!(*ORDER.lock().unwrap(), ["sample", "kill", "kill"], "the record of where it is stuck comes first (a dead process cannot be sampled), and only a deadline takes it");
}

static QUESTION_ENDED: AtomicUsize = AtomicUsize::new(0);

fn question_ended() {
    QUESTION_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn a_quit_request_is_not_ended_by_a_pause_in_frames_while_the_unsaved_edits_question_is_open() {
    let mut app = watched_app(DEADLINE, question_ended, never_down);
    app.world_mut().write_message(QuitRequested);
    app.update();
    // The question is up, and then frames stop (a title-bar drag, a live resize, App Nap, an occluded window).
    let state = app.world().resource::<ExitState>().clone();
    state.set_question_open(true);
    std::thread::sleep(DEADLINE * 2);
    assert_eq!(QUESTION_ENDED.load(Ordering::SeqCst), 0, "the process was ended under a question about unsaved edits");
    // The same pause with the question answered is a hang, as before.
    state.set_question_open(false);
    wait_for(&QUESTION_ENDED);
    assert_eq!(last_cut().expect("recorded").why, Cut::Deadline);
}

static QUESTION_CHORD: AtomicBool = AtomicBool::new(false);
static QUESTION_PRESS_ENDED: AtomicUsize = AtomicUsize::new(0);

fn question_chord_is_down() -> bool {
    QUESTION_CHORD.load(Ordering::SeqCst)
}

fn question_press_ended() {
    QUESTION_PRESS_ENDED.fetch_add(1, Ordering::SeqCst);
}

fn press_once(chord: &AtomicBool) {
    chord.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(60));
    chord.store(false, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(60));
}

#[test]
fn two_presses_on_a_frozen_app_do_not_end_it_while_the_unsaved_edits_question_is_open() {
    let app = watched_app(Duration::from_secs(60), question_press_ended, question_chord_is_down);
    let state = app.world().resource::<ExitState>().clone();
    state.set_question_open(true);
    std::thread::sleep(Duration::from_millis(FROZEN_AFTER_MS + 200));
    press_once(&QUESTION_CHORD);
    std::thread::sleep(DEBOUNCE);
    press_once(&QUESTION_CHORD);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(QUESTION_PRESS_ENDED.load(Ordering::SeqCst), 0, "a frozen app with the question open was ended by two presses");
    // Answered, and still frozen: the next press is the second one.
    state.set_question_open(false);
    QUESTION_CHORD.store(true, Ordering::SeqCst);
    wait_for(&QUESTION_PRESS_ENDED);
    QUESTION_CHORD.store(false, Ordering::SeqCst);
    assert_eq!(last_cut().expect("recorded").why, Cut::SecondPress);
    drop(app);
}

#[test]
fn the_state_follows_the_unsaved_edits_question() {
    use bevy_cauldron::confirm::{Asked, Confirm};
    let mut app = watched_app(Duration::from_secs(60), never_ended, never_down);
    app.init_resource::<Confirm>();
    app.update();
    let state = app.world().resource::<ExitState>().clone();
    assert!(!state.question_open(), "no question, none open");
    app.world_mut().resource_mut::<Confirm>().ask_three(Asked::QuitApp, "Quit Cauldron?", "It has unsaved edits.", "Save and quit", "Discard changes", KeyCode::KeyD, "Stay");
    app.update();
    assert!(state.question_open(), "the quit question is open, and the watchdog knows it");
}

fn never_ended() {}

static ACTIVE_CHORD: AtomicBool = AtomicBool::new(false);
static ACTIVE_ENDED: AtomicUsize = AtomicUsize::new(0);

fn active_chord_is_down() -> bool {
    ACTIVE_CHORD.load(Ordering::SeqCst)
}

fn active_ended() {
    ACTIVE_ENDED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn a_press_while_another_app_is_in_front_is_not_a_press_of_this_one() {
    // The quit has begun and has a long deadline: only a second press can end it.
    let state = ExitState::new();
    state.beat();
    state.exiting();
    state.set_app_active(false);
    spawn_watch(state.clone(), Some(SecondPress { debounce: Duration::from_millis(200), poll: Duration::from_millis(10), down: active_chord_is_down }), Duration::from_secs(60), active_ended)
        .unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_millis(300));
    press_once(&ACTIVE_CHORD);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(ACTIVE_ENDED.load(Ordering::SeqCst), 0, "Cmd+Q pressed in another app ended this one");
    state.set_app_active(true);
    std::thread::sleep(Duration::from_millis(50));
    ACTIVE_CHORD.store(true, Ordering::SeqCst);
    wait_for(&ACTIVE_ENDED);
    ACTIVE_CHORD.store(false, Ordering::SeqCst);
}

#[test]
fn the_primary_windows_focus_is_what_says_the_app_is_in_front() {
    let mut app = watched_app(Duration::from_secs(60), never_ended, never_down);
    let state = app.world().resource::<ExitState>().clone();
    assert!(state.app_active(), "an app with no window counts as in front");
    let window = app.world_mut().spawn((Window { focused: false, ..default() }, bevy::window::PrimaryWindow)).id();
    app.update();
    assert!(!state.app_active(), "a primary window that is not focused");
    app.world_mut().get_mut::<Window>(window).expect("the window").focused = true;
    app.update();
    assert!(state.app_active(), "and when it is");
}
