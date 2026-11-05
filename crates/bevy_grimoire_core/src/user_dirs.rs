//! **The per-user folders the editor keeps its own files in, defined once** (#306).
//!
//! The editor (`recent.ron`, `trusted.ron`, `settings.ron`, `cauldron.log`) and its MCP server (which reads
//! the log and writes captures when no project is named) must agree on these paths, so they are named here
//! and nowhere else. None of them is inside a project: a project is a folder a person owns and shares.
//!
//! `<data_dir>` is `dirs::data_dir()`: `~/.local/share` (or `$XDG_DATA_HOME`) on Linux,
//! `~/Library/Application Support` on macOS, `%APPDATA%` on Windows. `<config_dir>` is `dirs::config_dir()`.

use std::path::PathBuf;

/// The directory under the platform's data and config directories that holds the editor's own files.
pub const APP_DIR: &str = "Grimoire";

/// The editor's log, inside [`data_dir`].
pub const LOG_FILE: &str = "cauldron.log";

/// The folder under [`data_dir`] a capture goes to when no project is named.
const CAPTURES_DIR: &str = "captures";

/// The folder under [`data_dir`] the reports about the editor itself go to (`crate::feedback`).
const FEEDBACK_DIR: &str = "feedback";

/// `<data_dir>/Grimoire`, or `None` on a platform with no data directory.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP_DIR))
}

/// `<config_dir>/Grimoire`, or `None` on a platform with no config directory.
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(APP_DIR))
}

/// `<data_dir>/Grimoire/cauldron.log`: the file the editor logs to and the MCP server reads.
pub fn log_file() -> Option<PathBuf> {
    data_dir().map(|d| d.join(LOG_FILE))
}

/// `<data_dir>/Grimoire/captures`: where a capture goes when the MCP server follows the editor with no project.
pub fn captures_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join(CAPTURES_DIR))
}

/// `<data_dir>/Grimoire/feedback`: the reports the owner files about the editor itself.
///
/// **Not inside a project**, because a report about the editor is not about the project that
/// happened to be open when it was noticed — and because the home screen, where a project is a card
/// rather than an open thing, must be able to file one.
pub fn feedback_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join(FEEDBACK_DIR))
}
