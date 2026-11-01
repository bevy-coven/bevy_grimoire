//! **What became of each file an author edited, as a running game and the editor say it** (#203, H6).
//!
//! After an edit the author asks one question: did the game take it? Until now the answer was in two
//! places: the game's verdict (`grimoire_state.game.kinds`, one entry per file the game read, newest
//! decision only) and the editor's own (`grimoire_state.reloads`), and neither said which files had no
//! answer at all. This joins them, one row per file of the open project, and says one of:
//!
//! - **`no_game_connected`**: no game has written a verdict to `<project>/.live/game.ron`, so nothing
//!   has answered any file. Every row says it, whatever the editor itself made of the file.
//! - **`pending`**: a game answered, but the file was written after its decision, so its answer is
//!   about an earlier text. The row keeps that decision's time, so "applied before this edit" is
//!   visible beside the edit's own time.
//! - **`game_applied`** and **`game_refused`** (with the field and the reason): the game's decision on
//!   the file's current text, with the time it decided.
//! - **`needs_restart`**: the game's decision says the file reads but the running program holds that
//!   kind from its start, or the editor says it of a kind it holds (`rigs`, `game_data`) while a game
//!   is connected.
//! - **`not_read`**: a game is connected and its last decision did not read the file (the notes, the
//!   plan and the family settings are the editor's own; a game has no reader for them).
//!
//! The editor's own verdict on the file (`EditorReloads`) rides beside it as `editor`, so a file the
//! editor refuses reads `no_game_connected` and `editor: refused` at once instead of one hiding the
//! other.
//!
//! **A connected game is one that has written a verdict.** The game writes nothing at start, so a game
//! that has closed leaves its last verdict standing, and the row's time is how stale it is. A heartbeat
//! would say more; this says what is on disk.
//!
//! The files are every file of the open project that [`bevy_grimoire_core::reload::KINDS`] names, found
//! on disk, plus any file a verdict names that is not there (a refused file that was deleted, a file
//! `data.ron` registers). Nothing here reads a file's text: it stats it.

use std::path::Path;
use std::time::UNIX_EPOCH;

use bevy_grimoire_core::live::{KindVerdict, Outcome, Verdict};
use bevy_grimoire_core::reload::{self, Base, KINDS};
use serde_json::{json, Value};

/// **What a running game made of one file.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    NoGame,
    Pending,
    Applied,
    Refused,
    NeedsRestart,
    NotRead,
}

impl GameState {
    /// The word `grimoire_state` uses.
    pub fn word(self) -> &'static str {
        match self {
            GameState::NoGame => "no_game_connected",
            GameState::Pending => "pending",
            GameState::Applied => "game_applied",
            GameState::Refused => "game_refused",
            GameState::NeedsRestart => "needs_restart",
            GameState::NotRead => "not_read",
        }
    }
}

/// **One file of the open project and what became of its latest edit.**
#[derive(Clone, Debug, PartialEq)]
pub struct FileReload {
    pub kind: String,
    /// Relative to the project directory for the project's files, to the game's root for a kind that
    /// lives there (`family/bevy_lights.ron`), as the verdicts name them.
    pub file: String,
    pub state: GameState,
    /// The field a refusal names, from the game's verdict, or the editor's for `needs_restart`.
    pub field: Option<String>,
    /// Why, for a refusal or a restart; empty otherwise.
    pub reason: String,
    /// When the game decided (ms since the epoch), for every state a game answered, `pending` included:
    /// there it is the decision the edit has overtaken. `None` when no game answered the file.
    pub at_ms: Option<u64>,
    /// When the file was last written, when it is on disk.
    pub modified_ms: Option<u64>,
    /// The editor's own verdict on the file, when it has decided one.
    pub editor: Option<KindVerdict>,
}

impl FileReload {
    fn json(&self) -> Value {
        let editor = self.editor.as_ref().map(|e| json!({"outcome": e.outcome.word(), "field": e.field, "reason": e.reason}));
        json!({
            "kind": self.kind,
            "file": self.file,
            "state": self.state.word(),
            "field": self.field,
            "reason": self.reason,
            "at_unix_ms": self.at_ms,
            "modified_unix_ms": self.modified_ms,
            "editor": editor,
        })
    }
}

fn ms_of(t: std::time::SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub fn now_ms() -> u64 {
    ms_of(std::time::SystemTime::now())
}

/// How deep a kind's pattern goes under its base, so the walk stops where no kind can match.
fn depth_of(base: Base) -> usize {
    KINDS.iter().filter(|k| k.base == base).map(|k| k.pattern.split('/').count()).max().unwrap_or(0)
}

/// Whether the walk goes into directory `name` under the game's root: only the ones a kind's path
/// starts with, because the root holds the whole game (`target/`, `.git`, every asset).
fn root_enters(name: &str) -> bool {
    KINDS.iter().any(|k| k.base == Base::Root && k.pattern.split_once('/').is_some_and(|(first, _)| first == name))
}

struct Listed {
    kind: &'static str,
    file: String,
    modified_ms: Option<u64>,
}

fn walk(dir: &Path, rel: &str, base: Base, depth: usize, out: &mut Vec<Listed>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // A hidden name is the game's own verdict directory or an atomic write's temporary file.
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let is_dir = match entry.file_type() {
            Ok(t) if t.is_symlink() => path.is_dir(),
            Ok(t) => t.is_dir(),
            Err(_) => false,
        };
        if is_dir {
            if depth > 1 && (base == Base::Grimoire || root_enters(&name)) {
                walk(&path, &file, base, depth - 1, out);
            }
        } else if let Some(kind) = reload::kind_of(base, &file) {
            let modified_ms = entry.metadata().ok().and_then(|m| m.modified().ok()).map(ms_of);
            out.push(Listed { kind: kind.id, file, modified_ms });
        }
    }
}

fn modified_of(project_dir: &Path, root: &Path, kind: &str, file: &str) -> Option<u64> {
    let from = |base: &Path| std::fs::metadata(base.join(file)).ok().and_then(|m| m.modified().ok()).map(ms_of);
    match reload::kind_by_id(kind).map(|k| k.base) {
        Some(Base::Root) => from(root).or_else(|| from(project_dir)),
        _ => from(project_dir).or_else(|| from(root)),
    }
}

/// **Every file of the open project with what became of its latest edit**, in file order.
///
/// `game` is the newest verdict read from `<project>/.live/game.ron`, `None` while no game has written
/// one; `editor` is the editor's own decisions ([`crate::live::EditorReloads::kinds`]).
pub fn files(project_dir: &Path, root: &Path, game: Option<&Verdict>, editor: &[KindVerdict]) -> Vec<FileReload> {
    let mut listed = Vec::new();
    walk(project_dir, "", Base::Grimoire, depth_of(Base::Grimoire), &mut listed);
    walk(root, "", Base::Root, depth_of(Base::Root), &mut listed);
    // A file a verdict names is a row even when it is not on disk or no kind's path matches it: a
    // refused file that was then deleted, a file `data.ron` registers, the `project` entry.
    let named = game.into_iter().flat_map(|v| v.kinds.iter()).chain(editor.iter());
    for k in named {
        if !listed.iter().any(|l| l.file == k.file) {
            let modified_ms = modified_of(project_dir, root, &k.kind, &k.file);
            let kind = reload::kind_by_id(&k.kind).map_or("project", |kind| kind.id);
            listed.push(Listed { kind, file: k.file.clone(), modified_ms });
        }
    }
    listed.sort_by(|a, b| a.file.cmp(&b.file));

    listed
        .into_iter()
        .map(|l| {
            let said = game.and_then(|v| v.kinds.iter().find(|k| k.file == l.file));
            let ed = editor.iter().find(|k| k.file == l.file && k.kind == l.kind).cloned();
            let restart_held = ed.as_ref().filter(|e| e.outcome == Outcome::NeedsRestart);
            let (state, at_ms, field, reason) = match (game, said) {
                (None, _) => (GameState::NoGame, None, None, String::new()),
                (Some(v), Some(k)) => {
                    let state = if l.modified_ms.is_some_and(|m| m > v.at_unix_ms) {
                        GameState::Pending
                    } else {
                        match k.outcome {
                            Outcome::Applied => GameState::Applied,
                            Outcome::Refused => GameState::Refused,
                            Outcome::NeedsRestart => GameState::NeedsRestart,
                        }
                    };
                    match state {
                        GameState::Pending => (state, Some(v.at_unix_ms), None, String::new()),
                        _ => (state, Some(v.at_unix_ms), k.field.clone(), k.reason.clone()),
                    }
                }
                (Some(_), None) => match restart_held {
                    Some(e) => (GameState::NeedsRestart, None, e.field.clone(), e.reason.clone()),
                    None => (GameState::NotRead, None, None, String::new()),
                },
            };
            FileReload { kind: l.kind.to_owned(), file: l.file, state, field, reason, at_ms, modified_ms: l.modified_ms, editor: ed }
        })
        .collect()
}

/// **The `reload_files` field of `grimoire_state`**: `game` says whether a game has answered
/// (`connected`) and when and as which decision, `now_unix_ms` is the clock the times are read against,
/// and `files` has one entry per file.
pub fn state_json(files: &[FileReload], game: Option<&Verdict>) -> Value {
    let game = match game {
        Some(v) => json!({"connected": true, "at_unix_ms": v.at_unix_ms, "seq": v.seq}),
        None => json!({"connected": false, "at_unix_ms": null, "seq": null}),
    };
    json!({"now_unix_ms": now_ms(), "game": game, "files": files.iter().map(FileReload::json).collect::<Vec<_>>()})
}

/// **How long ago**, as the panel says it: `2 s ago`, `3 min ago`, `1 h ago`.
pub fn ago(now_ms: u64, then_ms: u64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    match secs {
        0..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        _ => format!("{} h ago", secs / 3600),
    }
}

/// How a panel line is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Dim,
    Text,
    Accent,
    Danger,
}

/// **The lines of the Reload panel**: a header that says whether a game has answered, then one line for
/// each file that has news. A file nobody has an answer about is not listed; the header counts them.
pub fn panel_lines(files: &[FileReload], game: Option<&Verdict>, now_ms: u64) -> Vec<(String, Tone)> {
    let mut lines = Vec::new();
    match game {
        None => lines.push((format!("no game connected ({} files)", files.len()), Tone::Dim)),
        Some(v) => lines.push((format!("game decided {} (decision {})", ago(now_ms, v.at_unix_ms), v.seq), Tone::Dim)),
    }
    let when = |t: Option<u64>| t.map_or(String::new(), |t| format!(" {}", ago(now_ms, t)));
    let place = |field: &Option<String>| field.as_ref().map_or(String::new(), |f| format!(" ({f})"));
    for f in files {
        let game_says = match f.state {
            GameState::Applied => Some((format!("game applied{}", when(f.at_ms)), Tone::Text)),
            GameState::Refused => Some((format!("game refused{}: {} ·{}", place(&f.field), f.reason, when(f.at_ms)), Tone::Danger)),
            GameState::NeedsRestart if f.at_ms.is_some() => {
                Some((format!("needs restart{}: {} ·{}", place(&f.field), f.reason, when(f.at_ms)), Tone::Accent))
            }
            GameState::NeedsRestart => Some((format!("needs restart{}: {}", place(&f.field), f.reason), Tone::Accent)),
            GameState::Pending => Some((
                format!("edited{}, the game has not answered yet (its last answer was{})", when(f.modified_ms), when(f.at_ms)),
                Tone::Accent,
            )),
            GameState::NoGame | GameState::NotRead => None,
        };
        let editor_says = f.editor.as_ref().and_then(|e| match e.outcome {
            Outcome::Applied => None,
            Outcome::Refused => Some((format!("editor refused{}: {}", place(&e.field), e.reason), Tone::Danger)),
            Outcome::NeedsRestart => Some((format!("editor needs restart: {}", e.reason), Tone::Accent)),
        });
        // The game's own refusal or restart already says it, so the editor's rides beside any other answer.
        let (text, tone) = match (game_says, editor_says) {
            (Some(game), Some(_)) if matches!(f.state, GameState::Refused | GameState::NeedsRestart) => game,
            (Some((game, tone)), Some((editor, editor_tone))) => {
                (format!("{game} · {editor}"), if editor_tone == Tone::Danger { editor_tone } else { tone })
            }
            (Some(game), None) => game,
            (None, Some(editor)) => editor,
            (None, None) => continue,
        };
        lines.push((format!("{} · {text}", f.file), tone));
    }
    lines
}
