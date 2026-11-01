//! **The project's own checks, as the editor holds them** (#200, H3).
//!
//! `Checks::for_project` reads `assets/grimoire/checks.ron` fresh on every run, so one bad edit (a hand
//! edit, a truncated write, an agent's mistake) made every after-edit check silently not run: the
//! journal said "not run" with the parser's position and nothing said which field was wrong, and the
//! second reader dropped the error altogether. Now one system looks at the file, decides each edit
//! once, and holds the last good [`Checks`]: `grimoire_state.reloads` says `applied`, or `refused`
//! with the file and the field, and the checks the editor asks keep running as the last good ones
//! define them until the file reads again.
//!
//! The file is looked at every frame by its size and modification time, and read again when either
//! moved, or once a second regardless, because two writes inside one coarse timestamp tick look
//! alike. A read that finds the same text as the last one decides nothing, so one edit is one verdict
//! however many frames pass.
//!
//! The type is the decision crate's, above core, so [`bevy_grimoire_core::reload_check::check`] cannot
//! read it; [`bevy_grimoire_core::reload_check::read`] names the field of a file that does not parse,
//! and [`Checks::from_ron`] says why a file that did parse breaks a rule (two checks with one name, a
//! threshold outside 0..=1), which has no one field.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::live::KindVerdict;
use bevy_grimoire_core::reload_check;
use bevy_grimoire_decide::{Checks, Error as DecideError, PROJECT_CHECKS_FILE};

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::Project;

/// How long a file whose size and modification time did not move goes unread.
const RECHECK_SECS: f32 = 1.0;

/// **The last good checks of the open project**, and what the view last saw of their file.
#[derive(Resource, Default)]
pub struct ChecksView {
    /// The `checks.ron` this holds the checks of; another project's file starts the view afresh.
    path: PathBuf,
    stat: Option<(SystemTime, u64)>,
    /// The text last read, good or refused, so the same bytes are decided once.
    text: Option<String>,
    since: f32,
    /// The editor's checks with the project's over them, as of the last read that was good (no file is
    /// the editor's alone). `None` until one: a file that was broken from the first look has no earlier
    /// good state, so [`checks_for`] falls back to the reader, which says why.
    good: Option<Checks>,
}

/// `<project root>/assets/grimoire/checks.ron`, the path `Checks::for_project` reads.
fn path_in(root: &Path) -> PathBuf {
    root.join("assets").join("grimoire").join(PROJECT_CHECKS_FILE)
}

/// **The checks to run for the project at `root`**: the last good ones the view holds for its file, or
/// a fresh read when the view holds none (it has not looked yet, or the file was broken at the first
/// look). A refused edit therefore leaves the checks as they were instead of stopping them.
pub fn checks_for(view: Option<&ChecksView>, root: &Path) -> Result<Checks, String> {
    match view.filter(|v| v.path == path_in(root)).and_then(|v| v.good.clone()) {
        Some(good) => Ok(good),
        None => Checks::for_project(root),
    }
}

/// **A file's text as the checks it defines, or why not**: the field it is wrong at when it did not
/// parse (`None` for a rule it breaks after parsing, which the reason names), and the reason.
fn read(file: &str, text: &str) -> Result<Checks, (Option<String>, String)> {
    let e = match Checks::from_ron(text) {
        Ok(checks) => return Ok(checks),
        Err(e) => e,
    };
    let reason = match e {
        DecideError::Checks(m) => m,
        other => other.to_string(),
    };
    // The decision crate's reader says where in the text, not which field; the shared one says both for
    // a file that does not parse. A file that parses and still refuses broke a rule: no field.
    match reload_check::read::<Checks>("checks", file, text) {
        Err(r) => Err((r.field, r.reason)),
        Ok(_) => Err((None, reason)),
    }
}

/// **Read the project's `checks.ron` when it changed, and decide it once.**
///
/// A file that reads replaces the held checks; one that is refused (not RON, a wrong type, a missing
/// field, two checks of one name, a threshold outside 0..=1) leaves them; a file that is gone is the
/// editor's checks alone and clears its verdict. A refusal names the file as the project's directory
/// sees it and the field it is wrong at.
pub fn sync_checks_view(
    time: Res<Time>,
    project: Option<Res<Project>>,
    mut view: ResMut<ChecksView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(project) = project else { return };
    let path = path_in(&project.root);
    view.since += time.delta_secs();
    let stat = std::fs::metadata(&path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
    let same_file = view.path == path;
    if same_file && view.stat == stat && view.since < RECHECK_SECS {
        return;
    }
    view.since = 0.0;
    let text = std::fs::read_to_string(&path).ok();
    if same_file && view.text == text {
        view.stat = stat;
        return;
    }
    if !same_file {
        // Another project has no earlier value of its own: nothing carries over.
        view.good = None;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["checks"]);
        }
    }
    view.path = path.clone();
    view.stat = stat;
    view.text = text.clone();
    let Some(text) = text else {
        // No file is the editor's own checks, which are valid; the edit that deleted a refused file
        // is the one that ends its refusal.
        view.good = Some(Checks::builtin());
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["checks"]);
        }
        return;
    };
    let file = path.strip_prefix(&project.project_dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
    let verdict = match read(&file, &text) {
        Ok(own) => {
            view.good = Some(Checks::builtin().merged(own));
            KindVerdict::applied("checks", file)
        }
        Err((field, reason)) => {
            let refused = KindVerdict::refused("checks", file, field, reason);
            if let Some(state) = state.as_deref_mut() {
                let kept = if view.good.is_some() { "the checks already held stay" } else { "no project checks run until it reads" };
                state.status.problem(format!("{}: refused, so {kept}", refused.sentence()));
            }
            refused
        }
    };
    if let Some(reloads) = reloads.as_deref_mut() {
        reloads.forget(&["checks"]);
        reloads.decide(vec![verdict]);
    }
}
