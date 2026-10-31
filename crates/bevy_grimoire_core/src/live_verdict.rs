//! **The game's answer to a reload**, written where the editor can read it.
//!
//! A game that reloads its grimoire project when the editor writes it either takes the new map or
//! refuses it (a descriptor id that does not resolve, a plan that cannot be built). Without a trace
//! the editor, and an agent driving it, keeps editing a map the game stopped following. The game
//! writes a [`Verdict`] to `<grimoire_dir>/.live/game.ron`; the dot-directory is skipped by
//! [`super::FileWatch`], so this write never triggers a reload.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ron_surgery::save_atomic;

/// The schema this build reads and writes. Version 2 adds [`Verdict::seq`] and [`Verdict::kinds`]; a
/// version 1 file reads with both empty.
pub const VERDICT_VERSION: u32 = 2;

/// **What became of one file's change.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// The file parsed, validated, and what was made from it is in use.
    Applied,
    /// The file was refused and the previous state stays; [`KindVerdict::reason`] says why.
    Refused,
}

impl Outcome {
    /// The word `grimoire_state` and the journal use.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Applied => "applied",
            Outcome::Refused => "refused",
        }
    }
}

/// **One file's verdict**: its kind (an id from [`crate::reload::KINDS`], or `project` for a refusal
/// about how files fit together that belongs to no single one), its path under the project directory,
/// and what became of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindVerdict {
    pub kind: String,
    /// Relative to the project directory (`assets/grimoire`), `/`-separated; `.` for a `project` one.
    pub file: String,
    pub outcome: Outcome,
    /// The field the file is wrong at (`version`, `placements[3].yaw`), when one can be named.
    #[serde(default)]
    pub field: Option<String>,
    /// Why it was refused, with the position in the file; empty when applied.
    #[serde(default)]
    pub reason: String,
}

impl KindVerdict {
    pub fn applied(kind: impl Into<String>, file: impl Into<String>) -> KindVerdict {
        KindVerdict { kind: kind.into(), file: file.into(), outcome: Outcome::Applied, field: None, reason: String::new() }
    }

    pub fn refused(
        kind: impl Into<String>,
        file: impl Into<String>,
        field: Option<String>,
        reason: impl Into<String>,
    ) -> KindVerdict {
        KindVerdict { kind: kind.into(), file: file.into(), outcome: Outcome::Refused, field, reason: reason.into() }
    }

    /// `maps/yard.map.ron (placements[3].yaw): 1:47: Expected float`, for the journal.
    pub fn sentence(&self) -> String {
        match &self.field {
            Some(field) => format!("{} ({field}): {}", self.file, self.reason),
            None => format!("{}: {}", self.file, self.reason),
        }
    }
}

/// **What the game said about its latest reload.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub version: u32,
    /// Milliseconds since the Unix epoch when the game decided.
    pub at_unix_ms: u64,
    /// True when the reload landed.
    pub ok: bool,
    /// What the game said: the diff on success, the refusal's reason otherwise.
    pub message: String,
    /// Which decision this is, counted by the game from its start: one edit is one number, however
    /// many frames pass before the next. `0` in a version 1 file.
    #[serde(default)]
    pub seq: u64,
    /// Every file the game reads, with what became of it in this decision. Empty in a version 1 file.
    #[serde(default)]
    pub kinds: Vec<KindVerdict>,
}

impl Verdict {
    /// A verdict stamped now, with no per-file detail.
    pub fn now(ok: bool, message: impl Into<String>) -> Verdict {
        let at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        Verdict { version: VERDICT_VERSION, at_unix_ms, ok, message: message.into(), seq: 0, kinds: Vec::new() }
    }

    /// A verdict stamped now whose `ok` is that no file was refused.
    pub fn with_kinds(seq: u64, message: impl Into<String>, kinds: Vec<KindVerdict>) -> Verdict {
        let ok = kinds.iter().all(|k| k.outcome == Outcome::Applied);
        Verdict { seq, kinds, ..Verdict::now(ok, message) }
    }

    /// `<grimoire_dir>/.live/game.ron`.
    pub fn path(grimoire_dir: &Path) -> PathBuf {
        grimoire_dir.join(".live").join("game.ron")
    }

    /// Writes the verdict atomically, creating `.live/`.
    pub fn write(&self, grimoire_dir: &Path) -> Result<(), String> {
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("live verdict: {e}"))?;
        save_atomic(&Self::path(grimoire_dir), &text)
    }

    /// An absent file is `Ok(None)`; a file that does not parse, or comes from a newer version,
    /// is an `Err` naming its path.
    pub fn read(grimoire_dir: &Path) -> Result<Option<Verdict>, String> {
        let path = Self::path(grimoire_dir);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let v: Verdict =
            ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if v.version > VERDICT_VERSION {
            return Err(format!(
                "{}: version {} but this build reads {VERDICT_VERSION} - refusing a verdict \
                 written by a newer tool",
                path.display(),
                v.version
            ));
        }
        Ok(Some(v))
    }
}
