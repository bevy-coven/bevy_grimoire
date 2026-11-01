//! **The world's gizmos are drawn by the world's camera, and by no other.**
//!
//! B-011. Bevy draws a gizmo on every camera whose `RenderLayers` meet the gizmo group's, and both
//! default to layer 0. The camera that draws the interface into the surface image
//! (`surface::SurfaceCamera`) carried no `RenderLayers`, so it drew the map's bounds box too,
//! through its own 2D projection: a 300 x 38 px rectangle (the 300 x 38 m front face, at one pixel a
//! metre) sat at the window's centre over the map, whatever the 3D camera was looking at. Found in a
//! full-window still on 2026-10-21; changing the map's height through `map_set_settings` changed the
//! rectangle's height, and zooming the map changed nothing about it.
//!
//! The headless harness lays out but does not rasterize, so this asserts what decides the picture:
//! which cameras share a render layer with a gizmo group.

use bevy::camera::visibility::RenderLayers;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::prelude::*;

use crate::fixtures;

#[test]
fn only_the_world_camera_draws_gizmos() {
    let root = fixtures::Fixture::new("gizmo-layers")
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }

    let gizmo_layers: Vec<RenderLayers> = app
        .world()
        .resource::<GizmoConfigStore>()
        .iter()
        .filter(|(_, config, _)| config.enabled)
        .map(|(_, config, _)| config.render_layers.clone())
        .collect();
    assert!(
        !gizmo_layers.is_empty(),
        "no gizmo group is enabled, so there is nothing to test"
    );

    let world = app.world_mut();
    let mut q = world.query::<(
        Entity,
        Option<&Name>,
        Has<bevy_cauldron::view::MainCamera>,
        Has<Camera2d>,
        Option<&RenderLayers>,
    )>();
    let mut world_camera_draws = false;
    let mut leaks = Vec::new();
    for (entity, name, main, two_d, layers) in q.iter(world) {
        if !main && !two_d {
            continue;
        }
        // A camera with no `RenderLayers` sees layer 0, Bevy's default.
        let layers = layers.cloned().unwrap_or_default();
        let sees = gizmo_layers.iter().any(|g| g.intersects(&layers));
        if main {
            world_camera_draws |= sees;
        } else if sees {
            leaks.push(format!("{entity} {name:?} on {layers:?}"));
        }
    }
    assert!(
        world_camera_draws,
        "the Map's camera no longer shares a layer with any gizmo group, so the bounds, the grid and \
         every marker would vanish from the world"
    );
    assert!(
        leaks.is_empty(),
        "these 2D cameras share a render layer with a gizmo group, so they draw the world's gizmos \
         flat over the interface: {leaks:?}"
    );
}
