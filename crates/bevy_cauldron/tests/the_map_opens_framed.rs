//! **The Map opens framed on the whole map, not on an 18 m close-up of the origin.**
//!
//! `view::Rig::default()` puts the camera at the origin with a 18 m viewport height, so a person who
//! launches the editor on a 1120 m town sees a patch of ground and has to zoom out to find the map
//! (backlog E-022). Opening the Map now frames the whole map once, through the same
//! `editor::resolve_frame` / `editor::apply_frame` path that an agent's `grimoire/frame_view` uses.
//!
//! The assertions read the camera the way the projection does: every corner of the map's box is
//! taken into the MainCamera's own space and must fall inside the orthographic area it reports.
//! Nothing here trusts the framing arithmetic.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

fn editor(tag: &str, side: f32) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .place("floor", (300.0, 200.0))
        .side(side)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// The eight corners of a world-space box.
fn corners(centre: Vec3, size: Vec3) -> [Vec3; 8] {
    let half = size * 0.5;
    let mut out = [Vec3::ZERO; 8];
    let mut i = 0;
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                out[i] = centre + half * Vec3::new(sx, sy, sz);
                i += 1;
            }
        }
    }
    out
}

/// Whether every corner of a box is inside the MainCamera's orthographic area, and how much of the
/// area's limiting dimension the box fills.
fn seen(app: &mut App, centre: Vec3, size: Vec3) -> (bool, f32) {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<MainCamera>>();
    let (tf, proj) = q.single(world).expect("the editor has one main camera");
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic")
    };
    let area = ortho.area;
    assert!(
        area.width() > 0.0 && area.height() > 0.0,
        "the camera reports an empty orthographic area {area:?}"
    );
    let inverse = tf.affine().inverse();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for corner in corners(centre, size) {
        let local = inverse.transform_point3(corner);
        lo = lo.min(local.xy());
        hi = hi.max(local.xy());
    }
    let inside =
        lo.x >= area.min.x && hi.x <= area.max.x && lo.y >= area.min.y && hi.y <= area.max.y;
    let fill = ((hi.x - lo.x) / area.width()).max((hi.y - lo.y) / area.height());
    (inside, fill)
}

/// **Opened, the Map shows every corner of the 1120 m map, and the map fills most of the view.**
/// Failed before the change: the Rig stayed at its 18 m default, so a corner of the map was outside
/// the view on the first frame a person sees.
#[test]
fn the_map_opens_framed_on_the_whole_map() {
    let mut app = editor("opens-framed", 1120.0);
    let (centre, size) = bevy_cauldron::editor::map_bounds(app.world().resource::<OpenMap>());
    let (inside, fill) = seen(&mut app, centre, size);
    assert!(
        inside,
        "on opening, a corner of the 1120 m map is outside the camera's view, so the person sees a \
         close-up and not the map"
    );
    assert!(
        fill >= 0.6,
        "on opening, the map fills only {:.0}% of the view's limiting dimension",
        fill * 100.0
    );
}

/// **The Map opens framed again after the door's state resets.** The framing flag and the Rig are both
/// `Door` resources, so a door change puts both back to their opening values. The flag used to be a
/// `Local`, which a door change could not reach: after the first opening the Rig stayed at the 18 m
/// close-up and the Map was never framed again.
///
/// `close_the_door` cannot be called here: it despawns the MainCamera, and the headless harness never
/// respawns it. So the test puts the two door resources back to their opening values, which is what
/// `reset_door_state` does to them, and checks that the next updates frame the Map. The list itself is
/// held by `the_door_resets_what_it_says_it_owns`.
#[test]
fn the_map_reopens_framed_after_the_door_state_resets() {
    let mut app = editor("reopens-framed", 1120.0);
    let (centre, size) = bevy_cauldron::editor::map_bounds(app.world().resource::<OpenMap>());
    app.world_mut()
        .insert_resource(bevy_cauldron::view::Rig::default());
    app.world_mut()
        .insert_resource(bevy_cauldron::editor::MapFraming::default());
    for _ in 0..6 {
        app.update();
    }
    let (inside, fill) = seen(&mut app, centre, size);
    assert!(
        inside,
        "after the door's state resets and the Map reopens, a corner of the 1120 m map is outside the view"
    );
    assert!(
        fill >= 0.6,
        "after reopening, the map fills only {:.0}% of the view's limiting dimension",
        fill * 100.0
    );
}
