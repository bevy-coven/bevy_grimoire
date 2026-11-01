//! `destructible.ron` and `destructible/<piece>.bake.ron`: which pieces of a project break, and the
//! chunks each one breaks into (#108).
//!
//! Engine-free: these are the two files' schemas and their readers. The fracture that makes the
//! chunks, and the audit that checks them, are the editor's (it is built with the crate that does
//! that); a hand edit has to be refused the same way whether or not it is.
//!
//! A bake is a cache, keyed by the piece's mesh and its cut settings, and a bake whose `version` or
//! `key` is not the one wanted is stale, not damaged: the editor bakes again. A file that does not
//! read at all is damaged, and is refused by name ([`crate::reload_check`]).

use serde::{Deserialize, Serialize};

/// The `version` a fresh marks file or bake is written with.
pub const DESTRUCTIBLE_VERSION: u32 = 1;

/// **`<project>/destructible.ron`**: the pieces marked destructible.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marks {
    pub version: u32,
    pub pieces: Vec<String>,
}

/// One chunk a piece breaks into: a closed convex solid as points and faces (indices into `points`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    pub points: Vec<[f32; 3]>,
    pub faces: Vec<Vec<u32>>,
}

/// **`<project>/destructible/<piece>.bake.ron`**: a piece's chunks, cached.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bake {
    pub version: u32,
    /// What the chunks were baked from: the mesh's bytes and the cut settings.
    pub key: u64,
    pub chunks: Vec<Chunk>,
}

impl Marks {
    /// **Read the marks from their file's text.** A file of another version is refused: `destructible_set`
    /// reads the marks, adds a piece and writes them back at this version, so a file from a newer build
    /// would be written back as this one's, and what the newer build put there lost. (A [`Bake`] of
    /// another version is not refused: it is a cache, stale and baked again.)
    pub fn parse(text: &str) -> Result<Marks, String> {
        let marks: Marks = ron::from_str(text).map_err(|e| format!("destructible marks: {e}"))?;
        if marks.version != DESTRUCTIBLE_VERSION {
            return Err(format!(
                "destructible marks: version {} is not the {DESTRUCTIBLE_VERSION} this program reads",
                marks.version
            ));
        }
        Ok(marks)
    }
}

impl Bake {
    /// **Read a bake from its file's text.**
    pub fn parse(text: &str) -> Result<Bake, String> {
        ron::from_str(text).map_err(|e| format!("destructible bake: {e}"))
    }
}
