//! **Kit standards: the project's dimensions every kit piece must meet** (#138, research lens #4).
//!
//! Shipped modular kits conform to project-wide standards: a footprint module that every piece is an
//! integer multiple of (so sub-kits snap together), a door opening the character fits through, a
//! storey height. Until this existed nothing here stated them, so a 3.1 m wall in a 2 m module or a
//! 0.6 m door was found by placing it. This module keeps them as data, the project's [`FILE`] beside
//! `kits.ron`, and [`check`] names every piece in a library that breaks one, with what it measured.
//!
//! A [`Rule`] covers the pieces whose `kind` holds its token, or every piece when it names none, and
//! bounds one [`Dim`]: width and depth are [`crate::descriptor::placed_footprint`] (before yaw),
//! height is [`crate::descriptor::placed_height`] (after the project's `stretch_y`). A covered piece
//! whose dimension was never measured, or measured as NaN or infinite, is [`Why::Unmeasured`],
//! never silently met. The rules are checked against each other too: [`unnested`] names every pair
//! of sub-kit modules that do not nest, which no single piece can show.
//!
//! Engine-free: reads descriptors, nothing else. A breach is reported, not refused; refusing a
//! piece at import is a separate choice for the caller.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use crate::descriptor::{Descriptor, placed_footprint, placed_height};
use crate::library::Library;

/// The format version of a standards file.
pub const STANDARDS_VERSION: u32 = 1;

/// A project's standards live in this file, in the same directory as its `kits.ron`.
pub const FILE: &str = "standards.ron";

/// How far, in metres, a dimension may sit off a multiple of the module and still be on it. Exported
/// meshes are rarely exact; a centimetre is below anything a player sees as a gap.
pub const MULTIPLE_TOLERANCE: f32 = 0.01;

/// Every standard one project's kits are meant to meet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Standards {
    pub version: u32,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl Default for Standards {
    fn default() -> Self {
        Standards {
            version: STANDARDS_VERSION,
            rules: Vec::new(),
        }
    }
}

impl Standards {
    /// Read a standards file, refusing a version this build does not read and a rule that could
    /// never hold or says nothing: no bound, a bound that is negative or not finite, a module of
    /// zero or less, or a `min` above its `max`.
    pub fn parse(text: &str) -> Result<Standards, String> {
        let s: Standards = ron::from_str(text).map_err(|e| format!("standards: parse: {e}"))?;
        if s.version != STANDARDS_VERSION {
            return Err(format!(
                "standards: version {} is not the {STANDARDS_VERSION} this build reads",
                s.version
            ));
        }
        for r in &s.rules {
            r.validate()
                .map_err(|e| format!("standards: rule {:?}: {e}", r.name))?;
        }
        Ok(s)
    }

    /// The file's text, in the form [`Self::parse`] reads.
    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("standards: write: {e}"))
    }
}

/// One dimension of a piece a rule bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Dim {
    /// The footprint's first axis, before yaw.
    Width,
    /// The footprint's second axis, before yaw.
    Depth,
    /// How tall the piece stands, after the project's `stretch_y`.
    Height,
}

/// One standard: a dimension of the pieces it covers, and the bounds it must keep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// What the rule is called in a report: `"door passage"`, `"wall module"`.
    pub name: String,
    /// The `kind` token a piece must carry to be covered; `None` covers every piece.
    #[serde(default)]
    pub kind: Option<String>,
    pub dim: Dim,
    #[serde(default)]
    pub min: Option<f32>,
    #[serde(default)]
    pub max: Option<f32>,
    /// The module the dimension must be an integer multiple of, within [`MULTIPLE_TOLERANCE`].
    #[serde(default)]
    pub multiple_of: Option<f32>,
}

impl Rule {
    fn validate(&self) -> Result<(), String> {
        if self.min.is_none() && self.max.is_none() && self.multiple_of.is_none() {
            return Err("has no min, max or multiple_of, so it says nothing".into());
        }
        for (what, v) in [
            ("min", self.min),
            ("max", self.max),
            ("multiple_of", self.multiple_of),
        ] {
            if let Some(v) = v
                && (!v.is_finite() || v < 0.0)
            {
                return Err(format!("{what} {v} is not a finite length of zero or more"));
            }
        }
        if self
            .multiple_of
            .is_some_and(|m| m <= 2.0 * MULTIPLE_TOLERANCE)
        {
            return Err(format!(
                "multiple_of must be above {} m, or every length is within tolerance of it",
                2.0 * MULTIPLE_TOLERANCE
            ));
        }
        if let (Some(lo), Some(hi)) = (self.min, self.max)
            && lo > hi
        {
            return Err(format!(
                "min {lo} is above max {hi}, so nothing can meet it"
            ));
        }
        Ok(())
    }

    /// Whether this rule covers `d`: it names no `kind`, or `d` carries the token it names.
    pub fn covers(&self, d: &Descriptor) -> bool {
        self.kind.as_ref().is_none_or(|k| d.kind.contains(k))
    }
}

/// Why a piece breaks a rule.
#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// The rule covers the piece, but its dimension was never measured.
    Unmeasured,
    /// Below the rule's `min`, which it carries.
    BelowMin(f32),
    /// Above the rule's `max`, which it carries.
    AboveMax(f32),
    /// Off the rule's module, which it carries.
    NotMultipleOf(f32),
}

/// One piece breaking one rule, with what was measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    /// The descriptor's id.
    pub piece: String,
    /// The rule's name.
    pub rule: String,
    pub dim: Dim,
    /// The measured dimension, or `None` when it was never measured.
    pub found: Option<f32>,
    pub why: Why,
}

fn measure(d: &Descriptor, dim: Dim) -> Option<f32> {
    match dim {
        Dim::Width => placed_footprint(d).map(|f| f.0),
        Dim::Depth => placed_footprint(d).map(|f| f.1),
        Dim::Height => placed_height(d),
    }
}

/// Every breach of `standards` in `library`, piece by piece in library order and rule by rule in
/// file order within a piece. A piece breaking one rule several ways (below `min` and off the
/// module) is reported once per way.
pub fn check(library: &Library, standards: &Standards) -> Vec<Breach> {
    let mut out = Vec::new();
    for d in &library.descriptors {
        for r in standards.rules.iter().filter(|r| r.covers(d)) {
            let breach = |found, why| Breach {
                piece: d.id.clone(),
                rule: r.name.clone(),
                dim: r.dim,
                found,
                why,
            };
            let Some(v) = measure(d, r.dim).filter(|v| v.is_finite()) else {
                out.push(breach(None, Why::Unmeasured));
                continue;
            };
            if let Some(lo) = r.min.filter(|&lo| v < lo) {
                out.push(breach(Some(v), Why::BelowMin(lo)));
            }
            if let Some(hi) = r.max.filter(|&hi| v > hi) {
                out.push(breach(Some(v), Why::AboveMax(hi)));
            }
            if let Some(m) = r.multiple_of {
                let off = v - (v / m).round() * m;
                if off.abs() > MULTIPLE_TOLERANCE || v < m - MULTIPLE_TOLERANCE {
                    out.push(breach(Some(v), Why::NotMultipleOf(m)));
                }
            }
        }
    }
    out
}

/// **The rules that cover no piece in `library`**, by name in file order. A rule scoped to a `kind`
/// no piece carries (most often a misspelt token) checks nothing, so a kit would read as meeting it;
/// naming it is how the author finds out.
pub fn idle<'a>(library: &Library, standards: &'a Standards) -> Vec<&'a str> {
    standards
        .rules
        .iter()
        .filter(|r| !library.descriptors.iter().any(|d| r.covers(d)))
        .map(|r| r.name.as_str())
        .collect()
}

/// How many lengths of the larger module [`unnested`] walks out looking for a grid line the two
/// modules share. 64 two-metre modules is 128 m, past the side of any one room or street.
pub const MEET_WITHIN: u32 = 64;

/// **Two rules whose modules do not nest** (#168): neither is a whole multiple of the other, so
/// runs of the pieces each covers drift off one another's grid lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Unnested {
    /// The two rules' names, the earlier in the file first.
    pub rules: [String; 2],
    /// Their dimensions, in the same order.
    pub dims: [Dim; 2],
    /// Their `multiple_of`, in the same order.
    pub modules: [f32; 2],
    /// The shortest run, in metres, on which both modules land on one grid line again, or `None`
    /// when they do not within [`MEET_WITHIN`] lengths of the larger.
    pub meet: Option<f32>,
}

/// Whether `v` is a whole multiple of `m`, within [`MULTIPLE_TOLERANCE`].
fn on_module(v: f32, m: f32) -> bool {
    (v - (v / m).round() * m).abs() <= MULTIPLE_TOLERANCE
}

/// **The pairs of rules whose modules do not nest** (#168, lens #4), each pair once, the earlier
/// rule first, in file order.
///
/// Shipped modular kits put every sub-kit on one grid: a sub-kit's module divides the base module or
/// is a whole multiple of it, so a wall run and a fence run meet on a grid line every module. [`check`]
/// holds each piece to its own rule's module, so a 2 m wall kit and a 1.5 m fence kit both pass
/// while they share a line only every 6 m. Two modules nest when the larger is a whole multiple of
/// the smaller within [`MULTIPLE_TOLERANCE`], as [`check`] measures a piece against one.
///
/// Width and depth are compared with each other, because a quarter turn puts a piece's width along
/// another's depth; height only with height. Rules without `multiple_of` are not compared.
pub fn unnested(standards: &Standards) -> Vec<Unnested> {
    let across = |d: Dim| d != Dim::Height;
    let modules: Vec<(&Rule, f32)> = standards
        .rules
        .iter()
        .filter_map(|r| r.multiple_of.map(|m| (r, m)))
        .collect();
    let mut out = Vec::new();
    for (i, &(a, ma)) in modules.iter().enumerate() {
        for &(b, mb) in &modules[i + 1..] {
            let (big, small) = if ma >= mb { (ma, mb) } else { (mb, ma) };
            if across(a.dim) != across(b.dim) || on_module(big, small) {
                continue;
            }
            let meet = (2..=MEET_WITHIN)
                .map(|k| k as f32 * big)
                .find(|&run| on_module(run, small));
            out.push(Unnested {
                rules: [a.name.clone(), b.name.clone()],
                dims: [a.dim, b.dim],
                modules: [ma, mb],
                meet,
            });
        }
    }
    out
}
