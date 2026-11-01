//! **Which repeats a player will notice** (#136, research lens #5).
//!
//! A count of copies says nothing useful: a town holds hundreds of identical fence pieces and wall
//! segments because structure is *meant* to tile, and a handful of barrels that should not look
//! stamped. Shipped level-art practice treats the two differently — repeated clutter reads as
//! copy-paste, repeated structure reads as a building — so this weighs a repeat by the role of the
//! piece repeated.
//!
//! # What counts as a repeat
//!
//! A **visible repeat** is two placements of the same descriptor (patch included), at the same yaw
//! within [`YAW_TOLERANCE_DEG`] (a full turn is the same look), with the same tip, closer together
//! than [`RepetitionRules::radius_m`]. Turning or tipping a copy is the cheapest variation an author
//! has, so it ends the repeat; a copy out of sight of the other is no repeat either. A **round**
//! piece ([`RepetitionRules::round`]: a barrel, a pot) looks the same at any yaw, so for it only the
//! tip and the distance count, and [`vary`] never turns it (#184). A piece whose floor is longer one
//! way than the other (a bench, a cart) is turned only a half turn by [`vary`], the one turn that
//! keeps it on the cells it covered (#186).
//!
//! A piece's **role** is read off its `kind` tokens: any token in [`RepetitionRules::structure`]
//! makes it [`Role::Structure`], everything else is [`Role::Clutter`]. The score of a descriptor is
//! the number of its placements that have a look-alike in range, times its role's weight — linear in
//! pieces rather than pairs, so a long fence run does not grow quadratically past a cluster of crates.
//!
//! Engine-free: reads a map's placements and a library, nothing else.

use serde::{Deserialize, Serialize};

use crate::descriptor::tipped_footprint;
use crate::library::Library;
use crate::map::Placed;
use crate::stack::descriptor_of;

/// Two yaws closer than this, in degrees, are one look.
pub const YAW_TOLERANCE_DEG: f32 = 1.0;

/// The project's rules, beside `kits.ron` (#139). Absent means [`RepetitionRules::default`].
pub const FILE: &str = "repetition.ron";

/// The format version of a `repetition.ron` (#202). A file with no `version` is the first.
pub const REPETITION_VERSION: u32 = 1;

/// What a repeat is, and what it costs, as data an author can tune.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RepetitionRules {
    /// The format version: [`REPETITION_VERSION`]. A file from before it existed has none and reads as
    /// the first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "crate::format_version::first", deserialize_with = "crate::format_version::read::<_, REPETITION_VERSION>")]
    pub version: u32,
    /// Two look-alikes closer than this, in metres, are seen together.
    pub radius_m: f32,
    /// `kind` tokens that make a piece structure: meant to repeat.
    pub structure: Vec<String>,
    /// What one repeated structure piece weighs against one repeated clutter piece (1.0).
    pub structure_weight: f32,
    /// `kind` tokens of pieces that look the same at any yaw (#184): a round barrel, a pot, a well.
    /// Their copies repeat whatever their yaw, and [`vary`] never turns one. Empty by default,
    /// because whether a piece is round is a fact about its art that only the author can state.
    pub round: Vec<String>,
}

impl Default for RepetitionRules {
    fn default() -> Self {
        Self {
            version: REPETITION_VERSION,
            radius_m: 6.0,
            structure: ["building-module", "fence", "tile", "landmark", "grave", "door"]
                .map(str::to_owned)
                .to_vec(),
            structure_weight: 0.1,
            round: Vec::new(),
        }
    }
}

impl RepetitionRules {
    /// Parse a project's `repetition.ron`. A field it leaves out keeps its default; a radius that
    /// is not a positive number of metres, or a weight that is negative or not a number, is refused
    /// rather than read as a report nobody could trust.
    pub fn parse(text: &str) -> Result<RepetitionRules, String> {
        let r: RepetitionRules =
            ron::from_str(text).map_err(|e| format!("repetition: parse: {e}"))?;
        if !(r.radius_m.is_finite() && r.radius_m > 0.0) {
            return Err(format!(
                "repetition: radius_m {} is not a positive number of metres",
                r.radius_m
            ));
        }
        if !(r.structure_weight.is_finite() && r.structure_weight >= 0.0) {
            return Err(format!(
                "repetition: structure_weight {} is not a weight of zero or more",
                r.structure_weight
            ));
        }
        Ok(r)
    }
}

/// Whether a piece is meant to repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Walls, fences, tiles: a run of copies is the design.
    Structure,
    /// Props and dressing: a run of copies reads as stamped.
    Clutter,
}

/// One descriptor's visible repeats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Repeat {
    /// The descriptor repeated. A patched copy is a different look, so one descriptor can head
    /// several rows: its plain copies, and its copies under each patch.
    pub descriptor: String,
    /// The patch every copy in this row carries, `None` for plain copies.
    pub patch: Option<crate::descriptor::Descriptor>,
    pub role: Role,
    /// Pairs of look-alikes in range of each other.
    pub pairs: usize,
    /// Placements with at least one look-alike in range.
    pub pieces: usize,
    /// `pieces` times the role's weight. The report is sorted by this, highest first.
    pub score: f32,
}

/// Every descriptor of `placements` that repeats visibly, the one a player notices first, first.
///
/// Placements whose descriptor the library does not hold are skipped: they have no role to weigh.
/// Ties are broken by descriptor id, then plain before patched, then by where the look first
/// appears in `placements`, so the report is deterministic.
pub fn repeats(placements: &[Placed], library: &Library, rules: &RepetitionRules) -> Vec<Repeat> {
    // Group by the look: descriptor id plus patch, so a patched copy is a different piece.
    let mut groups: Vec<(String, Role, bool, Vec<&Placed>)> = Vec::new();
    for p in placements {
        let Some(d) = descriptor_of(library, p) else {
            continue;
        };
        let key = look_key(p);
        match groups.iter_mut().find(|(k, ..)| *k == key) {
            Some((.., rows)) => rows.push(p),
            None => {
                let role = role_of(&d, rules);
                groups.push((key, role, is_round(&d, rules), vec![p]));
            }
        }
    }

    let radius2 = rules.radius_m * rules.radius_m;
    let mut report: Vec<Repeat> = groups
        .into_iter()
        .filter_map(|(_, role, round, rows)| {
            let mut paired = vec![false; rows.len()];
            let mut pairs = 0;
            for i in 0..rows.len() {
                for j in i + 1..rows.len() {
                    if look_alike(rows[i], rows[j], radius2, round) {
                        pairs += 1;
                        paired[i] = true;
                        paired[j] = true;
                    }
                }
            }
            if pairs == 0 {
                return None;
            }
            let pieces = paired.iter().filter(|p| **p).count();
            let weight = match role {
                Role::Structure => rules.structure_weight,
                Role::Clutter => 1.0,
            };
            Some(Repeat {
                descriptor: rows[0].descriptor.clone(),
                patch: rows[0].patch.clone(),
                role,
                pairs,
                pieces,
                score: pieces as f32 * weight,
            })
        })
        .collect();
    report.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.descriptor.cmp(&b.descriptor))
            .then_with(|| a.patch.is_some().cmp(&b.patch.is_some()))
    });
    report
}

/// The role `rules` gives a descriptor: structure if any of its `kind` tokens says so.
fn role_of(d: &crate::descriptor::Descriptor, rules: &RepetitionRules) -> Role {
    if d.kind.iter().any(|k| rules.structure.contains(k)) {
        Role::Structure
    } else {
        Role::Clutter
    }
}

/// The turns [`vary`] tries, in order. A half turn first, because a placement's `at` is its centre,
/// so a half turn leaves any footprint on the cells it stood on; then the quarter turns, which do
/// that only for a square footprint, so a piece whose floor is longer one way than the other is
/// given the half turn alone (#186). No other angle: a turn off the grid's right angles would move a
/// footprint off its cells, and a copy the turns it may take cannot end is reported instead.
pub const TURNS_DEG: [f32; 3] = [180.0, 90.0, 270.0];

/// How far apart, in metres, a floor footprint's width and depth may be for a quarter turn to leave
/// it on its cells. A centimetre, below anything a player sees as a gap, as an exported mesh is
/// rarely exact.
pub const SQUARE_TOLERANCE_M: f32 = 0.01;

/// What [`vary`] would do to a map's placements.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Variation {
    /// `(index into placements, the yaw it turns to)`, in placement order.
    pub turned: Vec<(usize, f32)>,
    /// Clutter placements that still have a look-alike in range after the turns: more copies close
    /// together than the turns they may take have looks, or round copies, which no turn changes.
    pub left: usize,
}

/// **Turn the clutter copies a player would see as stamped** (#143), so none looks like another in
/// range. Structure is never touched: a run of identical wall is the design.
///
/// Deterministic, with no seed: placements are walked in map order, and a clutter copy that looks
/// like an earlier one takes the first of [`TURNS_DEG`] that leaves it a look-alike of no other
/// placement. The first copy of each look keeps its yaw, so an author's chosen facing survives on it.
/// A round copy ([`RepetitionRules::round`]) is never turned, since no turn changes how it looks; it
/// is counted in [`Variation::left`] instead. A copy whose floor footprint (after its tip) is measured
/// and not square takes only the half turn, which keeps it on the cells it covered; a quarter turn
/// would swap its width and depth into the pieces beside it (#186). An unmeasured piece keeps all
/// three turns, as nothing is known that a turn would break.
pub fn vary(placements: &[Placed], library: &Library, rules: &RepetitionRules) -> Variation {
    let radius2 = rules.radius_m * rules.radius_m;
    // (clutter, round, long) per placement, from one descriptor lookup each.
    let traits: Vec<(bool, bool, bool)> = placements
        .iter()
        .map(|p| {
            descriptor_of(library, p).map_or((false, false, false), |d| {
                let long = tipped_footprint(&d, p.tip)
                    .is_some_and(|(w, depth)| (w - depth).abs() > SQUARE_TOLERANCE_M);
                (role_of(&d, rules) == Role::Clutter, is_round(&d, rules), long)
            })
        })
        .collect();
    // Each look's key once, not once per pair: a patched key formats the whole patch.
    let keys: Vec<String> = placements.iter().map(look_key).collect();
    let mut yaws: Vec<f32> = placements.iter().map(|p| p.yaw).collect();
    // Whether placement `i`, at `yaw`, looks like any of the first `upto` others at their yaws now.
    let alike = |yaws: &[f32], i: usize, yaw: f32, upto: usize| {
        let p = &placements[i];
        (0..upto).any(|j| {
            let q = &placements[j];
            let (dx, dz) = (p.at.0 - q.at.0, p.at.1 - q.at.1);
            let turn = (yaw - yaws[j]).rem_euclid(360.0);
            j != i
                && dx * dx + dz * dz < radius2
                && p.tip == q.tip
                && (traits[i].1 || turn.min(360.0 - turn) < YAW_TOLERANCE_DEG)
                && keys[j] == keys[i]
        })
    };
    let n = placements.len();
    let mut turned = Vec::new();
    for i in 0..n {
        let (clutter, round, long) = traits[i];
        if !clutter || round || !alike(&yaws, i, yaws[i], i) {
            continue;
        }
        // A long piece stops after the half turn, the first of TURNS_DEG.
        let found = TURNS_DEG[..if long { 1 } else { TURNS_DEG.len() }]
            .iter()
            .map(|turn| (yaws[i] + turn).rem_euclid(360.0))
            .find(|&yaw| !alike(&yaws, i, yaw, n));
        if let Some(yaw) = found {
            yaws[i] = yaw;
            turned.push((i, yaw));
        }
    }
    let left = (0..n).filter(|&i| traits[i].0 && alike(&yaws, i, yaws[i], n)).count();
    Variation { turned, left }
}

/// A descriptor and its patch, as one comparable key.
fn look_key(p: &Placed) -> String {
    match &p.patch {
        None => p.descriptor.clone(),
        Some(patch) => format!("{}{patch:?}", p.descriptor),
    }
}

/// Whether `d` is round under `rules`: one of its `kind` tokens is in [`RepetitionRules::round`].
fn is_round(d: &crate::descriptor::Descriptor, rules: &RepetitionRules) -> bool {
    d.kind.iter().any(|k| rules.round.contains(k))
}

/// Two copies of one look, in range, with the same tip, at the same yaw unless the piece is round.
fn look_alike(a: &Placed, b: &Placed, radius2: f32, round: bool) -> bool {
    let (dx, dz) = (a.at.0 - b.at.0, a.at.1 - b.at.1);
    let turn = (a.yaw - b.yaw).rem_euclid(360.0);
    a.tip == b.tip
        && dx * dx + dz * dz < radius2
        && (round || turn.min(360.0 - turn) < YAW_TOLERANCE_DEG)
}
