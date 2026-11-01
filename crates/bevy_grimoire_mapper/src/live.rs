//! **`--live`: the editor writes every committed change through to disk.**
//!
//! A game that watches its grimoire project (`bevy_grimoire_core::live::FileWatch`) reloads when a file
//! changes, so the only thing between an edit and the running game is the editor's save. The
//! registered operations already save as they commit; the keys that are not yet operations (lift,
//! tip, move, straighten, the editor's own undo) change the map in memory and leave it unsaved until
//! `Cmd+S`. With `--live`, every frame that ends with the map dirty saves it, after every edit of the
//! frame has run.
//!
//! **No second write path.** The write is `OpenMap::save`, the same validation and the same atomic
//! write the save key uses, and the editor's own file watcher reads its own write back as the map it
//! already holds (`watch::adopt_map` calls `mark_saved`). A map that fails to save stays dirty, so the
//! next valid state is written; the refusal is logged once, not every frame.

use std::time::{Duration, Instant, UNIX_EPOCH};

use bevy::prelude::*;
use bevy_grimoire_core::live::{KindVerdict, Verdict};
use serde_json::{Value, json};

use crate::project::{OpenMap, Project};

/// The write-through. Inert unless [`LiveWrite`] is inserted.
pub struct LivePlugin;

/// **Present when the editor runs with `--live`.** `main` inserts it; a test inserts it by hand.
#[derive(Resource)]
pub struct LiveWrite;

impl Plugin for LivePlugin {
    fn build(&self, app: &mut App) {
        // `PostUpdate`, so every `Update` dispatch and undo of the frame has landed first.
        app.init_resource::<LiveGame>().init_resource::<EditorReloads>().add_systems(
            PostUpdate,
            write_through.run_if(resource_exists::<LiveWrite>),
        );
        // Four looks a second at the game's verdict, only with `--live`.
        app.add_systems(Update, poll_verdict.run_if(resource_exists::<LiveWrite>));
    }
}

/// **The game's newest verdict on a reload**, as last read from `.live/game.ron`. Empty without
/// `--live` and while no game has answered.
#[derive(Resource, Default)]
pub struct LiveGame {
    pub verdict: Option<Verdict>,
    /// The `at_unix_ms` already shown in the journal, so one verdict is reported once.
    reported_ms: u64,
}

impl LiveGame {
    /// **The `game` field of `grimoire_state`.** `null` with no verdict; `"pending"` when the newest
    /// verdict is not newer than the map file's last write (it answers an earlier write, never the
    /// new one); otherwise the verdict, with one entry in `kinds` for every file the game read: its
    /// kind, path, `applied` or `refused`, and for a refusal the field and the reason (#200).
    pub fn state_json(&self, map_path: Option<&std::path::Path>) -> Value {
        let Some(v) = &self.verdict else {
            return Value::Null;
        };
        let wrote_ms = map_path
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis() as u64);
        if v.at_unix_ms < wrote_ms {
            return json!("pending");
        }
        let kinds: Vec<Value> = v
            .kinds
            .iter()
            .map(|k| json!({"kind": k.kind, "file": k.file, "outcome": k.outcome.word(), "field": k.field, "reason": k.reason}))
            .collect();
        json!({"ok": v.ok, "message": v.message, "at_unix_ms": v.at_unix_ms, "seq": v.seq, "kinds": kinds})
    }
}

/// **The editor's own verdicts on the files it watches** (#200, H3): what became of each file the
/// editor reads and draws from, as the game's [`Verdict`] says it for the game's.
///
/// A watcher that reads a file decides it once per edit (it only looks again when the file's
/// modification time moves) and hands the decision to [`Self::decide`]. One decision lists every file
/// that watcher read, because the editor keeps the last good state when any of them is refused. `seq`
/// counts decisions, so one edit is one number however many frames pass.
#[derive(Resource, Default)]
pub struct EditorReloads {
    seq: u64,
    kinds: Vec<KindVerdict>,
}

impl EditorReloads {
    /// **Record one decision**: each verdict replaces the earlier one for the same kind and file.
    pub fn decide(&mut self, verdicts: Vec<KindVerdict>) {
        self.seq += 1;
        for v in verdicts {
            match self.kinds.iter_mut().find(|k| k.kind == v.kind && k.file == v.file) {
                Some(slot) => *slot = v,
                None => self.kinds.push(v),
            }
        }
    }

    /// Forget the verdicts of these kinds, when the files they named belong to a map no longer open.
    pub fn forget(&mut self, kinds: &[&str]) {
        self.kinds.retain(|k| !kinds.contains(&k.kind.as_str()));
    }

    /// Forget the verdict of one file, when it is gone: a file a watcher no longer finds is no longer
    /// refused, and its kind may still hold other files.
    pub fn forget_file(&mut self, kind: &str, file: &str) {
        self.kinds.retain(|k| !(k.kind == kind && k.file == file));
    }

    /// The newest verdict for every file, in the order they were first decided.
    pub fn kinds(&self) -> &[KindVerdict] {
        &self.kinds
    }

    /// **The `reloads` field of `grimoire_state`**: `ok` (every file applied), `seq`, and one entry per
    /// file with its kind, path, `applied`, `refused` or `needs_restart`, and for the last two the
    /// reason, and for a refusal the field.
    pub fn state_json(&self) -> Value {
        let kinds: Vec<Value> = self
            .kinds
            .iter()
            .map(|k| json!({"kind": k.kind, "file": k.file, "outcome": k.outcome.word(), "field": k.field, "reason": k.reason}))
            .collect();
        let ok = self.kinds.iter().all(|k| k.outcome == bevy_grimoire_core::live::Outcome::Applied);
        json!({"ok": ok, "seq": self.seq, "kinds": kinds})
    }
}

/// Read the verdict four times a second; say a new one in the journal (a refusal as a problem).
fn poll_verdict(
    project: Option<Res<Project>>,
    mut game: ResMut<LiveGame>,
    state: Option<ResMut<crate::editor::EditorState>>,
    mut last: Local<Option<Instant>>,
    mut last_error: Local<Option<String>>,
) {
    let Some(project) = project else { return };
    if last.is_some_and(|t| t.elapsed() < Duration::from_millis(250)) {
        return;
    }
    *last = Some(Instant::now());
    let v = match Verdict::read(&project.grimoire_dir) {
        Ok(v) => v,
        Err(e) => {
            if last_error.as_deref() != Some(e.as_str()) {
                warn!("live: {e}");
                *last_error = Some(e);
            }
            return;
        }
    };
    *last_error = None;
    let Some(v) = v else { return };
    if v.at_unix_ms > game.reported_ms {
        game.reported_ms = v.at_unix_ms;
        if let Some(mut state) = state {
            if v.ok {
                state.status.note(format!("game: {}", v.message));
            } else {
                let refused: Vec<String> = v
                    .kinds
                    .iter()
                    .filter(|k| k.outcome == bevy_grimoire_core::live::Outcome::Refused)
                    .map(|k| k.sentence())
                    .collect();
                let why = if refused.is_empty() { v.message.clone() } else { refused.join("; ") };
                state.status.problem(format!("game refused the reload: {why}"));
            }
        }
    }
    if game.verdict.as_ref() != Some(&v) {
        game.verdict = Some(v);
    }
}

/// Save the open map whenever it is dirty, and say what happened.
fn write_through(
    open: Option<ResMut<OpenMap>>,
    project: Option<Res<Project>>,
    mut last_refusal: Local<Option<String>>,
) {
    let (Some(mut open), Some(project)) = (open, project) else {
        return;
    };
    // Read through the immutable view first, so a clean frame does not mark the map changed.
    if !open.as_ref().dirty() {
        return;
    }
    match open.save(&project) {
        Ok(()) => {
            info!("live: wrote {}", open.map_path.display());
            *last_refusal = None;
        }
        Err(e) => {
            if last_refusal.as_deref() != Some(e.as_str()) {
                warn!("live: `{}` not written: {e}", open.map.name);
                *last_refusal = Some(e);
            }
        }
    }
}
