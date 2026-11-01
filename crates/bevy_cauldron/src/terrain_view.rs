//! **The sculpted ground and the kept water, drawn in the viewport** (#99, #100).
//!
//! `terrain_stroke` and `water_run` write a heightfield and a water field into the project; until
//! now nothing showed them, so an author sculpted and ran water blind. This draws both from the
//! files, the same way a game would read them, and follows them when they change on disk (a stroke,
//! a run, an undo, an agent's edit): the files are polled twice a second and the meshes rebuilt only
//! when one's modification time moved.
//!
//! The ground sits [`crate::editor::ground_drop`] under the datum, like the bounds floor, so a floor
//! piece lying on flat ground is not z-fighting it. Water is a translucent quad per wet cell at
//! `ground + depth`.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::live::{KindVerdict, Outcome};
use bevy_grimoire_core::reload_check::{self, Check};
use bevy_grimoire_core::volume::Volume;
use bevy_grimoire_core::water::WaterField;

use crate::project::{OpenMap, Project};

/// Water shallower than this, metres, is not drawn: the simulation leaves a film on every wet slope.
pub const MIN_DRAWN_DEPTH_M: f32 = 0.02;

/// Water's tint: translucent, because the ground under it is part of the answer.
const WATER_TINT: [f32; 4] = [0.16, 0.42, 0.78, 0.62]; // CHROME-OK: world ink — a translucent water surface over the ground, not panel text

/// Which of the two drawn things an entity is.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainView {
    Ground,
    Water,
    /// One chunk of the carved ground (#101); drawn instead of [`TerrainView::Ground`] while the map has volume edits.
    Volume,
}

/// **The ground as a lit mesh**, one vertex per cell, tinted low (green) to high (tan) over the
/// field's own range so relief reads from above.
pub fn ground_mesh(ground: &Heightfield) -> Mesh {
    let (w, d) = (ground.width, ground.depth);
    let (lo, hi) = ground.range();
    let span = (hi - lo).max(1.0e-3);
    let at = |ix: usize, iz: usize| ground.heights[iz.min(d - 1) * w + ix.min(w - 1)];
    let mut positions = Vec::with_capacity(w * d);
    let mut normals = Vec::with_capacity(w * d);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(w * d);
    for iz in 0..d {
        for ix in 0..w {
            let h = at(ix, iz);
            positions.push([ground.origin.0 + ix as f32 * ground.cell_m, h, ground.origin.1 + iz as f32 * ground.cell_m]);
            let dx = (at(ix + 1, iz) - at(ix.saturating_sub(1), iz)) / (2.0 * ground.cell_m);
            let dz = (at(ix, iz + 1) - at(ix, iz.saturating_sub(1))) / (2.0 * ground.cell_m);
            normals.push(Vec3::new(-dx, 1.0, -dz).normalize().to_array());
            let t = ((h - lo) / span).clamp(0.0, 1.0);
            colors.push([0.30 + 0.38 * t, 0.46 + 0.14 * t, 0.24 + 0.22 * t, 1.0]); // CHROME-OK: world ink — relief tint
        }
    }
    let mut indices = Vec::with_capacity((w - 1) * (d - 1) * 6);
    for iz in 0..d - 1 {
        for ix in 0..w - 1 {
            let a = (iz * w + ix) as u32;
            let (b, c, e) = (a + 1, a + w as u32, a + w as u32 + 1);
            indices.extend_from_slice(&[a, c, b, b, c, e]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// **The kept water as one mesh**: a flat quad over each cell deeper than [`MIN_DRAWN_DEPTH_M`], at
/// the ground's height there plus the depth. `None` when nothing is wet, or when the water was run on
/// a ground of another shape than this one.
pub fn water_mesh(ground: &Heightfield, water: &WaterField) -> Option<Mesh> {
    if (water.width, water.depth) != (ground.width, ground.depth) || water.depth_m.len() != ground.heights.len() {
        return None;
    }
    let half = water.cell_m * 0.5;
    let (mut positions, mut indices) = (Vec::new(), Vec::new());
    for iz in 0..water.depth {
        for ix in 0..water.width {
            let i = iz * water.width + ix;
            if water.depth_m[i] <= MIN_DRAWN_DEPTH_M {
                continue;
            }
            let (x, z, y) = (water.origin.0 + ix as f32 * water.cell_m, water.origin.1 + iz as f32 * water.cell_m, ground.heights[i] + water.depth_m[i]);
            let base = positions.len() as u32;
            positions.extend_from_slice(&[[x - half, y, z - half], [x + half, y, z - half], [x - half, y, z + half], [x + half, y, z + half]]);
            indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
        }
    }
    if positions.is_empty() {
        return None;
    }
    let n = positions.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![WATER_TINT; n]);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

/// What the drawn meshes were built from; a change in any of it rebuilds them.
#[derive(Default, PartialEq)]
pub struct Built {
    map_path: std::path::PathBuf,
    ground: Option<std::time::SystemTime>,
    water: Option<std::time::SystemTime>,
    volume: Option<std::time::SystemTime>,
    origin: [u32; 3],
    drop: u32,
}

fn modified(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The ground's plain material, heightfield or carved; `environment::dress_volume` weathers the carved one.
pub fn ground_base() -> StandardMaterial {
    StandardMaterial { perceptual_roughness: 1.0, ..default() }
}

/// **The last good value of each file the view draws from**, kept in memory (#200): a refused file
/// leaves its slot as it was, so a bad edit never forces a reload from a bad file, and the drawing
/// can be rebuilt (the origin moved, the ground dropped) from what is known to be good.
#[derive(Default)]
pub struct Good {
    ground: Option<Heightfield>,
    water: Option<WaterField>,
    volume: Option<Volume>,
}

/// **Draw the open map's terrain and water**, and redraw when any of its files changes.
///
/// **A file that does not read is refused, and the last good value of it stays** (#187, #200). Each
/// file the view reads (the ground, the water, the carved volume) is decided on its own: one that
/// reads replaces its slot in [`Good`], one that is refused ([`Heightfield::parse`]: heights that do
/// not fill their shape, say) leaves it. The view is redrawn from the slots only when one of them, or
/// the map, its origin or the ground's drop, changed, so a refused edit leaves the entities drawn
/// before it untouched. A map that opens with a file refused has no earlier value and draws without
/// it. Each edit is one decision, recorded in [`crate::live::EditorReloads`] with the file and the
/// field it is wrong at and said once on the status line, because the files are only read again
/// when one's modification time moves.
#[allow(clippy::too_many_arguments)]
pub fn sync_terrain_view(
    time: Res<Time>,
    project: Res<Project>,
    open: Res<OpenMap>,
    mut state: ResMut<crate::editor::EditorState>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut drawn_state: Local<(f32, Option<Built>, Vec<Entity>, Good)>,
) {
    let (since, built, drawn, good) = &mut *drawn_state;
    *since += time.delta_secs();
    if built.is_some() && *since < 0.5 {
        return;
    }
    *since = 0.0;
    let name = open.map_path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".map.ron")).unwrap_or_default().to_owned();
    let ground_path = project.grimoire_dir.join("terrain").join(format!("{name}.ron"));
    let water_path = project.grimoire_dir.join("water").join(format!("{name}.field.ron"));
    let volume_path = project.grimoire_dir.join("volume").join(format!("{name}.ron"));
    let drop = crate::editor::ground_drop(&project);
    let now = Built {
        map_path: open.map_path.clone(),
        ground: modified(&ground_path),
        water: modified(&water_path),
        volume: modified(&volume_path),
        origin: [open.map.origin.0.to_bits(), open.map.origin.1.to_bits(), open.map.origin.2.to_bits()],
        drop: drop.to_bits(),
    };
    if built.as_ref() == Some(&now) {
        return;
    }
    // Another map has no earlier value of its own: nothing carries over.
    if built.as_ref().is_none_or(|b| b.map_path != now.map_path) {
        *good = Good::default();
    }
    let mut changed = built.as_ref().is_none_or(|b| (b.origin, b.drop) != (now.origin, now.drop));
    // Read each file that exists, and decide it: its value, or the refusal that names file and field.
    let ground = read_kind("terrain", &project, &ground_path, Heightfield::parse);
    let water = read_kind("water_field", &project, &water_path, WaterField::parse);
    let volume = read_kind("volume", &project, &volume_path, Volume::parse);
    let verdicts: Vec<KindVerdict> = ground
        .iter()
        .map(|(_, v)| v)
        .chain(water.iter().map(|(_, v)| v))
        .chain(volume.iter().map(|(_, v)| v))
        .cloned()
        .collect();
    if let Some(reloads) = reloads.as_deref_mut() {
        // One decision lists every file read, so a file that is gone is no longer refused.
        reloads.forget(&["terrain", "water_field", "volume"]);
        reloads.decide(verdicts);
    }
    for said in [
        refusal(&ground, good.ground.is_some(), "ground"),
        refusal(&water, good.water.is_some(), "water"),
        refusal(&volume, good.volume.is_some(), "carved ground"),
    ]
    .into_iter()
    .flatten()
    {
        state.status.problem(said);
    }
    adopt(&mut good.ground, ground, &mut changed);
    adopt(&mut good.water, water, &mut changed);
    adopt(&mut good.volume, volume, &mut changed);
    *built = Some(now);
    if !changed {
        // Last good wins: what is drawn is what these values draw, so leave it.
        return;
    }
    for e in drawn.drain(..) {
        commands.entity(e).despawn();
    }
    if let Some(ground) = &good.ground {
        let at = Transform::from_xyz(open.map.origin.0, open.map.origin.1 - drop, open.map.origin.2);
        let carved = volume_chunks(good.volume.as_ref(), ground);
        let material = materials.add(ground_base());
        if carved.is_empty() {
            drawn.push(commands.spawn((TerrainView::Ground, Mesh3d(meshes.add(ground_mesh(ground))), MeshMaterial3d(material), at)).id());
        } else {
            for mesh in carved {
                drawn.push(commands.spawn((TerrainView::Volume, Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), at)).id());
            }
        }
        if let Some(mesh) = good.water.as_ref().and_then(|w| water_mesh(ground, w)) {
            drawn.push(commands.spawn((
                TerrainView::Water,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial { alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.15, ..default() })),
                // The ground is dropped; the water is measured from the datum like pieces are.
                Transform::from_xyz(open.map.origin.0, open.map.origin.1, open.map.origin.2),
            )).id());
        }
    }
}

/// What the status line says about a refused file: which file and field, and what becomes of the
/// drawing. `kept` is whether an earlier good value of it is held. `None` for a file that read, or
/// that is not there.
fn refusal<T>(read: &Option<(Option<T>, KindVerdict)>, kept: bool, what: &str) -> Option<String> {
    let (_, verdict) = read.as_ref()?;
    if verdict.outcome != Outcome::Refused {
        return None;
    }
    let sentence = verdict.sentence();
    Some(if kept {
        format!("{sentence}: refused, so the earlier {what} stays")
    } else {
        format!("{sentence}: refused, so no {what} is drawn from it")
    })
}

/// **Keep the last good value of one file**: a file that read replaces the slot, a refused one leaves
/// it, a file that is gone empties it. `changed` is set when the slot's value is now another one.
fn adopt<T: PartialEq>(slot: &mut Option<T>, read: Option<(Option<T>, KindVerdict)>, changed: &mut bool) {
    match read {
        None => *changed |= slot.take().is_some(),
        Some((Some(value), _)) => {
            if slot.as_ref() != Some(&value) {
                *slot = Some(value);
                *changed = true;
            }
        }
        Some((None, _)) => {}
    }
}

/// **One file the view reads**, decided: `None` when it does not exist; otherwise its value when it
/// reads, and the verdict either way. A refusal names the file as the project's directory sees it
/// (`site/terrain/yard.ron`) and the field the file is wrong at.
fn read_kind<T>(
    kind: &'static str,
    project: &Project,
    path: &std::path::Path,
    parse: fn(&str) -> Result<T, String>,
) -> Option<(Option<T>, KindVerdict)> {
    let text = std::fs::read_to_string(path).ok()?;
    let file = path.strip_prefix(&project.project_dir).unwrap_or(path).to_string_lossy().replace('\\', "/");
    Some(match parse(&text) {
        Ok(value) => (Some(value), KindVerdict::applied(kind, file)),
        Err(reason) => {
            let field = bevy_grimoire_core::reload::kind_by_id(kind).and_then(|k| match reload_check::check(k, &file, &text) {
                Check::Refused(r) => r.field,
                _ => None,
            });
            (None, KindVerdict::refused(kind, file, field, reason))
        }
    })
}

/// **The carved ground's chunk meshes** (#101): empty when the map has no volume edits, or when the
/// `isomesh` feature is off, and the heightfield mesh is drawn instead.
#[cfg(feature = "isomesh")]
fn volume_chunks(volume: Option<&Volume>, ground: &Heightfield) -> Vec<Mesh> {
    let Some(volume) = volume else { return Vec::new() };
    let buffers = crate::volume_mesh::mesh_volume(volume, ground).unwrap_or_default();
    buffers
        .into_iter()
        .map(|b| {
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.normals);
            mesh.insert_indices(Indices::U32(b.indices));
            mesh
        })
        .collect()
}

#[cfg(not(feature = "isomesh"))]
fn volume_chunks(_: Option<&Volume>, _: &Heightfield) -> Vec<Mesh> {
    Vec::new()
}
