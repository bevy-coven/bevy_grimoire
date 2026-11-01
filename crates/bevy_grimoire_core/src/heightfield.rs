//! A heightfield a map's ground is sculpted into (#99, F3).
//!
//! Engine-free: a regular grid of metre heights over the XZ plane, with the brush strokes an editor
//! applies to it, a bilinear ground query, and the 16-bit height-map import/export that is the
//! interchange format with `bevy_elements`' terrain (`Terrain::from_heightmap` reads a height map and a
//! metre range). A stroke is a value ([`Stroke`]) so one gesture is one operation and one undo step:
//! [`Heightfield::apply`] returns the cells it changed, [`Heightfield::revert`] puts them back.
//!
//! The grid's cell `(ix, iz)` sits at world `(origin.0 + ix * cell_m, origin.1 + iz * cell_m)`.

use serde::{Deserialize, Serialize};

/// What a brush does to the heights under it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Brush {
    /// Add `strength` metres at the centre, falling off to nothing at the radius.
    Raise,
    /// Subtract `strength` metres, the same falloff.
    Lower,
    /// Move each cell `strength` (0..=1, times the falloff) of the way to the mean of its neighbours.
    Smooth,
    /// Move each cell `strength` (0..=1, times the falloff) of the way to this height in metres.
    Flatten(f32),
    /// Add seeded value noise of amplitude `strength` metres and wavelength `scale_m`.
    Noise { seed: u32, scale_m: f32 },
}

/// One gesture: a brush dragged over points. Emitted as one operation when the stroke ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub brush: Brush,
    /// Radius in metres; cells farther than this from a point are untouched.
    pub radius_m: f32,
    pub strength: f32,
    /// World `(x, z)` points the brush visits in order.
    pub points: Vec<(f32, f32)>,
}

/// The heights a stroke changed, for undo: `(cell index, before, after)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changed(pub Vec<(usize, f32, f32)>);

/// Why a height map could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum HeightmapError {
    /// `samples.len()` is not `width * height`, or a side is under 2.
    Shape { len: usize, width: usize, height: usize },
    /// The metre range is empty or not finite.
    Range,
}

impl std::fmt::Display for HeightmapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape { len, width, height } => {
                write!(f, "{len} samples do not fill a {width}x{height} height map (each side needs 2)")
            }
            Self::Range => write!(f, "the height range must be finite with min < max"),
        }
    }
}

impl std::error::Error for HeightmapError {}

/// The format version of a terrain file (#202). A file with no `version` is the first.
pub const TERRAIN_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Heightfield {
    /// The format version: [`TERRAIN_VERSION`]. A file from before it existed has none and reads as the
    /// first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "crate::format_version::first", deserialize_with = "crate::format_version::read::<_, TERRAIN_VERSION>")]
    pub version: u32,
    /// World `(x, z)` of cell `(0, 0)`.
    pub origin: (f32, f32),
    pub cell_m: f32,
    pub width: usize,
    pub depth: usize,
    /// Row-major, `depth` rows of `width`.
    pub heights: Vec<f32>,
}

impl Heightfield {
    /// A flat field at `height_m`.
    pub fn flat(origin: (f32, f32), cell_m: f32, width: usize, depth: usize, height_m: f32) -> Self {
        assert!(width >= 2 && depth >= 2 && cell_m > 0.0, "a heightfield needs 2x2 cells of positive size");
        Self { version: TERRAIN_VERSION, origin, cell_m, width, depth, heights: vec![height_m; width * depth] }
    }

    /// A field from a 16-bit height map: sample `0` is `range_m.0`, `65535` is `range_m.1`.
    pub fn from_u16(
        origin: (f32, f32),
        cell_m: f32,
        width: usize,
        depth: usize,
        samples: &[u16],
        range_m: (f32, f32),
    ) -> Result<Self, HeightmapError> {
        if width < 2 || depth < 2 || samples.len() != width * depth {
            return Err(HeightmapError::Shape { len: samples.len(), width, height: depth });
        }
        let (lo, hi) = range_m;
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(HeightmapError::Range);
        }
        let heights = samples.iter().map(|&s| lo + (hi - lo) * (f32::from(s) / 65535.0)).collect();
        Ok(Self { version: TERRAIN_VERSION, origin, cell_m, width, depth, heights })
    }

    /// **Read a terrain file** (`<grimoire_dir>/terrain/<map>.ron`), refusing one whose shape and data
    /// disagree (#187). Every reader indexes `heights` by `iz * width + ix` and clamps to
    /// `width - 1`, so a short or overfull `heights`, a side under 2, or a size or height that is not
    /// a finite number would panic the editor or wrap. This is the one way a file becomes a field; the
    /// refusal names the shape, as `Relations::parse` and `Standards::parse` name theirs.
    pub fn parse(text: &str) -> Result<Heightfield, String> {
        let field: Heightfield = ron::from_str(text).map_err(|e| format!("terrain: {e}"))?;
        field.validate()?;
        Ok(field)
    }

    /// What [`Self::parse`] checks, for a field built some other way.
    pub fn validate(&self) -> Result<(), String> {
        let (w, d) = (self.width, self.depth);
        if w < 2 || d < 2 {
            return Err(format!("terrain: a {w}x{d} field has no cell to stand in (each side needs 2)"));
        }
        if w.checked_mul(d) != Some(self.heights.len()) {
            return Err(format!("terrain: {} heights for {w}x{d} (it needs {})", self.heights.len(), w.saturating_mul(d)));
        }
        if !(self.cell_m.is_finite() && self.cell_m > 0.0) {
            return Err(format!("terrain: cell_m {} is not a positive size", self.cell_m));
        }
        if !(self.origin.0.is_finite() && self.origin.1.is_finite()) {
            return Err(format!("terrain: origin {:?} is not a finite point", self.origin));
        }
        if let Some(i) = self.heights.iter().position(|h| !h.is_finite()) {
            return Err(format!("terrain: height {i} (cell {}, {}) is {}", i % w, i / w, self.heights[i]));
        }
        Ok(())
    }

    /// The field as a 16-bit height map over `range_m`; heights outside it clamp.
    pub fn to_u16(&self, range_m: (f32, f32)) -> Result<Vec<u16>, HeightmapError> {
        let (lo, hi) = range_m;
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(HeightmapError::Range);
        }
        Ok(self
            .heights
            .iter()
            .map(|&h| (((h - lo) / (hi - lo)).clamp(0.0, 1.0) * 65535.0).round() as u16)
            .collect())
    }

    /// The lowest and highest height, the range a height map must cover to hold this field.
    pub fn range(&self) -> (f32, f32) {
        self.heights.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &h| (lo.min(h), hi.max(h)))
    }

    fn at(&self, ix: usize, iz: usize) -> f32 {
        self.heights[iz * self.width + ix]
    }

    /// The ground height at world `(x, z)`, bilinear between cells; clamped to the edge outside.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.origin.0) / self.cell_m).clamp(0.0, (self.width - 1) as f32);
        let fz = ((z - self.origin.1) / self.cell_m).clamp(0.0, (self.depth - 1) as f32);
        let (x0, z0) = (fx.floor() as usize, fz.floor() as usize);
        let (x1, z1) = ((x0 + 1).min(self.width - 1), (z0 + 1).min(self.depth - 1));
        let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
        let top = self.at(x0, z0) * (1.0 - tx) + self.at(x1, z0) * tx;
        let bottom = self.at(x0, z1) * (1.0 - tx) + self.at(x1, z1) * tx;
        top * (1.0 - tz) + bottom * tz
    }

    /// The height a piece with the given footprint corners (world `(x, z)`) rests at: the highest
    /// ground under any corner or the centre, so no corner sinks into a slope.
    pub fn rest_height(&self, corners: &[(f32, f32)]) -> f32 {
        if corners.is_empty() {
            return f32::NEG_INFINITY;
        }
        let n = corners.len() as f32;
        let centre = (corners.iter().map(|c| c.0).sum::<f32>() / n, corners.iter().map(|c| c.1).sum::<f32>() / n);
        corners.iter().copied().chain([centre]).map(|(x, z)| self.height_at(x, z)).fold(f32::NEG_INFINITY, f32::max)
    }

    /// Apply one stroke. Cells farther than `radius_m` from every point are untouched. Returns what
    /// changed, which [`Self::revert`] undoes.
    pub fn apply(&mut self, stroke: &Stroke) -> Changed {
        let before = self.heights.clone();
        if stroke.radius_m > 0.0 {
            for &(px, pz) in &stroke.points {
                self.dab(stroke, px, pz);
            }
        }
        let mut changed = Changed::default();
        for (i, (&b, &a)) in before.iter().zip(&self.heights).enumerate() {
            if b != a {
                changed.0.push((i, b, a));
            }
        }
        changed
    }

    /// Put back every cell a stroke changed.
    pub fn revert(&mut self, changed: &Changed) {
        for &(i, before, _) in &changed.0 {
            self.heights[i] = before;
        }
    }

    /// Re-apply a stroke's recorded result (redo).
    pub fn reapply(&mut self, changed: &Changed) {
        for &(i, _, after) in &changed.0 {
            self.heights[i] = after;
        }
    }

    fn dab(&mut self, stroke: &Stroke, px: f32, pz: f32) {
        let r = stroke.radius_m;
        let lo_x = (((px - r - self.origin.0) / self.cell_m).floor().max(0.0)) as usize;
        let lo_z = (((pz - r - self.origin.1) / self.cell_m).floor().max(0.0)) as usize;
        let hi_x = ((((px + r - self.origin.0) / self.cell_m).ceil()).max(0.0) as usize).min(self.width - 1);
        let hi_z = ((((pz + r - self.origin.1) / self.cell_m).ceil()).max(0.0) as usize).min(self.depth - 1);
        // Smoothing reads the field as it was before this dab, so the result does not depend on scan order.
        let snapshot = matches!(stroke.brush, Brush::Smooth).then(|| self.heights.clone());
        for iz in lo_z..=hi_z {
            for ix in lo_x..=hi_x {
                let (wx, wz) = (self.origin.0 + ix as f32 * self.cell_m, self.origin.1 + iz as f32 * self.cell_m);
                let d = ((wx - px).powi(2) + (wz - pz).powi(2)).sqrt();
                if d > r {
                    continue;
                }
                let t = 1.0 - d / r;
                let falloff = t * t * (3.0 - 2.0 * t);
                let i = iz * self.width + ix;
                let h = self.heights[i];
                self.heights[i] = match stroke.brush {
                    Brush::Raise => h + stroke.strength * falloff,
                    Brush::Lower => h - stroke.strength * falloff,
                    Brush::Flatten(target) => h + (target - h) * (stroke.strength.clamp(0.0, 1.0) * falloff),
                    Brush::Smooth => {
                        let s = snapshot.as_ref().expect("smooth keeps a snapshot");
                        let (mut sum, mut n) = (0.0, 0.0);
                        for (nx, nz) in [
                            (ix.wrapping_sub(1), iz),
                            (ix + 1, iz),
                            (ix, iz.wrapping_sub(1)),
                            (ix, iz + 1),
                        ] {
                            if nx < self.width && nz < self.depth {
                                sum += s[nz * self.width + nx];
                                n += 1.0;
                            }
                        }
                        h + (sum / n - h) * (stroke.strength.clamp(0.0, 1.0) * falloff)
                    }
                    Brush::Noise { seed, scale_m } => {
                        h + stroke.strength * falloff * value_noise(seed, wx / scale_m.max(1e-3), wz / scale_m.max(1e-3))
                    }
                };
            }
        }
    }
}

/// Smooth value noise in `-1..=1`, a pure function of `(seed, x, z)`.
fn value_noise(seed: u32, x: f32, z: f32) -> f32 {
    let lattice = |ix: i32, iz: i32| -> f32 {
        let mut h = (ix as u32).wrapping_mul(0x9E37_79B1) ^ (iz as u32).wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        (h as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let (x0, z0) = (x.floor(), z.floor());
    let (tx, tz) = (x - x0, z - z0);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let (ix, iz) = (x0 as i32, z0 as i32);
    let top = lattice(ix, iz) * (1.0 - sx) + lattice(ix + 1, iz) * sx;
    let bottom = lattice(ix, iz + 1) * (1.0 - sx) + lattice(ix + 1, iz + 1) * sx;
    top * (1.0 - sz) + bottom * sz
}
