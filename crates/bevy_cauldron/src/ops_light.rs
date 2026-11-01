//! **The light map** (#103): `bevy_light_grid`'s illuminance field over the open map, as one
//! registered, read-only operation.
//!
//! Every placement that carries a `bevy_lights` `Flicker` (set by `placement_set_family`) is a fixture:
//! its `base` over `reference` is the intensity and the map's cells are `cell_m` metres. A placement
//! whose descriptor is `kind: ["wall"]` blocks line of sight, so a torch behind a wall does not light
//! the room beyond it. The values are the crate's own `LightGrid::sample_cell`, not a second formula,
//! so what the author reads is what a light-shy creature in the game reads.

use bevy_light_grid::LightGrid;
use bevy::math::IVec2;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Metres per grid cell.
    #[serde(default = "one")]
    pub cell_m: f32,
    /// How far a fixture reaches, in metres.
    #[serde(default = "six")]
    pub range_m: f32,
    /// The `Flicker::base` that counts as intensity 1.0.
    #[serde(default = "thousand")]
    pub reference: f32,
    /// Map-space points `[x, z]` to read; each answers with the grid's own value for its cell.
    #[serde(default)]
    pub at: Vec<(f32, f32)>,
}

impl Default for Params {
    /// The operation's own defaults, so the viewport overlay bakes what a bare `light_map` reads.
    fn default() -> Self {
        Params { cell_m: one(), range_m: six(), reference: thousand(), at: Vec::new() }
    }
}

fn one() -> f32 {
    1.0
}
fn six() -> f32 {
    6.0
}
fn thousand() -> f32 {
    1000.0
}

/// A baked light map and how its cells sit on the map.
pub struct Baked {
    pub grid: LightGrid,
    pub cell_m: f32,
    /// Map-space metres of cell (0, 0)'s corner.
    pub min: (f32, f32),
    pub fixtures: usize,
}

impl Baked {
    /// The cell a map-space point falls in.
    pub fn cell_of(&self, at: (f32, f32)) -> IVec2 {
        IVec2::new(((at.0 - self.min.0) / self.cell_m).floor() as i32, ((at.1 - self.min.1) / self.cell_m).floor() as i32)
    }
}

/// Bake the map's light field. One place, so the overlay and the operation cannot disagree.
pub fn bake(map: &bevy_grimoire_core::map::Map, library: &bevy_grimoire_core::library::Library, p: &Params) -> Result<Baked, Refusal> {
    if !(p.cell_m > 0.0 && p.range_m > 0.0 && p.reference > 0.0) {
        return Err(Refusal::new("`cell_m`, `range_m` and `reference` must be above zero."));
    }
    let (x0, z0, x1, z1) = map.floor_rect();
    let (w, h) = ((((x1 - x0) / p.cell_m).ceil() as usize).max(1), (((z1 - z0) / p.cell_m).ceil() as usize).max(1));
    let mut baked = Baked { grid: LightGrid::new(w, h, std::iter::empty()), cell_m: p.cell_m, min: (x0, z0), fixtures: 0 };
    let cell = |at: (f32, f32)| baked.cell_of(at);
    let mut walls = std::collections::HashSet::new();
    let mut fixtures = Vec::new();
    for pl in &map.placements {
        if library.get(&pl.descriptor).is_some_and(|d| d.kind.iter().any(|k| k == "wall")) {
            walls.insert(cell(pl.at));
        }
        if let Some(text) = pl.family.get("bevy_lights") {
            let f: bevy_lights::Flicker = ron::from_str(text).map_err(|e| Refusal::new(format!("{}'s bevy_lights settings do not read: {e}", pl.id)))?;
            fixtures.push((cell(pl.at), f.base / p.reference, p.range_m / p.cell_m));
        }
    }
    let floor = (0..h as i32).flat_map(|y| (0..w as i32).map(move |x| IVec2::new(x, y))).filter(|c| !walls.contains(c));
    let mut grid = LightGrid::new(w, h, floor);
    // Walk the segment a fixture sees along; a wall cell strictly between the two blocks it.
    let los = |a: IVec2, b: IVec2| {
        let n = (b - a).abs().max_element();
        (1..n).all(|i| {
            let t = i as f32 / n as f32;
            let c = a.as_vec2().lerp(b.as_vec2(), t).round().as_ivec2();
            !walls.contains(&c)
        })
    };
    grid.bake(&fixtures, los);
    grid.compose(&[], |_, _| true);
    baked.fixtures = fixtures.len();
    baked.grid = grid;
    Ok(baked)
}

/// **Read the light map** — the overlay's numbers, for a person's overlay and an agent's `apply`.
pub struct LightMap;

impl LightMap {
    fn run(ctx: &Ctx, raw: &serde_json::Value) -> Result<(String, serde_json::Value), Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not light map parameters: {e}. The fields are `cell_m`, `range_m`, `reference` and `at`."))
        })?;
        let (Some(open), Some(project)) = (ctx.open.as_ref(), ctx.project.as_ref()) else {
            return Err(Refusal::new("no map is open, so there is no light to map."));
        };
        let b = bake(&open.map, &project.library, &p)?;
        let (w, h) = b.grid.dimensions();
        let samples: Vec<_> = p.at.iter().map(|&a| json!({"at": [a.0, a.1], "cell": [b.cell_of(a).x, b.cell_of(a).y], "lux": b.grid.sample_cell(b.cell_of(a))})).collect();
        let after = json!({
            "map": open.map.name, "width": w, "height": h, "cell_m": b.cell_m, "fixtures": b.fixtures,
            "peak": b.grid.peak(), "samples": samples, "cells": b.grid.cells(),
        });
        Ok((format!("light map: {} fixtures, peak {:.2} over {w}x{h} cells", b.fixtures, b.grid.peak()), after))
    }
}

impl Operation for LightMap {
    fn name(&self) -> &'static str {
        "light_map"
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "cell_m": {"type": "number", "description": "Metres per grid cell. Default 1."},
                "range_m": {"type": "number", "description": "How far a fixture reaches, metres. Default 6."},
                "reference": {"type": "number", "description": "The bevy_lights Flicker base that counts as intensity 1.0. Default 1000."},
                "at": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}, "description": "Map-space [x, z] points to read."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Self::run(ctx, params).map(|(_, after)| after)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (summary, after) = Self::run(ctx, params)?;
        Ok(Outcome { summary, affected: 0, saved: false, inverse: None, after: Some(after) })
    }
}
