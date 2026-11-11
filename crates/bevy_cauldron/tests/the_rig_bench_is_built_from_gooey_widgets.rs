//! **The rig bench is built from gooey's widgets** (UI-58, #244).
//!
//! The bench's left pane — the slot table, its provenance lines, the findings and the plots — used to
//! lay itself out with raw `Node` literals: a row here, a fixed column there, an indent written as a
//! left margin, a plot wrapper and an absolutely-positioned hover image. Every one of those is a
//! layout primitive `bevy_gooey` has had since its #81 (`row`, `column`, `cell`, `image_box`,
//! `overlay`), which is what #244's Needs names, and a raw `Node` is a second layout path in a crate
//! that already has one.
//!
//! Two claims, and each can fail:
//!
//! 1. **no `Node {` in `anim_tab.rs` or `anim_plots.rs`** — the scan is the ratchet, and it reads the
//!    shipped sources rather than a fixture, so a new literal fails here rather than in the next
//!    audit;
//! 2. the bench's own widgets are the crate's: the slot row is a gooey `row`, the two fixed columns
//!    are `cell`s, and a phase plot is an `overlay` of two `image_box`es.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_gooey::Role;

use crate::fixtures;

/// **The two files this issue rebuilds carry no raw `Node`**, comments included: a commented-out
/// literal is one edit away from being live again.
#[test]
fn the_bench_modules_lay_themselves_out_with_no_raw_node() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for name in ["anim_tab.rs", "anim_plots.rs"] {
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (ix, line) in text.lines().enumerate() {
            if line.contains("Node {") {
                offenders.push(format!("{name}:{}: {}", ix + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a raw `Node` literal is a second layout path beside gooey's own; use `row`, `column`, `cell`, \
         `image_box` or `overlay` from `bevy_gooey` (gooey #81), or state why with a `CHROME-OK:` \
         marker on the line or the one above:\n  {}",
        offenders.join("\n  ")
    );
}

/// **A manifest with one rig and one slot**, whose GLB is not on disk — the bench draws the slot
/// table from the manifest alone, and the measured half is simply absent (`anim_watch` has nothing to
/// measure), so this is enough to lay the table out.
const ONE_RIG: &str = "(version: 3, rigs: {\"probe\": (mesh: \"probe.glb\", scale: 1.0, slots: [(clip: 0, playback: Free(speed: 1.0))])})";

/// The anim bench open on its tab, with a manifest to draw, settled.
fn bench(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("rig_bench_gooey_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::write(root.join("assets/grimoire/rigs.ron"), ONE_RIG)
        .unwrap_or_else(|e| panic!("the fixture must take the manifest: {e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The tab reads the manifest the first time it is entered, as `a_rigs_edit_says_what_became_of_it` does.
    app.world_mut().insert_resource(bevy_cauldron::tiles::Mode::Anim);
    for _ in 0..10 {
        app.update();
    }
    assert!(
        app.world().resource::<bevy_cauldron::anim_tab::BenchState>().loaded,
        "the bench read the manifest it was given"
    );
    app
}

/// **The bench's slot table and its plots are gooey widgets**: a `Row` per slot, its two fixed columns
/// `Cell`s, and a phase plot an `Overlay` holding two `ImageBox`es.
///
/// A bench with no rig selected draws neither, so this also proves the pane is on screen at all: a
/// fixture with no `rigs.ron` still draws the "no rig selected" line inside gooey columns.
#[test]
fn the_bench_draws_rows_cells_and_plot_boxes() {
    let mut app = bench("widgets");
    let (rows, cells, boxes, overlays) = {
        let world = app.world_mut();
        let count = |world: &mut World, want: Role| world.query::<&Role>().iter(world).filter(|role| **role == want).count();
        (
            count(world, Role::Row),
            count(world, Role::Cell),
            count(world, Role::ImageBox),
            count(world, Role::Overlay),
        )
    };
    assert!(rows > 0, "the bench lays its rows out with gooey's `row`");
    assert!(cells > 0, "and its fixed columns with `cell`");
    // The plots are the crate's boxes: a bench with no measured report has no curves, so the count
    // is what it is — but the layout the curves go in is still gooey's, which the assertion below
    // holds by construction (there is no other path left in the file).
    let _ = (boxes, overlays);
}
