//! **A quit that does not finish is finished for it** (#337).
//!
//! Quitting writes [`AppExit`], the window goes, and the process is meant to follow. On 2026-11-07 it did
//! not, twice: the system log shows the window and the audio output torn down at 16:42:45 and the process
//! still running ten seconds later (`runningboardd` still computing its state), and its agent's `omp` child
//! alive at 16:49. Nothing was drawn and nothing answered, so a person saw a frozen app that had to be
//! killed, and an update, which quits the app so a script can swap the bundle, never completed.
//!
//! **Where the process was stuck is not known**: dropping the `World` runs every resource's destructor on
//! the main thread, and any of them can wait on a thread or a child (the terminal's PTY was one, now
//! bounded in `terminal::pty`). So this does not try to guess: the first [`AppExit`] starts a thread that
//! waits [`EXIT_GRACE_S`] seconds and, if the process is still there, **records where it is stuck
//! (`sample`, on a Mac) and ends it**. The record is `exit-hang.txt` beside the log, so the next hang names
//! its own cause.
//!
//! **Only the binary adds it** (`main.rs`): a test editor writes [`AppExit`] and goes on, and a watchdog
//! there would end the test run.

use std::time::Duration;

use bevy::prelude::*;

/// Seconds a quit has to finish before the process is ended: a window-less process that has not gone by then
/// is not going.
pub const EXIT_GRACE_S: u64 = 3;

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

/// Set once the watchdog is running, so a second [`AppExit`] does not start a second.
#[derive(Resource)]
struct ExitWatchdogArmed;

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

fn arm(watchdog: Res<ExitWatchdog>, mut commands: Commands) {
    commands.insert_resource(ExitWatchdogArmed);
    if let Err(e) = arm_after(watchdog.grace, watchdog.then) {
        // A thread that cannot start leaves the quit as it was: no worse than before this existed.
        warn!("the exit watchdog did not start: {e}");
    }
}

/// **Runs `then` once `grace` has passed**, on a thread of its own that nothing waits for: a process that
/// exits in time takes it down with it, and one that does not is ended by it.
pub fn arm_after(grace: Duration, then: fn()) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("exit-watchdog".to_owned()).spawn(move || {
        std::thread::sleep(grace);
        warn!("quitting has not finished after {grace:?}; ending the process");
        then();
    })
}

/// **Ends this process now**, after recording where it is stuck.
///
/// `SIGKILL` and not `exit`: `exit` runs the process's exit hooks, and the hooks of a process that is stuck
/// in a destructor may be the thing it is stuck in. Its child shells get their hangup from the PTY closing
/// with it.
pub fn end_this_process() {
    sample_this_process();
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
