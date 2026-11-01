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
    /// **Read the marks from their file's text.**
    pub fn parse(text: &str) -> Result<Marks, String> {
        ron::from_str(text).map_err(|e| format!("destructible marks: {e}"))
    }
}

impl Bake {
    /// **Read a bake from its file's text.**
    pub fn parse(text: &str) -> Result<Bake, String> {
        ron::from_str(text).map_err(|e| format!("destructible bake: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_reads_back() {
        let marks = Marks { version: DESTRUCTIBLE_VERSION, pieces: vec!["site/cart".to_owned()] };
        assert_eq!(Marks::parse(&ron::to_string(&marks).unwrap()), Ok(marks));
        let bake = Bake { version: DESTRUCTIBLE_VERSION, key: u64::MAX, chunks: vec![Chunk { points: vec![[0.0, 0.5, 1.0]], faces: vec![vec![0, 0, 0]] }] };
        assert_eq!(Bake::parse(&ron::to_string(&bake).unwrap()), Ok(bake));
    }
}
