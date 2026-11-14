//! **A quit that does not finish is finished for it** (#337), **and a second press of the quit key finishes it at once** (#427).
//!
//! Quitting writes [`AppExit`], the window goes, and the process is meant to follow. On 2026-11-07 it did
//! not, twice: the system log shows the window and the audio output torn down at 16:42:45 and the process
//! still running ten seconds later (`runningboardd` still computing its state), and its agent's `omp` child
//! alive at 16:49. Nothing was drawn and nothing answered, so a person saw a frozen app that had to be
//! killed, and an update, which quits the app so a script can swap the bundle, never completed. On 2026-11-09
//! the owner met it again on 0.5.3, with Cmd+Q: "it doesn't exit gracefully, it gets stuck", and asked that a
//! quit key start a graceful exit and that "repeatedly hitting the exit key combo, with some debounce, then
//! kills the application".
//!
//! # The graceful exit has a deadline, and its waits have names
//!
//! **Where the process was stuck is not known**: dropping the `World` runs every resource's destructor on
//! the main thread, and any of them can wait on a thread or a child (the terminal's PTY was one, now
//! bounded in `terminal::pty`). So a thread waits [`EXIT_GRACE_S`] seconds after the [`AppExit`] and, if the
//! process is still there, **records where it is stuck (`sample`, on a Mac, for at most [`SAMPLE_PATIENCE_MS`] ms: a
//! live process is the only one it can look at, so the record is taken first and bounded) and ends it**. The record is
//! `exit-hang.txt` beside the log, so the next hang names its own cause.
//!
//! A step of the exit that can wait registers itself with [`waiting`], its name and its own bound, and the guard
//! it returns takes it off again when it finishes. The watchdog logs **the first wait that is past its bound, by
//! name** (once each), and when it ends the process the log says **which waits it cut short** ([`CutShort`],
//! [`last_cut`]). Waits registered today: the quit's own teardown (dropping the app: every terminal's shell, the agent
//! tab, BRP and the updater are in it, until each registers its own), the window's saved frame, and the log file.
//!
//! # A quit request is the start, whatever asked
//!
//! The window's close button, File → Quit and the Cmd+Q chord are each a quit *request* ([`note_the_requests`]); the
//! unsaved-edits question may sit between the request and the [`AppExit`] a person answers it with. A request alone has
//! **no deadline while the app is drawing** (a question waiting for its answer is not a hang), but **an app that has stopped
//! drawing for [`EXIT_GRACE_S`] seconds after a request is ended** like a quit that does not finish: a frozen main thread
//! before the [`AppExit`] is written is the other place a quit gets stuck. The heartbeat is a frame counter ([`ExitState`]).
//!
//! # A second press ends it
//!
//! A quit that is stuck is stuck on the main thread, so no key event reaches the app and the second press cannot be read
//! the way the first was. The watchdog thread therefore asks the operating system whether the quit chord is down
//! ([`SecondPress`], [`quit_chord_is_down`]: on a Mac, `CGEventSourceKeyState` for Cmd and Q, which any thread may ask
//! and which needs no permission) every [`QUIT_POLL_MS`] ms, from the start of the app: **a press while the app has not
//! drawn for [`FROZEN_AFTER_MS`] ms is a quit request the frozen app cannot read, and the press after that, more than
//! [`QUIT_DEBOUNCE_MS`] later, ends the process at once** (no `sample`: the person has already waited), naming the waits it cut
//! short. A press inside the debounce, and a key held since the first press, are not a second press.
//!
//! **The chord watched is the physical Q key** (`kVK_ANSI_Q`, 0x0C) with either Command key. On a layout where Q is
//! elsewhere (AZERTY puts it where A is on QWERTY, Dvorak puts it at X) the chord is not seen, so only the deadline
//! ends a stuck quit there; the layout-aware key code needs `UCKeyTranslate` over the current input source's `uchr` data
//! (Carbon `TISCopyCurrentKeyboardLayoutInputSource`), which is more than a probe that runs every 25 ms should carry.
//!
//! **Process-global state.** The wait list ([`waiting`]) and the last cut ([`last_cut`]) are statics, because a wait is
//! registered from anywhere in the app and the watchdog is a thread that reads them. Tests under `cargo nextest` run one
//! to a process; run together in one process, a test must use wait names of its own.
//!
//! **Only the binary adds it** (`main.rs`): a test editor writes [`AppExit`] and goes on, and a watchdog
//! there would end the test run.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::window::WindowCloseRequested;

/// Seconds a quit has to finish before the process is ended: a window-less process that has not gone by then
/// is not going.
pub const EXIT_GRACE_S: u64 = 3;

/// Seconds the quit's own teardown (dropping the app) may take before the log says it is the wait that is overdue:
/// inside [`EXIT_GRACE_S`], so the name is in the log before the process is ended.
pub const TEARDOWN_BOUND_S: u64 = 2;

/// Milliseconds after a quit began before another press of the quit key is a second press and not the first one
/// bouncing or being held: the person has plainly waited.
pub const QUIT_DEBOUNCE_MS: u64 = 800;

/// Milliseconds between the watchdog's looks at the quit chord and at the waits' bounds.
pub const QUIT_POLL_MS: u64 = 25;

/// Milliseconds without a drawn frame after which the app counts as frozen to the watchdog: a press of the quit key then is
/// a request the app cannot read, and a second press may end it.
pub const FROZEN_AFTER_MS: u64 = 500;

/// Seconds a quit request that was not followed by an exit, with the app drawing, stands before it is forgotten (the person
/// answered *Stay*), so a later freeze is not read as a stuck quit.
pub const REQUEST_FORGET_S: u64 = 10;

/// Seconds an app that was asked to quit may stop drawing while a question that blocks the quit (the unsaved-edits dialog) is open,
/// before it is ended all the same: a title-bar drag, a live resize, App Nap or an occluded window stop frames for a while, and the
/// process must not be killed under a question about unsaved edits; a minute is long enough for those and not for ever.
pub const QUESTION_GRACE_S: u64 = 60;

/// Milliseconds `sample` may take to write its record, on a Mac, before it is given up on and the process is ended: a
/// second of sampling and the file's write, and no more of the person's wait than that.
#[cfg(target_os = "macos")]
pub const SAMPLE_PATIENCE_MS: u64 = 2_000;

/// **What happens when a quit has not finished**, and after how long.
#[derive(Resource, Clone, Copy)]
pub struct ExitWatchdog {
    /// How long the quit has.
    pub grace: Duration,
    /// What ends the process; [`end_this_process`] in the binary.
    pub then: fn(),
}

/// **How a second press of the quit key is seen from outside the main thread.** Inserted by the binary beside the
/// [`ExitWatchdogPlugin`]; without it the deadline is the only thing that ends a stuck quit.
#[derive(Resource, Clone, Copy)]
pub struct SecondPress {
    /// A press this soon after the quit began is not a second press.
    pub debounce: Duration,
    /// How often the chord is looked at.
    pub poll: Duration,
    /// Whether the quit chord is down now: [`quit_chord_is_down`] in the binary, a stub in a test.
    pub down: fn() -> bool,
}

impl SecondPress {
    /// This machine's: [`QUIT_DEBOUNCE_MS`], [`QUIT_POLL_MS`] and [`quit_chord_is_down`].
    pub fn this_machine() -> Self {
        Self { debounce: Duration::from_millis(QUIT_DEBOUNCE_MS), poll: Duration::from_millis(QUIT_POLL_MS), down: quit_chord_is_down }
    }
}

/// **Is the quit chord down right now?** On a Mac, Cmd and the physical Q key by `CGEventSourceKeyState`, which reads the
/// session's key state from any thread and needs no permission. Elsewhere there is no look, so only the deadline ends a
/// stuck quit.
#[cfg(target_os = "macos")]
pub fn quit_chord_is_down() -> bool {
    /// `kCGEventSourceStateCombinedSessionState`: the keys down for the whole login session.
    const COMBINED_SESSION: i32 = 0;
    /// Virtual key codes (`HIToolbox/Events.h`): `kVK_ANSI_Q`, `kVK_Command` and `kVK_RightCommand`.
    const KEY_Q: u16 = 0x0C;
    const KEY_COMMAND: u16 = 0x37;
    const KEY_RIGHT_COMMAND: u16 = 0x36;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        /// Whether `key` is down in `state`'s event source: a pure query of the session's key state.
        safe fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
    }
    CGEventSourceKeyState(COMBINED_SESSION, KEY_Q) && (CGEventSourceKeyState(COMBINED_SESSION, KEY_COMMAND) || CGEventSourceKeyState(COMBINED_SESSION, KEY_RIGHT_COMMAND))
}

/// Only a Mac is asked: elsewhere the chord is never seen down from here.
#[cfg(not(target_os = "macos"))]
pub fn quit_chord_is_down() -> bool {
    false
}

/// **What the watchdog thread knows of the app**, shared with it: when a frame was last drawn, when a quit was asked for, and
/// when the app began to exit. Times are milliseconds since the state was made, stored as `ms + 1` so that `0` is "never".
#[derive(Resource, Clone)]
pub struct ExitState(Arc<StateTimes>);

struct StateTimes {
    born: Instant,
    beat: AtomicU64,
    requested: AtomicU64,
    exiting: AtomicU64,
    question: AtomicBool,
    active: AtomicBool,
}

impl Default for ExitState {
    fn default() -> Self {
        Self::new()
    }
}

impl ExitState {
    pub fn new() -> Self {
        Self(Arc::new(StateTimes {
            born: Instant::now(),
            beat: AtomicU64::new(0),
            requested: AtomicU64::new(0),
            exiting: AtomicU64::new(0),
            question: AtomicBool::new(false),
            active: AtomicBool::new(true),
        }))
    }

    fn now(&self) -> u64 {
        u64::try_from(self.0.born.elapsed().as_millis()).unwrap_or(u64::MAX - 1)
    }

    /// A frame was drawn: the app is not frozen.
    pub fn beat(&self) {
        self.0.beat.store(self.now() + 1, Ordering::Relaxed);
    }

    /// A person asked to quit (the window's close button, File → Quit or the chord). The first request stands.
    pub fn request(&self) {
        let _ = self.0.requested.compare_exchange(0, self.now() + 1, Ordering::Relaxed, Ordering::Relaxed);
    }

    /// The app began to exit ([`AppExit`] was written). The quit begins from here, so the debounce and the deadline count from it.
    pub fn exiting(&self) {
        let now = self.now() + 1;
        if self.0.exiting.compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            self.0.requested.store(now, Ordering::Relaxed);
        }
    }

    /// The request was answered with *Stay*: forgotten.
    fn forget_the_request(&self) {
        self.0.requested.store(0, Ordering::Relaxed);
    }

    /// Time of the last frame, the request and the exit, each `None` if it has not happened.
    fn times(&self) -> (u64, Option<u64>, Option<u64>) {
        let at = |cell: &AtomicU64| cell.load(Ordering::Relaxed).checked_sub(1);
        (at(&self.0.beat).unwrap_or(0), at(&self.0.requested), at(&self.0.exiting))
    }

    /// A question that blocks the quit is open, or was answered.
    pub fn set_question_open(&self, open: bool) {
        self.0.question.store(open, Ordering::Relaxed);
    }

    pub fn question_open(&self) -> bool {
        self.0.question.load(Ordering::Relaxed)
    }

    /// This app is the one in front, or is not.
    pub fn set_app_active(&self, active: bool) {
        self.0.active.store(active, Ordering::Relaxed);
    }

    pub fn app_active(&self) -> bool {
        self.0.active.load(Ordering::Relaxed)
    }
}

/// One step of the exit that can wait, while it does.
struct Wait {
    id: u64,
    name: &'static str,
    started: Instant,
    bound: Duration,
    /// Said once it was past its bound.
    said: bool,
}

static WAITS: Mutex<Vec<Wait>> = Mutex::new(Vec::new());
static NEXT_WAIT: AtomicU64 = AtomicU64::new(1);
static LAST_CUT: Mutex<Option<CutShort>> = Mutex::new(None);

fn waits() -> MutexGuard<'static, Vec<Wait>> {
    WAITS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// **A wait of the graceful exit, while it is waited on.** Dropping it takes the wait off the list.
#[must_use = "the wait is listed only while this is held"]
pub struct ExitWait(u64);

impl Drop for ExitWait {
    fn drop(&mut self) {
        waits().retain(|w| w.id != self.0);
    }
}

/// **Lists a wait of the exit** by `name`, with the `bound` it is meant to finish inside. Hold the result across the wait.
/// The watchdog says the first wait that outlives its bound, by name, and which waits it cut short.
pub fn waiting(name: &'static str, bound: Duration) -> ExitWait {
    let id = NEXT_WAIT.fetch_add(1, Ordering::Relaxed);
    waits().push(Wait { id, name, started: Instant::now(), bound, said: false });
    ExitWait(id)
}

/// The waits in progress now, each with how long it has gone on and its bound.
pub fn waits_now() -> Vec<String> {
    waits().iter().map(|w| format!("{} ({:.1} s of {:.1} s)", w.name, w.started.elapsed().as_secs_f32(), w.bound.as_secs_f32())).collect()
}

/// The names of the waits in progress that have outlived their bound.
pub fn overdue_waits() -> Vec<&'static str> {
    waits().iter().filter(|w| w.started.elapsed() > w.bound).map(|w| w.name).collect()
}

/// Says each wait that has outlived its bound, once.
fn say_the_overdue() {
    for wait in waits().iter_mut().filter(|w| !w.said && w.started.elapsed() > w.bound) {
        wait.said = true;
        warn!("quitting: `{}` is still waiting after its bound of {:?}", wait.name, wait.bound);
    }
}

/// Why the process was ended by the watchdog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// The quit had not finished in [`EXIT_GRACE_S`], or the app stopped drawing for that long after a request to quit.
    Deadline,
    /// The quit key was pressed again, after the debounce, while it had not finished.
    SecondPress,
}

/// What the watchdog ended the process in the middle of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutShort {
    pub why: Cut,
    /// The waits in progress, each with how long it had gone on and its bound.
    pub waits: Vec<String>,
}

/// What the watchdog last cut short, for a test and for the next start's log.
pub fn last_cut() -> Option<CutShort> {
    LAST_CUT.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Records, says and ends: the one place the watchdog acts.
fn cut(why: Cut, grace: Duration, then: fn()) {
    let waits = waits_now();
    let said = if waits.is_empty() { "no named wait is in progress".to_owned() } else { waits.join("; ") };
    match why {
        Cut::Deadline => warn!("quitting has not finished after {grace:?}; ending the process. Cut short: {said}"),
        Cut::SecondPress => warn!("the quit key was pressed again while quitting had not finished; ending the process at once. Cut short: {said}"),
    }
    *LAST_CUT.lock().unwrap_or_else(PoisonError::into_inner) = Some(CutShort { why, waits });
    then();
}

/// Set once the watchdog is running, so a second [`AppExit`] does not start a second; it holds the quit's own
/// teardown as a named wait, which the world's drop (the end of the exit) takes off.
#[derive(Resource)]
struct ExitWatchdogArmed(#[allow(dead_code)] ExitWait);

/// Set once the watchdog's thread is running, whichever started it.
#[derive(Resource)]
struct ExitWatchRunning;

/// Adds [`ExitWatchdog`], the heartbeat, the notes of quit requests, and the system that arms the deadline on the first
/// [`AppExit`]. With a [`SecondPress`] the thread runs from the start of the app, so a freeze before any [`AppExit`] is covered.
pub struct ExitWatchdogPlugin {
    pub watchdog: ExitWatchdog,
}

impl ExitWatchdogPlugin {
    /// The binary's: [`EXIT_GRACE_S`], then [`end_this_process`].
    pub fn ends_the_process() -> Self {
        Self { watchdog: ExitWatchdog { grace: Duration::from_secs(EXIT_GRACE_S), then: end_this_process } }
    }
}

impl Plugin for ExitWatchdogPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.watchdog)
            .init_resource::<ExitState>()
            .add_message::<crate::app_verbs::QuitRequested>()
            .add_message::<WindowCloseRequested>()
            .add_systems(Startup, start_the_watch)
            .add_systems(First, beat)
            .add_systems(Update, (note_the_requests, note_the_question, note_the_focus))
            .add_systems(Last, arm.run_if(on_message::<AppExit>).run_if(not(resource_exists::<ExitWatchdogArmed>)));
    }
}

/// A frame is being drawn: the app is not frozen.
fn beat(state: Res<ExitState>) {
    state.beat();
}

/// **A quit request of any kind starts the watch**: the window's close button, File → Quit and the Cmd+Q chord.
pub fn note_the_requests(
    state: Res<ExitState>,
    mut chord_and_menu: MessageReader<crate::app_verbs::QuitRequested>,
    mut closes: MessageReader<WindowCloseRequested>,
) {
    let asked = chord_and_menu.read().count() + closes.read().count();
    if asked > 0 {
        state.request();
    }
}

/// **Whether a question that blocks the quit is waiting for its answer** (the unsaved-edits dialog is a [`Confirm`](crate::confirm::Confirm)),
/// kept where the watchdog thread can read it. An editor with no [`Confirm`](crate::confirm::Confirm) has none.
pub fn note_the_question(state: Res<ExitState>, confirm: Option<Res<crate::confirm::Confirm>>) {
    state.set_question_open(confirm.is_some_and(|confirm| confirm.is_open()));
}

/// **Whether this app is the one in front**, kept where the watchdog thread can read it: the primary window's focus. An app with no
/// window (a test editor) counts as in front.
pub fn note_the_focus(state: Res<ExitState>, windows: Query<&Window, With<bevy::window::PrimaryWindow>>) {
    state.set_app_active(windows.iter().next().is_none_or(|window| window.focused));
}

/// With a [`SecondPress`], the thread runs for the life of the app: a press of the quit key while the app is frozen is a
/// request, and the one after it ends the process, even when no [`AppExit`] was ever written.
fn start_the_watch(
    watchdog: Res<ExitWatchdog>,
    second: Option<Res<SecondPress>>,
    state: Res<ExitState>,
    running: Option<Res<ExitWatchRunning>>,
    mut commands: Commands,
) {
    let Some(second) = second else { return };
    if running.is_some() {
        return;
    }
    state.beat();
    match spawn_watch(state.clone(), Some(*second), watchdog.grace, watchdog.then) {
        Ok(_) => commands.insert_resource(ExitWatchRunning),
        Err(e) => warn!("the exit watchdog did not start: {e}"),
    }
}

fn arm(
    watchdog: Res<ExitWatchdog>,
    second: Option<Res<SecondPress>>,
    state: Res<ExitState>,
    running: Option<Res<ExitWatchRunning>>,
    mut commands: Commands,
) {
    state.exiting();
    commands.insert_resource(ExitWatchdogArmed(waiting(
        "the quit's teardown (dropping the app: each terminal's shell, the agent tab, BRP, the updater)",
        Duration::from_secs(TEARDOWN_BOUND_S),
    )));
    // Already running for the life of the app (a second press is watched): it has seen the exit begin.
    if running.is_some() {
        return;
    }
    match spawn_watch(state.clone(), second.as_deref().copied(), watchdog.grace, watchdog.then) {
        Ok(_) => commands.insert_resource(ExitWatchRunning),
        // A thread that cannot start leaves the quit as it was: no worse than before this existed.
        Err(e) => warn!("the exit watchdog did not start: {e}"),
    }
}

/// **Runs `then` once `grace` has passed**, on a thread of its own that nothing waits for: a process that
/// exits in time takes it down with it, and one that does not is ended by it.
pub fn arm_after(grace: Duration, then: fn()) -> std::io::Result<std::thread::JoinHandle<()>> {
    arm_exit(grace, None, then)
}

/// **[`arm_after`], and also `then` when the quit chord is pressed again** after `second`'s debounce, if `second` is given:
/// the exit began now.
pub fn arm_exit(grace: Duration, second: Option<SecondPress>, then: fn()) -> std::io::Result<std::thread::JoinHandle<()>> {
    let state = ExitState::new();
    state.beat();
    state.exiting();
    spawn_watch(state, second, grace, then)
}

/// The watchdog's thread: it looks at the app, the waits and the chord every poll, acts once and ends.
pub fn spawn_watch(state: ExitState, second: Option<SecondPress>, grace: Duration, then: fn()) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("exit-watchdog".to_owned()).spawn(move || watch(&state, second, grace, then))
}

/// What the watchdog sees on one look.
struct Look {
    now: u64,
    /// When the last frame was drawn.
    beat: u64,
    requested: Option<u64>,
    exiting: Option<u64>,
    /// The chord was just pressed (it was up on the last look).
    rising: bool,
    /// A question that blocks the quit (the unsaved-edits dialog) is waiting for its answer.
    question_open: bool,
}

/// What the watchdog decides on one look, given the debounce and the grace in milliseconds.
fn verdict(look: &Look, debounce_ms: u64, grace_ms: u64) -> Option<Cut> {
    let age = look.now.saturating_sub(look.beat);
    let frozen = age >= FROZEN_AFTER_MS;
    // **A question waiting for its answer is not a hang** (#427): frames may stop for a title-bar drag, a live resize, App Nap or an
    // occluded window with the unsaved-edits question open, and ending the process then would lose the very edits it asks about.
    // While it is open the request alone has the much longer bound, and a press of a frozen app is not a second press.
    let request_bound_ms = if look.question_open { QUESTION_GRACE_S * 1_000 } else { grace_ms };
    // The exit has had its time; or the app was asked to quit and then stopped drawing for as long.
    match (look.exiting, look.requested) {
        (Some(began), _) if look.now.saturating_sub(began) >= grace_ms => return Some(Cut::Deadline),
        (None, Some(_)) if age >= request_bound_ms => return Some(Cut::Deadline),
        _ => {}
    }
    // A press after the debounce, while the quit is going on or the app is frozen, is the second one.
    let stuck = look.exiting.is_some() || (frozen && !look.question_open);
    if look.rising && look.requested.is_some_and(|asked| look.now.saturating_sub(asked) > debounce_ms) && stuck {
        return Some(Cut::SecondPress);
    }
    None
}

fn watch(state: &ExitState, second: Option<SecondPress>, grace: Duration, then: fn()) {
    let poll = second.map_or(Duration::from_millis(QUIT_POLL_MS), |s| s.poll);
    let debounce_ms = second.map_or(QUIT_DEBOUNCE_MS, |s| u64::try_from(s.debounce.as_millis()).unwrap_or(u64::MAX));
    let grace_ms = u64::try_from(grace.as_millis()).unwrap_or(u64::MAX);
    // The chord counts only while this app is the one in front: the probe reads the whole session's keys, and a Cmd+Q meant for
    // another app is not a quit of this one.
    let pressed = |second: Option<SecondPress>| second.is_some_and(|s| (s.down)()) && state.app_active();
    // A chord still held from the first press is not a second one: only a press after it was let go is.
    let mut was_down = pressed(second);
    loop {
        std::thread::sleep(poll);
        say_the_overdue();
        let down = pressed(second);
        let rising = down && !was_down;
        was_down = down;
        let now = state.now();
        let (beat, requested, exiting) = state.times();
        // A press while the app is frozen is a quit request that the frozen app cannot read.
        if rising && requested.is_none() && now.saturating_sub(beat) >= FROZEN_AFTER_MS {
            state.request();
            continue;
        }
        // An answered *Stay*, with the app drawing: forgotten, so a later freeze is not read as a stuck quit.
        if let (None, Some(asked)) = (exiting, requested)
            && now.saturating_sub(beat) < FROZEN_AFTER_MS
            && now.saturating_sub(asked) > REQUEST_FORGET_S * 1_000
        {
            state.forget_the_request();
            continue;
        }
        let look = Look { now, beat, requested, exiting, rising, question_open: state.question_open() };
        if let Some(why) = verdict(&look, debounce_ms, grace_ms) {
            cut(why, grace, then);
            return;
        }
    }
}

/// **Ends the process, recording where it is stuck first** unless a second press asked for it (that person has waited long
/// enough): `sample` and `kill` are what the binary uses, and a test hands in its own to see the order.
pub fn end_process_with(why: Option<Cut>, sample: fn(), kill: fn()) {
    if why != Some(Cut::SecondPress) {
        sample();
    }
    kill();
}

/// **Ends this process now**, after recording where it is stuck (unless a second press asked for it).
///
/// `SIGKILL` and not `exit`: `exit` runs the process's exit hooks, and the hooks of a process that is stuck
/// in a destructor may be the thing it is stuck in. Its child shells get their hangup from the PTY closing
/// with it.
pub fn end_this_process() {
    end_process_with(last_cut().map(|cut| cut.why), sample_this_process, kill_this_process);
}

fn kill_this_process() {
    let _ = rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL);
    // Unreachable once the signal is delivered; here so that a refused signal still ends the process.
    std::process::exit(1);
}

/// **`sample` of this process into `exit-hang.txt` beside the log**, for at most [`SAMPLE_PATIENCE_MS`].
#[cfg(target_os = "macos")]
fn sample_this_process() {
    use std::process::{Command, Stdio};
    let Some(file) = crate::log_file::user_file().map(|log| log.with_file_name("exit-hang.txt")) else {
        return;
    };
    let Ok(mut sampling) = Command::new("/usr/bin/sample")
        .arg(std::process::id().to_string())
        .arg("1")
        .arg("-file")
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(SAMPLE_PATIENCE_MS);
    while std::time::Instant::now() < deadline {
        match sampling.try_wait() {
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Ok(Some(_)) | Err(_) => return,
        }
    }
    let _ = sampling.kill();
}

/// Only a Mac has `sample`; elsewhere there is nothing to record with.
#[cfg(not(target_os = "macos"))]
fn sample_this_process() {}
