//! **A rectangle frames the ground it names, and only the height of what stands inside it.**
//!
//! `grimoire/frame_view` with a rectangle used the whole map's height as the box's height, so a 3 m
//! rectangle on a 180 m map framed about 182 m of air above it (cycle-3 item 1). The vertical
//! extent is now the union of the placed boxes whose footprint meets the rectangle, and the
//! rectangle's own ground when nothing stands in it.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .place("floor", (300.0, 200.0))
        .side(1120.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    app
}

/// Through the registered method, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
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

/// Whether every corner of a box is inside the MainCamera's orthographic area.
fn inside_view(app: &mut App, centre: Vec3, size: Vec3) -> bool {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<MainCamera>>();
    let (tf, proj) = q.single(world).expect("the editor has one main camera");
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic")
    };
    let area = ortho.area;
    let inverse = tf.affine().inverse();
    corners(centre, size).iter().all(|corner| {
        let local = inverse.transform_point3(*corner);
        local.x >= area.min.x
            && local.x <= area.max.x
            && local.y >= area.min.y
            && local.y <= area.max.y
    })
}

/// **A 3 m rectangle on the ground of a 180 m map frames close to the ground.** Failed before the change:
/// the rectangle's vertical size was the map's full 180 m, so the framed height was about 182 m.
#[test]
fn a_small_rectangle_on_a_tall_map_frames_close_to_the_ground() {
    let mut app = editor("rect-tall");
    // A 180 m map, as a tower's stack of storeys would make it. Nothing stands in the rectangle.
    app.world_mut().resource_mut::<OpenMap>().map.bounds.1 = 180.0;
    app.update();
    let answer = call(
        &mut app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"rect": [200.0, 100.0, 203.0, 103.0]}}),
    );
    assert_eq!(answer["outcome"], "framed", "{answer}");
    app.update();
    app.update();
    let height = app.world().resource::<Rig>().height;
    assert!(
        height < 20.0,
        "a 3 m rectangle on the ground framed at {height:.1} m of height, so the view is taken up by \
         air above it: {answer}"
    );
}

/// **A rectangle around a placed piece frames that piece's own height, and the piece is in view.**
/// Failed before the change for the same reason: the height was the 180 m of the map, not the piece's.
#[test]
fn a_rectangle_round_a_piece_frames_the_piece_and_not_the_map() {
    let mut app = editor("rect-piece");
    app.world_mut().resource_mut::<OpenMap>().map.bounds.1 = 180.0;
    app.update();
    let (centre, size) = {
        let project = app
            .world()
            .resource::<bevy_cauldron::project::Project>();
        let open = app.world().resource::<OpenMap>();
        bevy_cauldron::editor::placement_bounds(project, open, 0)
            .expect("the placed floor has a measured box")
    };
    let answer = call(
        &mut app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"rect": [centre.x - 2.0, centre.z - 2.0, centre.x + 2.0, centre.z + 2.0]}}),
    );
    assert_eq!(answer["outcome"], "framed", "{answer}");
    app.update();
    app.update();
    let height = app.world().resource::<Rig>().height;
    assert!(
        height < 20.0,
        "a rectangle round one piece framed at {height:.1} m of height: {answer}"
    );
    assert!(
        inside_view(&mut app, centre, size),
        "the piece inside the rectangle is not in the framed view: {answer}"
    );
}
