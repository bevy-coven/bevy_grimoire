//! **Every pack on the Kits door starts folded** (`feedback-1793968919654`, filed against 0.2.9 on
//! `mac_air`; oracle `spec`).
//!
//! The note: *"These should be collapsable, should have a shortcut key, preferrably, same default
//! mappings as VSCode. They should default to collapsed."* Collapsing (a chevron per header) and the
//! key (`Space` on a focused pack) already exist; the gap is the default. The first scan opened every
//! pack the kit draws from, which on the Kits door is most of the shelf, so the first look was the
//! whole list.
//!
//! The spec as observable state after the first scan: every pack is folded except the one holding
//! the default selection, which stays open because a tab must never open with its selection hidden.

use bevy_cauldron::harness;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures::Fixture;

fn pack_of(mesh: &str) -> String {
    mesh.rsplit_once('/').map_or(".", |(d, _)| d).to_owned()
}

#[test]
fn after_the_first_scan_only_the_selections_pack_is_open() {
    // `alpha` and `beta` are both packs the kit draws from (each holds a library piece) and each
    // also holds an unimported mesh; `gamma` is a pack the kit does not touch.
    let root = Fixture::new("capture_default_folded")
        .descriptor("wall", "alpha")
        .descriptor("crate", "beta")
        .pack("alpha", &["extra_a"])
        .pack("beta", &["extra_b"])
        .pack("gamma", &["extra_c"])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let state = app.world().get_resource::<ImportState>().unwrap_or_else(|| panic!("no import state"));
    assert!(state.scanned, "entering the door must have scanned");

    let mut all: Vec<String> = state.candidates.iter().map(|c| pack_of(&c.mesh)).collect();
    all.sort();
    all.dedup();
    assert!(all.len() >= 3, "the fixture must offer several packs, got {all:?}");

    let selected = state
        .candidates
        .get(state.selected)
        .map(|c| pack_of(&c.mesh))
        .unwrap_or_else(|| panic!("the default selection points at nothing"));
    let open: Vec<&String> = all.iter().filter(|p| !state.folded_packs.contains(*p)).collect();
    assert_eq!(
        open,
        vec![&selected],
        "every pack should start folded except the selection's; folded: {:?}",
        state.folded_packs
    );
}
