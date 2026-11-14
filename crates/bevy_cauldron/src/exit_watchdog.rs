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
//! bounded in `terminal::pty`). So the first [`AppExit`] starts a thread that waits [`EXIT_GRACE_S`] seconds
//! and, if the process is still there, **records where it is stuck (`sample`, on a Mac) and ends it**. The
//! record is `exit-hang.txt` beside the log, so the next hang names its own cause.
//!
//! A step of the exit that can wait registers itself with [`waiting`], its name and its own bound, and the guard
//! it returns takes it off again when it finishes. The watchdog logs **the first wait that is past its bound, by
//! name** (once each), and when it ends the process the log says **which waits it cut short** ([`CutShort`],
//! [`last_cut`]). Waits registered today: the quit's own teardown (dropping the app: every terminal's shell, the agent
//! tab, BRP and the updater are in it, until each registers its own), the window's saved frame, and the log file.
//!
//! # A second press ends it
//!
//! A quit that is stuck is stuck on the main thread, so no key event reaches the app and the second press cannot be read
//! the way the first was. The watchdog thread therefore asks the operating system whether the quit chord is down
//! ([`SecondPress`], [`quit_chord_is_down`]: on a Mac, `CGEventSourceKeyState` for Cmd and Q, which any thread may ask
//! and which needs no permission) every [`QUIT_POLL_MS`] ms. A press **more than [`QUIT_DEBOUNCE_MS`] after the
//! quit began** ends the process at once (no `sample`: the person has already waited), and the log names the
//! waits it cut short. A press inside the debounce, and a key held down since the first press, are not a second
//! press. The unsaved-edits question comes before all of this: it is answered by the quit chord's own handler, and
//! only the answer writes the [`AppExit`] this watches for.
//!
//! **Only the binary adds it** (`main.rs`): a test editor writes [`AppExit`] and goes on, and a watchdog
//! there would end the test run.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use bevy::prelude::*;

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

/// Seconds `sample` may take to write its record, on a Mac, before it is given up on and the process is ended.
#[cfg(target_os = "macos")]
pub const SAMPLE_PATIENCE_S: u64 = 6;

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

/// **Is the quit chord down right now?** On a Mac, Cmd and Q by `CGEventSourceKeyState`, which reads the session's key
/// state from any thread and needs no permission. Elsewhere there is no look, so only the deadline ends a stuck quit.
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
    /// The quit had not finished in [`EXIT_GRACE_S`].
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
fn cut(why: Cut, then: fn()) {
    let waits = waits_now();
    let said = if waits.is_empty() { "no named wait is in progress".to_owned() } else { waits.join("; ") };
    match why {
        Cut::Deadline => warn!("quitting has not finished after {EXIT_GRACE_S} s; ending the process. Cut short: {said}"),
        Cut::SecondPress => warn!("the quit key was pressed again while quitting had not finished; ending the process at once. Cut short: {said}"),
    }
    *LAST_CUT.lock().unwrap_or_else(PoisonError::into_inner) = Some(CutShort { why, waits });
    then();
}

/// Set once the watchdog is running, so a second [`AppExit`] does not start a second; it holds the quit's own
/// teardown as a named wait, which the world's drop (the end of the exit) takes off.
#[derive(Resource)]
struct ExitWatchdogArmed(#[allow(dead_code)] ExitWait);

/// Adds [`ExitWatchdog`] and the system that arms it on the first [`AppExit`].
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
            .add_systems(Last, arm.run_if(on_message::<AppExit>).run_if(not(resource_exists::<ExitWatchdogArmed>)));
    }
}

fn arm(watchdog: Res<ExitWatchdog>, second: Option<Res<SecondPress>>, mut commands: Commands) {
    commands.insert_resource(ExitWatchdogArmed(waiting(
        "the quit's teardown (dropping the app: each terminal's shell, the agent tab, BRP, the updater)",
        Duration::from_secs(TEARDOWN_BOUND_S),
    )));
    if let Err(e) = arm_exit(watchdog.grace, second.as_deref().copied(), watchdog.then) {
        // A thread that cannot start leaves the quit as it was: no worse than before this existed.
        warn!("the exit watchdog did not start: {e}");
    }
}

/// **Runs `then` once `grace` has passed**, on a thread of its own that nothing waits for: a process that
/// exits in time takes it down with it, and one that does not is ended by it.
pub fn arm_after(grace: Duration, then: fn()) -> std::io::Result<std::thread::JoinHandle<()>> {
    arm_exit(grace, None, then)
}

/// **[`arm_after`], and also `then` when the quit chord is pressed again** after `second`'s debounce, if `second` is given.
/// The thread looks at the chord and at the waits' bounds every `poll`; it acts once and ends.
pub fn arm_exit(grace: Duration, second: Option<SecondPress>, then: fn()) -> std::io::Result<std::thread::JoinHandle<()>> {
    let armed = Instant::now();
    std::thread::Builder::new().name("exit-watchdog".to_owned()).spawn(move || {
        // A chord still held from the first press is not a second one: only a press after it was let go is.
        let mut was_down = second.is_some_and(|s| (s.down)());
        loop {
            let left = grace.saturating_sub(armed.elapsed());
            if left.is_zero() {
                cut(Cut::Deadline, then);
                return;
            }
            let poll = second.map_or(Duration::from_millis(QUIT_POLL_MS), |s| s.poll);
            std::thread::sleep(poll.min(left));
            say_the_overdue();
            if let Some(second) = second {
                let down = (second.down)();
                if down && !was_down && armed.elapsed() > second.debounce {
                    cut(Cut::SecondPress, then);
                    return;
                }
                was_down = down;
            }
        }
    })
}

/// **Ends this process now**, after recording where it is stuck (unless a second press asked for it: that person has
/// waited long enough).
///
/// `SIGKILL` and not `exit`: `exit` runs the process's exit hooks, and the hooks of a process that is stuck
/// in a destructor may be the thing it is stuck in. Its child shells get their hangup from the PTY closing
/// with it.
pub fn end_this_process() {
    if last_cut().is_none_or(|cut| cut.why != Cut::SecondPress) {
        sample_this_process();
    }
    let _ = rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL);
    // Unreachable once the signal is delivered; here so that a refused signal still ends the process.
    std::process::exit(1);
}

/// **`sample` of this process into `exit-hang.txt` beside the log**, for at most [`SAMPLE_PATIENCE_S`].
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
    let deadline = std::time::Instant::now() + Duration::from_secs(SAMPLE_PATIENCE_S);
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
