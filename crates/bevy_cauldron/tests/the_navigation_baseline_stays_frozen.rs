//! **The NV-01 baseline is kept, not overwritten** (#385, #388; series getting around, #377).
//!
//! `docs/ui/navigation-baseline.md` is the table the series started from: the routes as they were before the workspaces
//! moved the panels. `navigation.rs` now measures the live editor into `docs/ui/navigation.md`, so nothing regenerates the
//! baseline any more — and a document nothing checks is one a later `UPDATE_NAVIGATION_BASELINE=1` could rewrite without a
//! reader noticing. NV-11 (#388) compares the new model against it, so it has to be the old one.
//!
//! The pin is the file's length and an FNV-1a hash of its bytes. A deliberate change to the baseline (there should never be
//! one) is a change to these two numbers in the same commit, which is a visible act.

use std::path::Path;

use crate::fixtures;

/// FNV-1a over the file's bytes.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// The baseline's length in bytes, as NV-01 (#378) left it.
const BASELINE_BYTES: usize = 4036;
/// The baseline's FNV-1a hash, as NV-01 (#378) left it.
const BASELINE_HASH: u64 = 0x50ec_9816_002c_036e;

#[test]
fn the_baseline_is_there_and_is_the_one_nv_01_left() {
    let _ = fixtures::trusted_store; // this module is part of the editor test binary
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/navigation-baseline.md");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(bytes.len(), BASELINE_BYTES, "the baseline's length changed: it is frozen, and NV-11 compares against it");
    assert_eq!(fnv(&bytes), BASELINE_HASH, "the baseline's content changed: it is frozen, and NV-11 compares against it");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("# Getting around, as it is today"), "the baseline keeps its own title");
    assert!(text.contains("| Destination | Route | Keys | Pointer | Arrives |"), "the baseline keeps its table");
}
