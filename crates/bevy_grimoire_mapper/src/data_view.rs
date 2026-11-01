//! **A game's data files, as the editor looks at them** (#200, H3).
//!
//! `<grimoire dir>/data.ron` registers the tuning files a game owns (`data_files`), and the Data panel
//! and `data_set` read them. This repository ships no reader that applies them: the registry classes
//! them `Restart`, a running game uses an edit only if it re-reads its data. So the editor owes the
//! author two things, and neither used to be said: that a hand edit which no longer reads (a cut-off
//! save, a bounded leaf that became a word) left the panel with no rows and nothing to say why, and that
//! an edit which does read has not reached a running game.
//!
//! One system looks at the registry and every file it lists when one of them moved, or once a second,
//! and decides into `grimoire_state.reloads`:
//!
//! - kind `game_data`, per file: `refused` with the file, the leaf and why when it does not pass
//!   [`data_files::check`] (or is registered and gone), and `needs_restart` when an edit leaves a file
//!   that reads;
//! - kind `data_registry`: `applied` when an edit leaves a registry that reads, `refused` with the field
//!   when it does not (the same reader `data_set` uses).
//!
//! **A file seen reading for the first time is no news**: opening a project, or registering a file, is
//! not an edit, so nothing is decided until a file changes or fails. **One edit is one verdict**: a look
//! decides only what differs from the look before, and everything one look finds is one `seq`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::data_files::{self, DataFiles};
use bevy_grimoire_core::live::{KindVerdict, Outcome};
use bevy_grimoire_core::{reload, reload_check};

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::Project;

/// How long files whose size and modification time did not move go unread.
const RECHECK_SECS: f32 = 1.0;

/// What a look at one file found.
#[derive(Clone, PartialEq)]
enum Look {
    /// It reads, and this is the hash of its text.
    Reads(u64),
    /// It does not, or the registry names it and it is not there.
    Refused { field: Option<String>, reason: String },
}

type Stat = Option<(SystemTime, u64)>;

/// **What the editor last made of the open project's data registry and the files it lists.**
#[derive(Resource, Default)]
pub struct DataView {
    /// The grimoire directory this describes; another project starts the view afresh.
    dir: PathBuf,
    /// Every file looked at (the registry first) and its size and modification time then.
    stats: Vec<(PathBuf, Stat)>,
    /// Seconds since the files were last read.
    since: f32,
    looked: bool,
    /// By the file as a verdict names it: its kind and what the last look found.
    last: BTreeMap<String, (&'static str, Look)>,
    /// By the file: the hash of the text first seen reading, kept while the project stays open even when a
    /// refused registry takes the file out of `last`.
    baseline: BTreeMap<String, u64>,
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn stat_of(path: &std::path::Path) -> Stat {
    std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// **Look at the registry and the files it lists, and decide what differs from the look before.**
///
/// Looked at every frame by size and modification time, and read again when one moved or once a second
/// regardless (two writes inside one coarse timestamp tick look alike). A registry that is gone clears
/// every verdict this view made.
pub fn sync_data_view(
    time: Res<Time>,
    project: Option<Res<Project>>,
    mut view: ResMut<DataView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(project) = project else { return };
    let dir = project.grimoire_dir.clone();
    view.since += time.delta_secs();
    let stats: Vec<(PathBuf, Stat)> = if view.dir == dir && view.looked {
        view.stats.iter().map(|(p, _)| (p.clone(), stat_of(p))).collect()
    } else {
        vec![(dir.join(data_files::REGISTRY_FILE), None)]
    };
    if view.dir == dir && view.looked && view.stats == stats && view.since < RECHECK_SECS {
        return;
    }
    view.since = 0.0;
    if view.dir != dir {
        view.dir = dir.clone();
        view.last.clear();
        view.baseline.clear();
        view.looked = false;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["data_registry", "game_data"]);
        }
    }

    let registry = dir.join(data_files::REGISTRY_FILE);
    let mut paths = vec![registry.clone()];
    let mut now: BTreeMap<String, (&'static str, Look)> = BTreeMap::new();
    let reg_name = crate::map_files::file_name(&project, &registry);
    match std::fs::read_to_string(&registry) {
        // No registry is no data files to say anything about.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            now.insert(reg_name.clone(), ("data_registry", Look::Refused { field: None, reason: e.to_string() }));
        }
        Ok(text) => {
            let kind = reload::kind_by_id("data_registry").expect("the registry kind is in KINDS");
            match reload_check::check(kind, &reg_name, &text) {
                reload_check::Check::Refused(r) => {
                    now.insert(reg_name.clone(), ("data_registry", Look::Refused { field: r.field, reason: r.reason }));
                }
                _ => {
                    now.insert(reg_name.clone(), ("data_registry", Look::Reads(hash_of(&text))));
                    if let Ok(registered) = DataFiles::parse(&text) {
                        for f in &registered.files {
                            let path = dir.join(&f.path);
                            paths.push(path.clone());
                            let name = crate::map_files::file_name(&project, &path);
                            let look = match std::fs::read_to_string(&path) {
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                    Look::Refused { field: None, reason: format!("`{}` is registered in data.ron and is not there", f.path) }
                                }
                                Err(e) => Look::Refused { field: None, reason: e.to_string() },
                                Ok(text) => match data_files::check(&name, f, &text) {
                                    Ok(()) => Look::Reads(hash_of(&text)),
                                    Err(r) => Look::Refused { field: r.field, reason: r.reason },
                                },
                            };
                            now.insert(name, ("game_data", look));
                        }
                    }
                }
            }
        }
    }
    view.stats = paths.iter().map(|p| (p.clone(), stat_of(p))).collect();
    view.looked = true;

    let view = &mut *view;
    let mut verdicts = Vec::new();
    for (file, (kind, look)) in &now {
        let before = view.last.get(file).map(|(_, l)| l);
        if before == Some(look) {
            continue;
        }
        let verdict = match look {
            Look::Reads(hash) if *kind == "game_data" => {
                // The text the editor first saw reading is what a game started with, as far as it can
                // tell, and it is the one a file can come back to: a net-zero edit leaves nothing waiting.
                // A file first seen refused has no baseline, so what mends it is news.
                match (view.baseline.get(file).copied(), before) {
                    (None, None) => {
                        view.baseline.insert(file.clone(), *hash);
                        continue;
                    }
                    // A file registered again unchanged: nothing is waiting.
                    (Some(b), None) if b == *hash => continue,
                    (Some(b), Some(_)) if b == *hash => KindVerdict::applied(*kind, file.clone()),
                    _ => KindVerdict::needs_restart(
                        *kind,
                        file.clone(),
                        "reads, and the editor uses it at once (the Data panel, `data_set`); this repository ships no reader that applies it to a running game, which uses it only if it re-reads its data, and otherwise at its next start",
                    ),
                }
            }
            // A registry seen reading for the first time is a baseline too.
            Look::Reads(_) if before.is_none() => continue,
            Look::Reads(_) => KindVerdict::applied(*kind, file.clone()),
            Look::Refused { field, reason } => KindVerdict::refused(*kind, file.clone(), field.clone(), reason.clone()),
        };
        if verdict.outcome == Outcome::Refused
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        verdicts.push(verdict);
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        for (file, (kind, _)) in &view.last {
            if !now.contains_key(file) {
                reloads.forget_file(kind, file);
            }
        }
        if !verdicts.is_empty() {
            reloads.decide(verdicts);
        }
    }
    view.last = now;
}
