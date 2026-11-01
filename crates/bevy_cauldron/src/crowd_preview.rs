//! **Crowd preview: see how people will move** (#107).
//!
//! N walkers go from one area of a map to another. Each steps with `bevy_orca`'s reciprocal
//! avoidance (against its neighbours and against the cells the map's pieces make solid), and lays a
//! trail and a congestion mark into a `bevy_stigmergy` field. The result is a **measurement**: the
//! two fields, how many arrived and where the peak congestion is. It saves nothing to the map.
//!
//! No ECS and no clock: [`Preview::step`] advances one fixed `dt`, so the same map, spec and seed
//! give the same run on every machine. The path is a breadth-first distance field over walkable cells
//! (8-connected, never cutting a corner of a solid cell), so walkers route around buildings and queue
//! at a gap rather than at a wall.
//!
//! **What is solid.** A floor-standing piece (`Mount::OnFloor` or none) at least [`SOLID_HEIGHT`] tall
//! whose measured footprint covers a cell's centre. A piece with no measured footprint or height is
//! unmeasured, not solid (the same "unknown is not everywhere" rule as `stack::covers`). Pass a map
//! already expanded (`Map::expanded`) when it holds stamps or runs.

use bevy_grimoire_core::descriptor::{Mount, placed_height};
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::stack;
use bevy::math::{IVec2, Vec2};
use bevy_orca::{Crowd, CrowdMember, CrowdParams};
use bevy_stigmergy::{CellFrame, ChannelDef, StigGrid};

/// A floor-standing piece at least this tall, in metres, stops a walker. Knee-high clutter does not.
pub const SOLID_HEIGHT: f32 = 0.5;

/// A cell this far below its channel's peak is not drawn: the overlay shows where, not everywhere.
pub const HEAT_FLOOR: f32 = 0.05;

/// Channel indices of [`Preview::fields`].
pub const TRAIL: usize = 0;
pub const CONGESTION: usize = 1;

/// An axis-aligned rectangle in map space, metres: `(min_x, min_z, max_x, max_z)`.
pub type Area = (f32, f32, f32, f32);

/// What to run. All lengths are metres, all times seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub from: Area,
    pub to: Area,
    pub walkers: usize,
    pub seed: u64,
    /// Fixed step.
    pub dt: f32,
    /// Grid cell size; the map grid's own snap by default.
    pub cell: f32,
    pub radius: f32,
    pub speed: f32,
}

impl Spec {
    pub fn new(from: Area, to: Area, walkers: usize, seed: u64) -> Self {
        Spec { from, to, walkers, seed, dt: 0.1, cell: 0.5, radius: 0.2, speed: 1.4 }
    }
}

/// One walker, as a test or a drawing sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    pub key: u64,
    pub pos: Vec2,
    pub vel: Vec2,
}

/// The measurement of a run so far.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub steps: usize,
    pub arrived: usize,
    pub walking: usize,
    /// Total congestion laid down (walker-seconds spent below half speed).
    pub congestion_total: f32,
    /// The most congested cell, its cell centre in map space and its value.
    pub hotspot: Option<(Vec2, f32)>,
}

/// A run in progress.
pub struct Preview {
    frame: CellFrame,
    solid: Vec<bool>,
    /// Steps to the goal area, `u32::MAX` for unreachable.
    dist: Vec<u32>,
    goal: Vec<bool>,
    walkers: Vec<Walker>,
    arrived: usize,
    steps: usize,
    spec: Spec,
    crowd: Crowd,
    fields: StigGrid<2>,
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The cells of the map's floor rectangle that its pieces make solid, row-major.
pub fn solid_cells(map: &Map, library: &Library, frame: &CellFrame) -> Vec<bool> {
    let blockers: Vec<_> = map
        .placements
        .iter()
        .filter_map(|p| {
            let d = stack::descriptor_of(library, p)?;
            let floor_standing = matches!(d.mount, None | Some(Mount::OnFloor));
            (floor_standing && placed_height(&d)? >= SOLID_HEIGHT && d.extent.footprint.is_some())
                .then(|| (d.into_owned(), p.at, p.yaw))
        })
        .collect();
    frame
        .cells()
        .map(|c| {
            let centre = frame.cell_centre(c);
            blockers.iter().any(|(d, at, yaw)| stack::covers(d, *at, *yaw, (centre.x, centre.y)))
        })
        .collect()
}

impl Preview {
    /// Set a run up, or say why it cannot start.
    pub fn new(map: &Map, library: &Library, spec: Spec) -> Result<Preview, String> {
        let (x0, z0, x1, z1) = map.floor_rect();
        let frame = CellFrame::covering(Vec2::new(x0, z0), Vec2::new(x1, z1), spec.cell);
        if frame.width == 0 || frame.height == 0 {
            return Err("the map has no floor to walk on".to_owned());
        }
        if spec.walkers == 0 {
            return Err("a crowd needs at least one walker".to_owned());
        }
        if !(spec.dt > 0.0 && spec.speed > 0.0 && spec.radius > 0.0) {
            return Err("dt, speed and radius must be positive".to_owned());
        }
        let solid = solid_cells(map, library, &frame);
        let in_area = |c: IVec2, a: Area| {
            let p = frame.cell_centre(c);
            p.x >= a.0 && p.x <= a.2 && p.y >= a.1 && p.y <= a.3
        };
        let idx = |c: IVec2| c.y as usize * frame.width + c.x as usize;
        let free = |c: IVec2| frame.contains(c) && !solid[idx(c)];
        let goal: Vec<bool> = frame.cells().map(|c| free(c) && in_area(c, spec.to)).collect();
        if !goal.iter().any(|g| *g) {
            return Err("the destination area holds no walkable cell".to_owned());
        }

        // Breadth-first from every goal cell; a diagonal needs both of its side cells free.
        let mut dist = vec![u32::MAX; solid.len()];
        let mut queue = std::collections::VecDeque::new();
        for (i, g) in goal.iter().enumerate() {
            if *g {
                dist[i] = 0;
                queue.push_back(IVec2::new((i % frame.width) as i32, (i / frame.width) as i32));
            }
        }
        while let Some(c) = queue.pop_front() {
            let d = dist[idx(c)];
            for (dx, dy) in NEIGHBOURS {
                let n = c + IVec2::new(dx, dy);
                if !free(n) || dist[idx(n)] != u32::MAX {
                    continue;
                }
                if dx != 0 && dy != 0 && !(free(c + IVec2::new(dx, 0)) && free(c + IVec2::new(0, dy))) {
                    continue;
                }
                dist[idx(n)] = d + 1;
                queue.push_back(n);
            }
        }

        // One walker per start cell, the cells ordered by a seeded hash: deterministic and never
        // overlapping (the cell is at least a body wide).
        let mut starts: Vec<IVec2> = frame
            .cells()
            .filter(|c| free(*c) && in_area(*c, spec.from) && dist[idx(*c)] != u32::MAX && !goal[idx(*c)])
            .collect();
        if starts.len() < spec.walkers {
            return Err(format!(
                "the start area has {} reachable cells for {} walkers",
                starts.len(),
                spec.walkers
            ));
        }
        starts.sort_by_key(|c| (splitmix(spec.seed ^ ((c.y as u64) << 32 | c.x as u64)), c.y, c.x));
        let walkers = starts[..spec.walkers]
            .iter()
            .enumerate()
            .map(|(i, c)| Walker { key: i as u64, pos: frame.cell_centre(*c), vel: Vec2::ZERO })
            .collect();

        let floor = frame.floor_cells(|p| {
            let c = frame.world_to_cell(p);
            free(c)
        });
        let fields = StigGrid::<2>::new(
            frame.width,
            frame.height,
            floor,
            [
                ChannelDef { evaporate: 0.02, diffuse: 0.0, deposit_radius: 1.0 },
                ChannelDef { evaporate: 0.0, diffuse: 0.0, deposit_radius: 1.0 },
            ],
        );
        Ok(Preview { frame, solid, dist, goal, walkers, arrived: 0, steps: 0, spec, crowd: Crowd::default(), fields })
    }

    fn idx(&self, c: IVec2) -> usize {
        c.y as usize * self.frame.width + c.x as usize
    }

    fn free(&self, c: IVec2) -> bool {
        self.frame.contains(c) && !self.solid[self.idx(c)]
    }

    /// Whether a map-space point stands in a solid cell (or off the map).
    pub fn is_solid_at(&self, p: Vec2) -> bool {
        !self.free(self.frame.world_to_cell(p))
    }

    /// Where a walker in `cell` wants to go: the centre of the neighbour nearest the goal.
    fn wish(&self, cell: IVec2) -> Vec2 {
        let mut best = (self.dist[self.idx(cell)], None);
        for (dx, dy) in NEIGHBOURS {
            let n = cell + IVec2::new(dx, dy);
            if !self.free(n) {
                continue;
            }
            if dx != 0 && dy != 0 && !(self.free(cell + IVec2::new(dx, 0)) && self.free(cell + IVec2::new(0, dy))) {
                continue;
            }
            let d = self.dist[self.idx(n)];
            if d < best.0 {
                best = (d, Some(n));
            }
        }
        match best.1 {
            Some(n) => self.frame.cell_centre(n),
            None => self.frame.cell_centre(cell),
        }
    }

    /// Advance one `dt`. Returns whether anyone is still walking.
    pub fn step(&mut self) -> bool {
        let dt = self.spec.dt;
        let (radius, speed) = (self.spec.radius, self.spec.speed);
        let members: Vec<CrowdMember> = self
            .walkers
            .iter()
            .map(|w| {
                let cell = self.frame.world_to_cell(w.pos);
                let to = self.wish(cell) - w.pos;
                let pref = if to.length_squared() > 1e-6 { to.normalize() * speed } else { Vec2::ZERO };
                CrowdMember { key: w.key, pos: w.pos, vel: w.vel, radius, max_speed: speed, pref_vel: Some(pref) }
            })
            .collect();
        let params = CrowdParams::new(2.0);
        let (frame, solid) = (self.frame, &self.solid);
        let free = |c: IVec2| frame.contains(c) && !solid[c.y as usize * frame.width + c.x as usize];
        let positions: Vec<Vec2> = members.iter().map(|m| m.pos).collect();
        let solved = self.crowd.solve(
            &members,
            &params,
            dt,
            |_, _| true,
            |i, walls| {
                // A solid 4-neighbour cell is a wall: the walker may coast up to it, never into it.
                let pos = positions[i];
                let cell = frame.world_to_cell(pos);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if free(cell + IVec2::new(dx, dy)) {
                        continue;
                    }
                    let b = Vec2::new(dx as f32, dy as f32);
                    let edge = frame.cell_centre(cell) + b * (frame.cell_size * 0.5);
                    let gap = (edge - pos).dot(b) - radius;
                    walls.push((b, gap.max(0.0) / dt));
                }
            },
        );
        let solved: Vec<Option<Vec2>> = solved.to_vec();

        let mut congestion = Vec::new();
        let mut trail = Vec::new();
        for (w, v) in self.walkers.iter_mut().zip(solved) {
            let vel = v.unwrap_or(Vec2::ZERO);
            let try_to = |p: Vec2| free(frame.world_to_cell(p));
            let mut next = w.pos + vel * dt;
            let mut vel = vel;
            if !try_to(next) {
                // Slide along the wall on whichever axis is open; stop if neither is.
                let x_only = Vec2::new(next.x, w.pos.y);
                let y_only = Vec2::new(w.pos.x, next.y);
                if try_to(x_only) {
                    next = x_only;
                    vel.y = 0.0;
                } else if try_to(y_only) {
                    next = y_only;
                    vel.x = 0.0;
                } else {
                    next = w.pos;
                    vel = Vec2::ZERO;
                }
            }
            let cell = frame.world_to_cell(w.pos);
            trail.push(cell);
            if vel.length() < 0.5 * speed {
                congestion.push(cell);
            }
            w.pos = next;
            w.vel = vel;
        }
        for c in trail {
            self.fields.deposit(TRAIL, c, dt);
        }
        for c in congestion {
            self.fields.deposit(CONGESTION, c, dt);
        }
        self.fields.evaporate_diffuse(dt);

        // Arrivals leave the run.
        let before = self.walkers.len();
        let (frame, goal) = (self.frame, &self.goal);
        self.walkers.retain(|w| {
            let c = frame.world_to_cell(w.pos);
            !goal[c.y as usize * frame.width + c.x as usize]
        });
        self.arrived += before - self.walkers.len();
        self.steps += 1;
        !self.walkers.is_empty()
    }

    /// Step until everyone arrives or `max_steps` have passed.
    pub fn run(&mut self, max_steps: usize) -> Report {
        while self.steps < max_steps && self.step() {}
        self.report()
    }

    pub fn walkers(&self) -> &[Walker] {
        &self.walkers
    }

    /// The trail and congestion fields, for a heat overlay.
    pub fn fields(&self) -> &StigGrid<2> {
        &self.fields
    }

    pub fn frame(&self) -> &CellFrame {
        &self.frame
    }

    /// **The heat overlay's data**: every cell where the trail or the congestion reaches
    /// [`HEAT_FLOOR`] of its own peak, as `(cell centre in map space, trail, congestion)`, each
    /// normalised to 0..=1 by its channel's peak. The peak cell is exactly 1.0. Row-major, so the
    /// same run lists the same cells.
    pub fn heat(&self) -> Vec<(Vec2, f32, f32)> {
        let [trail, congestion] = self.fields.channels();
        let peak = |v: &[f32]| v.iter().copied().fold(0.0_f32, f32::max);
        let (tp, cp) = (peak(trail), peak(congestion));
        let norm = |v: f32, p: f32| if p > 0.0 { (v / p).clamp(0.0, 1.0) } else { 0.0 };
        self.frame
            .cells()
            .enumerate()
            .filter_map(|(i, c)| {
                let (t, g) = (norm(trail[i], tp), norm(congestion[i], cp));
                (t >= HEAT_FLOOR || g >= HEAT_FLOOR).then(|| (self.frame.cell_centre(c), t, g))
            })
            .collect()
    }

    pub fn report(&self) -> Report {
        let congestion_total: f32 = self.fields.channels()[CONGESTION].iter().sum();
        let (cell, value) = self.fields.hotspot_cell(CONGESTION);
        Report {
            steps: self.steps,
            arrived: self.arrived,
            walking: self.walkers.len(),
            congestion_total,
            hotspot: cell.map(|c| (self.frame.cell_centre(c), value)),
        }
    }
}

const NEIGHBOURS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
