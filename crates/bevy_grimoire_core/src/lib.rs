//! **bevy_grimoire_core** — the engine-free half of world building.
//!
//! Constraint IR, solver backends, Wave Function Collapse, Poisson/Delaunay geometry, and the seeded
//! RNG they all draw from. Nothing here imports `bevy`, and nothing here knows what an SCP is.
//!
//! # Why this is a crate and not a module
//!
//! It always was engine-free — `placement/ir.rs` has carried the sentence *"Nothing here imports
//! `bevy::`"* since long before this crate existed, and `wfc.rs`, `geom.rs` and `scatter.rs` each say
//! the same about themselves. But a comment cannot fail a build. Now the manifest can: `bevy_grimoire_core`
//! depends on `serde`, `ron`, `rand` and `rand_chacha`, so reaching for a `bevy` type or a game
//! concept from in here does not compile.
//!
//! That matters because three consumers need this code and only one of them is the game: the game
//! itself, the offline search (`bin/train`), and the standalone editor described in
//! `docs/2026-08-03-emerge-mapper-plan.md`. The editor is why the split happened now.
//!
//! # The module layout is deliberately unchanged
//!
//! `placement::ir`, `placement::solver`, `placement::solvers`, `placement::scatter` and
//! `placement::manifest` keep the paths they had inside the game, so every `crate::placement::ir::…`
//! inside these files still resolves and the move is reviewable as a pure `git mv`. The game
//! re-exports them at their old paths for the same reason — a workspace split that also rewrites a
//! thousand import lines is a split nobody can diff.
//!
//! # What stayed behind, and why
//!
//! `placement::furnish` is the Bevy boundary of the placement stack (it spawns entities);
//! `placement::mod` binds to the game's run lifecycle; `placement::anomalies` is SCP content. Those
//! are the game's, and `ir.rs:7` already named `furnish.rs` as the boundary before the split.

pub mod adjacency;
pub mod atomic_file;
pub mod census;
pub mod clips;
pub mod composition;
pub mod constraints;
pub mod data_files;
pub mod convert;
pub mod destructible;
pub mod descriptor;
pub mod draw_order;
pub mod family;
pub mod play;
pub mod gait;
pub mod geom;
pub mod glb;
pub mod grammar;
pub mod grid;
pub mod heightfield;
pub mod import;
pub mod kits;
pub mod library;
pub mod live;
pub mod map;
pub mod naming;
pub mod note_agent;
pub mod notes;
pub mod placement;
pub mod plan;
pub mod plot;
pub mod policy;
pub mod project_data;
pub mod range;
pub mod references;
pub mod reload;
pub mod reload_check;
pub mod relations;
pub mod repetition;
pub mod rig_check;
pub mod rigs;
pub mod rigs_edit;
pub mod rules;
pub mod run;
/// The deterministic RNG, re-exported at its original path.
///
/// It lives in the `bevy_grimoire_rng` crate — a leaf with no dependencies of its own, so every crate
/// here shares the same generator. Copying it instead would create a SECOND definition of the stream
/// every reproducibility claim here rests on, which is the one outcome that had to be avoided. This
/// alias means no caller moved.
pub use bevy_grimoire_rng as rng;
pub mod ron_surgery;
pub mod session;
pub mod smart;
pub mod stack;
pub mod standards;
pub mod vocab;
pub mod wfc;
pub mod volume;
pub mod water;

/// Hermite ease between two edges. Pure math, so it lives here rather than in a crate that has a
/// renderer — `bevy_grimoire_anim`'s weight blending and the game's own `util` both take it from one place.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
