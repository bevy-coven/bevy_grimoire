//! **A preview's draw level is the level the piece gets once it is in the map** — E-020.
//!
//! # What was wrong
//!
//! `drawn_paints` is what every spawn path reads: `spawn_existing`, `spawn_range`, `redraw_placements`
//! and `relevel_placements` all insert its answer as [`bevy_grimoire::Paint`], and `apply_paint` and
//! `lift_flat_markers` turn that into the world-height separation two flat pieces need. `drive_ghost`
//! — the brush preview — spawned through `spawn_piece` and inserted **no paint at all**, so a ghost
//! always drew at level 0. Over a spot where flat pieces already overlap (a road crossing a field, an
//! authored map, a generated one — `a_larger_piece_laid_under_a_standing_road_lifts_the_road` is the
//! existing proof those exist) the preview's top face is coplanar with theirs, which is the z-fight
//! `roads_and_fields_do_not_z_fight` exists to prevent, in the one picture whose whole job is to show
//! what the click will look like.
//!
//! `bevy_grimoire::paint_for_candidate` is now that answer, asked of a row that is not in the map yet,
//! and `drive_ghost` inserts it.
//!
//! # What is asserted
//!
//! 1. **The candidate's level is the level the row gets.** For a candidate appended to a map whose
//!    pieces overlap it, `paint_for_candidate` and `drawn_paints` agree — the ghost's claim, and the
//!    thing that was 0 against a committed level.
//! 2. **A candidate nothing overlaps keeps its own paint**, so the fix does not invent a level.
//! 3. **An authored paint still outranks the automatic order** through the candidate path, because
//!    the rule lives in `draw_levels` and this must not become a second copy of it.
//!
//! # Why the ghost is not driven live here
//!
//! `drive_ghost` needs `cursor_ground` and a viewport, and in the headless harness its spawn was
//! **not reproducible run to run**: three consecutive runs of the same binary on the same fixture
//! produced one ghost, then none, then none (measured 2026-10-27). A test that sometimes has no
//! subject cannot pin a number, so this pins the helper the system now calls and says so rather than
//! asserting against an empty query and passing for the wrong reason.

use bevy_grimoire_core::map::Placed;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

/// A project with a road crossing a field, the shape the shipped town has: two flat pieces laid over
/// one another, authored data rather than a click (`place` does not run the blocker rule, and the
/// z-fight test's map is the same shape).
fn crossing(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .flat_descriptor("road", 4.877, 20.0)
        .flat_descriptor("field", 12.0, 12.0)
        .place("road", (0.0, 0.0))
        .place("field", (0.0, 0.0))
        .build("m")
}

fn library_of(root: &std::path::Path) -> (bevy_grimoire_core::map::Map, bevy_grimoire_core::library::Library) {
    let app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let world = app.world();
    (
        world.resource::<OpenMap>().map.clone(),
        (*world.resource::<Project>().library).clone(),
    )
}

/// **The preview's answer and the map's answer are one answer.**
#[test]
fn the_candidates_level_is_the_level_it_gets_once_it_is_in_the_map() {
    let root = crossing("candidate-level");
    let (map, library) = library_of(&root);

    // A small tile over the crossing: it overlaps both, and being the smallest it draws on top of
    // both under the area key.
    let candidate = Placed {
        id: "tile@1".to_owned(),
        descriptor: "road".to_owned(),
        at: (0.0, 0.0),
        ..Placed::default()
    };
    let previewed = bevy_grimoire::paint_for_candidate(&map, &library, &candidate);

    let mut landed = map.clone();
    landed.placements.push(candidate);
    let committed = *bevy_grimoire::drawn_paints(&landed, &library)
        .last()
        .expect("the landed map has rows");

    assert_eq!(
        previewed, committed,
        "the ghost would draw at {previewed} and the piece lands at {committed} — a preview \
         computing either differently is a promise about a landing that will not happen"
    );
    assert!(
        committed >= 2,
        "the candidate overlaps both the road and the field and is the smallest of the three, so it \
         must sit above both; got {committed}. If this number is 0 the fixture is no longer a \
         crossing and this test is not asking what it thinks"
    );
}

/// **A candidate nothing overlaps keeps its own paint** — the fix must not invent a level.
#[test]
fn a_candidate_over_empty_ground_keeps_its_paint() {
    let root = crossing("candidate-empty");
    let (map, library) = library_of(&root);
    let candidate = Placed {
        id: "road@1".to_owned(),
        descriptor: "road".to_owned(),
        // Off the 12 m field entirely, so nothing overlaps it.
        at: (40.0, 40.0),
        paint: 2,
        ..Placed::default()
    };
    assert_eq!(
        bevy_grimoire::paint_for_candidate(&map, &library, &candidate),
        2,
        "an overlapping-free candidate draws at its authored paint, and only that"
    );
}

/// **An authored paint still outranks the automatic order through the candidate path too.**
///
/// The rule lives in `draw_order::draw_levels`, and this asserts the candidate path adds nothing of
/// its own: an authored 9 sits above the two paint-0 pieces it overlaps **and** is lifted over them
/// by the same relative rule `drawn_paints` applies — 9 plus one level for each overlapping piece
/// below it, which is 11 here (the field at level 0, the road at 1). What matters is that the
/// candidate path agrees with the map, not which arithmetic gets there.
#[test]
fn an_authored_paint_still_outranks_the_order_for_a_candidate() {
    let root = crossing("candidate-painted");
    let (map, library) = library_of(&root);
    let candidate = Placed {
        id: "road@1".to_owned(),
        descriptor: "road".to_owned(),
        at: (0.0, 0.0),
        paint: 9,
        ..Placed::default()
    };
    let previewed = bevy_grimoire::paint_for_candidate(&map, &library, &candidate);
    let mut landed = map.clone();
    landed.placements.push(candidate);
    let committed = *bevy_grimoire::drawn_paints(&landed, &library)
        .last()
        .expect("the landed map has rows");
    assert_eq!(
        previewed, committed,
        "the candidate path and the map must agree for an authored paint too"
    );
    assert!(
        committed > 9,
        "an authored paint of 9 is above both overlapping pieces, and the level is relative — it \
         gains one for each overlapping piece below it — so the answer is above 9; got {committed}"
    );
}
