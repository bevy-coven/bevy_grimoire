//! **Planting a forest under a stroke, as one registered operation** (#102): `forest_scatter`.
//!
//! The trees are `bevy_flora`'s own [`Scatter`](bevy_flora::Scatter): its Poisson-disc trunks, its
//! species weights and groves, its seed. The editor only masks the scatter to the stroke (a trunk
//! stays only within `radius_m` of the stroke's polyline), stands each tree on the map's terrain and
//! writes the result to `<grimoire_dir>/forest/<map>.ron`. The game reads the same file and spawns the
//! trees through the crate's pools and LOD; the editor never builds one entity per leaf.
//!
//! A stroke is a gesture: it arrives whole and makes one change and one undo. The inverse is the same
//! operation carrying the forest file as it was (`put_back`), which only an undo may send.
//!
//! Without the `bevy_flora` feature the operation is still registered (the surface does not change
//! with the build) and refuses, naming the feature.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// One planted tree. Same seed, same stroke, same trees: `id` is the tree's number in its forest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tree {
    pub id: u32,
    /// A preset name or a `.tree.ron` path: whatever the game's pools are keyed by.
    pub species: String,
    /// World `(x, z)` of the trunk.
    pub at: (f32, f32),
    /// The terrain surface under the trunk, metres.
    pub ground_m: f32,
    pub yaw: f32,
    pub scale: f32,
    /// The scatter's draw for the pool variant (taken modulo the pool's variant count).
    pub variant: u32,
    /// Years grown, for a tree placed by hand (`place`); a scattered tree has none and the game's pool decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_years: Option<f32>,
    /// The seed a placed tree was grown from: the same seed and species grow the same tree in the editor and the game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
}

/// A map's planted forest: `<grimoire_dir>/forest/<map>.ron`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forest {
    pub trees: Vec<Tree>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Species {
    pub name: String,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    /// World `(x, z)` points of the stroke, joined in order.
    pub points: Vec<(f32, f32)>,
    /// Trees stand only within this distance of the stroke.
    pub radius_m: f32,
    /// The least distance between two trunks, metres (the density).
    pub spacing_m: f32,
    #[serde(default)]
    pub seed: i32,
    /// The species mix. Never empty.
    pub species: Vec<Species>,
    /// Width of a one-species grove, metres; 0 draws every tree's species on its own.
    #[serde(default)]
    pub grove_m: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<(f32, f32)>,
}

/// One tree placed by hand: a preset name or `.tree.ron` path, at `(x, z)`, with an age and a seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    pub species: String,
    pub at: (f32, f32),
    #[serde(default)]
    pub age_years: f32,
    #[serde(default)]
    pub seed: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant: Option<Plant>,
    /// The forest file as it was (`None` inside: there was none). Only an undo sends this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The map whose forest is planted."},
                "place": {"type": "object", "description": "One tree by hand, instead of a stroke: {species, at: [x, z], age_years, seed}. The same species and seed give the same yaw, scale and variant, so the same tree."},
                "plant": {"type": "object", "description": "One gesture: {points: [[x, z], ...], radius_m, spacing_m, seed, species: [{name, weight}], grove_m, scale: [min, max]}. Trees are planted only within radius_m of the stroke, on the map's terrain, at least spacing_m apart and from trees already there."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a forest stroke: {e}. The fields are `map` and `plant`, and nothing else.")))?;
        if p.plant.is_none() && p.place.is_none() && p.put_back.is_none() {
            return Err(Refusal::new("a forest stroke needs a `plant` or a `place`. Nothing was changed."));
        }
        if p.plant.is_some() && p.place.is_some() {
            return Err(Refusal::new("send a `plant` stroke or a `place`d tree, not both. Nothing was changed."));
        }
        if let Some(pl) = &p.place {
            if pl.species.trim().is_empty() {
                return Err(Refusal::new("a placed tree needs a species (a preset name or a .tree.ron path). Nothing was changed."));
            }
            if !(pl.age_years.is_finite() && pl.age_years >= 0.0) {
                return Err(Refusal::new("`age_years` must be 0 or more. Nothing was changed."));
            }
        }
        if let Some(pl) = &p.plant {
            if pl.points.is_empty() {
                return Err(Refusal::new("a forest stroke needs at least one point. Nothing was changed."));
            }
            if pl.radius_m <= 0.0 || pl.spacing_m <= 0.0 {
                return Err(Refusal::new("`radius_m` and `spacing_m` must be positive. Nothing was changed."));
            }
            if pl.species.is_empty() || pl.species.iter().all(|s| s.weight <= 0.0) {
                return Err(Refusal::new("a forest needs a species with a weight above 0. Nothing was changed."));
            }
        }
        Ok(p)
    }
}

fn map_name(map: &str) -> Result<&str, Refusal> {
    if map.is_empty() || map.contains(['/', '\\', '.']) {
        return Err(Refusal::new(format!("`{map}` is not a map name. Nothing was written.")));
    }
    Ok(map)
}

fn forest_path(ctx: &Ctx, map: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open, so there is no forest to plant."));
    };
    Ok(project.grimoire_dir.join("forest").join(format!("{}.ron", map_name(map)?)))
}

/// **The saved forest of a map**, empty when there is none.
pub fn forest_of(project: &crate::project::Project, map: &str) -> Forest {
    std::fs::read_to_string(project.grimoire_dir.join("forest").join(format!("{map}.ron")))
        .ok()
        .and_then(|t| ron::from_str(&t).ok())
        .unwrap_or_default()
}

/// Squared distance from `p` to the segment `a`-`b`.
fn dist2_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, abz) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + abz * abz;
    let t = if len2 > 0.0 { (((p.0 - a.0) * abx + (p.1 - a.1) * abz) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cz) = (a.0 + abx * t, a.1 + abz * t);
    (p.0 - cx).powi(2) + (p.1 - cz).powi(2)
}

/// Whether `p` is within `radius` of the polyline `points` (a lone point is a disc).
pub fn under_stroke(p: (f32, f32), points: &[(f32, f32)], radius: f32) -> bool {
    let r2 = radius * radius;
    match points {
        [] => false,
        [only] => dist2_to_segment(p, *only, *only) <= r2,
        _ => points.windows(2).any(|w| dist2_to_segment(p, w[0], w[1]) <= r2),
    }
}

/// **Place one tree**: the draws a hand-placed tree's look comes from are a pure function of its species and seed.
pub fn place(forest: &Forest, at: &Place, ground: Option<&bevy_grimoire_core::heightfield::Heightfield>) -> Tree {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ (at.seed as u32 as u64);
    for b in at.species.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    let mut draw = || {
        h = h.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = h;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let unit = |v: u64| (v >> 40) as f32 / (1u64 << 24) as f32;
    Tree {
        id: forest.trees.iter().map(|t| t.id + 1).max().unwrap_or(0),
        species: at.species.clone(),
        at: at.at,
        ground_m: ground.map_or(0.0, |g| g.height_at(at.at.0, at.at.1)),
        yaw: unit(draw()) * std::f32::consts::TAU,
        scale: 0.9 + unit(draw()) * 0.2,
        variant: (draw() >> 32) as u32,
        age_years: Some(at.age_years),
        seed: Some(at.seed),
    }
}

/// **Plant**: the trees `plant` adds to `forest`, with the next ids, standing on `ground`.
#[cfg(feature = "bevy_flora")]
pub fn plant(forest: &Forest, plant: &Plant, ground: Option<&bevy_grimoire_core::heightfield::Heightfield>) -> Vec<Tree> {
    use bevy::math::{Rect, Vec2};
    let r = plant.radius_m;
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for &(x, z) in &plant.points {
        lo = (lo.0.min(x - r), lo.1.min(z - r));
        hi = (hi.0.max(x + r), hi.1.max(z + r));
    }
    let mut scatter = bevy_flora::Scatter::new(Rect::new(lo.0, lo.1, hi.0, hi.1), plant.spacing_m);
    scatter.seed = plant.seed;
    scatter.species = plant.species.iter().map(|s| s.weight).collect();
    scatter.grove_size = plant.grove_m;
    if let Some(s) = plant.scale {
        scatter.scale = s;
    }
    let near2 = plant.spacing_m * plant.spacing_m;
    let mut next = forest.trees.iter().map(|t| t.id + 1).max().unwrap_or(0);
    let mut placed: Vec<Tree> = Vec::new();
    for p in scatter.points_where(|v: Vec2| if under_stroke((v.x, v.y), &plant.points, r) { 1.0 } else { 0.0 }) {
        let at = (p.position.x, p.position.y);
        let crowded = forest.trees.iter().any(|t| (t.at.0 - at.0).powi(2) + (t.at.1 - at.1).powi(2) < near2);
        if crowded {
            continue;
        }
        placed.push(Tree {
            id: next,
            species: plant.species[p.species.min(plant.species.len() - 1)].name.clone(),
            at,
            ground_m: ground.map_or(0.0, |g| g.height_at(at.0, at.1)),
            yaw: p.yaw,
            scale: p.scale,
            variant: p.variant,
            age_years: None,
            seed: None,
        });
        next += 1;
    }
    placed
}

/// **Plant a forest under one stroke.**
pub struct ForestScatter;

impl ForestScatter {
    #[cfg(feature = "bevy_flora")]
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let path = forest_path(ctx, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});
        let (text, affected) = if let Some(back) = &p.put_back {
            (back.text.clone(), 0)
        } else {
            let project = ctx.project.as_ref().expect("forest_path needs a project");
            let mut forest = forest_of(project, &p.map);
            // A terrain file that does not read is flat ground here, as the terrain view draws it (#187).
            let ground = std::fs::read_to_string(project.grimoire_dir.join("terrain").join(format!("{}.ron", p.map)))
                .ok()
                .and_then(|t| bevy_grimoire_core::heightfield::Heightfield::parse(&t).ok());
            let added = match (&p.place, &p.plant) {
                (Some(at), _) => vec![place(&forest, at, ground.as_ref())],
                (None, Some(stroke)) => plant(&forest, stroke, ground.as_ref()),
                (None, None) => unreachable!("parse needs a plant or a place"),
            };
            let n = added.len();
            forest.trees.extend(added);
            let text = ron::ser::to_string_pretty(&forest, ron::ser::PrettyConfig::default())
                .map_err(|e| Refusal::new(format!("the forest could not be written: {e}")))?;
            (Some(text), n)
        };
        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    std::fs::write(&path, t)
                        .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(Outcome {
            summary: format!("forest {}: {affected} trees planted", p.map),
            affected,
            saved: write,
            inverse: Some(inverse),
            after: Some(json!({"map": p.map, "text": text})),
        })
    }

    #[cfg(not(feature = "bevy_flora"))]
    fn apply(_ctx: &Ctx, params: &serde_json::Value, _write: bool) -> Result<Outcome, Refusal> {
        Params::parse(params)?;
        Err(Refusal::new("this build has no forest: rebuild the editor with `--features bevy_flora`. Nothing was changed."))
    }
}

impl Operation for ForestScatter {
    fn name(&self) -> &'static str {
        "forest_scatter"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["forest"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).map(|p| p.put_back.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        Some(json!({"map": map, "text": std::fs::read_to_string(forest_path(ctx, map).ok()?).ok()}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        Ok(json!({"map": Params::parse(params)?.map, "trees": o.affected}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}
