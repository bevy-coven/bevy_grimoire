//! **The pictures the open map references, as the editor looks at them** (#200, H3).
//!
//! A reference (`reference_set`) is a project-relative picture an agent is told to look at. Nothing in
//! this repository decodes it, so a file that a full disk cut off, that a text saved over or that was
//! renamed from another format cost the agent a picture it could not open, and the author a sentence
//! nobody said. One system looks at the map's `references.ron` and every picture it lists when one of
//! them moved, and decides into `grimoire_state.reloads`, kind `reference_images`, per file:
//!
//! - `refused` with the file and the header field (`signature`, `format`, `header`, `width`, `height`;
//!   [`bevy_grimoire_core::image_header`]) and why, when it is no longer the picture its name says;
//! - `applied` when a refused picture is a picture again, or an edit left one that reads as other
//!   dimensions or another format.
//!
//! **A picture first seen reading is no news**: opening a map, or adding a reference, is not an edit.
//! **A listed picture that is not there is not decided here**: `grimoire_state.references` already says
//! `missing: true` for it, and a project may name a file it is about to add. **One edit is one verdict**:
//! a look decides only what differs from the look before, and everything one look finds is one `seq`.
//!
//! The pixels are never decoded and the file is never written: only the first megabyte is read, and only
//! when the file's size or modification time moved.

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
    /// Every file looked at (the references file first) and its size and modification time then.
    stats: Vec<(PathBuf, Stat)>,
    looked: bool,
    /// By the picture's path as the reference names it: what the last look found.
    last: BTreeMap<String, Look>,
}

fn stat_of(path: &Path) -> Stat {
    std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// The start of a file, [`image_header::HEAD`] bytes of it at most.
fn head_of(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(image_header::HEAD as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// A reference path that stays inside the project: `normalise` makes every path the editor writes one, and
/// a hand edit of the file may not be.
fn inside(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}

/// **Look at the open map's referenced pictures, and decide what differs from the look before.**
///
/// Looked at every frame by size and modification time, and read again when one moved. A map that is
/// no longer open, or another one, clears every verdict this view made.
pub fn sync_reference_view(
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
    let stats: Vec<(PathBuf, Stat)> = if same {
        view.stats.iter().map(|(p, _)| (p.clone(), stat_of(p))).collect()
    } else {
        vec![(refs_file.clone(), None)]
    };
    if same && view.stats == stats {
        return;
    }
    if !same {
        view.refs_file = refs_file.clone();
        view.root = root.clone();
        view.last.clear();
        view.looked = false;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&[KIND]);
        }
    }

    let mut paths = vec![refs_file.clone()];
    let mut now: BTreeMap<String, Look> = BTreeMap::new();
    // A references file that does not read is the `references` kind's to say; nothing is listed from it.
    for r in References::load(&refs_file).map(|r| r.refs).unwrap_or_default() {
        if !inside(&r.path) {
            continue;
        }
        let path = root.join(&r.path);
        paths.push(path.clone());
        match head_of(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                now.insert(r.path, Look::Refused { field: None, reason: e.to_string() });
            }
            Ok(bytes) => {
                let look = match image_header::read(&r.path, &bytes) {
                    Ok(header) => Look::Reads(header),
                    Err(refusal) => Look::Refused { field: refusal.field, reason: refusal.reason },
                };
                now.insert(r.path, look);
            }
        }
    }
    view.stats = paths.iter().map(|p| (p.clone(), stat_of(p))).collect();
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
