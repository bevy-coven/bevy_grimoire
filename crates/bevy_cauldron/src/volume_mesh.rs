//! **Meshing a carved volume in chunks** (#101, F5), through `isomesh`.
//!
//! The edit list ([`Volume`]) is the data; this samples it, over a ground, one chunk at a time. A
//! chunk samples `cells + 1` planes so two neighbours share their plane exactly; the seam test in
//! `tests/volume_meshes_without_cracks.rs` holds that.

use bevy_grimoire_core::volume::Volume;
use isomesh::chunk::{ChunkId, ChunkLayout};
use isomesh::marching_cubes::MarchingCubes;
use isomesh::{MeshBuffer, Sdf};

/// The volume over a heightfield ground: `p.y - ground(x, z)`, negative in rock, then every edit.
pub struct VolumeField<'a, G: Fn(f32, f32) -> f32> {
    pub volume: &'a Volume,
    pub ground: G,
}

impl<G: Fn(f32, f32) -> f32> Sdf for VolumeField<'_, G> {
    type Scalar = f32;
    fn sample(&self, p: [f32; 3]) -> f32 {
        self.volume.sdf(p, p[1] - (self.ground)(p[0], p[2]))
    }
}

/// Mesh one chunk of `layout`.
pub fn mesh_chunk<G: Fn(f32, f32) -> f32>(
    field: &VolumeField<'_, G>,
    layout: &ChunkLayout<f32>,
    id: ChunkId,
) -> Result<MeshBuffer<f32>, isomesh::Error> {
    let shape = layout.sample_shape()?;
    let mut out = MeshBuffer::<f32>::new();
    MarchingCubes::<f32>::new().extract(field, &shape, layout.sample_origin(id), layout.cell_size(), &mut out)?;
    Ok(out)
}

/// **Snow on a carved volume** (#101), through `bevy_elements`' own `SnowyField`: the volume over a
/// heightfield ground (`tops`, world heights on `grid`), with `snow` metres (depths on `snow_grid`) laid
/// over each column's top. `SnowyField`'s contract keeps a tunnel under a roof thicker than one voxel
/// free of snow, so a carve is never filled by it. Needs `--features volume_snow`.
#[cfg(feature = "volume_snow")]
pub fn snowy_volume(
    volume: Volume,
    cell_size: f32,
    tops: std::sync::Arc<[f32]>,
    grid: bevy_elements::TerrainGrid,
    snow: std::sync::Arc<[f32]>,
    snow_grid: bevy_elements::TerrainGrid,
) -> bevy_elements::SnowyField {
    let owned = OwnedVolume { volume, tops: tops.clone(), grid };
    bevy_elements::SnowyField::new(std::sync::Arc::new(owned), cell_size, tops, grid, snow, snow_grid)
}

/// [`VolumeField`] that owns its data, so a field can be handed to a plugin as `Arc<dyn VolumeField>`.
#[cfg(feature = "volume_snow")]
struct OwnedVolume {
    volume: Volume,
    tops: std::sync::Arc<[f32]>,
    grid: bevy_elements::TerrainGrid,
}

#[cfg(feature = "volume_snow")]
impl Sdf for OwnedVolume {
    type Scalar = f32;
    fn sample(&self, p: [f32; 3]) -> f32 {
        let ground = self.grid.sample(&self.tops, bevy::math::Vec2::new(p[0], p[2]));
        self.volume.sdf(p, p[1] - ground)
    }
}

/// Metres per cell and cells per chunk edge of the drawn volume: powers of two, so two neighbours' shared sample plane is the same float from both sides.
pub const DRAWN_CELL_M: f32 = 1.0;
pub const DRAWN_CHUNK_CELLS: u32 = 16;

/// **The carved ground as chunk meshes**, one per chunk that has a surface, over a heightfield's
/// extent and its height range padded by one chunk each way (room for a carve under or an add above
/// the ground). Positions are world metres on the heightfield's own datum. Empty when the volume has
/// no edits: then the ground is just the heightfield.
pub fn mesh_volume(volume: &Volume, ground: &bevy_grimoire_core::heightfield::Heightfield) -> Result<Vec<MeshBuffer<f32>>, isomesh::Error> {
    if volume.is_empty() {
        return Ok(Vec::new());
    }
    let span = DRAWN_CHUNK_CELLS as f32 * DRAWN_CELL_M;
    let (lo, hi) = ground.range();
    let (min_x, min_z) = (((ground.origin.0) / span).floor() * span, ((ground.origin.1) / span).floor() * span);
    let min_y = ((lo - span) / span).floor() * span;
    let layout = ChunkLayout::<f32>::new(DRAWN_CHUNK_CELLS, DRAWN_CELL_M, [min_x, min_y, min_z])?;
    let max_x = ground.origin.0 + (ground.width - 1) as f32 * ground.cell_m;
    let max_z = ground.origin.1 + (ground.depth - 1) as f32 * ground.cell_m;
    let count = |max: f32, min: f32| (((max - min) / span).ceil() as i32).max(1);
    let (nx, nz) = (count(max_x, min_x), count(max_z, min_z));
    let ny = count(hi + span, min_y);
    let field = VolumeField { volume, ground: |x, z| ground.height_at(x, z) };
    let mut out = Vec::new();
    for cy in 0..ny {
        for cz in 0..nz {
            for cx in 0..nx {
                let mesh = mesh_chunk(&field, &layout, ChunkId::new([cx, cy, cz]))?;
                if !mesh.indices.is_empty() {
                    out.push(mesh);
                }
            }
        }
    }
    Ok(out)
}
