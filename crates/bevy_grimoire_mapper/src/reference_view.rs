//! **The pictures the open map references, as the editor looks at them** (#200, H3).
//!
//! A reference (`reference_set`) is a project-relative picture an agent is told to look at. Nothing in
//! this repository decodes it, so a file that a full disk cut off, that a text saved over or that was
//! renamed from another format cost the agent a picture it could not open, and the author a sentence
//! nobody said. One system looks at the map's `references.ron` and every picture it lists when one of
//! them moved, or once a second, and decides into `grimoire_state.reloads`, kind `reference_images`, per
//! file:
//!
//! - `refused` with the file and the header field (`signature`, `format`, `header`, `width`, `height`;
//!   [`bevy_grimoire_core::image_header`]) and why, when it is no longer the picture its name says;
//! - `applied` when a refused picture is a picture again, or an edit left one that reads as other
//!   dimensions.
//!
//! **A picture first seen reading is no news**: opening a map, or adding a reference, is not an edit.
//! **A listed picture that is not there is not decided here**: `grimoire_state.references` already says
//! `missing: true` for it, and a project may name a file it is about to add. **One edit is one verdict**:
//! a look decides only what differs from the look before, and everything one look finds is one `seq`.
//!
//! The pixels are never decoded and the file is never written: a picture is read a few kilobytes at a
//! time (a JPEG's frame header can sit behind a large profile, so a header that is not reached is read
//! again from the first megabyte), and only when its size or modification time moved, or once a second
//! regardless (a file written in two steps, or twice inside one coarse timestamp tick, looks alike).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::image_header::{self, Header};
use bevy_grimoire_core::live::{KindVerdict, Outcome};
use bevy_grimoire_core::references::{self, References};

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::{OpenMap, Project};

/// The kind this view decides.
const KIND: &str = "reference_images";

/// How long pictures whose size and modification time did not move go unread.
const RECHECK_SECS: f32 = 1.0;

/// What a first read of a picture takes: every header but a JPEG's is inside it.
const FIRST_READ: usize = 4096;

/// What a look at one picture found.
#[derive(Clone, PartialEq)]
enum Look {
    /// It is the picture its name says.
    Reads(Header),
    /// It is not, or it could not be read.
    Refused { field: Option<String>, reason: String },
}

type Stat = Option<(SystemTime, u64)>;

/// **What the editor last made of the open map's referenced pictures.**
#[derive(Resource, Default)]
pub struct ReferenceView {
    /// The references file this describes; another map (or project) starts the view afresh.
    refs_file: PathBuf,
    /// The project's root the pictures are under.
    root: PathBuf,
    /// Every file looked at (the references file first) and its size and modification time **before** it
    /// was read, so a picture written while it was being read looks moved at the next frame.
    stats: Vec<(PathBuf, Stat)>,
    /// Seconds since the files were last read.
    since: f32,
    looked: bool,
    /// By the picture's path as the reference names it: what the last look found.
    last: BTreeMap<String, Look>,
}

fn stat_of(path: &Path) -> Stat {
    std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// The start of a file, `limit` bytes of it at most.
fn head_of(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(limit as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// **Look at one picture**, or `None` when it is not there. The first few kilobytes are read; a header that
/// ends before its width or height there is read again from as much as [`image_header::HEAD`] of the file,
/// because that is how a JPEG with a large profile in front of its frame header looks.
fn look_at(path: &Path, name: &str) -> Option<Look> {
    let mut bytes = match head_of(path, FIRST_READ) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(Look::Refused { field: None, reason: e.to_string() }),
    };
    let mut cut = bytes.len() == FIRST_READ;
    let mut read = image_header::read(name, &bytes);
    if cut && matches!(&read, Err(r) if matches!(r.field.as_deref(), Some("width" | "height"))) {
        match head_of(path, image_header::HEAD) {
            Ok(more) => {
                cut = more.len() == image_header::HEAD;
                bytes = more;
                read = image_header::read(name, &bytes);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => return Some(Look::Refused { field: None, reason: e.to_string() }),
        }
    }
    Some(match read {
        Ok(header) => Look::Reads(header),
        Err(refusal) => {
            let note = if cut && matches!(refusal.field.as_deref(), Some("width" | "height")) { " (only the first megabyte was read)" } else { "" };
            Look::Refused { field: refusal.field, reason: format!("{}{note}", refusal.reason) }
        }
    })
}

/// A reference path that stays inside the project: `normalise` makes every path the editor writes one, and
/// a hand edit of the file may not be.
fn inside(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}

/// **Look at the open map's referenced pictures, and decide what differs from the look before.**
///
/// Looked at every frame by size and modification time, and read again when one moved or once a second
/// regardless. A map that is no longer open, or another one, clears every verdict this view made.
pub fn sync_reference_view(
    time: Res<Time>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut view: ResMut<ReferenceView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let (Some(project), Some(open)) = (project, open) else {
        // A door the map is not open in: what was said about its pictures is not news any more.
        if view.looked {
            *view = ReferenceView::default();
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.forget(&[KIND]);
            }
        }
        return;
    };
    let refs_file = references::path_for_map(&open.map_path);
    let root = project.root.clone();
    let same = view.looked && view.refs_file == refs_file && view.root == root;
    view.since += time.delta_secs();
    let stats: Vec<(PathBuf, Stat)> = if same {
        view.stats.iter().map(|(p, _)| (p.clone(), stat_of(p))).collect()
    } else {
        vec![(refs_file.clone(), None)]
    };
    if same && view.stats == stats && view.since < RECHECK_SECS {
        return;
    }
    view.since = 0.0;
    if !same {
        view.refs_file = refs_file.clone();
        view.root = root.clone();
        view.last.clear();
        view.looked = false;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&[KIND]);
        }
    }

    // A references file that does not read is the `references` kind's to say; nothing is listed from it.
    let mut paths = vec![(refs_file.clone(), stat_of(&refs_file))];
    let mut now: BTreeMap<String, Look> = BTreeMap::new();
    for r in References::load(&refs_file).map(|r| r.refs).unwrap_or_default() {
        if !inside(&r.path) {
            continue;
        }
        let path = root.join(&r.path);
        // The stat is taken before the read: a write that lands during it moves the stat again.
        paths.push((path.clone(), stat_of(&path)));
        if let Some(look) = look_at(&path, &r.path) {
            now.insert(r.path, look);
        }
    }
    view.stats = paths;
    view.looked = true;

    let mut verdicts = Vec::new();
    for (file, look) in &now {
        let before = view.last.get(file);
        if before == Some(look) {
            continue;
        }
        let verdict = match look {
            // A picture seen reading for the first time is a baseline.
            Look::Reads(_) if before.is_none() => continue,
            Look::Reads(_) => KindVerdict::applied(KIND, file.clone()),
            Look::Refused { field, reason } => KindVerdict::refused(KIND, file.clone(), field.clone(), reason.clone()),
        };
        if verdict.outcome == Outcome::Refused
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        verdicts.push(verdict);
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        // A picture that is gone, or no longer listed, is no longer refused.
        for file in view.last.keys().filter(|f| !now.contains_key(*f)) {
            reloads.forget_file(KIND, file);
        }
        if !verdicts.is_empty() {
            reloads.decide(verdicts);
        }
    }
    view.last = now;
}
