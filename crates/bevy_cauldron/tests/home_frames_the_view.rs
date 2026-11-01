//! **Home frames the view: the locked placement if there is one, otherwise the whole map.**
//!
//! The Map camera zooms in on the origin, and nothing in the editor brings the whole map back
//! under the author's hand without wheel-scrolling out. Home is the frame the agent already has
//! through `grimoire/frame_view` (`editor::resolve_frame` + `editor::apply_frame`), bound to a key.
//!
//! The assertion reads the camera the way the projection does, as `frame_the_view.rs` does: each
//! corner of the map's box is taken into the camera's own space and must fall inside the
//! orthographic area the projection reports. Nothing here trusts the framing arithmetic.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::editor::{FrameTarget, resolve_frame};
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::{MainCamera, Rig};

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

/// Are all eight corners of the box inside the MainCamera's own orthographic area?
fn inside_the_view(app: &mut App, centre: Vec3, size: Vec3) -> bool {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<MainCamera>>();
    let (tf, proj) = q.single(world).expect("the editor has one main camera");
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic")
    };
    let area = ortho.area;
    let inverse = tf.affine().inverse();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for corner in corners(centre, size) {
        let local = inverse.transform_point3(corner);
        lo = lo.min(local.xy());
        hi = hi.max(local.xy());
    }
    lo.x >= area.min.x && hi.x <= area.max.x && lo.y >= area.min.y && hi.y <= area.max.y
}

/// **One key press, as the window delivers it**: pressed, then released.
fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

#[test]
fn home_frames_the_whole_map_when_nothing_is_locked() {
    let mut app = editor("home_map", 1120.0);
    let (centre, size) = {
        let project = app.world().resource::<Project>();
        let open = app.world().resource::<OpenMap>();
        let framing = resolve_frame(&FrameTarget::Map, project, open, None)
            .expect("an open map has a whole-map frame");
        (framing.centre, framing.size)
    };

    // Zoom in on the origin: the map is no longer whole in view.
    app.world_mut().resource_mut::<Rig>().height = 5.0;
    for _ in 0..3 {
        app.update();
    }
    assert!(
        !inside_the_view(&mut app, centre, size),
        "zoomed in, the whole map must be out of view before Home is pressed"
    );

    tap(&mut app, Key::Home, KeyCode::Home);

    assert!(
        inside_the_view(&mut app, centre, size),
        "Home must frame the whole map, but the map's corners are still outside the view"
    );
}
