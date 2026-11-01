//! **A file that an operation rewrites is read strictly** (#200, H3).
//!
//! `forest_scatter`, `person_set` and `trigger_set` read the map's file, change one row and write the
//! whole file back. Read leniently (a file that did not parse counted as no file), one typo in a hand
//! edit, or a half-written file, made the next stroke, person or trigger write a file holding only
//! itself over everything else in it. The reload contract says last good wins, so a file that does not
//! read refuses the operation with the file and the field, and the file stays exactly as it was.
//!
//! The reader is `bevy_grimoire_core::reload_check::read`, the one `check` shares for the kinds core
//! owns, so what applies and what is reported cannot disagree.

use std::path::Path;

use bevy_grimoire_core::reload_check;
use serde::de::DeserializeOwned;

use crate::ops::Refusal;
use crate::project::Project;

/// **The registry kinds this layer reads with a named refusal**: their types live here, or (`trees`) in
/// a family crate the editor is built with, so `reload_check::check` in core cannot. They are on core's
/// unchecked list and on no other.
pub const READ_HERE: &[&str] = &["forest", "people", "scripts", "trees"];

/// **The registry kinds a watcher in this layer reads with a named refusal** (`checks`: the decision
/// crate's type, held by [`crate::checks_view::ChecksView`]). No operation rewrites these files, so
/// they have no row in the table of operations that refuse to overwrite a file that does not read;
/// `tests/a_refused_checks_file_keeps_the_last_good_checks.rs` has their three bad files instead. Like
/// [`READ_HERE`] they are on core's unchecked list and on no other.
pub const WATCHED_HERE: &[&str] = &["checks"];

/// **The path of `path` as the project's directory sees it**: what a verdict and a refusal name.
pub fn file_name(project: &Project, path: &Path) -> String {
    path.strip_prefix(&project.project_dir).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// **`text`, the contents of the file at `path`, as a `T`**, and a refusal that names the file and the
/// field when it does not read.
pub fn read_text<T: DeserializeOwned>(kind: &'static str, project: &Project, path: &Path, text: &str) -> Result<T, Refusal> {
    reload_check::read(kind, &file_name(project, path), text).map_err(|r| {
        Refusal::new(format!("{}. The file is left as it is: fix it or remove it, then try again. Nothing was changed.", r.sentence()))
    })
}

/// **The file at `path` as a `T`**, `T::default()` when there is no file, and a refusal that names the
/// file (as the project's directory sees it) and the field when there is one that does not read.
pub fn read_or_empty<T: DeserializeOwned + Default>(kind: &'static str, project: &Project, path: &Path) -> Result<T, Refusal> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(e) => return Err(Refusal::new(format!("{} cannot be read: {e}. Nothing was changed.", path.display()))),
    };
    read_text(kind, project, path, &text)
}
