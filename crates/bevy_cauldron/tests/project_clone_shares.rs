//! **A `Project` clone shares the library; it does not copy it** — E-051 (#117).
//!
//! Every click that dispatches an operation hands the operation context a clone of the project, so
//! the heavy half of a project (the measured library, the layered library, the masks, the kit
//! layers, the triangle counts) is shared behind an `Arc`, and a write through the clone copies
//! only what it writes and never reaches the original.

use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

fn project(name: &str) -> Project {
    let root = fixtures::Fixture::new(name)
        .flat_descriptor("road", 4.877, 20.0)
        .place("road", (0.0, 0.0))
        .build("m");
    let app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world().resource::<Project>().clone()
}

#[test]
fn a_clone_shares_every_heavy_field() {
    let live = project("clone_shares");
    let copy = live.clone();
    assert!(copy.measured.ptr_eq(&live.measured), "measured copied");
    assert!(copy.library.ptr_eq(&live.library), "library copied");
    assert!(copy.masks.ptr_eq(&live.masks), "masks copied");
    assert!(copy.kits.ptr_eq(&live.kits), "kits copied");
    assert!(copy.triangles.ptr_eq(&live.triangles), "triangles copied");
}

#[test]
fn a_write_through_a_clone_never_reaches_the_original() {
    let live = project("clone_writes");
    let before = live.library.descriptors.len();
    assert!(before > 0, "the fixture has a piece to remove");
    let mut scratch = live.clone();
    scratch.library.descriptors.clear();
    scratch.measured.descriptors.clear();
    assert_eq!(live.library.descriptors.len(), before);
    assert!(!live.measured.descriptors.is_empty());
    assert!(!scratch.library.ptr_eq(&live.library), "the write copied first");
}
