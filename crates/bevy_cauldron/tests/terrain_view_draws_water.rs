//! **The sculpted ground and the kept water are drawn** (#99, #100).

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::water::{WATER_FIELD_VERSION, WaterField};
use bevy_cauldron::terrain_view::{ground_mesh, water_mesh};

fn positions(m: &Mesh) -> Vec<[f32; 3]> {
    match m.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
        other => panic!("no positions: {other:?}"),
    }
}

#[test]
fn water_is_drawn_at_ground_plus_depth_and_only_where_it_is_wet() {
    let mut g = Heightfield::flat((0.0, 0.0), 1.0, 4, 4, 1.0);
    g.heights[5] = 0.0; // a pit at cell (1, 1)
    let mut depth_m = vec![0.0; 16];
    depth_m[5] = 1.0; // fills the pit to 1 m
    depth_m[6] = 0.01; // a film, not drawn
    let w = WaterField { version: WATER_FIELD_VERSION, origin: (0.0, 0.0), cell_m: 1.0, width: 4, depth: 4, depth_m, settled: true, steps: 1 };
    let ground = ground_mesh(&g);
    assert_eq!(positions(&ground).len(), 16);
    let mesh = water_mesh(&g, &w).expect("one wet cell");
    let p = positions(&mesh);
    assert_eq!(p.len(), 4, "one quad for the one wet cell");
    assert!(p.iter().all(|v| (v[1] - 1.0).abs() < 1e-6), "surface at ground 0 + depth 1: {p:?}");
    assert!(p.iter().all(|v| v[0] >= 0.5 && v[0] <= 1.5 && v[2] >= 0.5 && v[2] <= 1.5), "centred on cell (1,1): {p:?}");
    let mut dry = w.clone();
    dry.depth_m = vec![0.0; 16];
    assert!(water_mesh(&g, &dry).is_none());
    let mut other = w;
    other.width = 3;
    assert!(water_mesh(&g, &other).is_none(), "water run on another ground's shape is not drawn");
}
