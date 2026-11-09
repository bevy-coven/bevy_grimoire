//! **Block rules: what a laid-out block of tiles has to satisfy, shared by the editor and the game.**
//!
//! A game draws one example block in the editor; [`crate::grammar::learn`] reads the tile
//! adjacencies off it and [`crate::wfc::collapse_grid`] fills blocks from them. What local
//! adjacency cannot say is stated here, in a `rules.ron` the project carries beside `kits.ron`, so
//! the editor's preview and the game build the same block from the same file (Sanctuary decision
//! 0091). The rules were Sanctuary's own `town_plan.rs`; they are generic now.
//!
//! # What a rule says
//!
//! - **`border`**: a descriptor pinned at yaw 0 on every cell of the block's ring and removed from
//!   every other cell. The grammar learns that a ring street may touch street, and left open the
//!   solver grew streets across a block's interior.
//! - **`no_empty`**: the empty prototype is removed from every cell that is not pinned.
//! - **`weights`**: a calibration, not part of the learning. Learned weights are the example's tile
//!   counts, which put houses at 16 percent of a block's cells; the collapse does not keep that
//!   share, so the weight of every piece whose id starts with a prefix is multiplied.
//! - **`footprints`**: a multi-cell thing (a house) is nine parts, `<prefix>front_left` ..
//!   `<prefix>back_right` ([`PARTS`]), by position in its rectangle. Local rules cannot make a
//!   rectangle, so [`validate`] checks afterwards that every connected group of a footprint's parts
//!   is a whole rectangle of an allowed size, turned one way (yaw 0 or 180), with every cell the part
//!   its place says, a `front` tile before each front-row cell, and a gap of ground between two
//!   rectangles. A collapse that fails is retried with another seed ([`solve_block`]).
//!
//! # References
//!
//! - Karth and Smith, "WaveFunctionCollapse is Constraint Solving in the Wild", FDG 2017: the
//!   search is arc-consistent constraint solving over tile domains, which is what `collapse_grid`
//!   does.
//! - Sandhu, Chen and McCoy, "Enhancing Wave Function Collapse with Design-Level Constraints",
//!   FDG 2019, doi 10.1145/3337722.3337752: WFC's rules are local, so non-local design constraints
//!   (here, footprints) need a further layer; this module states them in [`validate`] and rejects
//!   and retries rather than modifying the solver.
//! - Cooper, "Sturgeon: Tile-Based Procedural Level Generation via Learned and Designed
//!   Constraints", AIIDE 2022, doi 10.1609/aiide.v18i1.21944: the same split between constraints
//!   learned from an example and constraints the designer states.
//! - Nie, Zheng, Zhuang and Song, "Extend Wave Function Collapse to Large-Scale Content
//!   Generation", arXiv 2308.07307 (2023): contradictions grow with grid size, which is why a town
//!   is collapsed one block at a time and not as one grid.

use serde::{Deserialize, Serialize};

use crate::glb::fnv1a;
use crate::grammar::{self, Grammar, Prototype};
use crate::library::Library;
use crate::map::{Map, Placed};
use crate::wfc;

/// Bumped whenever the shape below changes. A file with another number is refused, not migrated.
pub const RULES_VERSION: u32 = 1;

/// Where a project keeps its rules, relative to the project directory (`assets/emerge`). Absent
/// means no rules.
pub const FILE: &str = "rules.ron";

/// The nine parts of a footprint, by position: the front row is the door side, left and right are
/// the footprint's own axes. The index of a part is `row * 3 + column`, row 0 being the front.
pub const PARTS: [&str; 9] = [
    "front_left",
    "front",
    "front_right",
    "side_left",
    "core",
    "side_right",
    "back_left",
    "back",
    "back_right",
];

/// The rules a project states for its blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub version: u32,
    #[serde(default)]
    pub blocks: Vec<BlockRules>,
}

/// The rules for one kind of block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockRules {
    /// The map the grammar is learned from (its name, so `maps/<example>.map.ron`).
    pub example: String,
    /// Descriptor ids whose placements each ask for one block, laid out around them.
    #[serde(default)]
    pub regions: Vec<String>,
    /// A cell's edge in metres.
    pub cell: f32,
    /// The block's size in cells, columns then rows. The example's bounds are `size * cell`.
    pub size: (u32, u32),
    /// Pinned at yaw 0 on every border cell, and removed from every other cell's domain.
    #[serde(default)]
    pub border: Option<String>,
    /// The empty prototype is removed from every cell that is not pinned.
    #[serde(default)]
    pub no_empty: bool,
    #[serde(default)]
    pub weights: Vec<Weight>,
    #[serde(default)]
    pub footprints: Vec<Footprint>,
    /// Seeds tried per block before it is given up.
    pub attempts: u32,
    /// The seed every block's own seed is derived from.
    pub seed: u64,
}

/// Multiply the learned weight of every piece whose id starts with `prefix` by `times`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weight {
    pub prefix: String,
    pub times: f64,
}

/// A multi-cell thing made of nine parts named `<prefix><part>` ([`PARTS`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footprint {
    pub prefix: String,
    /// Allowed sizes in cells, width then depth, along the footprint's own axes.
    pub sizes: Vec<(u32, u32)>,
    /// Allowed door yaws in degrees. Only 0 and 180 are implemented.
    pub yaws: Vec<f32>,
    /// The descriptor that must lie before each front-row cell, if any.
    #[serde(default)]
    pub front: Option<String>,
    /// Cells of ground required between two footprints of this kind.
    pub gap: u32,
    /// The fewest footprints a block of this kind may hold.
    pub min_count: u32,
}

impl Rules {
    pub fn parse(text: &str) -> Result<Rules, String> {
        ron::from_str(text).map_err(|e| format!("rules: {e}"))
    }

    /// Everything that can be checked without the example: the version, the numbers, and that every
    /// descriptor the rules name is in `library`.
    pub fn validate_against(&self, library: &Library) -> Result<(), String> {
        if self.version != RULES_VERSION {
            return Err(format!(
                "rules: version {} is not the {RULES_VERSION} this program reads",
                self.version
            ));
        }
        let known = |id: &str, what: &str| -> Result<(), String> {
            if library.get(id).is_none() {
                return Err(format!("rules: {what} `{id}` is not a descriptor in the library"));
            }
            Ok(())
        };
        for rule in &self.blocks {
            let who = &rule.example;
            if !(rule.cell.is_finite() && rule.cell > 0.0) {
                return Err(format!("rules: `{who}` has cell {}, which is not a usable step", rule.cell));
            }
            if rule.attempts < 1 {
                return Err(format!("rules: `{who}` needs at least one attempt"));
            }
            if rule.size.0 < 1 || rule.size.1 < 1 {
                return Err(format!("rules: `{who}` has a block of {}x{} cells", rule.size.0, rule.size.1));
            }
            if let Some(border) = &rule.border {
                known(border, "the border")?;
            }
            for region in &rule.regions {
                known(region, "the region")?;
            }
            for footprint in &rule.footprints {
                if footprint.sizes.is_empty() || footprint.yaws.is_empty() {
                    return Err(format!("rules: footprint `{}` needs a size and a yaw", footprint.prefix));
                }
                for &(w, d) in &footprint.sizes {
                    if w < 2 || d < 2 {
                        return Err(format!(
                            "rules: footprint `{}` has a {w}x{d} size; every size is at least 2x2 \
                             (a footprint has a front row, a back row and, at 3, a middle)",
                            footprint.prefix
                        ));
                    }
                }
                if footprint.yaws.iter().any(|&y| y != 0.0 && y != 180.0) {
                    return Err(
                        "rules: footprint yaws must be 0 or 180 (90 and 270 are not implemented)".to_owned()
                    );
                }
                if let Some(front) = &footprint.front {
                    known(front, "the front tile")?;
                }
                for part in PARTS {
                    known(&format!("{}{part}", footprint.prefix), "the footprint part")?;
                }
            }
        }
        Ok(())
    }
}

/// The grammar learned from an example, and what each of its prototypes means to the rules.
#[derive(Debug)]
pub struct Alphabet {
    pub grammar: Grammar,
    /// The selection weights the collapse uses: the learned ones with [`BlockRules::weights`] applied.
    pub weights: Vec<f64>,
    /// Per prototype: its footprint (an index into [`BlockRules::footprints`]), its part (an index
    /// into [`PARTS`]) and its yaw; `None` for a prototype that is no footprint part.
    pub parts: Vec<Option<(usize, usize, f32)>>,
}

impl Alphabet {
    /// The prototype that is `descriptor` turned `yaw`, if the example has one.
    pub fn index_of(&self, descriptor: &str, yaw: f32) -> Option<usize> {
        self.grammar.prototypes.iter().position(|p| {
            matches!(p, Prototype::Piece { descriptor: d, yaw: y } if d == descriptor && (y - yaw).abs() < 1.0)
        })
    }

    fn piece_at(&self, prototype: usize) -> Option<(&str, f32)> {
        match self.grammar.prototypes.get(prototype)? {
            Prototype::Piece { descriptor, yaw } => Some((descriptor.as_str(), *yaw)),
            _ => None,
        }
    }
}

/// A footprint on a block's grid and the way its door faces: 0 turns the door south (+Z), 180
/// north. Columns run east, rows south, so row 0 is the north edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub footprint: usize,
    pub c0: usize,
    pub r0: usize,
    pub w: usize,
    pub d: usize,
    pub yaw: f32,
}

impl Rect {
    /// The grid cell of local cell (`u`, `v`): the footprint's left is its own -X, so for one turned
    /// north (yaw 180) left lies to the east.
    pub fn cell(&self, u: usize, v: usize) -> (usize, usize) {
        if self.yaw == 0.0 {
            (self.c0 + u, self.r0 + self.d - 1 - v)
        } else {
            (self.c0 + self.w - 1 - u, self.r0 + v)
        }
    }

    /// The local cell of grid cell (`c`, `r`), the inverse of [`Rect::cell`]. Only meaningful for a
    /// cell inside the rectangle.
    pub fn local(&self, c: usize, r: usize) -> (usize, usize) {
        if self.yaw == 0.0 {
            (c - self.c0, self.r0 + self.d - 1 - r)
        } else {
            (self.c0 + self.w - 1 - c, r - self.r0)
        }
    }

    /// The grid cell in front of front-row cell (`c`, `r`), if it is on a grid of `rows` rows.
    pub fn in_front_of(&self, c: usize, r: usize, rows: usize) -> Option<(usize, usize)> {
        if self.yaw == 0.0 {
            Some((c, r + 1)).filter(|&(_, r)| r < rows)
        } else {
            r.checked_sub(1).map(|r| (c, r))
        }
    }
}

/// A collapsed block: the prototype per cell, row-major, and the footprints found on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub grid: Vec<usize>,
    pub rects: Vec<Rect>,
}

/// The index into [`PARTS`] of local cell (`u`, `v`) of a `w` wide, `d` deep footprint.
pub fn part_index(u: usize, v: usize, w: usize, d: usize) -> usize {
    let across = if u == 0 {
        0
    } else if u == w - 1 {
        2
    } else {
        1
    };
    let down = if v == 0 {
        0
    } else if v == d - 1 {
        2
    } else {
        1
    };
    down * 3 + across
}

/// Learn the grammar from `example` under `rules`.
pub fn learn(rules: &BlockRules, example: &Map) -> Result<Alphabet, String> {
    let who = &rules.example;
    let want = (rules.size.0 as f32 * rules.cell, rules.size.1 as f32 * rules.cell);
    if (example.bounds.0 - want.0).abs() > 0.01 || (example.bounds.2 - want.1).abs() > 0.01 {
        return Err(format!(
            "rules: `{who}` is {}x{} m, but a {}x{} block of {} m cells is {}x{} m",
            example.bounds.0, example.bounds.2, rules.size.0, rules.size.1, rules.cell, want.0, want.1
        ));
    }
    let grammar = grammar::learn(example, rules.cell)
        .map_err(|e| format!("rules: the example `{who}` teaches nothing: {e}"))?;
    if grammar.len() > grammar::MAX_PROTOTYPES {
        return Err(format!(
            "rules: the example `{who}` has {} prototypes, over the solver's {}",
            grammar.len(),
            grammar::MAX_PROTOTYPES
        ));
    }
    let weights = grammar
        .weights
        .iter()
        .zip(&grammar.prototypes)
        .map(|(w, p)| match p {
            Prototype::Piece { descriptor, .. } => rules
                .weights
                .iter()
                .filter(|m| descriptor.starts_with(&m.prefix))
                .fold(*w, |w, m| w * m.times),
            _ => *w,
        })
        .collect();
    let parts = grammar
        .prototypes
        .iter()
        .map(|p| {
            let Prototype::Piece { descriptor, yaw } = p else {
                return None;
            };
            rules.footprints.iter().enumerate().find_map(|(f, footprint)| {
                let rest = descriptor.strip_prefix(footprint.prefix.as_str())?;
                let part = PARTS.iter().position(|name| *name == rest)?;
                Some((f, part, *yaw))
            })
        })
        .collect();
    Ok(Alphabet { grammar, weights, parts })
}

/// The pins that make `rect` stand in a block: each cell of it, as (cell index, prototype), so the
/// solver keeps those cells as the parts their place says.
pub fn rect_pins(
    alphabet: &Alphabet,
    rules: &BlockRules,
    rect: &Rect,
) -> Result<Vec<(usize, usize)>, String> {
    let footprint = rules
        .footprints
        .get(rect.footprint)
        .ok_or_else(|| format!("rules: no footprint {}", rect.footprint))?;
    let cols = rules.size.0 as usize;
    let rows = rules.size.1 as usize;
    let mut pins = Vec::with_capacity(rect.w * rect.d);
    for v in 0..rect.d {
        for u in 0..rect.w {
            let (c, r) = rect.cell(u, v);
            if c >= cols || r >= rows {
                return Err(format!("rules: a pinned {}x{} footprint falls off the block at ({c}, {r})", rect.w, rect.d));
            }
            let part = part_index(u, v, rect.w, rect.d);
            let name = format!("{}{}", footprint.prefix, PARTS[part]);
            let prototype = alphabet.index_of(&name, rect.yaw).ok_or_else(|| {
                format!("rules: the example has no `{name}` turned {} degrees", rect.yaw)
            })?;
            pins.push((r * cols + c, prototype));
        }
    }
    Ok(pins)
}

/// Each cell's starting domain, as a bitmask over prototypes: everything, minus the empty
/// prototype (if `no_empty`) and the border's, with the ring pinned to the border and each of
/// `pins` (cell index, prototype) on top.
///
/// **A pin outside the grid, or two pins on one cell, is an error**: the second would silently win.
pub fn initial(
    alphabet: &Alphabet,
    rules: &BlockRules,
    pins: &[(usize, usize)],
) -> Result<Vec<u32>, String> {
    let n = alphabet.grammar.len();
    let cols = rules.size.0 as usize;
    let rows = rules.size.1 as usize;
    let full = if n >= 32 { u32::MAX } else { (1u32 << n) - 1 };
    let mut open = full;
    // Prototype 0 is the empty one: `grammar::learn` starts its list with it.
    if rules.no_empty {
        open &= !1;
    }
    let border = match &rules.border {
        Some(id) => {
            let at = alphabet
                .index_of(id, 0.0)
                .ok_or_else(|| format!("rules: the example has no `{id}` turned 0 degrees for the border"))?;
            Some(1u32 << at)
        }
        None => None,
    };
    if let Some(bit) = border {
        open &= !bit;
    }
    let mut domains = vec![open; cols * rows];
    if let Some(bit) = border {
        for r in 0..rows {
            for c in 0..cols {
                if r == 0 || c == 0 || r == rows - 1 || c == cols - 1 {
                    domains[r * cols + c] = bit;
                }
            }
        }
    }
    let mut pinned = vec![false; cols * rows];
    for &(cell, prototype) in pins {
        if cell >= domains.len() {
            return Err(format!("rules: a pin at cell {cell} is outside the {cols}x{rows} block"));
        }
        if prototype >= n {
            return Err(format!("rules: a pin names prototype {prototype} of {n}"));
        }
        if pinned[cell] {
            return Err(format!(
                "rules: two pins on cell ({}, {}) of the block",
                cell % cols,
                cell / cols
            ));
        }
        pinned[cell] = true;
        domains[cell] = 1u32 << prototype;
    }
    Ok(domains)
}

/// Whether every footprint on a collapsed `grid` is whole and well placed. Returns the rectangles,
/// in row-major order of their first cell, or `None` for a grid that breaks a rule: a group that
/// is not a whole rectangle of an allowed size (two footprints touching side to side merge into one
/// component and fail the size test), cells that disagree about the way they face or about which
/// part they are, a front cell that does not face the `front` descriptor, two rectangles with less
/// than `gap` cells between them, or fewer than `min_count` of a kind.
pub fn validate(alphabet: &Alphabet, rules: &BlockRules, grid: &[usize]) -> Option<Vec<Rect>> {
    let cols = rules.size.0 as usize;
    let rows = rules.size.1 as usize;
    let part_of = |c: usize, r: usize| -> Option<(usize, usize, f32)> {
        alphabet.parts.get(*grid.get(r * cols + c)?).copied().flatten()
    };

    let mut seen = vec![false; cols * rows];
    let mut rects: Vec<Rect> = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let Some((footprint, _, yaw)) = part_of(c, r) else {
                continue;
            };
            if seen[r * cols + c] {
                continue;
            }
            let of_kind = |c: usize, r: usize| part_of(c, r).is_some_and(|(f, _, _)| f == footprint);
            // The 4-connected component of this footprint's parts, by flood fill.
            let mut cells = vec![(c, r)];
            seen[r * cols + c] = true;
            let mut next = 0;
            while let Some(&(cc, cr)) = cells.get(next) {
                next += 1;
                let neighbours = [
                    cr.checked_sub(1).map(|r| (cc, r)),
                    Some((cc + 1, cr)).filter(|&(c, _)| c < cols),
                    Some((cc, cr + 1)).filter(|&(_, r)| r < rows),
                    cc.checked_sub(1).map(|c| (c, cr)),
                ];
                for (nc, nr) in neighbours.into_iter().flatten() {
                    if !seen[nr * cols + nc] && of_kind(nc, nr) {
                        seen[nr * cols + nc] = true;
                        cells.push((nc, nr));
                    }
                }
            }
            let rule = rules.footprints.get(footprint)?;
            let c_min = cells.iter().map(|&(c, _)| c).min()?;
            let c_max = cells.iter().map(|&(c, _)| c).max()?;
            let r_min = cells.iter().map(|&(_, r)| r).min()?;
            let r_max = cells.iter().map(|&(_, r)| r).max()?;
            let (w, d) = (c_max - c_min + 1, r_max - r_min + 1);
            if !rule.sizes.contains(&(w as u32, d as u32)) || cells.len() != w * d {
                return None;
            }
            if !rule.yaws.iter().any(|&y| (y - yaw).abs() < 1.0) {
                return None;
            }
            let rect = Rect { footprint, c0: c_min, r0: r_min, w, d, yaw };
            for &(cc, cr) in &cells {
                let (_, part, cell_yaw) = part_of(cc, cr)?;
                let (u, v) = rect.local(cc, cr);
                if (cell_yaw - yaw).abs() > 1.0 || part != part_index(u, v, w, d) {
                    return None;
                }
                if v == 0
                    && let Some(front) = &rule.front
                {
                    let (fc, fr) = rect.in_front_of(cc, cr, rows)?;
                    let (descriptor, _) = alphabet.piece_at(*grid.get(fr * cols + fc)?)?;
                    if descriptor != front {
                        return None;
                    }
                }
            }
            rects.push(rect);
        }
    }

    // **A gap of ground between any two of a kind, corners included.** The 4-neighbour rules the
    // grammar learns cannot see a diagonal, and two houses meeting at a corner leave no room to walk
    // between them. The example never does it either.
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            if a.footprint != b.footprint {
                continue;
            }
            let gap = rules.footprints.get(a.footprint)?.gap as usize;
            let apart = |a0: usize, a1: usize, b0: usize, b1: usize| a1 + gap < b0 || b1 + gap < a0;
            let columns = apart(a.c0, a.c0 + a.w - 1, b.c0, b.c0 + b.w - 1);
            let rows = apart(a.r0, a.r0 + a.d - 1, b.r0, b.r0 + b.d - 1);
            if !(columns || rows) {
                return None;
            }
        }
    }
    for (f, rule) in rules.footprints.iter().enumerate() {
        if rects.iter().filter(|r| r.footprint == f).count() < rule.min_count as usize {
            return None;
        }
    }
    Some(rects)
}

/// One block's grid of prototypes, collapsed and validated, or the reason there is none.
/// `seed_base` plus each attempt number is the seed of that attempt, so a block is reproducible.
pub fn solve_block(
    alphabet: &Alphabet,
    rules: &BlockRules,
    pins: &[(usize, usize)],
    seed_base: u64,
) -> Result<Block, String> {
    let domains = initial(alphabet, rules, pins)?;
    for attempt in 0..u64::from(rules.attempts) {
        let Some(grid) = wfc::collapse_grid(
            rules.size.0 as usize,
            rules.size.1 as usize,
            &alphabet.weights,
            &alphabet.grammar.support,
            &domains,
            seed_base.wrapping_add(attempt),
        ) else {
            continue;
        };
        if let Some(rects) = validate(alphabet, rules, &grid) {
            return Ok(Block { grid, rects });
        }
    }
    Err(format!("no layout passed validation in {} attempts", rules.attempts))
}

/// The seed of the block a region placement asks for: `rules.seed` offset by a hash of the
/// region's id, so every region lays out differently and the same one the same way every time.
pub fn region_seed(rules: &BlockRules, region_id: &str) -> u64 {
    rules
        .seed
        .wrapping_add(fnv1a(region_id.as_bytes()).wrapping_mul(1000))
}

/// The pins a region's block takes from the map: every owned placement inside the square the block
/// covers around `region.at`, at the cell it falls in, as the prototype of its descriptor and yaw.
/// An owned placement the example has no prototype for is an error naming it.
pub fn region_pins(
    map: &Map,
    region: &Placed,
    alphabet: &Alphabet,
    rules: &BlockRules,
) -> Result<Vec<(usize, usize)>, String> {
    let cols = rules.size.0 as usize;
    let rows = rules.size.1 as usize;
    let min = (
        region.at.0 - cols as f32 * rules.cell * 0.5,
        region.at.1 - rows as f32 * rules.cell * 0.5,
    );
    let mut pins = Vec::new();
    for p in map.placements.iter().filter(|p| p.owned) {
        let (x, z) = ((p.at.0 - min.0) / rules.cell, (p.at.1 - min.1) / rules.cell);
        if x < 0.0 || z < 0.0 || x >= cols as f32 || z >= rows as f32 {
            continue;
        }
        let (c, r) = (x.floor() as usize, z.floor() as usize);
        // Quantised like the grammar's own prototypes, so a yaw a hair off is the same prototype.
        let yaw = (p.yaw / 15.0).round() * 15.0;
        let prototype = alphabet.index_of(&p.descriptor, yaw).ok_or_else(|| {
            format!(
                "rules: owned placement `{}` is a `{}` at {} degrees, which the example `{}` does not contain",
                p.id, p.descriptor, p.yaw, rules.example
            )
        })?;
        pins.push((r * cols + c, prototype));
    }
    Ok(pins)
}

/// The map-space position of cell (`c`, `r`) of a block centred on `centre`.
pub fn cell_at(rules: &BlockRules, centre: (f32, f32), c: usize, r: usize) -> (f32, f32) {
    let (w, h) = (rules.size.0 as f32, rules.size.1 as f32);
    (
        centre.0 + (c as f32 - (w - 1.0) * 0.5) * rules.cell,
        centre.1 + (r as f32 - (h - 1.0) * 0.5) * rules.cell,
    )
}

/// The placements a solved block stands for: one per cell that is neither empty nor pinned (a pin
/// is already a placement in the map). `next_id` mints each id.
pub fn placements(
    alphabet: &Alphabet,
    rules: &BlockRules,
    block: &Block,
    centre: (f32, f32),
    pins: &[(usize, usize)],
    mut next_id: impl FnMut() -> String,
) -> Vec<Placed> {
    let cols = rules.size.0 as usize;
    let mut out = Vec::new();
    for (i, &prototype) in block.grid.iter().enumerate() {
        if pins.iter().any(|&(cell, _)| cell == i) {
            continue;
        }
        let Some((descriptor, yaw)) = alphabet.piece_at(prototype) else {
            continue;
        };
        out.push(Placed {
            id: next_id(),
            descriptor: descriptor.to_owned(),
            at: cell_at(rules, centre, i % cols, i / cols),
            yaw,
            ..Placed::default()
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::Descriptor;

    const SIZE: usize = 15;

    fn rules() -> BlockRules {
        BlockRules {
            example: "block".to_owned(),
            regions: vec![],
            cell: 2.0,
            size: (SIZE as u32, SIZE as u32),
            border: Some("t/street".to_owned()),
            no_empty: true,
            weights: vec![Weight { prefix: "t/house_".to_owned(), times: 100.0 }],
            footprints: vec![Footprint {
                prefix: "t/house_".to_owned(),
                sizes: vec![(2, 2), (2, 3), (3, 3)],
                yaws: vec![0.0, 180.0],
                front: Some("t/frontage".to_owned()),
                gap: 1,
                min_count: 0,
            }],
            attempts: 64,
            seed: 7,
        }
    }

    fn library() -> Library {
        let mut ids = vec!["t/street", "t/frontage", "t/yard"];
        let parts: Vec<String> = PARTS.iter().map(|p| format!("t/house_{p}")).collect();
        let mut descriptors: Vec<Descriptor> = ids
            .drain(..)
            .map(|id| Descriptor { id: id.to_owned(), ..Descriptor::default() })
            .collect();
        descriptors.extend(parts.into_iter().map(|id| Descriptor { id, ..Descriptor::default() }));
        Library { descriptors, ..Library::default() }
    }

    fn put(map: &mut Map, id: &str, descriptor: &str, c: usize, r: usize, yaw: f32) {
        let half = SIZE as f32;
        map.placements.push(Placed {
            id: id.to_owned(),
            descriptor: descriptor.to_owned(),
            at: (-half + (c as f32 + 0.5) * 2.0, -half + (r as f32 + 0.5) * 2.0),
            yaw,
            ..Placed::default()
        });
    }

    /// **A small example block, drawn the way the author draws one** (and only whole houses, so the
    /// learned adjacencies are the ones a collapse can satisfy): a street ring, a frontage ring
    /// inside it, yard, and houses of every size facing the frontage on the north row (turned 180)
    /// and the south row (turned 0), which also teaches every house part at both yaws. Cells are 2 m;
    /// the block is 15 by 15.
    fn example() -> Map {
        let mut map = Map {
            name: "block".to_owned(),
            bounds: (SIZE as f32 * 2.0, 4.0, SIZE as f32 * 2.0),
            ..Map::default()
        };
        let mut cells: Vec<(String, f32)> = (0..SIZE * SIZE)
            .map(|i| {
                let (c, r) = (i % SIZE, i / SIZE);
                let ring = r == 0 || c == 0 || r == SIZE - 1 || c == SIZE - 1;
                let inner = r == 1 || c == 1 || r == SIZE - 2 || c == SIZE - 2;
                let d = if ring {
                    "t/street"
                } else if inner {
                    "t/frontage"
                } else {
                    "t/yard"
                };
                (d.to_owned(), 0.0)
            })
            .collect();
        // (c0, r0, w, d, yaw). North houses face the frontage on row 1, south houses the one on row 13.
        for (c0, r0, w, d, yaw) in [
            (3usize, 2usize, 2usize, 2usize, 180.0f32),
            (6, 2, 3, 3, 180.0),
            (10, 2, 2, 3, 180.0),
            (3, 11, 2, 2, 0.0),
            (7, 10, 3, 3, 0.0),
        ] {
            let rect = Rect { footprint: 0, c0, r0, w, d, yaw };
            for v in 0..d {
                for u in 0..w {
                    let (c, r) = rect.cell(u, v);
                    cells[r * SIZE + c] = (format!("t/house_{}", PARTS[part_index(u, v, w, d)]), yaw);
                }
            }
        }
        for (i, (descriptor, yaw)) in cells.iter().enumerate() {
            put(&mut map, &format!("p{i}"), descriptor, i % SIZE, i / SIZE, *yaw);
        }
        map
    }

    fn alphabet() -> Alphabet {
        learn(&rules(), &example()).unwrap_or_else(|e| panic!("{e}"))
    }

    fn index(a: &Alphabet, descriptor: &str, yaw: f32) -> usize {
        a.index_of(descriptor, yaw)
            .unwrap_or_else(|| panic!("the example has no {descriptor} at {yaw}"))
    }

    /// A grid of yard with a frontage row at row 1, the shape `validate` reads.
    fn yard_grid(a: &Alphabet) -> Vec<usize> {
        let yard = index(a, "t/yard", 0.0);
        let mut grid = vec![yard; SIZE * SIZE];
        let north = index(a, "t/frontage", 0.0);
        for c in 1..SIZE - 1 {
            grid[SIZE + c] = north;
        }
        grid
    }

    /// Stand a footprint of `rect` on `grid`, every cell the part the rules say.
    fn raise(a: &Alphabet, grid: &mut [usize], rect: &Rect) {
        for v in 0..rect.d {
            for u in 0..rect.w {
                let (c, r) = rect.cell(u, v);
                let name = format!("t/house_{}", PARTS[part_index(u, v, rect.w, rect.d)]);
                grid[r * SIZE + c] = index(a, &name, rect.yaw);
            }
        }
    }

    fn rect(c0: usize, r0: usize, w: usize, d: usize, yaw: f32) -> Rect {
        Rect { footprint: 0, c0, r0, w, d, yaw }
    }

    #[test]
    fn the_example_teaches_a_grammar_the_solver_can_hold() {
        let a = alphabet();
        assert!(a.grammar.len() <= grammar::MAX_PROTOTYPES, "{} prototypes", a.grammar.len());
        // Every house part is known as a part, and the empty prototype is not one.
        let parts = a.parts.iter().flatten().count();
        assert_eq!(parts, 18, "nine parts at two yaws");
        assert_eq!(a.parts[0], None);
        // The weight calibration reached the house parts and nothing else.
        let street = index(&a, "t/street", 0.0);
        let core = index(&a, "t/house_core", 0.0);
        assert_eq!(a.weights[street], a.grammar.weights[street]);
        assert_eq!(a.weights[core], a.grammar.weights[core] * 100.0);
    }

    #[test]
    fn an_example_of_the_wrong_size_is_refused() {
        let mut r = rules();
        r.size = (17, 17);
        let err = learn(&r, &example()).err().unwrap_or_default();
        assert!(err.contains("30x30") && err.contains("34x34"), "{err}");
    }

    #[test]
    fn validate_accepts_a_whole_footprint_in_front_of_frontage() {
        let a = alphabet();
        let r = rules();
        let mut grid = yard_grid(&a);
        let house = rect(3, 2, 2, 3, 180.0);
        raise(&a, &mut grid, &house);
        assert_eq!(validate(&a, &r, &grid), Some(vec![house]));
    }

    #[test]
    fn validate_refuses_what_the_local_rules_cannot_see() {
        let a = alphabet();
        let r = rules();
        let house = rect(3, 2, 2, 2, 180.0);

        // A door with no frontage in front of it.
        let mut grid = yard_grid(&a);
        raise(&a, &mut grid, &house);
        grid[SIZE + 3] = index(&a, "t/yard", 0.0);
        assert_eq!(validate(&a, &r, &grid), None, "a front cell must face frontage");

        // A house missing a cell: an L, not a rectangle.
        let mut grid = yard_grid(&a);
        raise(&a, &mut grid, &house);
        grid[3 * SIZE + 3] = index(&a, "t/yard", 0.0);
        assert_eq!(validate(&a, &r, &grid), None, "a house is a whole rectangle");

        // Two houses touching: one 4x2 component.
        let mut grid = yard_grid(&a);
        raise(&a, &mut grid, &house);
        raise(&a, &mut grid, &Rect { c0: 5, ..house });
        assert_eq!(validate(&a, &r, &grid), None, "touching houses are not two houses");

        // Two houses meeting at a corner: no 4-neighbour rule sees it, and nobody can walk between.
        // The same pair one column further apart is fine, so it is the corner that is refused.
        let frontage = index(&a, "t/frontage", 0.0);
        let pair = |south_c0: usize| {
            let south = rect(south_c0, 4, 2, 2, 0.0);
            let mut grid = yard_grid(&a);
            raise(&a, &mut grid, &house);
            raise(&a, &mut grid, &south);
            grid[6 * SIZE + south_c0] = frontage;
            if south_c0 + 1 < SIZE {
                grid[6 * SIZE + south_c0 + 1] = frontage;
            }
            validate(&a, &r, &grid)
        };
        assert_eq!(pair(6).map(|h| h.len()), Some(2), "a column of ground between them");
        assert_eq!(pair(5), None, "houses at a corner leave no room between them");

        // A 3 wide, 2 deep house: that size is not allowed.
        let mut grid = yard_grid(&a);
        raise(&a, &mut grid, &Rect { w: 3, d: 2, ..house });
        assert_eq!(validate(&a, &r, &grid), None, "3x2 is not a size");

        // A house turned the wrong way for its parts.
        let mut grid = yard_grid(&a);
        raise(&a, &mut grid, &house);
        let (c, row) = house.cell(0, 0);
        grid[row * SIZE + c] = index(&a, "t/house_front_right", 180.0);
        assert_eq!(validate(&a, &r, &grid), None, "a part in the wrong place");
    }

    #[test]
    fn min_count_refuses_too_few() {
        let a = alphabet();
        let mut r = rules();
        r.footprints[0].min_count = 2;
        let mut grid = yard_grid(&a);
        raise(&a, &mut grid, &rect(3, 2, 2, 2, 180.0));
        assert_eq!(validate(&a, &r, &grid), None, "one house is fewer than two");
        raise(&a, &mut grid, &rect(6, 4, 2, 2, 180.0));
        grid[3 * SIZE + 6] = index(&a, "t/frontage", 0.0);
        grid[3 * SIZE + 7] = index(&a, "t/frontage", 0.0);
        assert_eq!(validate(&a, &r, &grid).map(|h| h.len()), Some(2));
    }

    #[test]
    fn solve_block_honours_a_border_and_a_pin() {
        let a = alphabet();
        let r = rules();
        let pinned = rect(3, 3, 2, 2, 180.0);
        let pins = rect_pins(&a, &r, &pinned).unwrap_or_else(|e| panic!("{e}"));
        let block = solve_block(&a, &r, &pins, 100).unwrap_or_else(|e| panic!("{e}"));
        let street = index(&a, "t/street", 0.0);
        for k in 0..SIZE {
            for (c, row) in [(k, 0), (k, SIZE - 1), (0, k), (SIZE - 1, k)] {
                assert_eq!(block.grid[row * SIZE + c], street, "the ring is street at ({c}, {row})");
            }
        }
        for row in 1..SIZE - 1 {
            for c in 1..SIZE - 1 {
                assert_ne!(block.grid[row * SIZE + c], street, "street stays on the ring, found at ({c}, {row})");
            }
        }
        for &(cell, prototype) in &pins {
            assert_eq!(block.grid[cell], prototype, "the pin at cell {cell} held");
        }
        assert!(block.rects.contains(&pinned), "the pinned footprint is one of the rects");
        // The placements skip the pinned cells and the empty ones.
        let mut n = 0;
        let rows = placements(&a, &r, &block, (0.0, 0.0), &pins, || {
            n += 1;
            format!("g{n}")
        });
        assert_eq!(rows.len(), SIZE * SIZE - pins.len(), "no cell is empty here");
    }

    #[test]
    fn the_same_seed_gives_the_same_block() {
        let a = alphabet();
        let r = rules();
        let one = solve_block(&a, &r, &[], 5).unwrap_or_else(|e| panic!("{e}"));
        let two = solve_block(&a, &r, &[], 5).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(one, two);
    }

    #[test]
    fn two_pins_on_one_cell_are_refused() {
        let a = alphabet();
        let r = rules();
        let yard = index(&a, "t/yard", 0.0);
        let err = initial(&a, &r, &[(20, yard), (20, yard)]).err().unwrap_or_default();
        assert!(err.contains("two pins"), "{err}");
        assert!(initial(&a, &r, &[(SIZE * SIZE, yard)]).is_err(), "a pin off the grid");
    }

    #[test]
    fn a_region_takes_the_owned_placements_inside_it_as_pins() {
        let a = alphabet();
        let r = rules();
        let mut map = Map { name: "town".to_owned(), bounds: (200.0, 10.0, 200.0), ..Map::default() };
        // The region is centred at (50, 30): its 15 by 15 block of 2 m cells covers x 35..65, z 15..45.
        let region = Placed { id: "block_a".to_owned(), at: (50.0, 30.0), ..Placed::default() };
        let inside = Placed {
            id: "pin_in".to_owned(),
            descriptor: "t/house_core".to_owned(),
            at: (42.0, 22.0),
            yaw: 180.0,
            owned: true,
            ..Placed::default()
        };
        let outside = Placed { id: "pin_out".to_owned(), at: (70.0, 30.0), ..inside.clone() };
        let loose = Placed { id: "loose".to_owned(), owned: false, ..inside.clone() };
        map.placements = vec![inside, outside, loose];
        let pins = region_pins(&map, &region, &a, &r).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            pins,
            vec![(3 * SIZE + 3, index(&a, "t/house_core", 180.0))],
            "only the owned one inside, at cell (3, 3)"
        );

        map.placements[0].descriptor = "t/nonsense".to_owned();
        let err = region_pins(&map, &region, &a, &r).err().unwrap_or_default();
        assert!(err.contains("pin_in") && err.contains("t/nonsense"), "{err}");
    }

    #[test]
    fn a_region_seed_is_stable_and_differs_by_region() {
        let r = rules();
        assert_eq!(region_seed(&r, "block_a"), region_seed(&r, "block_a"));
        assert_ne!(region_seed(&r, "block_a"), region_seed(&r, "block_b"));
    }

    #[test]
    fn cells_are_laid_out_around_the_centre() {
        let r = rules();
        // A 15 by 15 block of 2 m cells: the middle cell is the centre, corners are 14 m out.
        assert_eq!(cell_at(&r, (10.0, 20.0), 7, 7), (10.0, 20.0));
        assert_eq!(cell_at(&r, (10.0, 20.0), 0, 14), (-4.0, 34.0));
    }

    #[test]
    fn validate_against_refuses_a_missing_part() {
        let mut lib = library();
        let all = Rules { version: RULES_VERSION, blocks: vec![rules()] };
        assert!(all.validate_against(&lib).is_ok());
        lib.descriptors.retain(|d| d.id != "t/house_core");
        let err = all.validate_against(&lib).err().unwrap_or_default();
        assert!(err.contains("t/house_core"), "{err}");
    }

    #[test]
    fn validate_against_refuses_what_is_not_implemented() {
        let lib = library();
        let mut r = Rules { version: RULES_VERSION, blocks: vec![rules()] };
        r.blocks[0].footprints[0].yaws = vec![0.0, 90.0];
        let err = r.validate_against(&lib).err().unwrap_or_default();
        assert!(err.contains("0 or 180") && err.contains("not implemented"), "{err}");

        let mut r = Rules { version: RULES_VERSION, blocks: vec![rules()] };
        r.blocks[0].footprints[0].sizes = vec![(1, 2)];
        assert!(r.validate_against(&lib).is_err(), "a 1 wide footprint has no middle");

        let mut r = Rules { version: RULES_VERSION, blocks: vec![rules()] };
        r.blocks[0].attempts = 0;
        assert!(r.validate_against(&lib).is_err());

        let mut r = Rules { version: RULES_VERSION + 1, blocks: vec![] };
        assert!(r.validate_against(&lib).is_err());
        r.version = RULES_VERSION;
        r.blocks = vec![BlockRules { border: Some("t/nowhere".to_owned()), ..rules() }];
        assert!(r.validate_against(&lib).err().unwrap_or_default().contains("t/nowhere"));
    }

    #[test]
    fn rules_round_trip_through_ron() {
        let text = "(version: 1, blocks: [(example: \"b\", cell: 2.0, size: (9, 9), attempts: 3, seed: 1)])";
        let parsed = Rules::parse(text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(parsed.blocks[0].border, None);
        assert!(parsed.blocks[0].footprints.is_empty());
        assert!(Rules::parse("(version: 1, blocks: [], surprise: 1)").is_err(), "unknown fields are refused");
    }
}
