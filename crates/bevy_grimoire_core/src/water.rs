//! Where water comes from and goes, saved beside a map (#100, F4).
//!
//! Engine-free. A [`WaterPlan`] is the author's input to `bevy_elements`' hydrology: springs
//! (`WaterSource`, m³/s over a footprint), sinks (`WaterSink`) and lakes (a level the water is held
//! to over a footprint). The field names match the crate's own `WaterSource` and `WaterSink`, so the
//! game's copy of the plugin reads the same numbers. The plan is data; running the simulation is a
//! separate, explicit step and never part of placing.

use serde::{Deserialize, Serialize};

/// Pours water in: `rate_m3_s` over the cells within `radius_m` of `(x, z)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spring {
    pub x: f32,
    pub z: f32,
    pub rate_m3_s: f32,
    pub radius_m: f32,
}

/// Drains water: up to `rate_m3_s` from the cells within `radius_m` of `(x, z)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sink {
    pub x: f32,
    pub z: f32,
    pub rate_m3_s: f32,
    pub radius_m: f32,
}

/// A lake: the water surface is held at `level_m` within `radius_m` of `(x, z)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lake {
    pub x: f32,
    pub z: f32,
    pub radius_m: f32,
    pub level_m: f32,
}

/// One thing to place.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Placed {
    Spring(Spring),
    Sink(Sink),
    Lake(Lake),
}

/// The format version of a water plan file (#202). A file with no `version` is the first.
pub const WATER_PLAN_VERSION: u32 = 1;

/// The format version of a water field file (#202). A file with no `version` is the first.
pub const WATER_FIELD_VERSION: u32 = 1;

/// Every water source, sink and lake of a map, in the order they were placed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterPlan {
    /// The format version: [`WATER_PLAN_VERSION`]. A file from before it existed has none and reads as
    /// the first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "crate::format_version::first", deserialize_with = "crate::format_version::read::<_, WATER_PLAN_VERSION>")]
    pub version: u32,
    #[serde(default)]
    pub springs: Vec<Spring>,
    #[serde(default)]
    pub sinks: Vec<Sink>,
    #[serde(default)]
    pub lakes: Vec<Lake>,
}

impl Default for WaterPlan {
    fn default() -> Self {
        WaterPlan { version: WATER_PLAN_VERSION, springs: Vec::new(), sinks: Vec::new(), lakes: Vec::new() }
    }
}

impl WaterPlan {
    pub fn is_empty(&self) -> bool {
        self.springs.is_empty() && self.sinks.is_empty() && self.lakes.is_empty()
    }

    /// Add one; a non-finite number or a non-positive radius or rate is refused, and nothing changes.
    pub fn place(&mut self, what: Placed) -> Result<(), String> {
        let (nums, radius, rate) = match what {
            Placed::Spring(s) => ([s.x, s.z, s.rate_m3_s, s.radius_m], s.radius_m, Some(s.rate_m3_s)),
            Placed::Sink(s) => ([s.x, s.z, s.rate_m3_s, s.radius_m], s.radius_m, Some(s.rate_m3_s)),
            Placed::Lake(l) => ([l.x, l.z, l.radius_m, l.level_m], l.radius_m, None),
        };
        if nums.iter().any(|n| !n.is_finite()) {
            return Err("every number must be finite".to_owned());
        }
        if radius <= 0.0 {
            return Err("the radius must be above zero".to_owned());
        }
        if rate.is_some_and(|r| r <= 0.0) {
            return Err("the rate must be above zero".to_owned());
        }
        match what {
            Placed::Spring(s) => self.springs.push(s),
            Placed::Sink(s) => self.sinks.push(s),
            Placed::Lake(l) => self.lakes.push(l),
        }
        Ok(())
    }

    /// **Read a plan from its file's text**, refusing what [`Self::place`] would refuse: a spring, sink
    /// or lake with a number that is not finite or a radius or rate that is not above zero. The
    /// refusal names the entry (`spring 2`), so an edit that cannot be run is refused when it lands
    /// and not when `water_run` reads it.
    pub fn parse(text: &str) -> Result<WaterPlan, String> {
        let read: WaterPlan = ron::from_str(text).map_err(|e| format!("water plan: {e}"))?;
        let mut checked = WaterPlan::default();
        for (i, s) in read.springs.iter().enumerate() {
            checked.place(Placed::Spring(*s)).map_err(|e| format!("water plan: spring {i}: {e}"))?;
        }
        for (i, s) in read.sinks.iter().enumerate() {
            checked.place(Placed::Sink(*s)).map_err(|e| format!("water plan: sink {i}: {e}"))?;
        }
        for (i, l) in read.lakes.iter().enumerate() {
            checked.place(Placed::Lake(*l)).map_err(|e| format!("water plan: lake {i}: {e}"))?;
        }
        Ok(read)
    }
}

/// **The water the plan settles into** (#100): a depth in metres over every cell of the ground it
/// ran on, kept as the map's starting water.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterField {
    /// The format version: [`WATER_FIELD_VERSION`]. A file from before it existed has none and reads as
    /// the first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "crate::format_version::first", deserialize_with = "crate::format_version::read::<_, WATER_FIELD_VERSION>")]
    pub version: u32,
    /// World `(x, z)` of cell `(0, 0)`, the ground's.
    pub origin: (f32, f32),
    pub cell_m: f32,
    pub width: usize,
    pub depth: usize,
    /// Water depth above the ground per cell, row-major like the ground.
    pub depth_m: Vec<f32>,
    /// Whether the run ended because the water stopped moving (not because it ran out of steps).
    pub settled: bool,
    /// Simulation steps the run took.
    pub steps: u32,
}

impl WaterField {
    /// **Read a field from its file's text**, refusing one whose depths do not fill its shape, whose
    /// cell is not a positive size, or with a number that is not finite. The shape check is the one
    /// the drawing relies on: a field of another shape than its ground draws no water, and one whose
    /// depths are short must not be indexed at all.
    pub fn parse(text: &str) -> Result<WaterField, String> {
        let field: WaterField = ron::from_str(text).map_err(|e| format!("water field: {e}"))?;
        let (w, d) = (field.width, field.depth);
        if w.checked_mul(d) != Some(field.depth_m.len()) {
            return Err(format!("water field: {} depths for {w}x{d} (it needs {})", field.depth_m.len(), w.saturating_mul(d)));
        }
        if !(field.cell_m.is_finite() && field.cell_m > 0.0) {
            return Err(format!("water field: cell_m {} is not a positive size", field.cell_m));
        }
        if !(field.origin.0.is_finite() && field.origin.1.is_finite()) {
            return Err(format!("water field: origin {:?} is not a finite point", field.origin));
        }
        if let Some(i) = field.depth_m.iter().position(|v| !v.is_finite()) {
            return Err(format!("water field: depth {i} is {}", field.depth_m[i]));
        }
        Ok(field)
    }

    /// Total water, m³.
    pub fn volume_m3(&self) -> f32 {
        self.depth_m.iter().sum::<f32>() * self.cell_m * self.cell_m
    }

    /// Cells holding more than `min_m` of water.
    pub fn wet_cells(&self, min_m: f32) -> usize {
        self.depth_m.iter().filter(|&&d| d > min_m).count()
    }

    /// Water depth at world `(x, z)` (nearest cell); `0` off the field.
    pub fn depth_at(&self, x: f32, z: f32) -> f32 {
        let (fx, fz) = ((x - self.origin.0) / self.cell_m, (z - self.origin.1) / self.cell_m);
        if fx < -0.5 || fz < -0.5 {
            return 0.0;
        }
        let (ix, iz) = (fx.round() as usize, fz.round() as usize);
        if ix >= self.width || iz >= self.depth {
            return 0.0;
        }
        self.depth_m[iz * self.width + ix]
    }
}

/// How long and how far a run goes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunSettings {
    /// Seconds the springs pour; after that the water runs on by itself until it settles.
    pub pour_s: f32,
    /// Step cap, so a run always ends.
    pub max_steps: u32,
    /// The water has settled when no cell's depth changes by more than this in one step, metres.
    pub tolerance_m: f32,
}

impl Default for RunSettings {
    fn default() -> Self {
        Self { pour_s: 60.0, max_steps: 20_000, tolerance_m: 1e-5 }
    }
}

const GRAVITY: f32 = 9.81;
/// Fraction of a pipe's flux kept per step: friction, so ripples die and the water can settle.
const FLUX_KEPT: f32 = 0.985;

/// **Run the plan on the ground** until the water settles: the virtual-pipes shallow-water model
/// (Mei et al. 2007) `bevy_elements`' hydrology uses, on the CPU so it runs anywhere. Springs pour
/// for `pour_s`, sinks drain, lakes hold their level inside their radius, and the edge of the
/// ground is open (water leaves as a free overfall). The same ground and plan always give the same
/// field.
pub fn simulate(ground: &crate::heightfield::Heightfield, plan: &WaterPlan, settings: RunSettings) -> WaterField {
    let (w, h, l) = (ground.width, ground.depth, ground.cell_m);
    let n = w * h;
    let area = l * l;
    let cells_in = |x: f32, z: f32, r: f32| -> Vec<usize> {
        let mut v = Vec::new();
        for iz in 0..h {
            for ix in 0..w {
                let (cx, cz) = (ground.origin.0 + ix as f32 * l, ground.origin.1 + iz as f32 * l);
                if (cx - x).hypot(cz - z) <= r.max(l * 0.5) {
                    v.push(iz * w + ix);
                }
            }
        }
        v
    };
    let springs: Vec<_> = plan.springs.iter().map(|s| (cells_in(s.x, s.z, s.radius_m), s.rate_m3_s)).collect();
    let sinks: Vec<_> = plan.sinks.iter().map(|s| (cells_in(s.x, s.z, s.radius_m), s.rate_m3_s)).collect();
    let lakes: Vec<_> = plan.lakes.iter().map(|k| (cells_in(k.x, k.z, k.radius_m), k.level_m)).collect();

    // One step moves at most dt²·g/l of the surface difference between neighbours; 0.05 keeps the
    // pipes from overshooting at any cell size.
    let dt = (0.05 * l / GRAVITY).sqrt();
    let mut d = vec![0.0f32; n];
    let mut flux = vec![[0.0f32; 4]; n]; // +x, -x, +z, -z
    for (cells, level) in &lakes {
        for &c in cells {
            d[c] = (level - ground.heights[c]).max(0.0);
        }
    }
    let neighbour = |ix: usize, iz: usize, k: usize| -> Option<usize> {
        match k {
            0 if ix + 1 < w => Some(iz * w + ix + 1),
            1 if ix > 0 => Some(iz * w + ix - 1),
            2 if iz + 1 < h => Some((iz + 1) * w + ix),
            3 if iz > 0 => Some((iz - 1) * w + ix),
            _ => None,
        }
    };
    let (mut steps, mut settled) = (0u32, false);
    while steps < settings.max_steps {
        let pouring = steps as f32 * dt < settings.pour_s;
        let before = d.clone();
        if pouring {
            for (cells, rate) in &springs {
                let add = rate * dt / (cells.len() as f32 * area);
                for &c in cells {
                    d[c] += add;
                }
            }
        }
        for (cells, rate) in &sinks {
            let take = rate * dt / (cells.len() as f32 * area);
            for &c in cells {
                d[c] = (d[c] - take).max(0.0);
            }
        }
        // Flux from the height difference to each neighbour; an off-field neighbour is an overfall
        // at the cell's own ground level.
        for iz in 0..h {
            for ix in 0..w {
                let c = iz * w + ix;
                let surface = ground.heights[c] + d[c];
                let mut out = [0.0f32; 4];
                for (k, o) in out.iter_mut().enumerate() {
                    let other = match neighbour(ix, iz, k) {
                        Some(m) => ground.heights[m] + d[m],
                        None => ground.heights[c],
                    };
                    *o = (flux[c][k] * FLUX_KEPT + dt * GRAVITY * l * (surface - other)).max(0.0);
                }
                let total: f32 = out.iter().sum::<f32>() * dt;
                let scale = if total > d[c] * area { d[c] * area / total } else { 1.0 };
                for (f, o) in flux[c].iter_mut().zip(out) {
                    *f = o * scale;
                }
            }
        }
        for iz in 0..h {
            for ix in 0..w {
                let c = iz * w + ix;
                let mut net = -flux[c].iter().sum::<f32>();
                for (k, back) in [(0, 1), (1, 0), (2, 3), (3, 2)] {
                    if let Some(m) = neighbour(ix, iz, k) {
                        net += flux[m][back];
                    }
                }
                d[c] = (d[c] + net * dt / area).max(0.0);
            }
        }
        for (cells, level) in &lakes {
            for &c in cells {
                d[c] = d[c].max(level - ground.heights[c]);
            }
        }
        steps += 1;
        if !pouring && d.iter().zip(&before).all(|(a, b)| (a - b).abs() <= settings.tolerance_m) {
            settled = true;
            break;
        }
    }
    WaterField { version: WATER_FIELD_VERSION, origin: ground.origin, cell_m: l, width: w, depth: h, depth_m: d, settled, steps }
}
