//! **The operation log, as the editor looks at it** (#200, H3).
//!
//! `assets/grimoire/oplog.jsonl` is append-only text a person can edit and a crash can cut short. Every
//! reader of it (the undo walk, the steps feed, `grimoire/oplog_read`, the after-edit queue) skips a
//! line that is not an entry, so a damaged line cost the author what that line recorded and said
//! nothing about it: the journal counted "line(s) of the log could not be read" with no line and no
//! field, and an undo refused without naming the field.
//!
//! Nothing holds the log, so there is no last good value to keep; what the editor owes is the verdict.
//! One system looks at the file when its size or modification time moves and decides it into
//! `grimoire_state.reloads` as kind `oplog`: `applied` when every line reads as an [`Entry`], or
//! `refused` with the file, the line and the field of the first line that does not (the same reader an
//! undo uses, [`reload_check::read_lines`]), and the reason, which counts the lines after it.
//!
//! **One edit is one verdict.** The editor appends a line for every operation, and a log that still
//! reads is not news, so a look decides only when what it found differs from the look before: a log
//! that went from clean to damaged, from one damage to another, or back. Frames in between decide
//! nothing.

use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::live::KindVerdict;
use bevy_grimoire_core::reload_check;

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::ops::Entry;
use crate::project::Project;

/// **What a look at the log found.**
#[derive(Clone, PartialEq)]
enum Seen {
    /// No log yet: a project nobody has made a change in.
    Missing,
    /// Every line reads.
    Clean,
    /// A line does not: the first one, as `line 3.seq` (or `line 3`), and why.
    Damaged { field: String, reason: String },
    /// The file is there and cannot be read at all (not UTF-8, no permission).
    Unreadable(String),
}

/// **What the editor last made of the open project's operation log.**
#[derive(Resource, Default)]
pub struct OplogView {
    /// The log this describes; another project's log starts the view afresh.
    path: PathBuf,
    stat: Option<(SystemTime, u64)>,
    last: Option<Seen>,
}

/// **Look at the project's `oplog.jsonl` when it changed, and decide what it found once.**
///
/// A log that is gone clears the verdict, as a removed file ends any refusal of it.
pub fn sync_oplog_view(
    project: Option<Res<Project>>,
    mut view: ResMut<OplogView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(project) = project else { return };
    let path = crate::ops::oplog_path(&project);
    let stat = std::fs::metadata(&path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
    let same_file = view.path == path;
    if same_file && view.stat == stat && view.last.is_some() {
        return;
    }
    if !same_file {
        view.last = None;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["oplog"]);
        }
    }
    view.path = path.clone();
    view.stat = stat;
    let file = crate::map_files::file_name(&project, &path);
    let seen = match std::fs::read_to_string(&path) {
        Ok(text) => match reload_check::read_lines::<Entry>("oplog", &file, &text) {
            Ok(_) => Seen::Clean,
            Err(refusal) => Seen::Damaged { field: refusal.field.unwrap_or_default(), reason: refusal.reason },
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Seen::Missing,
        Err(e) => Seen::Unreadable(e.to_string()),
    };
    if view.last.as_ref() == Some(&seen) {
        return;
    }
    view.last = Some(seen.clone());
    let verdict = match seen {
        Seen::Missing => {
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.forget(&["oplog"]);
            }
            return;
        }
        Seen::Clean => KindVerdict::applied("oplog", file),
        Seen::Damaged { field, reason } => {
            let reason = format!("{reason}; the log's readers skip a line like it and an undo refuses until it is repaired");
            KindVerdict::refused("oplog", file, Some(field), reason)
        }
        Seen::Unreadable(why) => KindVerdict::refused("oplog", file, None, why),
    };
    if verdict.outcome != bevy_grimoire_core::live::Outcome::Applied
        && let Some(state) = state.as_deref_mut()
    {
        state.status.problem(format!("{}: refused", verdict.sentence()));
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        reloads.decide(vec![verdict]);
    }
}
