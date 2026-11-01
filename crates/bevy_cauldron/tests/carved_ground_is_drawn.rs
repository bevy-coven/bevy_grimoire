//! **A carve is drawn as a hole in the ground** (#101). Needs the `isomesh` feature.
#![cfg(feature = "isomesh")]

use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
use bevy_cauldron::volume_mesh::mesh_volume;

#[test]
fn a_carved_sphere_opens_the_ground_and_an_uncarved_map_draws_the_heightfield() {
    let ground = Heightfield::flat((0.0, 0.0), 1.0, 33, 33, 0.3);
    assert!(mesh_volume(&Volume::default(), &ground).unwrap().is_empty(), "no edits: the heightfield is drawn, not chunks");

    let mut v = Volume::default();
    v.push(Edit { shape: Shape::Sphere { center: [16.0, 0.3, 16.0], radius_m: 5.0 }, mode: Mode::Carve, smooth_m: 0.0 }).unwrap();
    let chunks = mesh_volume(&v, &ground).unwrap();
    let verts: Vec<[f32; 3]> = chunks.iter().flat_map(|m| m.positions.iter().copied()).collect();
    let dist = |p: &[f32; 3]| ((p[0] - 16.0).powi(2) + (p[1] - 0.3).powi(2) + (p[2] - 16.0).powi(2)).sqrt();
    let bowl = verts.iter().filter(|p| p[1] < 0.0 && dist(p) < 5.2).count();
    assert!(bowl > 20, "the carve's bowl dips below the ground ({bowl} vertices)");
    let flat = verts.iter().filter(|p| (p[1] - 0.3).abs() < 1e-3 && dist(p) > 5.3).count();
    assert!(flat > 100, "ground away from the carve is still drawn ({flat} vertices)");
    assert!(!verts.iter().any(|p| dist(p) < 4.5), "nothing is left inside the hole");
}
