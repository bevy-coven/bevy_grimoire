//! **Destructible props, through `bevy_carnage`** (#108): `destructible_set` and `destructible_hit`.
//!
//! A piece marked destructible is fractured once, by the crate's own `fracture_mesh`, over its mesh
//! and a box proxy of its bounds. The chunks are cached in the project, `destructible/<piece>.bake.ron`,
//! keyed by the mesh's bytes and the cut settings, so marking it again (or reopening the project)
//! never bakes it a second time and a changed mesh does. The marks are `destructible.ron`.
//!
//! `destructible_hit` previews a hit on the cached chunks and reports which break loose and whether
//! each is a closed solid, by the crate's own audit. The violence dials (`GorePolicy`) are the crate's
//! own settings, `family/bevy_carnage.ron`, written through `family_set`; the editor ships no gore.

use bevy::prelude::*;
use bevy_carnage::{CutSettings, ProxyCell, audit_cell, fracture_mesh};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

const VERSION: u32 = 1;
/// Chunks a fresh bake aims for, and the smallest share of the piece one may be.
const TARGET_CHUNKS: usize = 6;
const MIN_FRACTION: f32 = 0.05;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marks {
    version: u32,
    pieces: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    points: Vec<[f32; 3]>,
    faces: Vec<Vec<u32>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bake {
    version: u32,
    key: u64,
    chunks: Vec<Chunk>,
}

fn marks_path(root: &Path) -> PathBuf {
    root.join("destructible.ron")
}

/// `<project>/destructible/<piece>.bake.ron`, the piece id's `/` written `__`.
pub fn bake_path(root: &Path, piece: &str) -> PathBuf {
    root.join("destructible").join(format!("{}.bake.ron", piece.replace('/', "__")))
}

fn load_marks(root: &Path) -> Result<Marks, Refusal> {
    match std::fs::read_to_string(marks_path(root)) {
        Ok(t) => ron::from_str(&t).map_err(|e| Refusal::new(format!("destructible.ron does not parse: {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Marks { version: VERSION, pieces: Vec::new() }),
        Err(e) => Err(Refusal::new(format!("destructible.ron cannot be read: {e}"))),
    }
}

fn write_marks(root: &Path, marks: &Marks) -> Result<(), Refusal> {
    let text = ron::ser::to_string_pretty(marks, ron::ser::PrettyConfig::default()).map_err(|e| Refusal::new(e.to_string()))?;
    std::fs::write(marks_path(root), text).map_err(|e| Refusal::new(format!("destructible.ron refused the write, so nothing changed: {e}")))
}

fn cut_for(piece: &str) -> CutSettings {
    // One seed per piece id, so a piece breaks the same way every time it is baked.
    CutSettings::new(TARGET_CHUNKS, MIN_FRACTION, bevy_grimoire_core::glb::fnv1a(piece.as_bytes()) as u32)
}

fn cell_of(c: &Chunk) -> Option<ProxyCell> {
    ProxyCell::new(c.points.iter().map(|p| Vec3::from_array(*p)).collect(), c.faces.clone())
}

/// The piece's mesh as a carnage subject, and the key its cache is valid for.
fn subject(ctx: &Ctx, piece: &str) -> Result<(Mesh, ProxyCell, u64), Refusal> {
    let project = ctx.project.as_ref().ok_or_else(|| Refusal::new("no project is open."))?;
    let d = project.library.get(piece).ok_or_else(|| Refusal::new(format!("`{piece}` is not a piece of this library.")))?;
    let mesh = d.mesh.as_deref().ok_or_else(|| Refusal::new(format!("`{piece}` has no mesh to break.")))?;
    let path = project.root.join("assets").join(mesh);
    let (glb, hash) = bevy_grimoire_core::glb::Glb::open_fingerprinted(&path).map_err(|e| Refusal::new(format!("{}: {e}", path.display())))?;
    let tris = glb.triangle_vertices().map_err(Refusal::new)?;
    if tris.is_empty() {
        return Err(Refusal::new(format!("`{piece}`'s mesh has no triangles to break.")));
    }
    let positions: Vec<[f32; 3]> = tris.iter().flatten().copied().collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &positions {
        lo = lo.min(Vec3::from_array(*p));
        hi = hi.max(Vec3::from_array(*p));
    }
    let mut m = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    let proxy = ProxyCell::from_box((lo + hi) * 0.5, ((hi - lo) * 0.5).max(Vec3::splat(1.0e-4)));
    let key = hash ^ bevy_grimoire_core::glb::fnv1a(ron::to_string(&cut_for(piece)).unwrap_or_default().as_bytes()).rotate_left(17);
    Ok((m, proxy, key))
}

fn read_bake(root: &Path, piece: &str) -> Option<Bake> {
    ron::from_str(&std::fs::read_to_string(bake_path(root, piece)).ok()?).ok()
}

/// The cached bake for `piece` when it matches `key`; otherwise bake, write and return it. The flag is
/// whether this call did the baking.
fn baked(ctx: &Ctx, piece: &str) -> Result<(Bake, bool), Refusal> {
    let root = ctx.project.as_ref().map(|p| p.root.clone()).ok_or_else(|| Refusal::new("no project is open."))?;
    let (mesh, proxy, key) = subject(ctx, piece)?;
    if let Some(b) = read_bake(&root, piece).filter(|b| b.version == VERSION && b.key == key) {
        return Ok((b, false));
    }
    let leaves = fracture_mesh(&[(&mesh, Mat4::IDENTITY)], &[proxy], &cut_for(piece)).into_leaves();
    let chunks: Vec<Chunk> = leaves
        .iter()
        .map(|f| Chunk { points: f.cell.points().iter().map(|p| p.to_array()).collect(), faces: f.cell.faces().map(<[u32]>::to_vec).collect() })
        .collect();
    if chunks.len() < 2 {
        return Err(Refusal::new(format!("`{piece}` did not break into more than one chunk, so there is nothing to bake.")));
    }
    let bake = Bake { version: VERSION, key, chunks };
    let path = bake_path(&root, piece);
    std::fs::create_dir_all(path.parent().expect("a bake path has a directory")).map_err(|e| Refusal::new(e.to_string()))?;
    let text = ron::ser::to_string(&bake).map_err(|e| Refusal::new(e.to_string()))?;
    std::fs::write(&path, text).map_err(|e| Refusal::new(format!("{} refused the bake, so nothing changed: {e}", path.display())))?;
    Ok((bake, true))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    piece: String,
    on: bool,
}

/// **Mark a piece destructible, or take the mark off.** Marking bakes it once and caches the chunks.
pub struct DestructibleSet;

impl DestructibleSet {
    fn parse(raw: &serde_json::Value) -> Result<SetParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| Refusal::new(format!("this is not a destructible mark: {e}. The fields are `piece` and `on`.")))
    }
}

impl Operation for DestructibleSet {
    fn name(&self) -> &'static str {
        "destructible_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["destructible_marks", "destructible_bake"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["piece", "on"],
            "properties": {
                "piece": {"type": "string", "description": "The library piece id."},
                "on": {"type": "boolean", "description": "true marks it destructible and bakes its fracture once; false takes the mark off and keeps the bake."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = Self::parse(params)?;
        subject(ctx, &p.piece)?;
        Ok(json!({"piece": p.piece, "on": p.on}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = Self::parse(params)?;
        let root = ctx.root_or_project().ok_or_else(|| Refusal::new("no project is open."))?;
        let mut marks = load_marks(&root)?;
        marks.version = VERSION;
        let was = marks.pieces.contains(&p.piece);
        let mut fresh = false;
        let mut chunks = 0;
        if p.on {
            let (bake, did) = baked(ctx, &p.piece)?;
            (fresh, chunks) = (did, bake.chunks.len());
            if !was {
                marks.pieces.push(p.piece.clone());
                marks.pieces.sort();
            }
        } else {
            marks.pieces.retain(|x| x != &p.piece);
        }
        write_marks(&root, &marks)?;
        Ok(Outcome {
            summary: if p.on { format!("{} is destructible: {chunks} chunks{}", p.piece, if fresh { ", baked" } else { ", from the cached bake" }) } else { format!("{} is no longer destructible", p.piece) },
            affected: usize::from(was != p.on),
            saved: true,
            inverse: Some(json!({"piece": p.piece, "on": was})),
            after: Some(json!({"piece": p.piece, "on": p.on, "baked": fresh, "chunks": chunks, "cache": bake_path(&root, &p.piece)})),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HitParams {
    piece: String,
    /// Where it lands, in the piece's own mesh frame, metres.
    point: [f32; 3],
    /// Chunks whose centre is within this of the point break loose.
    radius: f32,
}

/// **Preview a hit on a destructible piece.** A read: it changes nothing and has no undo.
pub struct DestructibleHit;

impl DestructibleHit {
    fn run(ctx: &Ctx, raw: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p: HitParams = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a hit: {e}. The fields are `piece`, `point` and `radius`.")))?;
        if p.radius.is_nan() || p.radius <= 0.0 {
            return Err(Refusal::new("a hit needs a radius above zero."));
        }
        let root = ctx.root_or_project().ok_or_else(|| Refusal::new("no project is open."))?;
        if !load_marks(&root)?.pieces.contains(&p.piece) {
            return Err(Refusal::new(format!("`{}` is not destructible, so a hit would not break it. Mark it with destructible_set first.", p.piece)));
        }
        let (bake, _) = baked(ctx, &p.piece)?;
        let at = Vec3::from_array(p.point);
        let mut out = Vec::new();
        for c in &bake.chunks {
            let cell = cell_of(c).ok_or_else(|| Refusal::new("a cached chunk is not a convex solid; remove the bake and mark the piece again."))?;
            let a = audit_cell(&cell).map_err(Refusal::new)?;
            let centre = cell.center();
            out.push(json!({
                "centre": centre.to_array(),
                "volume": cell.volume(),
                "broken": centre.distance(at) <= p.radius,
                "watertight": a.boundary_edges == 0 && a.non_manifold_edges == 0 && a.inconsistently_oriented_edges == 0,
            }));
        }
        let broken = out.iter().filter(|c| c["broken"] == true).count();
        let closed = out.iter().all(|c| c["watertight"] == true);
        Ok(json!({"piece": p.piece, "broken": broken, "total": out.len(), "all_watertight": closed, "chunks": out}))
    }
}

impl Operation for DestructibleHit {
    fn name(&self) -> &'static str {
        "destructible_hit"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["destructible_bake"]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["piece", "point", "radius"],
            "properties": {
                "piece": {"type": "string", "description": "A piece marked destructible."},
                "point": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Where the hit lands, in the piece's mesh frame, metres."},
                "radius": {"type": "number", "description": "Chunks whose centre is within this many metres of the point break loose."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Self::run(ctx, params)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let after = Self::run(ctx, params)?;
        Ok(Outcome {
            summary: format!("a hit breaks {} of {} chunks of {}", after["broken"], after["total"], after["piece"].as_str().unwrap_or("")),
            affected: 0,
            saved: false,
            inverse: None,
            after: Some(after),
        })
    }
}
