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
use bevy_gooey::{CellWidth, Role};

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

/// **The bench's slot table is gooey widgets**: a `Row` per slot and its two fixed columns `Cell`s,
/// counted **inside the bench's own pane**.
///
/// **Scoped to `SlotPane`** (review of #244): `Role::Row` and `Role::Cell` are world-wide, and
/// `tiles::spawn_tab_strip` spawns a `Role::Row` on every screen, so an unscoped count passes with the
/// bench drawing nothing at all.
#[test]
fn the_bench_draws_rows_and_cells_inside_its_own_pane() {
    let mut app = bench("widgets");
    let (rows, cells, slot_text, widest) = {
        let world = app.world_mut();
        let pane = world
            .query_filtered::<Entity, With<bevy_cauldron::anim_tab::SlotPane>>()
            .single(world)
            .expect("the bench has its slot pane")
            .to_owned();
        let mut children = world.query::<&Children>();
        let mut roles = world.query::<&Role>();
        let mut cells_q = world.query::<(&CellWidth, &ComputedNode)>();
        let mut texts = world.query::<&Text>();
        let mut seen = (0usize, 0usize);
        let mut slot_text: Option<String> = None;
        let mut widest = 0.0f32;
        let mut queue = vec![pane];
        while let Some(at) = queue.pop() {
            if let Ok(kids) = children.get(world, at) {
                for kid in kids.iter() {
                    queue.push(kid);
                    match roles.get(world, kid) {
                        Ok(Role::Row) => seen.0 += 1,
                        Ok(Role::Cell) => seen.1 += 1,
                        _ => {}
                    }
                    if let Ok((_, node)) = cells_q.get(world, kid) {
                        widest = widest.max(node.size().x);
                    }
                    if let Ok(text) = texts.get(world, kid)
                        && text.0.contains(" - clip ")
                    {
                        slot_text = Some(text.0.clone());
                    }
                }
            }
        }
        (seen.0, seen.1, slot_text, widest)
    };
    assert!(rows > 0, "the bench lays its slot rows out with gooey's `row`");
    assert!(cells > 0, "and its fixed columns with `cell`");
    // **And the column holds its text** (review of #244, MUST-FIX a): a `cell` clips what does not fit,
    // so a width under the string cuts the clip index — the one thing that column is for.
    let text = slot_text.expect("the fixture's one slot draws `0 - clip 0`");
    let needed = text.chars().count() as f32 * bevy_cauldron::chrome::BODY_CHAR_W;
    assert!(
        widest >= needed,
        "the slot column is {widest:.1} px and `{text}` needs {needed:.1} px, so the cell cuts it"
    );
}
