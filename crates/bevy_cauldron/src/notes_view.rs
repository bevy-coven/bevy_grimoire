//! **The open map's notes, as the editor holds them** (#200, H3).
//!
//! The sign over every standing note and the Notes panel both draw from the map's `.notes.ron`. Each
//! used to read the file itself and take an empty list when it did not parse, so one bad edit (a hand
//! edit, a truncated write, an agent's mistake) made every sign and every row vanish until the file
//! was fixed. Now one system reads it, decides each edit once, and holds the last good [`Notes`]: a
//! refused file leaves what is drawn as it was and is named in `grimoire_state.reloads` (kind `notes`,
//! the file and the field) and on the status line.
//!
//! The file is looked at every frame by its size and modification time, and read again when either
//! moved, or once a second regardless, because two writes inside one coarse timestamp tick look
//! alike. A read that finds the same text as the last one decides nothing, so one edit is one
//! verdict however many frames pass.

use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::live::KindVerdict;
use bevy_grimoire_core::notes::{self, Notes};
use bevy_grimoire_core::reload::kind_by_id;
use bevy_grimoire_core::reload_check::{self, Check};

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::{OpenMap, Project};

/// How long a file whose size and modification time did not move goes unread.
const RECHECK_SECS: f32 = 1.0;

/// **The last good notes of the open map**, and what the view last saw of their file.
#[derive(Resource, Default)]
pub struct NotesView {
    /// The notes file this holds the notes of; another map's file starts the view afresh.
    path: PathBuf,
    stat: Option<(SystemTime, u64)>,
    /// The text last read, good or refused, so the same bytes are decided once.
    text: Option<String>,
    since: f32,
    notes: Notes,
}

impl NotesView {
    /// The notes the editor draws: the file's, as of the last edit that read, or the earlier good
    /// ones when the newest edit was refused. Empty for a map with no notes file.
    pub fn notes(&self) -> &Notes {
        &self.notes
    }
}

/// **Read the open map's notes file when it changed, and decide it once.**
///
/// A file that reads replaces the held notes; one that is refused ([`Notes::from_ron`]: not RON, the
/// wrong type, a field missing, a version this build does not read, an id twice) leaves them; a file
/// that is gone empties them and clears its verdict. A refusal names the file as the project's
/// directory sees it and the field it is wrong at.
pub fn sync_notes_view(
    time: Res<Time>,
    open: Option<Res<OpenMap>>,
    project: Option<Res<Project>>,
    mut view: ResMut<NotesView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(open) = open else { return };
    let path = notes::path_for_map(&open.map_path);
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
        // Another map has no earlier value of its own: nothing carries over.
        view.notes = Notes::default();
    }
    view.path = path.clone();
    view.stat = stat;
    view.text = text.clone();
    let Some(text) = text else {
        view.notes = Notes::default();
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["notes"]);
        }
        return;
    };
    let file = project
        .as_deref()
        .and_then(|p| path.strip_prefix(&p.project_dir).ok())
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let verdict = match Notes::from_ron(&text) {
        Ok(read) => {
            view.notes = read;
            KindVerdict::applied("notes", file)
        }
        Err(reason) => {
            let field = kind_by_id("notes").and_then(|k| match reload_check::check(k, &file, &text) {
                Check::Refused(r) => r.field,
                _ => None,
            });
            let refused = KindVerdict::refused("notes", file, field, reason);
            if let Some(state) = state.as_deref_mut() {
                let kept = if same_file { "the notes already shown stay" } else { "no notes are shown from it" };
                state.status.raise("", refused.problem(kept));
            }
            refused
        }
    };
    if let Some(reloads) = reloads.as_deref_mut() {
        // One decision lists the file read, so another map's verdict is not left standing beside it.
        reloads.forget(&["notes"]);
        reloads.decide(vec![verdict]);
    }
}
