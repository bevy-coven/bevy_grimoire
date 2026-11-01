//! **Every part of the map is inside the camera's depth range, however large the map.**
//!
//! B-010. The Map's camera is orthographic and sits `ISO_DISTANCE` (about 20.8 m) from its focus,
//! with Bevy's `OrthographicProjection::default_3d()` depth range: near 0, far 1000. Zooming out to
//! see a large map (`view::max_zoom` allows 1.25 x its longest side) widens the view but not the
//! depth range, so on Sanctuary's 300 x 1120 m town everything nearer the camera than 20.8 m along
//! the view axis was cut away: the near part of the map and every piece on it vanished at a
//! straight edge, and what was left of the bounds box floated beside the ground as a stray
//! rectangle. Found in a full-window still on 2026-10-21; at the camera's own transform, two of the
//! map's four ground corners were 67 m and 240 m behind the near plane.
//!
//! The check is the projection's own arithmetic: each corner of the map's bounds, measured along
//! the camera's forward axis from the camera, must lie between `near` and `far`.

use bevy::camera::Projection;
use bevy::prelude::*;

use crate::fixtures;

/// Depth of each corner of the map's bounds box along the camera's view axis, and the
/// projection's `near` and `far`.
fn depths(app: &mut App) -> (Vec<f32>, f32, f32) {
    let (tf, proj) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<bevy_cauldron::view::MainCamera>>();
        let (tf, proj) = q.single(world).expect("the editor has one main camera");
        (*tf, proj.clone())
    };
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic; this test measures an orthographic depth range")
    };
    let open = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>();
    let (centre, size) = bevy_cauldron::editor::map_bounds(open);
    let half = size * 0.5;
    let forward = tf.forward();
    let mut out = Vec::new();
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let corner = centre + half * Vec3::new(sx, sy, sz);
                out.push((corner - tf.translation()).dot(*forward));
            }
        }
    }
    (out, ortho.near, ortho.far)
}

fn editor_on_a_map(tag: &str, side: f32) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .side(side)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// **A town-sized map is drawn whole.** Failed before the fix: corners lay up to hundreds of
/// metres in front of `near = 0`, so they were clipped.
#[test]
fn a_town_sized_map_is_not_cut_off_by_the_near_plane() {
    let mut app = editor_on_a_map("depth-town", 1120.0);
    let (corners, near, far) = depths(&mut app);
    let outside: Vec<f32> = corners
        .iter()
        .copied()
        .filter(|d| *d < near || *d > far)
        .collect();
    assert!(
        outside.is_empty(),
        "the camera draws depths {near:.1}..{far:.1} m, and these corners of the 1120 m map fall \
         outside it, so that part of the map and every piece on it is not drawn: {outside:?} \
         (all corners: {corners:?})"
    );
}

/// **An ordinary room is still drawn whole.** The default 32 m map was always inside the old range;
/// every corner must stay inside the new one (which is wider here too: `near` goes below zero).
#[test]
fn a_room_sized_map_is_still_drawn_whole() {
    let mut app = editor_on_a_map("depth-room", 32.0);
    let (corners, near, far) = depths(&mut app);
    assert!(
        corners.iter().all(|d| *d >= near && *d <= far),
        "a 32 m map left the camera's depth range {near:.1}..{far:.1}: {corners:?}"
    );
}
