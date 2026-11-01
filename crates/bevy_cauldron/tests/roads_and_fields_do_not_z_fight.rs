//! **Two flat pieces laid over one another do not draw their top faces at the same height.**
//!
//! The shipped `town/road` and `town/field` are mesh-less descriptors: the editor draws each as a
//! footprint marker, a 0.05 m slab standing on the floor. Every one of them has its top face at the
//! same height, so where a road crosses a field the two tops are coplanar and the depth buffer hands
//! the pixels to whichever surface it happens to test first. The picture trades pixels in a stair
//! step, which is the z-fight the developer saw on 2026-10-21 (`window-default.png`).
//!
//! This asserts what decides the picture: the world height of each top face. A depth bias was tried
//! first and is not enough. At the Map camera's depth range a field-sized quad's interpolation error
//! is larger than one or two bias steps, so the bias left the stair steps in place. The draw level
//! therefore has to show as a world-height difference of at least `LEVEL_LIFT` per level, in the
//! order the rule in `bevy_grimoire_core::draw_order` decides.

use bevy::prelude::*;
use bevy_grimoire_core::draw_order::LEVEL_LIFT;

use crate::fixtures;

/// Float slack for heights built from sums of transforms.
const TOL: f32 = 1e-6;

/// The top face of a footprint marker's slab, in world metres.
///
/// The slab is the first child of the piece's root (see `bevy_grimoire::spawn_footprint_marker`). Its
/// centre sits `MARKER_SLAB / 2` above the root plus the render lift the piece's draw level adds, so
/// its top sits half the slab above that centre. The root is the piece's own transform, so the two
/// together are the drawn top.
fn top_face(app: &mut App, id: &str) -> f32 {
    let world = app.world_mut();
    let mut roots = world.query::<(&Name, &Children)>();
    let children: Vec<Entity> = roots
        .iter(world)
        .find(|(name, _)| name.as_str() == id)
        .unwrap_or_else(|| panic!("no piece named `{id}` was drawn"))
        .1
        .iter()
        .collect();
    let slab = children
        .into_iter()
        .find(|c| world.get::<Mesh3d>(*c).is_some())
        .unwrap_or_else(|| panic!("`{id}` has no footprint slab"));
    world
        .get::<GlobalTransform>(slab)
        .unwrap_or_else(|| panic!("`{id}`'s slab has no global transform"))
        .translation()
        .y
        + 0.025
}

#[test]
fn overlapping_flat_pieces_have_distinct_top_faces() {
    // A road laid across a field, both mesh-less and both on the floor: the shape of the shipped
    // town, where a road crosses a field and both are footprint markers at the same height.
    let root = fixtures::Fixture::new("flat-overlap")
        .flat_descriptor("road", 4.877, 20.0)
        .flat_descriptor("field", 12.0, 12.0)
        .place("road", (0.0, 0.0))
        .place("field", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }

    let road_top = top_face(&mut app, "road");
    let field_top = top_face(&mut app, "field");

    // The road is the smaller footprint, so it must draw over the field it crosses by at least one
    // draw level. Equal heights are the coplanar z-fight.
    assert!(
        road_top - field_top >= LEVEL_LIFT - TOL,
        "the road is the smaller piece and must draw over the field it crosses by at least one \
         level ({LEVEL_LIFT} m), but its top is {road_top} m against the field's {field_top} m — \
         the tops are coplanar and the depth test decides their pixels by coin toss, the z-fight"
    );
}

#[test]
fn an_authored_paint_outranks_the_smaller_footprint() {
    // The field is the larger piece, so the footprint alone would draw it under the road. Its
    // authored paint is the first key of the order, so it draws over the road instead.
    let root = fixtures::Fixture::new("flat-painted")
        .flat_descriptor("road", 4.877, 20.0)
        .flat_descriptor("field", 12.0, 12.0)
        .place("road", (0.0, 0.0))
        .place_painted("field", (0.0, 0.0), 3)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }

    let road_top = top_face(&mut app, "road");
    let field_top = top_face(&mut app, "field");
    assert!(
        field_top - road_top >= LEVEL_LIFT - TOL,
        "the field carries authored paint 3 and must draw over the road by at least one level \
         ({LEVEL_LIFT} m), but its top is {field_top} m against the road's {road_top} m"
    );
}

#[test]
fn of_two_equal_flat_pieces_the_later_placement_draws_on_top() {
    let root = fixtures::Fixture::new("flat-equal")
        .flat_descriptor("tile_a", 4.0, 4.0)
        .flat_descriptor("tile_b", 4.0, 4.0)
        .place("tile_a", (0.0, 0.0))
        .place("tile_b", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }

    let a_top = top_face(&mut app, "tile_a");
    let b_top = top_face(&mut app, "tile_b");
    assert!(
        b_top - a_top >= LEVEL_LIFT - TOL,
        "the equal pieces overlap and the later placement must draw on top by at least one level \
         ({LEVEL_LIFT} m), but tile_b's top is {b_top} m against tile_a's {a_top} m"
    );
}

#[test]
fn a_larger_piece_laid_under_a_standing_road_lifts_the_road() {
    // The road stands alone, so nothing is under it and it draws at the base. Then a larger field is
    // laid beneath it. The road is now the smaller of an overlapping pair and must be redrawn above
    // the field, though the road itself was never edited.
    let root = fixtures::Fixture::new("flat-relevel")
        .flat_descriptor("road", 4.877, 20.0)
        .flat_descriptor("field", 12.0, 12.0)
        .place("road", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    let before = top_face(&mut app, "road");

    // Lay the field under it the way an edit records a placement: a new entry in the open map.
    {
        let mut open = app
            .world_mut()
            .resource_mut::<bevy_cauldron::project::OpenMap>();
        let mut field = open.map.placements[0].clone();
        field.id = "field".into();
        field.descriptor = "field".into();
        field.patch = None;
        open.map.placements.push(field);
    }
    for _ in 0..6 {
        app.update();
    }
    let after = top_face(&mut app, "road");
    assert!(
        after - before >= LEVEL_LIFT - TOL,
        "the field now lies under the road, so the road is the smaller piece and must be redrawn \
         at least one level ({LEVEL_LIFT} m) above it, but its top moved from {before} m to \
         {after} m"
    );
}
