//! **Snow lies on a carved hill and never fills its tunnel** (#101). Needs `volume_snow`:
//! `cargo test -p bevy_cauldron --features volume_snow --test editor volume_snow::`.
#![cfg(feature = "volume_snow")]

use std::sync::Arc;

use bevy::math::{UVec2, Vec2};
use bevy_elements::TerrainGrid;
use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
use bevy_cauldron::volume_mesh::snowy_volume;
use isomesh::Sdf;

#[test]
fn snow_covers_the_top_and_the_tunnel_stays_open() {
    let grid = TerrainGrid { origin: Vec2::ZERO, cell_m: 1.0, size: UVec2::new(16, 16), y0: 0.0 };
    let tops: Arc<[f32]> = vec![10.0; 256].into();
    let snow: Arc<[f32]> = vec![0.5; 256].into();
    let mut volume = Volume::default();
    // A tunnel along x at y = 3, radius 1.5: three metres of rock remain above it.
    volume.push(Edit { shape: Shape::Capsule { a: [0.0, 3.0, 8.0], b: [16.0, 3.0, 8.0], radius_m: 1.5 }, mode: Mode::Carve, smooth_m: 0.0 }).unwrap();
    let field = snowy_volume(volume, 0.5, tops, grid, snow, grid);

    // Above the rock top, inside the snow slab: solid. Above the slab: air.
    assert!(field.sample([8.0, 10.25, 4.0]) < 0.0, "no snow on the top");
    assert!(field.sample([8.0, 10.75, 4.0]) > 0.0, "snow deeper than asked");
    // The tunnel is air, snow or not, and its roof is rock.
    assert!(field.sample([8.0, 3.0, 8.0]) > 0.0, "the tunnel was filled");
    assert!(field.sample([8.0, 5.0, 8.0]) < 0.0, "the roof is not solid");
}
