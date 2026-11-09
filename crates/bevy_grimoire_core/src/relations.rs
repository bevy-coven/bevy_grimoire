//! **Relations, not coordinates** (#137, research lens #2).
//!
//! An agent placing pieces by number writes coordinates, and what it *meant* is lost: "the chair
//! faces the table", "nothing stands in front of the door". This module keeps that intent as data
//! beside the map (`<map>.relations.ron`, [`FILE_SUFFIX`]) and checks a map's placements against it,
//! so a broken relation is named with what was measured rather than discovered by looking.
//!
//! Each [`Relation`] is about one placement, [`Relation::of`], by its stable id
//! ([`crate::map::Placed::id`]); a kind that needs a second piece names it too. [`check`] returns one
//! [`Outcome`] per relation, in order. A relation naming an id the map does not hold is
//! [`Outcome::Missing`], never silently met.
//!
//! Forward is `(sin yaw, cos yaw)`, the convention [`crate::placement::ir::facing_cosine`] scores
//! with, and [`check`] and [`solve`] read that yaw in radians. A map's own rows store
//! [`crate::map::Placed::yaw`] in **degrees** (the `place` schema, the spawner), so a caller holding
//! map rows uses [`check_map`] and [`solve_map`] (#165). Distances are between placement centres on
//! the floor plan ([`crate::map::Placed::at`]).
//!
//! Engine-free: reads placements, nothing else. Runs and stamps are not expanded here; a relation is
//! about a placement row.

use serde::{Deserialize, Serialize};

use crate::map::Placed;

/// The format version of a relations file.
pub const RELATIONS_VERSION: u32 = 1;

/// A map's relations live in `<map name>` + this, beside the map file.
pub const FILE_SUFFIX: &str = ".relations.ron";

/// How far off dead-on, in degrees, a piece may point and still face its target.
pub const FACING_TOLERANCE_DEG: f32 = 30.0;

/// The relations file that belongs to a map file: `town.map.ron` → `town.relations.ron`, in the
/// same directory. A map path that does not end `.map.ron` gets [`FILE_SUFFIX`] appended to its stem.
pub fn path_for_map(map_path: &std::path::Path) -> std::path::PathBuf {
    let name = map_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name.strip_suffix(".map.ron").or_else(|| name.strip_suffix(".ron")).unwrap_or(&name);
    map_path.with_file_name(format!("{stem}{FILE_SUFFIX}"))
}

/// Every relation one map is meant to keep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relations {
    pub version: u32,
    #[serde(default)]
    pub relations: Vec<Relation>,
}

impl Default for Relations {
    fn default() -> Self {
        Relations {
            version: RELATIONS_VERSION,
            relations: Vec::new(),
        }
    }
}

impl Relations {
    /// Read a relations file, refusing a version this build does not read and a distance that is
    /// negative or not finite (a negative corridor would contain nothing, so a typo would read as a
    /// clear doorway).
    pub fn parse(text: &str) -> Result<Relations, String> {
        let r: Relations = ron::from_str(text).map_err(|e| format!("relations: parse: {e}"))?;
        if r.version != RELATIONS_VERSION {
            return Err(format!(
                "relations: version {} is not the {RELATIONS_VERSION} this build reads",
                r.version
            ));
        }
        for (i, rel) in r.relations.iter().enumerate() {
            // Faces and AlignedWith have no distance; 0.0 stands in for "nothing to check".
            let metres: [f32; 2] = match &rel.kind {
                Kind::Faces(_) | Kind::AlignedWith(_) => [0.0, 0.0],
                Kind::Near { within, .. }
                | Kind::InFrontOf { within, .. }
                | Kind::Behind { within, .. }
                | Kind::LeftOf { within, .. }
                | Kind::RightOf { within, .. }
                | Kind::MirrorOf { within, .. }
                | Kind::BacksOnto { within, .. }
                | Kind::Between { within, .. } => [*within, 0.0],
                Kind::Apart { at_least, .. } => [*at_least, 0.0],
                Kind::ClearInFront { depth, width } | Kind::Inside { width, depth, .. } => [*depth, *width],
                Kind::VisibleFrom { clearance, .. } | Kind::ReachableFrom { clearance, .. } => [*clearance, 0.0],
                Kind::Balances { with, within, .. } => {
                    if with.is_empty() {
                        return Err(format!(
                            "relations: relation {i} (of {:?}) balances no one; an empty group is a Near",
                            rel.of
                        ));
                    }
                    if let Some(k) = with.iter().enumerate().find_map(|(k, id)| {
                        (*id == rel.of || with[..k].contains(id)).then_some(k)
                    }) {
                        return Err(format!(
                            "relations: relation {i} (of {:?}) names {:?} twice in its group; each piece counts once",
                            rel.of, with[k]
                        ));
                    }
                    [*within, 0.0]
                }
                Kind::Along { path, within } => {
                    if path.len() < 2 {
                        return Err(format!(
                            "relations: relation {i} (of {:?}) has a path of {} waypoint(s); a path needs two or more",
                            rel.of,
                            path.len()
                        ));
                    }
                    [*within, 0.0]
                }
            };
            if let Some(bad) = metres.iter().find(|m| !(m.is_finite() && **m >= 0.0)) {
                return Err(format!(
                    "relations: relation {i} (of {:?}) has distance {bad}; distances are finite metres, 0 or more",
                    rel.of
                ));
            }
        }
        Ok(r)
    }

    /// The file's text.
    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("relations: write: {e}"))
    }
}

/// One thing a placement is meant to keep true.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    /// The placement id this relation is about.
    pub of: String,
    pub kind: Kind,
    /// **Must hold** (#151): [`solve`] never breaks a hard relation to mend soft ones, and mends a
    /// hard one even when that breaks soft ones. Soft by default, and a soft relation is written
    /// without the field, so a file from before it reads and writes unchanged.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hard: bool,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// What a relation asks of [`Relation::of`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Kind {
    /// Points at this placement, within [`FACING_TOLERANCE_DEG`]. Measures the angle off, degrees.
    Faces(String),
    /// Stands within this many metres of a placement. Measures the distance.
    Near { to: String, within: f32 },
    /// Stands at least this many metres from a placement. Measures the distance.
    Apart { from: String, at_least: f32 },
    /// Stands in front of a placement: ahead of its forward, inside a 45° half-cone, and no more than
    /// `within` metres along it. Measures the distance along the other's forward.
    InFrontOf { of: String, within: f32 },
    /// Stands behind a placement: against its forward, inside a 45° half-cone, and no more than
    /// `within` metres back along it (a chest behind the counter, a guard behind the throne).
    /// Measures the distance along the other's backward; a piece in front measures negative.
    Behind { of: String, within: f32 },
    /// Stands to a placement's left, in that placement's own frame as it faces its forward: out
    /// along its left, inside a 45° half-cone, and no more than `within` metres out (a lamp left of
    /// the door). Measures the distance along the other's left; a piece on its right measures
    /// negative.
    LeftOf { of: String, within: f32 },
    /// [`Kind::LeftOf`] on the other side: out along the placement's own right (a chair right of
    /// the desk, a guard at the throne's right hand). A piece on its left measures negative.
    RightOf { of: String, within: f32 },
    /// No other placement's centre stands in the `width` × `depth` corridor in front of this one (a
    /// door's swing, a counter's working side). This one's whole stack (what it rests on, and what
    /// rests on that, such as a jug on the counter) is part of it and never intrudes. Measures how
    /// far in the nearest intruder stands, or `depth` when clear.
    ClearInFront { depth: f32, width: f32 },
    /// Seen from a placement: no third placement's centre stands within `clearance` metres of the
    /// line between the two centres (a sign seen from the gate, a door a guard watches). Each end's
    /// whole stack (what it rests on, such as the stall a sign hangs on, and what rests on that) is
    /// part of that end and never blocks. Measures how far off the line the nearest blocker stands,
    /// or `clearance` when clear.
    VisibleFrom { from: String, clearance: f32 },
    /// Stands beside a path: within `within` metres of the polyline through these placements'
    /// centres, in order (a lamp along the road from the gate past the well to the hall). Two or more
    /// waypoints. Measures the distance to the nearest point of the path.
    Along { path: Vec<String>, within: f32 },
    /// Reachable from a placement: a walker who keeps `clearance` metres from every other
    /// placement's centre can walk from that one's centre to this one's, around whatever stands
    /// between (a chest behind a ring of crates, a door a row of barrels walls off). Each end's
    /// whole stack (what it rests on, and what rests on that) is part of that end. Measures the
    /// walk's length on a grid of `clearance / 2` cells, 8-connected (coarser on a map too large
    /// for that, where a near miss counts as in the way, so coarse never reads as open); when
    /// unmet, `by` names the piece whose removal opens a walk and `measured` is that walk's length
    /// (0 when none of the [`OPENER_TRIES`] pieces nearest the straight line does).
    ReachableFrom { from: String, clearance: f32 },
    /// On this placement's heading, within [`FACING_TOLERANCE_DEG`] (a row of pews facing the
    /// altar, stalls along a street). Measures the angle between the two headings, degrees, the
    /// short way round; facing the opposite way is 180° off.
    AlignedWith(String),
    /// Between two placements: this one's centre projects strictly inside the segment joining
    /// `a`'s and `b`'s centres and stands no more than `within` metres off it (a table between two
    /// chairs, a guard between the gate and the hall). Two ends on one spot have nothing between
    /// them. Measures the distance to the segment, so past either end it is the distance to that
    /// end.
    Between { a: String, b: String, within: f32 },
    /// Opposite another across a third's axis: this placement stands within `within` metres of
    /// `of`'s mirror image across `about`'s forward axis, the line through `about`'s centre along
    /// its forward (two lamps flanking a door, a pair of thrones either side of the altar's axis;
    /// Merrell et al.'s symmetry about a focal point). Turning `about` turns the axis. Measures the
    /// distance from the mirror point. Positions only: [`Kind::AlignedWith`] and [`Kind::Faces`]
    /// keep headings.
    MirrorOf { of: String, about: String, within: f32 },
    /// With its back to a placement (a bookshelf against a wall, a throne backed onto the apse, a
    /// sofa with its back to the window; Bukowski and Séquin's object association, cited by
    /// Merrell et al.): this placement's centre stands off the other's face, the plane through the
    /// other's centre square to its forward, by more than nothing and no more than `within` metres,
    /// on either side, and it faces straight out from that plane, away from it, within
    /// [`FACING_TOLERANCE_DEG`]. Sideways along the other is free, so a long wall backs a piece
    /// anywhere along it; [`Kind::Near`] bounds that. Measures the distance off the plane.
    BacksOnto { of: String, within: f32 },
    /// Inside an area of a placement (a table on the rug, a chest inside the room's floor, a stall
    /// in the market square; Merrell et al. keep every piece inside the room polygon, and ProcTHOR
    /// places each floor object inside a rectangle of the room): this placement's centre stands in
    /// the `width` × `depth` rectangle centred on the other, in the other's own frame (`width`
    /// across its left and right, `depth` along its forward), so turning the other turns the area.
    /// Headings are free. Measures the distance from the centre to the rectangle, 0 inside.
    Inside { of: String, width: f32, depth: f32 },
    /// Balanced about a point (a market square's stalls about its well, a table's chairs about the
    /// table; Merrell et al.'s visual balance: "place the mean of the distribution of visual weight
    /// at the center of the composition"): the mean of the centres of this placement and every one
    /// in `with` stands within `within` metres of `about`'s centre. Every piece weighs the same,
    /// since a relation reads placements and never the library. `with` names one or more others
    /// (an empty group is [`Kind::Near`]), none twice and none this placement; `about` may be a
    /// member, as a well at the centre of its stalls. Measures the distance from the mean to the
    /// point.
    Balances { with: Vec<String>, about: String, within: f32 },
}

/// What [`check`] found for one relation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Outcome {
    Met {
        measured: f32,
    },
    /// Broken. `by` names the placement that breaks it, where one does (the intruder in a clear
    /// corridor).
    Unmet {
        measured: f32,
        by: Option<String>,
    },
    /// A placement the relation names is not on the map.
    Missing {
        id: String,
    },
}

/// Check every relation against a map's placements: one outcome per relation, in order.
pub fn check(placements: &[Placed], relations: &Relations) -> Vec<Outcome> {
    relations
        .relations
        .iter()
        .map(|r| check_one(placements, r))
        .collect()
}

fn find<'a>(placements: &'a [Placed], id: &str) -> Result<&'a Placed, Outcome> {
    placements
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| Outcome::Missing { id: id.to_owned() })
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// `p` in `from`'s frame: (along its forward, sideways). Sideways is positive on `from`'s own
/// left (forward is local +z, turned about +y; its left is local +x).
fn local(from: &Placed, p: (f32, f32)) -> (f32, f32) {
    let (fx, fz) = (from.yaw.sin(), from.yaw.cos());
    let (dx, dz) = (p.0 - from.at.0, p.1 - from.at.1);
    (fx * dx + fz * dz, fz * dx - fx * dz)
}

/// `p` mirrored across `about`'s forward axis, the line through its centre along its forward:
/// the same distance ahead, the same distance out on the other side.
fn mirror(about: &Placed, p: (f32, f32)) -> (f32, f32) {
    let (ahead, left) = local(about, p);
    // Back from `about`'s frame: forward (fx, fz), left (fz, -fx), as `local` measures.
    let (fx, fz) = (about.yaw.sin(), about.yaw.cos());
    (
        about.at.0 + fx * ahead - fz * left,
        about.at.1 + fz * ahead + fx * left,
    )
}

fn verdict(ok: bool, measured: f32) -> Outcome {
    if ok {
        Outcome::Met { measured }
    } else {
        Outcome::Unmet { measured, by: None }
    }
}

fn check_one(placements: &[Placed], r: &Relation) -> Outcome {
    let a = match find(placements, &r.of) {
        Ok(a) => a,
        Err(missing) => return missing,
    };
    let other = |id: &str| find(placements, id);
    match &r.kind {
        Kind::Faces(to) => match other(to) {
            Ok(b) => {
                let cos = crate::placement::ir::facing_cosine(a.at, a.yaw, b.at).clamp(-1.0, 1.0);
                let off = cos.acos().to_degrees();
                verdict(off <= FACING_TOLERANCE_DEG, off)
            }
            Err(m) => m,
        },
        Kind::Near { to, within } => match other(to) {
            Ok(b) => {
                let d = distance(a.at, b.at);
                verdict(d <= *within, d)
            }
            Err(m) => m,
        },
        Kind::Apart { from, at_least } => match other(from) {
            Ok(b) => {
                let d = distance(a.at, b.at);
                verdict(d >= *at_least, d)
            }
            Err(m) => m,
        },
        Kind::InFrontOf { of, within } => match other(of) {
            Ok(b) => {
                let (ahead, side) = local(b, a.at);
                verdict(
                    ahead > 0.0 && ahead <= *within && side.abs() <= ahead,
                    ahead,
                )
            }
            Err(m) => m,
        },
        Kind::Behind { of, within } => match other(of) {
            Ok(b) => {
                let (ahead, side) = local(b, a.at);
                let back = -ahead;
                verdict(back > 0.0 && back <= *within && side.abs() <= back, back)
            }
            Err(m) => m,
        },
        Kind::LeftOf { of, within } => match other(of) {
            Ok(b) => {
                let (ahead, left) = local(b, a.at);
                verdict(left > 0.0 && left <= *within && ahead.abs() <= left, left)
            }
            Err(m) => m,
        },
        Kind::RightOf { of, within } => match other(of) {
            Ok(b) => {
                let (ahead, left) = local(b, a.at);
                let right = -left;
                verdict(right > 0.0 && right <= *within && ahead.abs() <= right, right)
            }
            Err(m) => m,
        },
        Kind::ClearInFront { depth, width } => {
            let own = &bottom(placements, a).id;
            let intruder = placements
                .iter()
                .filter(|p| &bottom(placements, p).id != own)
                .map(|p| (p, local(a, p.at)))
                .filter(|(_, (ahead, side))| {
                    *ahead > 0.0 && ahead <= depth && side.abs() <= width / 2.0
                })
                .min_by(|x, y| x.1 .0.total_cmp(&y.1 .0));
            match intruder {
                Some((p, (ahead, _))) => Outcome::Unmet {
                    measured: ahead,
                    by: Some(p.id.clone()),
                },
                None => Outcome::Met { measured: *depth },
            }
        }
        Kind::VisibleFrom { from, clearance } => match other(from) {
            Ok(b) => {
                let nearest = blockers(placements, b, a, *clearance)
                    .into_iter()
                    .min_by(|x, y| x.1.abs().total_cmp(&y.1.abs()));
                match nearest {
                    Some((p, side)) => Outcome::Unmet {
                        measured: side.abs(),
                        by: Some(p.id.clone()),
                    },
                    None => Outcome::Met { measured: *clearance },
                }
            }
            Err(m) => m,
        },
        Kind::ReachableFrom { from, clearance } => match other(from) {
            Ok(b) => match walk(placements, b, a, *clearance, None) {
                Some(length) => Outcome::Met { measured: length },
                None => match opener(placements, b, a, *clearance) {
                    Some((p, length)) => Outcome::Unmet {
                        measured: length,
                        by: Some(p.id.clone()),
                    },
                    None => Outcome::Unmet { measured: 0.0, by: None },
                },
            },
            Err(m) => m,
        },
        Kind::Along { path, within } => match nearest_on_path(placements, path, a.at) {
            Ok(q) => {
                let d = distance(a.at, q);
                verdict(d <= *within, d)
            }
            Err(m) => m,
        },
        Kind::AlignedWith(to) => match other(to) {
            Ok(b) => {
                let off = (a.yaw - b.yaw).rem_euclid(std::f32::consts::TAU);
                let off = off.min(std::f32::consts::TAU - off).to_degrees();
                verdict(off <= FACING_TOLERANCE_DEG, off)
            }
            Err(m) => m,
        },
        Kind::Between { a: from, b: to, within } => match (other(from), other(to)) {
            (Ok(p), Ok(q)) => {
                let (t, d) = onto_segment(p.at, q.at, a.at);
                verdict(t > 0.0 && t < 1.0 && d <= *within, d)
            }
            (Err(m), _) | (_, Err(m)) => m,
        },
        Kind::BacksOnto { of, within } => match other(of) {
            Ok(b) => {
                let (ahead, _) = local(b, a.at);
                let off = ahead.abs();
                // Away from the plane is the other's forward on its front side, backward behind it.
                let (fx, fz) = (b.yaw.sin(), b.yaw.cos());
                let away = ahead.signum() * (a.yaw.sin() * fx + a.yaw.cos() * fz);
                let turned = away.clamp(-1.0, 1.0).acos().to_degrees();
                verdict(off > 0.0 && off <= *within && turned <= FACING_TOLERANCE_DEG, off)
            }
            Err(m) => m,
        },
        Kind::Inside { of, width, depth } => match other(of) {
            Ok(b) => {
                let (ahead, left) = local(b, a.at);
                let out_side = (left.abs() - width / 2.0).max(0.0);
                let out_ahead = (ahead.abs() - depth / 2.0).max(0.0);
                verdict(
                    left.abs() <= width / 2.0 && ahead.abs() <= depth / 2.0,
                    out_side.hypot(out_ahead),
                )
            }
            Err(m) => m,
        },
        Kind::MirrorOf { of, about, within } => match (other(of), other(about)) {
            (Ok(p), Ok(axis)) => {
                let d = distance(a.at, mirror(axis, p.at));
                verdict(d <= *within, d)
            }
            (Err(m), _) | (_, Err(m)) => m,
        },
        Kind::Balances { with, about, within } => match other(about) {
            Ok(c) => {
                let mut sum = a.at;
                for id in with {
                    match other(id) {
                        Ok(p) => sum = (sum.0 + p.at.0, sum.1 + p.at.1),
                        Err(m) => return m,
                    }
                }
                let n = with.len() as f32 + 1.0;
                let d = distance((sum.0 / n, sum.1 / n), c.at);
                verdict(d <= *within, d)
            }
            Err(m) => m,
        },
    }
}

/// `p` against the segment `from`–`to`: the projection's fraction along it (unclamped; 0 when the
/// two ends are one spot), and the distance from `p` to the segment's nearest point.
fn onto_segment(from: (f32, f32), to: (f32, f32), p: (f32, f32)) -> (f32, f32) {
    let (dx, dz) = (to.0 - from.0, to.1 - from.1);
    let len2 = dx * dx + dz * dz;
    let t = if len2 > 1e-8 {
        ((p.0 - from.0) * dx + (p.1 - from.1) * dz) / len2
    } else {
        0.0
    };
    let c = t.clamp(0.0, 1.0);
    (t, distance(p, (from.0 + dx * c, from.1 + dz * c)))
}

/// The point of the polyline through `path`'s placements nearest `p`, or the first waypoint the map
/// lacks.
fn nearest_on_path(placements: &[Placed], path: &[String], p: (f32, f32)) -> Result<(f32, f32), Outcome> {
    let points = path
        .iter()
        .map(|id| find(placements, id).map(|w| w.at))
        .collect::<Result<Vec<_>, _>>()?;
    let closest = |(s, e): ((f32, f32), (f32, f32))| {
        let (dx, dz) = (e.0 - s.0, e.1 - s.1);
        let len2 = dx * dx + dz * dz;
        let t = if len2 > 1e-8 {
            (((p.0 - s.0) * dx + (p.1 - s.1) * dz) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (s.0 + dx * t, s.1 + dz * t)
    };
    Ok(points
        .windows(2)
        .map(|w| closest((w[0], w[1])))
        .min_by(|x, y| distance(p, *x).total_cmp(&distance(p, *y)))
        .unwrap_or(points[0]))
}

/// Whether `p` rests on `end`, directly or through a chain of hosts. A chain is never longer than
/// the map, so an `on` cycle in a malformed file ends.
fn rests_on(placements: &[Placed], p: &Placed, end: &str) -> bool {
    let mut host = p.on.as_deref();
    for _ in 0..placements.len() {
        match host {
            Some(h) if h == end => return true,
            Some(h) => host = find(placements, h).ok().and_then(|q| q.on.as_deref()),
            None => return false,
        }
    }
    false
}

/// The piece `p` rests on at the bottom of its `on` chain, or `p` itself. A chain is never longer
/// than the map, so an `on` cycle in a malformed file ends.
fn bottom<'a>(placements: &'a [Placed], mut p: &'a Placed) -> &'a Placed {
    for _ in 0..placements.len() {
        let Some(host) = p.on.as_deref().and_then(|h| find(placements, h).ok()) else {
            break;
        };
        p = host;
    }
    p
}

/// A sight line: its start, unit direction and length.
type Line = ((f32, f32), (f32, f32), f32);

/// The line from `from` to `to`; `None` when the two stand on one spot.
fn line(from: &Placed, to: &Placed) -> Option<Line> {
    let length = distance(from.at, to.at);
    (length > 1e-4).then(|| {
        let u = ((to.at.0 - from.at.0) / length, (to.at.1 - from.at.1) / length);
        (from.at, u, length)
    })
}

/// `p` in a line's frame: (along it, sideways toward `(uz, -ux)`).
fn on_line(start: (f32, f32), (ux, uz): (f32, f32), p: (f32, f32)) -> (f32, f32) {
    let (dx, dz) = (p.0 - start.0, p.1 - start.1);
    (ux * dx + uz * dz, uz * dx - ux * dz)
}

/// Every placement within `clearance` of the line from `from` to `to` and strictly between its
/// ends, other than each end's whole stack, with its signed distance off the line.
fn blockers<'a>(placements: &'a [Placed], from: &Placed, to: &Placed, clearance: f32) -> Vec<(&'a Placed, f32)> {
    let Some((start, u, length)) = line(from, to) else {
        return Vec::new();
    };
    let ends = [&bottom(placements, from).id, &bottom(placements, to).id];
    placements
        .iter()
        .filter(|p| !ends.contains(&&bottom(placements, p).id))
        .filter_map(|p| {
            let (along, side) = on_line(start, u, p.at);
            (along > 0.0 && along < length && side.abs() <= clearance).then_some((p, side))
        })
        .collect()
}

/// The moves that clear the sight line to `a` from `b`: each blocker's bottom host, once, slid
/// sideways just far enough that it and everything resting on it stand past `clearance`, to
/// whichever side needs the shorter slide. Riders keep their place on the host.
fn clear_sight(placements: &[Placed], a: &Placed, b: &Placed, clearance: f32) -> Vec<Move> {
    let Some((start, u, _)) = line(b, a) else {
        return Vec::new();
    };
    let past = clearance * 1.1 + 0.05;
    let mut hosts: Vec<&Placed> = Vec::new();
    for (p, _) in blockers(placements, b, a, clearance) {
        let host = bottom(placements, p);
        if !hosts.iter().any(|h| h.id == host.id) {
            hosts.push(host);
        }
    }
    hosts
        .into_iter()
        .map(|host| {
            let sides = placements
                .iter()
                .filter(|q| q.id == host.id || rests_on(placements, q, &host.id))
                .map(|q| on_line(start, u, q.at).1);
            let (low, high) = sides.fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), s| (l.min(s), h.max(s)));
            let (up, down) = (past - low, -past - high);
            let shift = if up.abs() <= down.abs() { up } else { down };
            // Sideways is (uz, -ux), as `on_line` measures.
            let to = (host.at.0 + u.1 * shift, host.at.1 - u.0 * shift);
            (host.id.clone(), to, host.yaw)
        })
        .collect()
}

/// The most cells [`walk`] lays over a map; a larger map walks on coarser cells.
const WALK_CELLS: f32 = 160_000.0;

/// The length of the shortest walk from `from`'s centre to `to`'s keeping `clearance` from every
/// other placement's centre, or `None` when there is none. Each end's whole stack is that end;
/// `skip` leaves that piece, and what rests on it, out of the way. The walk is on a grid over
/// every placement, padded by twice the clearance so the way round the outside counts, and cut to
/// a window round the ends; 8-connected, never cutting a blocked corner.
fn walk(placements: &[Placed], from: &Placed, to: &Placed, clearance: f32, skip: Option<&str>) -> Option<f32> {
    if clearance <= 0.0 {
        return Some(distance(from.at, to.at));
    }
    let ends = [&bottom(placements, from).id, &bottom(placements, to).id];
    let obstacles: Vec<(f32, f32)> = placements
        .iter()
        .filter(|p| !ends.contains(&&bottom(placements, p).id))
        .filter(|p| skip.is_none_or(|s| p.id != s && !rests_on(placements, p, s)))
        .map(|p| p.at)
        .collect();
    let pad = 2.0 * clearance;
    let (mut lo, mut hi) = ((f32::INFINITY, f32::INFINITY), (f32::NEG_INFINITY, f32::NEG_INFINITY));
    for p in placements {
        lo = (lo.0.min(p.at.0), lo.1.min(p.at.1));
        hi = (hi.0.max(p.at.0), hi.1.max(p.at.1));
    }
    // The map padded, cut to a window round the two ends as wide again as the walk is long: a
    // detour past the window reads as none, never the other way, and a far piece does not
    // coarsen a near walk.
    let reach_out = distance(from.at, to.at).max(2.0 * pad);
    let lo = (
        (lo.0 - pad).max(from.at.0.min(to.at.0) - reach_out),
        (lo.1 - pad).max(from.at.1.min(to.at.1) - reach_out),
    );
    let hi = (
        (hi.0 + pad).min(from.at.0.max(to.at.0) + reach_out),
        (hi.1 + pad).min(from.at.1.max(to.at.1) + reach_out),
    );
    let area = (hi.0 - lo.0) * (hi.1 - lo.1);
    let cell = (clearance / 2.0).max((area / WALK_CELLS).sqrt());
    // On a coarse grid an obstacle can stand between cell centres and block none; widen it by
    // half a cell's diagonal there, so a coarse walk errs toward blocked.
    let coarse = cell > clearance / 2.0;
    let radius = if coarse {
        clearance + cell * std::f32::consts::FRAC_1_SQRT_2
    } else {
        clearance
    };
    let (nx, nz) = (((hi.0 - lo.0) / cell).ceil() as usize + 1, ((hi.1 - lo.1) / cell).ceil() as usize + 1);
    let index = |p: (f32, f32)| {
        let i = (((p.0 - lo.0) / cell).round() as usize).min(nx - 1);
        let k = (((p.1 - lo.1) / cell).round() as usize).min(nz - 1);
        k * nx + i
    };
    let mut blocked = vec![false; nx * nz];
    let reach = (radius / cell).ceil() as isize;
    for o in &obstacles {
        let centre = index(*o);
        let (ci, ck) = ((centre % nx) as isize, (centre / nx) as isize);
        for k in (ck - reach).max(0)..=(ck + reach).min(nz as isize - 1) {
            for i in (ci - reach).max(0)..=(ci + reach).min(nx as isize - 1) {
                let c = (lo.0 + i as f32 * cell, lo.1 + k as f32 * cell);
                if distance(c, *o) < radius {
                    blocked[k as usize * nx + i as usize] = true;
                }
            }
        }
    }
    let (start, goal) = (index(from.at), index(to.at));
    // A fine cell holding an end's centre is the end's; a coarse one may hold a blocker too.
    if !coarse {
        blocked[start] = false;
        blocked[goal] = false;
    }
    // Dijkstra on whole steps: 1000 straight, 1414 diagonal.
    let mut best = vec![u32::MAX; nx * nz];
    let mut queue = std::collections::BinaryHeap::new();
    best[start] = 0;
    queue.push(std::cmp::Reverse((0u32, start)));
    while let Some(std::cmp::Reverse((cost, c))) = queue.pop() {
        if c == goal {
            return Some(cost as f32 / 1000.0 * cell);
        }
        if cost > best[c] {
            continue;
        }
        let (i, k) = ((c % nx) as isize, (c / nx) as isize);
        let free = |i: isize, k: isize| {
            i >= 0 && k >= 0 && i < nx as isize && k < nz as isize && !blocked[k as usize * nx + i as usize]
        };
        for (di, dk) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (ni, nk) = (i + di, k + dk);
            if !free(ni, nk) || (di != 0 && dk != 0 && !(free(i + di, k) && free(i, k + dk))) {
                continue;
            }
            let step = if di != 0 && dk != 0 { 1414 } else { 1000 };
            let n = nk as usize * nx + ni as usize;
            if cost + step < best[n] {
                best[n] = cost + step;
                queue.push(std::cmp::Reverse((cost + step, n)));
            }
        }
    }
    None
}

/// How many of the pieces nearest the straight line [`opener`] tries taking away.
pub const OPENER_TRIES: usize = 16;

/// The bottom host whose removal (with its riders) opens a walk from `from` to `to`, nearest the
/// straight line between them first, and that walk's length.
fn opener<'a>(placements: &'a [Placed], from: &Placed, to: &Placed, clearance: f32) -> Option<(&'a Placed, f32)> {
    let gap = |p: (f32, f32)| match line(from, to) {
        Some((start, u, length)) => {
            let (along, side) = on_line(start, u, p);
            if along < 0.0 {
                distance(p, from.at)
            } else if along > length {
                distance(p, to.at)
            } else {
                side.abs()
            }
        }
        None => distance(p, from.at),
    };
    let ends = [&bottom(placements, from).id, &bottom(placements, to).id];
    let mut hosts: Vec<&Placed> = Vec::new();
    for p in placements {
        let host = bottom(placements, p);
        if !ends.contains(&&host.id) && !hosts.iter().any(|h| h.id == host.id) {
            hosts.push(host);
        }
    }
    hosts.sort_by(|x, y| gap(x.at).total_cmp(&gap(y.at)));
    hosts
        .into_iter()
        .take(OPENER_TRIES)
        .find_map(|h| walk(placements, from, to, clearance, Some(&h.id)).map(|length| (h, length)))
}

/// The move that opens a walk to `a` from `b`: the piece `by` names, at its bottom host, slid in
/// one of eight directions by one, two or three times the walker's width, the shortest slide
/// that lets the walk through. Riders keep their place on the host.
fn open_walk(placements: &[Placed], a: &Placed, b: &Placed, clearance: f32, by: &str) -> Option<Move> {
    let host = bottom(placements, find(placements, by).ok()?);
    let step = 2.0 * clearance * 1.1 + 0.05;
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    let directions = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (diagonal, diagonal),
        (diagonal, -diagonal),
        (-diagonal, diagonal),
        (-diagonal, -diagonal),
    ];
    (1..=3).find_map(|k| {
        directions.iter().find_map(|(dx, dz)| {
            let to = (host.at.0 + dx * step * k as f32, host.at.1 + dz * step * k as f32);
            let mut trial = placements.to_vec();
            let depth = trial.len();
            move_piece(&mut trial, &host.id, to, host.yaw, depth);
            let (ta, tb) = (find(&trial, &a.id).ok()?, find(&trial, &b.id).ok()?);
            walk(&trial, tb, ta, clearance, None).map(|_| (host.id.clone(), to, host.yaw))
        })
    })
}

/// One proposed move: the placement, where it goes, and its yaw there.
type Move = (String, (f32, f32), f32);

/// What [`solve`] did to a map's placements.
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    /// Every placement, moved where the solver moved it; the rest unchanged and in input order.
    pub placements: Vec<Placed>,
    /// The ids the solver placed, first move first. A piece carried on a moved host is not listed.
    pub moved: Vec<String>,
    /// [`Outcome::Unmet`] relations before and after. Missing ones are neither and never moved for.
    pub unmet_before: usize,
    pub unmet_after: usize,
    /// The relations still [`Outcome::Unmet`] after the solve, by index into
    /// [`Relations::relations`], in order: what the solver could not satisfy.
    pub unsatisfied: Vec<usize>,
}

/// How many passes over the relations [`solve`] makes before it gives up on the rest.
const SOLVE_ROUNDS: usize = 8;

/// **Nudge the pieces of every broken relation until it holds**, greedily and never for the worse.
///
/// For each [`Outcome::Unmet`] relation the solver proposes one move: [`Kind::Faces`] turns the
/// piece to its target, [`Kind::AlignedWith`] turns it to the other's heading without moving it,
/// [`Kind::Near`] and [`Kind::Apart`] step it along the line between the two,
/// [`Kind::InFrontOf`] sets it on the other's forward, [`Kind::Behind`] on the other's backward,
/// [`Kind::LeftOf`] and [`Kind::RightOf`] on the other's own left and right,
/// and [`Kind::ClearInFront`] moves the intruder
/// sideways out of the corridor (the door stays where it was hung), [`Kind::VisibleFrom`] moves
/// the nearest blocker sideways off the sight line (both ends stay), [`Kind::Along`] slides the
/// piece toward the nearest point of its path, on its own side, [`Kind::Between`] sets it on the
/// segment between its two ends (inside them, on its own side), [`Kind::MirrorOf`] sets it on the
/// other's mirror point (the axis and the other stay), [`Kind::BacksOnto`] turns it straight out
/// from the other's face and sets it just inside `within` off it, on its own side and where it
/// stood along it, [`Kind::Inside`] sets it just inside the other's area across whichever edges
/// it stood past, on its own side and keeping its heading, [`Kind::Balances`] sets it where the
/// group's mean lands on the point (the others stay, and its heading), and [`Kind::ReachableFrom`] slides
/// the piece whose removal opens a walk just far enough that one opens (both ends stay). A move
/// is kept only when it lowers the map's (hard unmet, soft unmet) pair, compared hard first
/// ([`Relation::hard`]): so a hard relation is never broken to mend soft ones, a hard one is
/// mended even at the cost of soft ones, no relation is mended by breaking another of its own
/// weight, and contradictory ones end with as many met as one move each allows.
///
/// The moved piece is the relation's subject, or for a corridor the intruder; a piece resting on
/// another ([`Placed::on`]) is not pulled off it — an intruder on a host moves its host — and
/// whatever rests on a moved piece goes with it, turned about it.
pub fn solve(placements: &[Placed], relations: &Relations) -> Solved {
    // (hard unmet, soft unmet): tuples compare hard first.
    let unmet = |ps: &[Placed]| {
        check(ps, relations)
            .iter()
            .zip(&relations.relations)
            .filter(|(o, _)| matches!(o, Outcome::Unmet { .. }))
            .fold((0, 0), |(hard, soft), (_, r)| if r.hard { (hard + 1, soft) } else { (hard, soft + 1) })
    };
    let total = |(hard, soft): (usize, usize)| hard + soft;
    let mut current = placements.to_vec();
    let unmet_before = total(unmet(&current));
    let mut count = unmet(&current);
    let mut moved: Vec<String> = Vec::new();
    for _ in 0..SOLVE_ROUNDS {
        let mut progressed = false;
        for r in &relations.relations {
            let Outcome::Unmet { by, .. } = check_one(&current, r) else {
                continue;
            };
            let moves = propose(&current, r, by.as_deref());
            if moves.is_empty() {
                continue;
            }
            let mut trial = current.clone();
            let depth = trial.len();
            for (id, at, yaw) in &moves {
                move_piece(&mut trial, id, *at, *yaw, depth);
            }
            let trial_count = unmet(&trial);
            if trial_count < count {
                current = trial;
                count = trial_count;
                progressed = true;
                for (id, ..) in moves {
                    if !moved.contains(&id) {
                        moved.push(id);
                    }
                }
            }
        }
        if !progressed || count == (0, 0) {
            break;
        }
    }
    let unsatisfied = check(&current, relations)
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, Outcome::Unmet { .. }))
        .map(|(i, _)| i)
        .collect();
    Solved {
        placements: current,
        moved,
        unmet_before,
        unmet_after: total(count),
        unsatisfied,
    }
}

/// [`check`] on a map's own rows, whose [`Placed::yaw`] is degrees.
pub fn check_map(placements: &[Placed], relations: &Relations) -> Vec<Outcome> {
    check(&in_radians(placements), relations)
}

/// [`solve`] on a map's own rows (yaw in degrees), handing the rows back in degrees. A row whose
/// yaw the solve did not turn keeps its stored yaw bit for bit, so the degrees → radians → degrees
/// round trip never reads as an edit of a row nobody moved.
pub fn solve_map(placements: &[Placed], relations: &Relations) -> Solved {
    let radians = in_radians(placements);
    let mut solved = solve(&radians, relations);
    // `Solved::placements` is in input order, row for row.
    for ((row, given), stored) in solved.placements.iter_mut().zip(&radians).zip(placements) {
        row.yaw = if row.yaw == given.yaw { stored.yaw } else { row.yaw.to_degrees() };
    }
    solved
}

/// A map's rows with yaw turned from the degrees they store to the radians [`check`] reads.
pub(crate) fn in_radians(placements: &[Placed]) -> Vec<Placed> {
    placements
        .iter()
        .map(|p| Placed {
            yaw: p.yaw.to_radians(),
            ..p.clone()
        })
        .collect()
}

/// The moves that would mend `r`: which pieces, and where they go. Empty when the piece to move
/// rests on another, or the relation names a piece the map lacks. Every kind moves one piece but
/// a sight line, which clears all its blockers at once (one at a time could never lower the count).
fn propose(placements: &[Placed], r: &Relation, by: Option<&str>) -> Vec<Move> {
    let Ok(a) = find(placements, &r.of) else {
        return Vec::new();
    };
    let other = |id: &str| find(placements, id).ok();
    let free = |p: &Placed| p.on.is_none();
    // `toward` from `from`, unit length; `fallback` when the two stand on one spot.
    let unit = |from: (f32, f32), to: (f32, f32), fallback: (f32, f32)| {
        let d = distance(from, to);
        if d > 1e-4 {
            ((to.0 - from.0) / d, (to.1 - from.1) / d)
        } else {
            fallback
        }
    };
    let forward = |p: &Placed| (p.yaw.sin(), p.yaw.cos());
    if let Kind::VisibleFrom { from, clearance } = &r.kind {
        return other(from).map_or_else(Vec::new, |b| clear_sight(placements, a, b, *clearance));
    }
    let one = (|| match &r.kind {
        Kind::Faces(to) => {
            let b = other(to)?;
            let (dx, dz) = (b.at.0 - a.at.0, b.at.1 - a.at.1);
            free(a).then(|| (a.id.clone(), a.at, dx.atan2(dz)))
        }
        Kind::AlignedWith(to) => {
            let b = other(to)?;
            free(a).then(|| (a.id.clone(), a.at, b.yaw))
        }
        Kind::Near { to, within } => {
            let b = other(to)?;
            let (ux, uz) = unit(b.at, a.at, forward(b));
            let d = within * 0.9;
            free(a).then(|| (a.id.clone(), (b.at.0 + ux * d, b.at.1 + uz * d), a.yaw))
        }
        Kind::Apart { from, at_least } => {
            let b = other(from)?;
            let back = forward(a);
            let (ux, uz) = unit(b.at, a.at, (-back.0, -back.1));
            let d = at_least * 1.1 + 0.05;
            free(a).then(|| (a.id.clone(), (b.at.0 + ux * d, b.at.1 + uz * d), a.yaw))
        }
        Kind::InFrontOf { of, within } => {
            let b = other(of)?;
            let (fx, fz) = forward(b);
            let d = distance(a.at, b.at).clamp(within * 0.1, within * 0.9);
            free(a).then(|| (a.id.clone(), (b.at.0 + fx * d, b.at.1 + fz * d), a.yaw))
        }
        Kind::Behind { of, within } => {
            let b = other(of)?;
            let (fx, fz) = forward(b);
            let d = distance(a.at, b.at).clamp(within * 0.1, within * 0.9);
            free(a).then(|| (a.id.clone(), (b.at.0 - fx * d, b.at.1 - fz * d), a.yaw))
        }
        Kind::LeftOf { of, within } | Kind::RightOf { of, within } => {
            let b = other(of)?;
            // The other's left, as `local` measures sideways: (fz, -fx).
            let (fx, fz) = forward(b);
            let sign = if matches!(r.kind, Kind::LeftOf { .. }) { 1.0 } else { -1.0 };
            let d = sign * distance(a.at, b.at).clamp(within * 0.1, within * 0.9);
            free(a).then(|| (a.id.clone(), (b.at.0 + fz * d, b.at.1 - fx * d), a.yaw))
        }
        Kind::ClearInFront { width, .. } => {
            // The intruder, or the piece it rests on: moving the host carries it out.
            let intruder = bottom(placements, other(by?)?);
            let (ahead, side) = local(a, intruder.at);
            let sign = if side < 0.0 { -1.0 } else { 1.0 };
            let side = sign * (width / 2.0 * 1.1 + 0.05);
            // Back from `a`'s frame: forward (fx, fz), sideways (fz, -fx), as `local` measures.
            let (fx, fz) = forward(a);
            let to = (
                a.at.0 + fx * ahead + fz * side,
                a.at.1 + fz * ahead - fx * side,
            );
            Some((intruder.id.clone(), to, intruder.yaw))
        }
        Kind::Along { path, within } => {
            let q = nearest_on_path(placements, path, a.at).ok()?;
            let d = distance(a.at, q);
            // Keep its side and stand just inside; a piece on the path's centre line stays there.
            let keep = if d > 1e-4 { (within * 0.9).min(d) / d } else { 0.0 };
            let to = (q.0 + (a.at.0 - q.0) * keep, q.1 + (a.at.1 - q.1) * keep);
            free(a).then(|| (a.id.clone(), to, a.yaw))
        }
        Kind::Between { a: from, b: to, within } => {
            let (p, q) = (other(from)?.at, other(to)?.at);
            let len = distance(p, q);
            if len <= 1e-4 {
                return None;
            }
            let (t, _) = onto_segment(p, q, a.at);
            let t = t.clamp(0.1, 0.9);
            let (ux, uz) = ((q.0 - p.0) / len, (q.1 - p.1) / len);
            // Keep its side of the line, just inside `within`: (uz, -ux) is the segment's normal.
            let side = (a.at.0 - p.0) * uz - (a.at.1 - p.1) * ux;
            let side = side.signum() * side.abs().min(within * 0.9);
            let to = (
                p.0 + ux * len * t + uz * side,
                p.1 + uz * len * t - ux * side,
            );
            free(a).then(|| (a.id.clone(), to, a.yaw))
        }
        Kind::MirrorOf { of, about, .. } => {
            let p = other(of)?;
            let axis = other(about)?;
            free(a).then(|| (a.id.clone(), mirror(axis, p.at), a.yaw))
        }
        Kind::BacksOnto { of, within } => {
            let b = other(of)?;
            let (ahead, left) = local(b, a.at);
            // A piece on the plane goes out in front; otherwise it keeps its side.
            let sign = if ahead < 0.0 { -1.0 } else { 1.0 };
            let ahead = sign * ahead.abs().clamp(within * 0.1, within * 0.9);
            // Back from `b`'s frame: forward (fx, fz), left (fz, -fx), as `local` measures.
            let (fx, fz) = forward(b);
            let to = (b.at.0 + fx * ahead + fz * left, b.at.1 + fz * ahead - fx * left);
            let yaw = (sign * fx).atan2(sign * fz);
            free(a).then(|| (a.id.clone(), to, yaw))
        }
        Kind::Inside { of, width, depth } => {
            let b = other(of)?;
            let (ahead, left) = local(b, a.at);
            // Only an axis it stands past moves, and only to just inside that edge.
            let into = |v: f32, half: f32| if v.abs() > half { v.signum() * half * 0.9 } else { v };
            let (ahead, left) = (into(ahead, depth / 2.0), into(left, width / 2.0));
            // Back from `b`'s frame: forward (fx, fz), left (fz, -fx), as `local` measures.
            let (fx, fz) = forward(b);
            let to = (b.at.0 + fx * ahead + fz * left, b.at.1 + fz * ahead - fx * left);
            free(a).then(|| (a.id.clone(), to, a.yaw))
        }
        Kind::ReachableFrom { from, clearance } => {
            let b = other(from)?;
            open_walk(placements, a, b, *clearance, by?)
        }
        Kind::Balances { with, about, .. } => {
            // Equal weights: n centres whose mean is the point, so `of` stands at `n·c − Σ with`. When
            // `about` is `of` itself the point is where `of` ends up, so it stands on the others' mean.
            let members: Vec<(f32, f32)> = with.iter().map(|id| other(id).map(|p| p.at)).collect::<Option<_>>()?;
            let (sx, sz) = members.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
            let n = members.len() as f32 + 1.0;
            let to = if *about == a.id {
                (sx / (n - 1.0), sz / (n - 1.0))
            } else {
                let c = other(about)?.at;
                (n * c.0 - sx, n * c.1 - sz)
            };
            free(a).then(|| (a.id.clone(), to, a.yaw))
        }
        Kind::VisibleFrom { .. } => None,
    })();
    one.into_iter().collect()
}

/// Put `id` at `at` turned to `yaw`, carrying everything resting on it (and on that, and so on):
/// riders keep their offset in the host's frame, so they turn with it.
/// `depth` bounds the walk, so an `on` cycle in a malformed file ends.
fn move_piece(placements: &mut [Placed], id: &str, at: (f32, f32), yaw: f32, depth: usize) {
    if depth == 0 {
        return;
    }
    let Some(host) = placements.iter().position(|p| p.id == id) else {
        return;
    };
    let (from, turn) = (placements[host].at, yaw - placements[host].yaw);
    let (s, c) = turn.sin_cos();
    placements[host].at = at;
    placements[host].yaw = yaw;
    let riders: Vec<(String, (f32, f32), f32)> = placements
        .iter()
        .filter(|p| p.on.as_deref() == Some(id))
        .map(|p| {
            let (dx, dz) = (p.at.0 - from.0, p.at.1 - from.1);
            // Turning by `turn` about +y in this forward convention: (sin, cos) of yaw + turn.
            let (rx, rz) = (dx * c + dz * s, -dx * s + dz * c);
            (p.id.clone(), (at.0 + rx, at.1 + rz), p.yaw + turn)
        })
        .collect();
    for (rider, to, rider_yaw) in riders {
        move_piece(placements, &rider, to, rider_yaw, depth - 1);
    }
}
