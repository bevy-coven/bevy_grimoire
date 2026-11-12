//! **The editor writes its log to a file, so a window that misbehaves leaves evidence** (#306).
//!
//! **Measured 2026-11-04, on `mac_air`:** a release DMG launched from Finder showed a black window, and
//! nothing anywhere said why. `log show` had no entries for the process, the app's own stdout went
//! nowhere (a bundle launched by LaunchServices has no terminal), and macOS wrote no crash report. The
//! editor's log existed the whole time — on a stderr nobody could read.
//!
//! So the same events go to two places: stdout as before (a terminal launch, and CI, keep reading it),
//! and one file next to `recent.yaml`, `<data_dir>/Grimoire/cauldron.log`, which any agent or person can
//! read over ssh with no display and no console app.
//!
//! Two things are logged that no `tracing` layer could catch, and each was the thing missing:
//!
//! * **The process starting** — version, executable, argv, OS — written before `DefaultPlugins` exists, so a
//!   build that dies while the window is made still leaves a first line, and a copy running from a mounted
//!   DMG (`/Volumes/...`) is told apart from the installed one (`/Applications/...`).
//! * **A panic**, through a hook that chains to the previous one: a Bevy system panics, is printed to
//!   stderr, and a bundle launch loses it. The hook forces a backtrace.
//!
//! How far startup got is ordinary `tracing` under the target `cauldron::startup` (`main`, `startup_probe`),
//! so it is timestamped and reaches the terminal and the file alike.
//!
//! **One file, capped.** The audit that produced `log_filter` watched this editor write 13.5 GB in five
//! minutes from one repeated line, so the file is truncated to [`MAX_BYTES`] at startup and the previous
//! one kept as `cauldron.log.1`. A launch cannot grow it without bound; only a run can, and
//! `log_filter::LOG_SILENCES` is what stops the one line that did.
//!
//! **Where:** `bevy_grimoire_core::user_dirs::log_file()`, beside `recent.yaml`, so the two are found together
//! and neither is inside a project (a project is a folder a person owns and shares). The MCP server's
//! `grimoire_read_log` reads the same path.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::prelude::Resource;

/// The log is truncated to this before a launch writes to it. 8 MB is a few days of editor use at the
/// default level and small enough to read whole.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Where the log lives for this user, or `None` on a platform with no data directory.
pub fn user_file() -> Option<PathBuf> {
    bevy_grimoire_core::user_dirs::log_file()
}

/// **Keep the file bounded, keeping the last one.** Truncating `path` when it is over `cap` bytes, and
/// first moving it to `path` with `.1` appended, so a person who reproduces a fault and then restarts
/// once still has the run that mattered.
///
/// A missing directory or file is fine: both are made when the log is opened.
pub fn rotate(path: &Path, cap: u64) -> std::io::Result<()> {
    let Ok(meta) = std::fs::metadata(path) else { return Ok(()) };
    if meta.len() <= cap {
        return Ok(());
    }
    let mut previous = path.as_os_str().to_owned();
    previous.push(".1");
    std::fs::rename(path, PathBuf::from(previous))?;
    Ok(())
}

/// The open file, shared by the layer and the panic hook.
type Sink = Arc<Mutex<File>>;

/// **One line per event, unbuffered.** A `MakeWriter` rather than a `Mutex<File>` so the file is a single
/// handle: two writers would interleave mid-line, and a shutdown would need to flush both.
struct OneSink(Sink);

/// The writer a single event takes.
struct Held<'a>(MutexGuard<'a, File>);

impl Write for Held<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl<'a> bevy::log::tracing_subscriber::fmt::writer::MakeWriter<'a> for OneSink {
    type Writer = Held<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        // A poisoned lock means a panic happened while logging; the file is still the best record, so it
        // is used rather than a panic inside a panic hook.
        Held(self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// The open log. A `Resource` as well as a value, because `LogPlugin::custom_layer` is a plain `fn` pointer
/// (it cannot capture), so the layer finds the file through the app's world ([`plugin_layer`]).
#[derive(Clone, Resource)]
pub struct LogFile(Sink);

impl LogFile {
    /// **Open (or make) the log, after rotating an oversized one.** `None` when the data directory does
    /// not exist on this platform, which is a fact about the platform, not a fault to report.
    pub fn open() -> Option<LogFile> {
        let path = user_file()?;
        Self::open_at(&path)
    }

    /// The same, at a named path: a test's log goes nowhere near the user's.
    pub fn open_at(path: &Path) -> Option<LogFile> {
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_err()
        {
            return None;
        }
        // A file that cannot be rotated is still appendable; a full disk must not stop the editor.
        let _ = rotate(path, MAX_BYTES);
        let file = OpenOptions::new().create(true).append(true).open(path).ok()?;
        Some(LogFile(Arc::new(Mutex::new(file))))
    }

    /// Write one line, prefixed with no timestamp: this is for the messages a `tracing` layer cannot
    /// carry (the process starting, a panic).
    pub fn line(&self, text: &str) {
        // A lock poisoned by a panic in logging is still a usable file: the line is worth more than the
        // poison flag. Nothing here may panic — this runs inside a panic hook.
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(file, "{text}");
        let _ = file.flush();
    }
}

/// **The layer that puts the editor's own events in the file.** Goes in `LogPlugin`'s `custom_layer`; the
/// events it receives are already filtered by that plugin (`log_filter::LOG_SILENCES` and the level), so
/// the file and the terminal say the same thing.
pub fn layer(log: &LogFile) -> bevy::log::BoxedLayer {
    use bevy::log::tracing_subscriber::layer::Layer;
    bevy::log::tracing_subscriber::fmt::layer()
        .with_ansi(false)
        // The wall clock is what a person comparing the log to what they did reads.
        .with_writer(OneSink(log.0.clone()))
        .boxed()
}

/// **`LogPlugin::custom_layer`'s target**: the file layer, when this run has a log at all.
///
/// `custom_layer` takes a `fn` pointer rather than a closure (a capture is not coercible to one), so the
/// file is found in the world, where `main` put it before `DefaultPlugins` was added.
pub fn plugin_layer(app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    app.world().get_resource::<LogFile>().map(layer)
}

/// **A panic reaches the file, and the terminal as before.** Chains to the hook already installed, so
/// nothing the standard library, Bevy or the OS was doing is lost.
pub fn install_panic_hook(log: LogFile) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log.line(&format!("PANIC {info}"));
        // A panic on a thread that is not the main one still has a backtrace worth having; without the
        // variable it is one line, which is the default the rest of the system has.
        let backtrace = std::backtrace::Backtrace::force_capture();
        log.line(&format!("{backtrace}"));
        previous(info);
    }));
}

/// The first line of every run: what was asked for, from where, on what.
pub fn starting_line(argv: &[String]) -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    format!(
        "--- Cauldron {} on {} {} starting: exe {:?}, argv {:?}, unix time {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        exe,
        argv,
        secs
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cauldron-log-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        dir.join("cauldron.log")
    }

    #[test]
    fn an_oversized_log_rotates_instead_of_growing() {
        let path = scratch("rotate");
        std::fs::write(&path, vec![b'x'; 64]).unwrap_or_else(|e| panic!("{e}"));
        rotate(&path, 16).unwrap_or_else(|e| panic!("{e}"));
        assert!(!path.exists(), "an oversized log is moved aside, not appended to");
        let mut previous = path.as_os_str().to_owned();
        previous.push(".1");
        assert!(PathBuf::from(previous).exists(), "the run that reproduced the fault is kept");
    }

    #[test]
    fn a_small_log_is_left_alone() {
        let path = scratch("small");
        std::fs::write(&path, vec![b'x'; 8]).unwrap_or_else(|e| panic!("{e}"));
        rotate(&path, 1024).unwrap_or_else(|e| panic!("{e}"));
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(size, 8);
    }

    #[test]
    fn a_line_reaches_the_file_at_once() {
        let path = scratch("line");
        let log = LogFile::open_at(&path).unwrap_or_else(|| panic!("a log at a writable path"));
        log.line("PANIC here");
        // Read without closing anything: the point is that a crash later keeps this line.
        let read = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(read, "PANIC here\n");
    }

    #[test]
    fn the_starting_line_names_the_version_the_executable_and_the_arguments() {
        let line = starting_line(&["./game".to_owned(), "--door".to_owned(), "map".to_owned()]);
        assert!(line.starts_with(&format!("--- Cauldron {} ", env!("CARGO_PKG_VERSION"))), "{line}");
        assert!(line.contains("exe "), "{line}");
        assert!(line.contains("--door"), "{line}");
    }
}
