//! **A sphere carved into a cliff opens a hole that meshes without cracks at chunk seams** (#101).
//! Needs the `isomesh` feature: `cargo test -p bevy_cauldron --features isomesh --test editor volume_meshes_without_cracks::`.
#![cfg(feature = "isomesh")]

use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
use bevy_cauldron::volume_mesh::{mesh_chunk, VolumeField};
use isomesh::chunk::{ChunkId, ChunkLayout};
use isomesh::validate::sealing;

fn cliff(_x: f32, z: f32) -> f32 {
    // rock to y = 8.3 for z < 7.5 (a wall facing +z), a plain at y = 0.3 beyond: off the sample planes.
    if z < 7.5 { 8.3 } else { 0.3 }
}

fn carved() -> Volume {
    let mut v = Volume::default();
    // The sphere straddles the x = 0 and z = 8 chunk planes, so the hole crosses seams on two axes.
    v.push(Edit { shape: Shape::Sphere { center: [0.0, 4.0, 8.0], radius_m: 4.5 }, mode: Mode::Carve, smooth_m: 0.0 }).unwrap();
    v
}

#[test]
fn the_hole_is_there_and_every_seam_agrees() {
    // 8 cells of 1 m (a power of two, so neighbours' sample planes agree bit for bit): chunks at x = -8, 0 and z = 0, 8; y = 0..8 is chunk row 1.
    let layout = ChunkLayout::<f32>::new(8, 1.0, [-8.0, -8.0, 0.0]).unwrap();
    let volume = carved();
    let field = VolumeField { volume: &volume, ground: cliff };

    let mut hole_vertices = 0usize;
    for cx in 0..2 {
        for cz in 0..2 {
            let id = ChunkId::new([cx, 1, cz]);
            let mesh = mesh_chunk(&field, &layout, id).unwrap();
            // isomesh's own check: the mesh partitions this chunk's samples the way the field does.
            let report = sealing(&field, &layout.sample_shape().unwrap(), layout.sample_origin(id), layout.cell_size(), &mesh.positions, &mesh.indices);
            assert!(report.agrees(), "chunk {id:?} is not sealed: {report:?}");
            hole_vertices += mesh.positions.iter().filter(|p| ((p[0]).powi(2) + (p[1] - 4.0).powi(2) + (p[2] - 8.0).powi(2)).sqrt() < 4.6).count();
        }
    }
    assert!(hole_vertices > 20, "the carve produced no surface ({hole_vertices} vertices on the sphere)");

    // Seam coordinates: vertices on a shared plane are identical from both sides, bit for bit.
    for (axis, a, b) in [(0usize, ChunkId::new([0, 1, 0]), ChunkId::new([1, 1, 0])), (2, ChunkId::new([0, 1, 0]), ChunkId::new([0, 1, 1])), (0, ChunkId::new([0, 1, 1]), ChunkId::new([1, 1, 1]))] {
        let plane = layout.sample_origin(b)[axis];
        let on = |id| {
            let mut v: Vec<[u32; 3]> = mesh_chunk(&field, &layout, id).unwrap().positions.iter().filter(|p| p[axis] == plane).map(|p| p.map(f32::to_bits)).collect();
            v.sort();
            v
        };
        let (from_a, from_b) = (on(a), on(b));
        assert!(!from_a.is_empty(), "the seam {a:?}|{b:?} at {plane} carries no geometry, so it proves nothing");
        assert_eq!(from_a, from_b, "seam {a:?}|{b:?} cracks");
    }
}
