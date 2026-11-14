//! **The checks a task states, as a closed set of typed variants** (BM-01, #411).
//!
//! Every check is one question about **what a run left**, and it is answered from the saved files with
//! its own arithmetic: cells and yaws out of the map's YAML, extents out of the library that gives a
//! piece its footprint, line counts out of the operation log. **No check calls the operation that made
//! the change.** A check that asked `placement_set_yaw` whether the yaw moved would pass whenever the
//! operation's own bug made it agree with itself  -  the failure mode of every benchmark that shares a
//! derivation with its subject.
//!
//! # A closed set, written as one key
//!
//! A check is a mapping with **one** key, which names the variant, and the value is that variant's own
//! arguments:
//!
//! ```yaml
//! checks:
//!   - placement_near_wall: {id: bed, wall: north, max_gap_m: 0.10}
//!   - unchanged: [maps/kitchen.map.yaml]
//!   - project_unchanged: {}
//! ```
//!
//! **Serde's own tag form for YAML is `!placement_near_wall {…}`**, which is a tag a person would not write
//! and the one YAML style this workspace does not use, so [`Check`] is deserialized by hand: the variant is
//! chosen from the single key, the value goes to that variant's own `deny_unknown_fields` type, and a key
//! that is not a check is refused **by name with the list of the ones this build has**. There is no
//! "run this snippet" check and no free text, so a check is refused at load rather than skipped, and a
//! skipped check is one that silently stops measuring anything.
//!
//! # Compass, as the rest of the workspace spells it
//!
//! `north` is `−Z`, `east` is `+X`, `south` is `+Z`, `west` is `−X`, which is
//! [`bevy_grimoire_core::descriptor::Face`]'s own table. A task's prompt says "the back wall" or "left of
//! the bed"; its check says `north` or `west`, so the words in a prompt and the arithmetic in a check are
//! one convention rather than two.

use bevy_grimoire_core::descriptor::Descriptor;
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, Placed};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::grade::{Found, Snapshot};

/// **How many metres a gap may be off** before a placement is not "against" something. A fifth of the
/// 0.5 m snap: an author who means flush says 0.02, and the tolerance that lets a piece be a hand's width
/// out is 0.10 rather than a free pass.
pub const FLUSH_M: f32 = 0.10;

/// **The directory of the project's authoring kit, under `assets/grimoire/`**, where the editor keeps what
/// a map does not: the ground (`terrain/<map>.yaml`), the water (`water/<map>.yaml`) and the game's data
/// registry (`data.yaml`). It is `Project::grimoire_dir` for the benchmark's project.
pub const AUTHORING_KIT: &str = "furniture";

/// **The one field every check names a placement by**: its id, or its piece.
///
/// **A piece is the safer subject for a task**, because an id is minted by the editor (`bed_double@1`) and a
/// task that pinned one would fail whenever the id counter started elsewhere. An id is right for a task
/// about a piece a `setup` placed and named.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlacementRef {
    /// The placement's id, as the map file names it.
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    pub piece: Option<String>,
}

impl PlacementRef {
    /// **The placements this names**, from a map's rows — one for an id, every one of a piece.
    ///
    /// An empty answer is the check's own failure (`no placement is called …`), not an error: a run that
    /// never placed the piece is the failure the check exists to see.
    pub fn matches<'a>(&self, map: &'a Map) -> Vec<&'a Placed> {
        map.placements
            .iter()
            .filter(|p| match (&self.id, &self.piece) {
                (Some(id), _) => &p.id == id,
                (None, Some(piece)) => &p.descriptor == piece,
                (None, None) => false,
            })
            .collect()
    }

    /// How the check calls it, in a report.
    pub fn shown(&self) -> String {
        match (&self.id, &self.piece) {
            (Some(id), _) => format!("`{id}`"),
            (None, Some(piece)) => format!("a `{piece}`"),
            (None, None) => "nothing".to_owned(),
        }
    }

    /// **The parameters that name it are wrong**: neither an id nor a piece, or both.
    pub fn wrong(&self) -> Option<&'static str> {
        match (&self.id, &self.piece) {
            (None, None) => Some("names neither an `id` nor a `piece`"),
            (Some(_), Some(_)) => {
                Some("names both an `id` and a `piece`, and one of them is not the question")
            }
            _ => None,
        }
    }
}

/// **Whichever of `id` and `piece` a check states**, as the one subject type.
fn subject(id: &Option<String>, piece: &Option<String>) -> PlacementRef {
    PlacementRef { id: id.clone(), piece: piece.clone() }
}

/// **Which wall of the map's rectangle.** Its bounds are centred on the origin, so the north wall is the
/// `z = −bounds.z / 2` plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wall {
    /// `z = −bounds.z / 2`.
    North,
    /// `x = +bounds.x / 2`.
    East,
    /// `z = +bounds.z / 2`.
    South,
    /// `x = −bounds.x / 2`.
    West,
}

impl Wall {
    /// The word a report uses.
    pub fn name(self) -> &'static str {
        match self {
            Wall::North => "north",
            Wall::East => "east",
            Wall::South => "south",
            Wall::West => "west",
        }
    }
}

/// **Which axis a distance is measured along**, for a relation between two pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// The map's `x`, west to east.
    X,
    /// The map's `z`, north to south.
    Z,
}

impl Axis {
    /// The word a report uses.
    pub fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Z => "z",
        }
    }
}

/// **Which way one piece is from another**, in the workspace's compass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// `−Z`.
    North,
    /// `+X`.
    East,
    /// `+Z`.
    South,
    /// `−X`.
    West,
}

impl Direction {
    /// The axis it runs along, and whether it is the larger or the smaller end of it.
    pub fn axis(self) -> (Axis, bool) {
        match self {
            Direction::North => (Axis::Z, false),
            Direction::South => (Axis::Z, true),
            Direction::East => (Axis::X, true),
            Direction::West => (Axis::X, false),
        }
    }

    /// The word a report uses.
    pub fn name(self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::East => "east",
            Direction::South => "south",
            Direction::West => "west",
        }
    }
}

/// **A problem kind the map may hold**, computed by the core's own passes rather than the editor's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
    /// Two pieces whose plans interpenetrate on the same layer.
    Overlap,
    /// Two abutting tiles whose declared face tokens disagree.
    EdgeFault,
    /// A piece standing in a measured opening.
    Doorway,
    /// A piece inside the space another piece's clearance reserves.
    Clearance,
}

impl ProblemKind {
    /// The word a report uses.
    pub fn name(self) -> &'static str {
        match self {
            ProblemKind::Overlap => "overlap",
            ProblemKind::EdgeFault => "edge_fault",
            ProblemKind::Doorway => "doorway",
            ProblemKind::Clearance => "clearance",
        }
    }

    /// How many of this kind the map holds.
    pub fn count(self, found: &Found) -> usize {
        match self {
            ProblemKind::Overlap => found.overlaps.len(),
            ProblemKind::EdgeFault => found.faults.len(),
            ProblemKind::Doorway => found.doorways.len(),
            ProblemKind::Clearance => found.clearances.len(),
        }
    }

    /// The first of this kind, in a sentence, for a report.
    pub fn first(self, found: &Found) -> Option<String> {
        match self {
            ProblemKind::Overlap => found.overlaps.first().cloned(),
            ProblemKind::EdgeFault => found.faults.first().cloned(),
            ProblemKind::Doorway => found.doorways.first().cloned(),
            ProblemKind::Clearance => found.clearances.first().cloned(),
        }
    }
}

/// **One fact a hand-back has to state.** A substring, matched without case, so a hand-back that says
/// "velvet_armchair is not in the library" carries the fact `velvet_armchair`.
pub type HandbackFact = String;

/// **A placement, or a count of one, exists.**
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementExists {
    /// The placement's id, as the map file names it.
    #[serde(default)]
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    #[serde(default)]
    pub piece: Option<String>,
    /// **Exactly how many** must be on the map. One, so two beds where the task asked for one fails.
    #[serde(default = "one")]
    pub count: usize,
}

fn one() -> usize {
    1
}

/// **A placement is at a spot, facing a way, or lifted, within a tolerance.**
///
/// Every field but the subject is optional, and **at least one of them has to be given**: a check that asks
/// nothing would pass on any map, which is the do-nothing loophole at the scale of one check.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementAt {
    /// The placement's id, as the map file names it.
    #[serde(default)]
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    #[serde(default)]
    pub piece: Option<String>,
    /// The spot its centre must be within `tolerance_m` of, in map space.
    #[serde(default)]
    pub at: Option<(f32, f32)>,
    /// How far off the spot it may be, metres.
    #[serde(default)]
    pub tolerance_m: Option<f32>,
    /// The facing it must have, degrees.
    #[serde(default)]
    pub yaw_deg: Option<f32>,
    /// How far off the facing it may be, degrees.
    #[serde(default)]
    pub yaw_tolerance_deg: Option<f32>,
    /// The vertical offset it must have, metres.
    #[serde(default)]
    pub lift_m: Option<f32>,
    /// How far off the lift it may be, metres.
    #[serde(default)]
    pub lift_tolerance_m: Option<f32>,
}

/// **A placement stands flat against a wall of the map's rectangle.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementNearWall {
    /// The placement's id, as the map file names it.
    #[serde(default)]
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    #[serde(default)]
    pub piece: Option<String>,
    /// Which wall.
    pub wall: Wall,
    /// **The largest gap between the piece's own edge and the wall**, metres.
    #[serde(default = "flush_m")]
    pub max_gap_m: f32,
    /// **How far the piece's centre may be from the middle of that wall**, metres, when the task asks for
    /// it centred.
    #[serde(default)]
    pub centre_within_m: Option<f32>,
}

fn flush_m() -> f32 {
    FLUSH_M
}

/// **One placement is a stated direction from another**, measured edge to edge along one axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementRelative {
    /// The placement's id, as the map file names it.
    #[serde(default)]
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    #[serde(default)]
    pub piece: Option<String>,
    /// The piece it is placed relative to, by id or piece.
    pub other: PlacementRefField,
    /// Which way, in the workspace's compass.
    pub direction: Direction,
    /// **The largest gap between the two pieces' own edges**, metres, along that axis.
    #[serde(default = "flush_m")]
    pub max_gap_m: f32,
}

/// **A placement named inside another check's arguments**, written as `{id: …}` or `{piece: …}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementRefField {
    /// The placement's id, as the map file names it.
    #[serde(default)]
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    #[serde(default)]
    pub piece: Option<String>,
}

impl PlacementRefField {
    /// The subject it names.
    pub fn subject(&self) -> PlacementRef {
        subject(&self.id, &self.piece)
    }
}

/// **A placement rests on another**, as its `on` field states it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementRestsOn {
    /// The placement's id, as the map file names it.
    #[serde(default)]
    pub id: Option<String>,
    /// The descriptor this placement is an instance of.
    #[serde(default)]
    pub piece: Option<String>,
    /// **The id of the piece it must name as its host.** Right for a host the task's own map names; a
    /// host an agent placed has an id the editor minted, which a task cannot know, and it says
    /// [`Self::host_piece`] instead.
    #[serde(default)]
    pub host: String,
    /// **The piece its host must be an instance of**, whichever id it has.
    #[serde(default)]
    pub host_piece: Option<String>,
}

/// **The map holds exactly this many placements**, of any piece.
///
/// What a task "remove both crates and nothing else" needs beside the crates' absence: a run that removed
/// the crates *and* a drawer has the same crates and a smaller map.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementTotal {
    /// How many placements the map holds.
    pub count: usize,
}

/// **Exactly `count` placements of a piece stand on the map, and every one of them is in a region.**
///
/// The region is a rectangle of the map's `x` and `z`, centres only: a hedge along the south edge, a nook
/// in a corner. A piece of this kind across the room fails it even when the count is right, so five plants
/// in the band and a sixth elsewhere is not a hedge.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementsWithin {
    /// The descriptor the placements are instances of.
    pub piece: String,
    /// How many of that piece the map holds, all in the region.
    pub count: usize,
    /// The region's `x` extent, `[low, high]`, metres.
    pub x: (f32, f32),
    /// The region's `z` extent, `[low, high]`, metres.
    pub z: (f32, f32),
}

/// **A scalar in a project file is what the task wants**, read by a path through the file's YAML.
///
/// For what leaves no placement: a number in the game's data, a new map's size. `file` is relative to
/// `assets/grimoire/`; `path` is dotted with list indices (`gate.health`, `bounds[0]`, `lakes[1].level_m`).
/// At least one of `equals`, `min`, `max`, `contains` and `lacks` is required, because a check that asks nothing passes on any
/// file. `contains` and `lacks` read a string leaf by the words it holds (a substring, without case), so a task that asks for a
/// sentence is scored on the sentence the agent saved and not on what it said about it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileValue {
    /// The file, relative to `assets/grimoire/`.
    pub file: String,
    /// The leaf's path inside it.
    pub path: String,
    /// The value it must equal: a number, a string or a boolean.
    #[serde(default)]
    pub equals: Option<serde_json::Value>,
    /// The least a number may be.
    #[serde(default)]
    pub min: Option<f64>,
    /// The most a number may be.
    #[serde(default)]
    pub max: Option<f64>,
    /// **A word a string leaf must hold**, a substring matched without case: what a greeting says. Never empty.
    #[serde(default)]
    pub contains: Option<String>,
    /// **A word a string leaf must not hold**, a substring matched without case. Never empty.
    #[serde(default)]
    pub lacks: Option<String>,
}

/// **The ground is within a height at a point**, from the map's terrain file.
///
/// Sampled with the core's own bilinear `Heightfield::height_at`, so a hill "2 m high at the corner" is
/// measured where the ground actually is rather than where a brush was told to go. At least one of
/// `min_m` and `max_m` is required.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainHeight {
    /// The map whose terrain is read: `furniture/terrain/<map>.yaml` (the authoring kit's `terrain/`, which is where `terrain_stroke` writes).
    pub map: String,
    /// The point, `[x, z]` in world metres.
    pub at: (f32, f32),
    /// The least the ground may be there.
    #[serde(default)]
    pub min_m: Option<f32>,
    /// The most the ground may be there.
    #[serde(default)]
    pub max_m: Option<f32>,
}

/// **The map has exactly `count` lakes, every one centred in a region and at least a radius wide.**
///
/// Read from the map's water plan, `furniture/water/<map>.yaml`, which is where `water_place` writes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterLake {
    /// The map whose water plan is read.
    pub map: String,
    /// The region's `x` extent, `[low, high]`, metres.
    pub x: (f32, f32),
    /// The region's `z` extent, `[low, high]`, metres.
    pub z: (f32, f32),
    /// The least a lake's radius may be, metres.
    #[serde(default)]
    pub min_radius_m: Option<f32>,
    /// **How far above the ground at its centre the lake's level may stand**, `[least, most]` metres. The
    /// ground is the map's saved terrain (flat at 0 when it has none). A pond whose level is under the
    /// ground at its centre is not water there, and one a metre over it is a tower of water: a check on
    /// the position alone passes both.
    #[serde(default)]
    pub level_above_ground_m: Option<(f32, f32)>,
    /// How many lakes the plan holds. One.
    #[serde(default = "one")]
    pub count: usize,
}

/// **The named files are byte-identical to what the setup left.** Written as a list of paths relative to
/// `assets/grimoire/`, so a task says `maps/bench_bedroom.map.yaml` rather than the whole path.
///
/// A comparison is a digest of the bytes rather than a re-serialisation of a value: a file rewritten with
/// the same meaning but different whitespace is a change to a file this check exists to see unchanged.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Unchanged {
    /// The files, relative to `assets/grimoire/`.
    pub files: Vec<String>,
}

/// **The whole project is byte-identical to what the setup left**, every file in it.
///
/// This is the check a task that must be *refused* needs: an agent that does the work anyway fails it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectUnchanged {}

/// **The map holds no problem of the named kinds**, written as a list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoProblem {
    /// The kinds that must be absent. Empty is refused at load: it would pass on any map.
    pub kinds: Vec<ProblemKind>,
}

/// **The operation log has at least `min` applied lines for an operation.**
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OplogHas {
    /// The operation's name, as the registry spells it.
    pub op: String,
    /// How many applied lines it needs. One, so a task can require the change happened at all.
    #[serde(default = "one")]
    pub min: usize,
}

/// **The operation log has no applied line for an operation.**
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OplogLacks {
    /// The operation's name, as the registry spells it.
    pub op: String,
}

/// **A hand-back exists and states every one of these facts**, written as a list.
///
/// This is half of the rule that closes the do-nothing loophole for a task an agent must refuse: a run that
/// changes nothing and says nothing is not a refusal, it is silence.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandbackContains {
    /// The facts, each a substring matched without case across everything the hand-back says.
    pub facts: Vec<HandbackFact>,
}

/// **A hand-back exists and states none of these**, written as a list: the wrong answers a task names.
///
/// `handback_contains` can only say what a right answer includes, and a wrong answer often includes it too
/// ("it is on the floor" and "it is not on the floor" both say `floor`). A task that has a right and a wrong
/// answer pairs the two: the right one states a marker (`answer: no`) and the wrong one's marker is listed
/// here. **No hand-back at all fails this too**: silence is not a right answer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandbackLacks {
    /// The facts, each a substring matched without case across everything the hand-back says.
    pub facts: Vec<HandbackFact>,
}

/// **A calibrated decision the harness made about the result answers as the task wants.**
///
/// The decision is made by the harness (a `grimoire_decide` call, BM-02) and handed to the grader, so this
/// crate needs no model: a check that ran one itself could not be scored in the push gate.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decide {
    /// The check's name, as the harness asked it.
    pub check: String,
    /// The answer the task wants.
    pub want: bool,
    /// **How sure the decision must be**, `0.0..=1.0`. A no-sure answer is not an answer the task may be
    /// scored on.
    #[serde(default = "mostly")]
    pub min_confidence: f32,
}

fn mostly() -> f32 {
    0.8
}

/// **One check a task states.**
///
/// **Deserialized by hand**, because a project file writes a check as a mapping with one key —
/// `placement_near_wall: {…}` — and serde's own tag form for YAML is `!placement_near_wall {…}`, a tag a
/// person would not write and this workspace's YAML style does not use. See the module note.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// A placement, or a count of one, exists.
    PlacementExists(PlacementExists),
    /// A placement is at a spot, facing a way, or lifted.
    PlacementAt(PlacementAt),
    /// A placement stands flat against a wall.
    PlacementNearWall(PlacementNearWall),
    /// One placement is a stated direction from another.
    PlacementRelative(PlacementRelative),
    /// A placement rests on a named host.
    PlacementRestsOn(PlacementRestsOn),
    /// The named files are byte-identical to the setup state.
    Unchanged(Unchanged),
    /// The whole project is byte-identical to the setup state.
    ProjectUnchanged(ProjectUnchanged),
    /// The map holds no problem of the named kinds.
    NoProblem(NoProblem),
    /// The log has at least `min` applied lines for an operation.
    OplogHas(OplogHas),
    /// The log has no applied line for an operation.
    OplogLacks(OplogLacks),
    /// A hand-back states every named fact.
    HandbackContains(HandbackContains),
    HandbackLacks(HandbackLacks),
    /// A calibrated decision answers as the task wants.
    Decide(Decide),
    /// The map holds exactly this many placements.
    PlacementTotal(PlacementTotal),
    /// Exactly this many of a piece stand in a region.
    PlacementsWithin(PlacementsWithin),
    /// A scalar in a project file is what the task wants.
    FileValue(FileValue),
    /// The ground is within a height at a point.
    TerrainHeight(TerrainHeight),
    /// The map's water plan holds the stated lakes.
    WaterLake(WaterLake),
}

/// **Every check name this build has**, in the order a refusal lists them.
pub const CHECK_NAMES: &[&str] = &[
    "placement_exists",
    "placement_at",
    "placement_near_wall",
    "placement_relative",
    "placement_rests_on",
    "unchanged",
    "project_unchanged",
    "no_problem",
    "oplog_has",
    "oplog_lacks",
    "handback_contains",
    "handback_lacks",
    "decide",
    "placement_total",
    "placements_within",
    "file_value",
    "terrain_height",
    "water_lake",
];

impl<'de> Deserialize<'de> for Check {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // **The value is read as generic JSON-shaped data first**, because the variant is decided by the
        // mapping's single key rather than by a tag; the format is self-describing and a project file
        // holds nothing a JSON value cannot say.
        let value = Value::deserialize(d)?;
        Check::from_value(&value).map_err(D::Error::custom)
    }
}

impl Check {
    /// **The check a task's value states**, or a refusal naming the key and the ones this build has.
    fn from_value(value: &Value) -> Result<Check, String> {
        let Some(object) = value.as_object() else {
            return Err(format!(
                "a check is a mapping with one key — the check's name — and its arguments as the value, \
                 like `placement_near_wall: {{piece: bed_double, wall: north}}`. This is {}. The checks \
                 are: {}.",
                kind_of(value),
                CHECK_NAMES.join(", ")
            ));
        };
        if object.len() != 1 {
            return Err(format!(
                "a check names exactly one check and its arguments, and this names {}. The checks are: {}.",
                if object.is_empty() {
                    "none".to_owned()
                } else {
                    object.keys().cloned().collect::<Vec<_>>().join(", ")
                },
                CHECK_NAMES.join(", ")
            ));
        }
        let (name, args) = match object.iter().next() {
            Some((name, args)) => (name.as_str(), args),
            None => ("", &Value::Null),
        };
        let built = |what: Result<Check, String>| {
            what.map_err(|e| format!("`{name}`: {e}"))
        };
        match name {
            "placement_exists" => built(serde_json::from_value::<PlacementExists>(args.clone())
                .map(Check::PlacementExists)
                .map_err(|e| e.to_string())),
            "placement_at" => built(serde_json::from_value::<PlacementAt>(args.clone())
                .map(Check::PlacementAt)
                .map_err(|e| e.to_string())),
            "placement_near_wall" => built(serde_json::from_value::<PlacementNearWall>(args.clone())
                .map(Check::PlacementNearWall)
                .map_err(|e| e.to_string())),
            "placement_relative" => built(serde_json::from_value::<PlacementRelative>(args.clone())
                .map(Check::PlacementRelative)
                .map_err(|e| e.to_string())),
            "placement_rests_on" => built(serde_json::from_value::<PlacementRestsOn>(args.clone())
                .map(Check::PlacementRestsOn)
                .map_err(|e| e.to_string())),
            "unchanged" => built(serde_json::from_value::<Unchanged>(args.clone())
                .map(Check::Unchanged)
                .map_err(|e| format!("{e}; it is a list of paths under assets/grimoire/, like `[maps/bench_bedroom.map.yaml]`"))),
            "project_unchanged" => built(serde_json::from_value::<ProjectUnchanged>(args.clone())
                .map(Check::ProjectUnchanged)
                .map_err(|e| e.to_string())),
            "no_problem" => built(serde_json::from_value::<NoProblem>(args.clone())
                .map(Check::NoProblem)
                .map_err(|e| format!("{e}; it is a list of kinds, like `[overlap]`"))),
            "oplog_has" => built(serde_json::from_value::<OplogHas>(args.clone())
                .map(Check::OplogHas)
                .map_err(|e| e.to_string())),
            "oplog_lacks" => built(serde_json::from_value::<OplogLacks>(args.clone())
                .map(Check::OplogLacks)
                .map_err(|e| e.to_string())),
            "handback_contains" => built(serde_json::from_value::<HandbackContains>(args.clone())
                .map(Check::HandbackContains)
                .map_err(|e| format!("{e}; it is a list of facts, like `[north wall, centred]`"))),
            "handback_lacks" => built(serde_json::from_value::<HandbackLacks>(args.clone())
                .map(Check::HandbackLacks)
                .map_err(|e| format!("{e}; it is a list of facts, like `[answer: yes]`"))),
            "decide" => built(serde_json::from_value::<Decide>(args.clone())
                .map(Check::Decide)
                .map_err(|e| e.to_string())),
            "placement_total" => built(serde_json::from_value::<PlacementTotal>(args.clone())
                .map(Check::PlacementTotal)
                .map_err(|e| e.to_string())),
            "placements_within" => built(serde_json::from_value::<PlacementsWithin>(args.clone())
                .map(Check::PlacementsWithin)
                .map_err(|e| e.to_string())),
            "file_value" => built(serde_json::from_value::<FileValue>(args.clone())
                .map(Check::FileValue)
                .map_err(|e| e.to_string())),
            "terrain_height" => built(serde_json::from_value::<TerrainHeight>(args.clone())
                .map(Check::TerrainHeight)
                .map_err(|e| e.to_string())),
            "water_lake" => built(serde_json::from_value::<WaterLake>(args.clone())
                .map(Check::WaterLake)
                .map_err(|e| e.to_string())),
            other => Err(format!(
                "`{other}` is not a check this build has. The checks are: {}.",
                CHECK_NAMES.join(", ")
            )),
        }
    }
}

/// **What a value is**, for a refusal that says why a check did not read.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "nothing",
        Value::Bool(_) => "a yes/no value",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "a mapping",
    }
}

impl Check {
    /// **The variant's name**, which is what a report cites and what a task's file writes.
    pub fn name(&self) -> &'static str {
        match self {
            Check::PlacementExists(_) => "placement_exists",
            Check::PlacementAt(_) => "placement_at",
            Check::PlacementNearWall(_) => "placement_near_wall",
            Check::PlacementRelative(_) => "placement_relative",
            Check::PlacementRestsOn(_) => "placement_rests_on",
            Check::Unchanged(_) => "unchanged",
            Check::ProjectUnchanged(_) => "project_unchanged",
            Check::NoProblem(_) => "no_problem",
            Check::OplogHas(_) => "oplog_has",
            Check::OplogLacks(_) => "oplog_lacks",
            Check::HandbackContains(_) => "handback_contains",
            Check::HandbackLacks(_) => "handback_lacks",
            Check::Decide(_) => "decide",
            Check::PlacementTotal(_) => "placement_total",
            Check::PlacementsWithin(_) => "placements_within",
            Check::FileValue(_) => "file_value",
            Check::TerrainHeight(_) => "terrain_height",
            Check::WaterLake(_) => "water_lake",
        }
    }

    /// **The subject a placement check names**, or `None` for a check that is not about one.
    pub fn subject(&self) -> Option<PlacementRef> {
        match self {
            Check::PlacementExists(c) => Some(subject(&c.id, &c.piece)),
            Check::PlacementAt(c) => Some(subject(&c.id, &c.piece)),
            Check::PlacementNearWall(c) => Some(subject(&c.id, &c.piece)),
            Check::PlacementRelative(c) => Some(subject(&c.id, &c.piece)),
            Check::PlacementRestsOn(c) => Some(subject(&c.id, &c.piece)),
            _ => None,
        }
    }

    /// **Whether this check's answer comes from the map**, so a map that did not read fails it: every check
    /// but the byte comparisons, the hand-back and a harness decision reads the map.
    pub fn asks_about_the_map(&self) -> bool {
        !matches!(
            self,
            Check::Unchanged(_) | Check::ProjectUnchanged(_) | Check::OplogHas(_) | Check::OplogLacks(_)
                | Check::HandbackContains(_) | Check::HandbackLacks(_) | Check::Decide(_) | Check::FileValue(_)
                | Check::TerrainHeight(_) | Check::WaterLake(_)
        )
    }

    /// **The files this check names**, relative to `assets/grimoire/`, so the loader can refuse one naming a
    /// file nothing holds before any trial runs.
    pub fn files_named(&self) -> Vec<String> {
        match self {
            Check::Unchanged(u) => u.files.clone(),
            _ => Vec::new(),
        }
    }

    /// **What this check asks for, as a report states it** — never a value that would pass on its own.
    pub fn asks(&self) -> String {
        match self {
            Check::PlacementExists(e) => {
                format!("{} × {} on the map", e.count, subject(&e.id, &e.piece).shown())
            }
            Check::PlacementAt(a) => {
                let mut parts = Vec::new();
                if let Some(at) = a.at {
                    parts.push(format!(
                        "at ({:.3}, {:.3}) ±{:.3} m",
                        at.0,
                        at.1,
                        a.tolerance_m.unwrap_or(0.0)
                    ));
                }
                if let Some(yaw) = a.yaw_deg {
                    parts.push(format!(
                        "facing {yaw:.1}° ±{:.2}°",
                        a.yaw_tolerance_deg.unwrap_or(0.0)
                    ));
                }
                if let Some(lift) = a.lift_m {
                    parts.push(format!(
                        "lifted {lift:.3} m ±{:.3} m",
                        a.lift_tolerance_m.unwrap_or(0.0)
                    ));
                }
                format!("{} {}", subject(&a.id, &a.piece).shown(), parts.join(" and "))
            }
            Check::PlacementNearWall(w) => {
                let centre = w
                    .centre_within_m
                    .map(|m| format!(", centred within {m:.3} m"))
                    .unwrap_or_default();
                format!(
                    "{} against the {} wall within {:.3} m{centre}",
                    subject(&w.id, &w.piece).shown(),
                    w.wall.name(),
                    w.max_gap_m
                )
            }
            Check::PlacementRelative(r) => format!(
                "{} {} of {} within {:.3} m",
                subject(&r.id, &r.piece).shown(),
                r.direction.name(),
                r.other.subject().shown(),
                r.max_gap_m
            ),
            Check::PlacementRestsOn(r) => {
                let host = match (&r.host_piece, r.host.is_empty()) {
                    (Some(piece), true) => format!("a `{piece}`"),
                    (Some(piece), false) => format!("`{}` (a `{piece}`)", r.host),
                    (None, _) => format!("`{}`", r.host),
                };
                format!("{} resting on {host}", subject(&r.id, &r.piece).shown())
            }
            Check::PlacementTotal(t) => format!("{} placement(s) on the map in all", t.count),
            Check::PlacementsWithin(w) => format!(
                "{} × a `{}`, all inside x {:.2}..{:.2}, z {:.2}..{:.2}",
                w.count, w.piece, w.x.0, w.x.1, w.z.0, w.z.1
            ),
            Check::FileValue(f) => {
                let mut wants = Vec::new();
                if let Some(v) = &f.equals {
                    wants.push(format!("= {v}"));
                }
                if let Some(m) = f.min {
                    wants.push(format!("≥ {m}"));
                }
                if let Some(m) = f.max {
                    wants.push(format!("≤ {m}"));
                }
                if let Some(w) = &f.contains {
                    wants.push(format!("contains `{w}`"));
                }
                if let Some(w) = &f.lacks {
                    wants.push(format!("lacks `{w}`"));
                }
                format!("`{}` in `{}` {}", f.path, f.file, wants.join(" and "))
            }
            Check::TerrainHeight(t) => {
                let mut wants = Vec::new();
                if let Some(m) = t.min_m {
                    wants.push(format!("at least {m:.2} m"));
                }
                if let Some(m) = t.max_m {
                    wants.push(format!("at most {m:.2} m"));
                }
                format!(
                    "the ground of `{}` at ({:.2}, {:.2}) {}",
                    t.map,
                    t.at.0,
                    t.at.1,
                    wants.join(" and ")
                )
            }
            Check::WaterLake(w) => format!(
                "{} lake(s) on `{}`, centred inside x {:.2}..{:.2}, z {:.2}..{:.2}{}",
                w.count,
                w.map,
                w.x.0,
                w.x.1,
                w.z.0,
                w.z.1,
                w.min_radius_m.map(|r| format!(", each at least {r:.2} m wide")).unwrap_or_default()
            ),
            Check::Unchanged(u) => format!("{} unchanged", u.files.join(", ")),
            Check::ProjectUnchanged(_) => "the whole project unchanged".to_owned(),
            Check::NoProblem(p) => format!(
                "no {}",
                p.kinds.iter().map(|k| k.name()).collect::<Vec<_>>().join(" and no ")
            ),
            Check::OplogHas(o) => format!("at least {} applied `{}` line(s)", o.min, o.op),
            Check::OplogLacks(o) => format!("no applied `{}` line", o.op),
            Check::HandbackContains(h) => format!("a hand-back stating {}", h.facts.join(", ")),
            Check::HandbackLacks(h) => format!("a hand-back stating none of {}", h.facts.join(", ")),
            Check::Decide(d) => format!(
                "the decision `{}` answers {} with confidence ≥ {:.2}",
                d.check, d.want, d.min_confidence
            ),
        }
    }
}

/// **One check's verdict**, with the value it measured and, when it failed, why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    /// The check's variant name.
    pub check: String,
    /// What it asked for.
    pub asks: String,
    /// Whether it held.
    pub passed: bool,
    /// **What it measured**, in a sentence a person reads: the gap in metres, the count, the missing fact.
    /// A verdict without one says a check failed without saying what the run did.
    pub measured: String,
}

/// **Grade one check against what the setup left and what the run left.**
pub fn grade(check: &Check, baseline: &Snapshot, left: &Snapshot) -> Verdict {
    let verdict = |passed: bool, measured: String| Verdict {
        check: check.name().to_owned(),
        asks: check.asks(),
        passed,
        measured,
    };
    // **A map that does not read fails every check about the map, naming why.** See `Snapshot::map_problem`:
    // a default map would pass `no_problem` and read as "nothing placed" for a placement, which are the two
    // answers a person most needs told apart from "the run broke the project".
    if let Some(problem) = &left.map_problem
        && check.asks_about_the_map()
    {
        return verdict(
            false,
            format!("the map did not read, so this could not be measured: {problem}"),
        );
    }
    match check {
        Check::PlacementExists(e) => {
            let subject = subject(&e.id, &e.piece);
            if let Some(wrong) = subject.wrong() {
                return verdict(false, format!("the check {wrong}"));
            }
            let found = subject.matches(&left.map);
            verdict(
                found.len() == e.count,
                format!(
                    "{} × {} on the map, and the task wants {}",
                    found.len(),
                    subject.shown(),
                    e.count
                ),
            )
        }
        Check::PlacementAt(a) => {
            let subject = subject(&a.id, &a.piece);
            if let Some(wrong) = subject.wrong() {
                return verdict(false, format!("the check {wrong}"));
            }
            if a.at.is_none() && a.yaw_deg.is_none() && a.lift_m.is_none() {
                return verdict(
                    false,
                    "the check asks for no spot, facing or lift, so it would pass on any map".to_owned(),
                );
            }
            let found = subject.matches(&left.map);
            let Some(p) = found.first() else {
                return verdict(false, format!("no {} is on the map", subject.shown()));
            };
            let mut fails: Vec<String> = Vec::new();
            let mut says: Vec<String> = Vec::new();
            if let Some(at) = a.at {
                let tolerance = a.tolerance_m.unwrap_or(0.0);
                let off = ((p.at.0 - at.0).powi(2) + (p.at.1 - at.1).powi(2)).sqrt();
                says.push(format!(
                    "its centre is at ({:.3}, {:.3}), {off:.3} m from ({:.3}, {:.3})",
                    p.at.0, p.at.1, at.0, at.1
                ));
                if off > tolerance {
                    fails.push(format!("off by {off:.3} m, more than {tolerance:.3} m"));
                }
            }
            if let Some(yaw) = a.yaw_deg {
                let tolerance = a.yaw_tolerance_deg.unwrap_or(0.0);
                // **The shortest way round the circle**: 350° and 10° are 20° apart, not 340°.
                let off = ((p.yaw - yaw).rem_euclid(360.0)).min((yaw - p.yaw).rem_euclid(360.0));
                says.push(format!("it faces {:.1}°, {off:.2}° from {yaw:.1}°", p.yaw));
                if off > tolerance {
                    fails.push(format!("facing is off by {off:.2}°, more than {tolerance:.2}°"));
                }
            }
            if let Some(lift) = a.lift_m {
                let tolerance = a.lift_tolerance_m.unwrap_or(0.0);
                let off = (p.lift - lift).abs();
                says.push(format!(
                    "it is lifted {:.3} m, {off:.3} m from {lift:.3} m",
                    p.lift
                ));
                if off > tolerance {
                    fails.push(format!("lift is off by {off:.3} m, more than {tolerance:.3} m"));
                }
            }
            if fails.is_empty() {
                verdict(true, says.join("; "))
            } else {
                verdict(false, format!("{} — {}", says.join("; "), fails.join("; ")))
            }
        }
        Check::PlacementNearWall(w) => grade_near_wall(w, &left.map, &left.library, &verdict),
        Check::PlacementRelative(r) => grade_relative(r, &left.map, &left.library, &verdict),
        Check::PlacementRestsOn(r) => {
            let subject = subject(&r.id, &r.piece);
            if r.host.is_empty() && r.host_piece.is_none() {
                return verdict(false, "the check names neither a `host` nor a `host_piece`".to_owned());
            }
            let found = subject.matches(&left.map);
            let Some(p) = found.first() else {
                return verdict(false, format!("no {} is on the map", subject.shown()));
            };
            let wanted = match (&r.host_piece, r.host.is_empty()) {
                (Some(piece), true) => format!("a `{piece}`"),
                (Some(piece), false) => format!("`{}` (a `{piece}`)", r.host),
                (None, _) => format!("`{}`", r.host),
            };
            let is_wanted = |host: &str| {
                (r.host.is_empty() || host == r.host)
                    && r.host_piece.as_ref().is_none_or(|piece| {
                        left.map.placements.iter().any(|q| q.id == host && &q.descriptor == piece)
                    })
            };
            match &p.on {
                Some(host) if is_wanted(host) => verdict(true, format!("`{}` rests on `{host}`", p.id)),
                Some(host) => verdict(
                    false,
                    format!("`{}` rests on `{host}`, and the task wants {wanted}", p.id),
                ),
                None => verdict(
                    false,
                    format!("`{}` rests on the floor, and the task wants it on {wanted}", p.id),
                ),
            }
        }
        Check::Unchanged(u) => {
            let mut differs: Vec<String> = Vec::new();
            let mut same = 0usize;
            for rel in &u.files {
                let path = format!("assets/grimoire/{rel}");
                match (baseline.digest(&path), left.digest(&path)) {
                    (Some(before), Some(after)) if before == after => same += 1,
                    (Some(_), Some(_)) => differs.push(format!("`{rel}` changed")),
                    (Some(_), None) => differs.push(format!("`{rel}` was removed")),
                    (None, _) => differs.push(format!("`{rel}` was not in the setup's project")),
                }
            }
            verdict(
                differs.is_empty(),
                if differs.is_empty() {
                    format!("{same} file(s) byte-identical to the setup")
                } else {
                    differs.join("; ")
                },
            )
        }
        Check::ProjectUnchanged(_) => {
            let changed = baseline.differences(left);
            verdict(
                changed.is_empty(),
                if changed.is_empty() {
                    format!("all {} file(s) byte-identical to the setup", baseline.files.len())
                } else {
                    format!("{} file(s) changed: {}", changed.len(), changed.join(", "))
                },
            )
        }
        Check::NoProblem(p) => {
            if p.kinds.is_empty() {
                return verdict(
                    false,
                    "the check names no problem kinds, so it would pass on any map".to_owned(),
                );
            }
            let mut held: Vec<String> = Vec::new();
            for kind in &p.kinds {
                let count = kind.count(&left.found);
                if count == 0 {
                    held.push(format!("no {}", kind.name()));
                } else {
                    let first = kind.first(&left.found).unwrap_or_default();
                    held.push(format!("{count} {}, first {first}", kind.name()));
                }
            }
            verdict(p.kinds.iter().all(|k| k.count(&left.found) == 0), held.join("; "))
        }
        Check::OplogHas(o) => {
            let hits = left.oplog.iter().filter(|l| l.op == o.op && l.applied()).count();
            verdict(
                hits >= o.min,
                format!("{hits} applied `{}` line(s), and the task wants at least {}", o.op, o.min),
            )
        }
        Check::OplogLacks(o) => {
            let hits = left.oplog.iter().filter(|l| l.op == o.op && l.applied()).count();
            verdict(hits == 0, format!("{hits} applied `{}` line(s)", o.op))
        }
        Check::HandbackContains(h) => {
            let Some(handback) = left.handbacks.first() else {
                return verdict(false, "no hand-back was made".to_owned());
            };
            let text = handback.text().to_lowercase();
            let missing: Vec<&String> = h
                .facts
                .iter()
                .filter(|f| !text.contains(&f.to_lowercase()))
                .collect();
            verdict(
                missing.is_empty(),
                if missing.is_empty() {
                    format!("the hand-back `{}` states {} fact(s)", handback.title, h.facts.len())
                } else {
                    let says: Vec<String> = missing.iter().map(|f| format!("`{f}`")).collect();
                    format!(
                        "the hand-back `{}` does not state {}",
                        handback.title,
                        says.join(", ")
                    )
                },
            )
        }
        Check::HandbackLacks(h) => {
            let Some(handback) = left.handbacks.first() else {
                return verdict(false, "no hand-back was made".to_owned());
            };
            let text = handback.text().to_lowercase();
            let said: Vec<&String> = h.facts.iter().filter(|f| text.contains(&f.to_lowercase())).collect();
            verdict(
                said.is_empty(),
                if said.is_empty() {
                    format!("the hand-back `{}` states none of the {} wrong fact(s)", handback.title, h.facts.len())
                } else {
                    let says: Vec<String> = said.iter().map(|f| format!("`{f}`")).collect();
                    format!("the hand-back `{}` states {}", handback.title, says.join(", "))
                },
            )
        }
        Check::PlacementTotal(t) => {
            let n = left.map.placements.len();
            verdict(n == t.count, format!("{n} placement(s) on the map, and the task wants {}", t.count))
        }
        Check::PlacementsWithin(w) => {
            let of: Vec<&Placed> =
                left.map.placements.iter().filter(|p| p.descriptor == w.piece).collect();
            let outside: Vec<String> = of
                .iter()
                .filter(|p| {
                    !((w.x.0..=w.x.1).contains(&p.at.0) && (w.z.0..=w.z.1).contains(&p.at.1))
                })
                .map(|p| format!("`{}` at ({:.2}, {:.2})", p.id, p.at.0, p.at.1))
                .collect();
            let mut says = format!(
                "{} × a `{}` on the map, {} of them inside x {:.2}..{:.2}, z {:.2}..{:.2}, and the task wants {} all inside",
                of.len(),
                w.piece,
                of.len() - outside.len(),
                w.x.0,
                w.x.1,
                w.z.0,
                w.z.1,
                w.count
            );
            if !outside.is_empty() {
                says.push_str(&format!("; outside: {}", outside.join(", ")));
            }
            verdict(of.len() == w.count && outside.is_empty(), says)
        }
        Check::FileValue(f) => grade_file_value(f, left, &verdict),
        Check::TerrainHeight(t) => grade_terrain_height(t, left, &verdict),
        Check::WaterLake(w) => grade_water_lake(w, left, &verdict),
        Check::Decide(d) => {
            if !(0.0..=1.0).contains(&d.min_confidence) {
                return verdict(
                    false,
                    format!(
                        "the check's `min_confidence` {} is outside 0..=1, so it could never pass",
                        d.min_confidence
                    ),
                );
            }
            let Some(answer) = left.decisions.get(&d.check) else {
                return verdict(false, format!("no decision `{}` was made", d.check));
            };
            let passed = answer.answer == d.want && answer.confidence >= d.min_confidence;
            verdict(
                passed,
                format!(
                    "the decision `{}` answered {} at confidence {:.3}, and the task wants {} at ≥ {:.2}",
                    d.check, answer.answer, answer.confidence, d.want, d.min_confidence
                ),
            )
        }
    }
}

/// The near-wall arithmetic: the piece's own extent along the wall's normal, against the map's rectangle.
fn grade_near_wall(
    w: &PlacementNearWall,
    map: &Map,
    library: &Library,
    verdict: &impl Fn(bool, String) -> Verdict,
) -> Verdict {
    let subject = subject(&w.id, &w.piece);
    if let Some(wrong) = subject.wrong() {
        return verdict(false, format!("the check {wrong}"));
    }
    let found = subject.matches(map);
    let Some(p) = found.first() else {
        return verdict(false, format!("no {} is on the map", subject.shown()));
    };
    let Some(d) = bevy_grimoire_core::stack::descriptor_of(library, p) else {
        return verdict(
            false,
            format!("`{}`'s piece `{}` is not in this project's library", p.id, p.descriptor),
        );
    };
    let Some((half_x, half_z)) = half_extents(&d, p) else {
        return verdict(
            false,
            format!(
                "`{}`'s piece `{}` has no measured footprint, so its edge cannot be found",
                p.id, p.descriptor
            ),
        );
    };
    let (hx, hz) = (map.bounds.0 * 0.5, map.bounds.2 * 0.5);
    // **The gap between the piece's own edge and the wall, in metres.** Positive is clearance, zero is
    // flush, negative is through the wall.
    let (gap, centre_offset) = match w.wall {
        Wall::North => (p.at.1 - half_z + hz, p.at.0),
        Wall::South => (hz - (p.at.1 + half_z), p.at.0),
        Wall::West => (p.at.0 - half_x + hx, p.at.1),
        Wall::East => (hx - (p.at.0 + half_x), p.at.1),
    };
    let mut says = format!(
        "`{}` is {gap:.3} m from the {} wall{}",
        p.id,
        w.wall.name(),
        if gap < 0.0 { " (through it)" } else { "" }
    );
    // **A gap is signed, and only a positive one is clearance.** A negative gap is the piece through the
    // wall, and `abs()` here would have accepted a bed sunk `max_gap_m` into it — measured on the review of
    // #411: with `max_gap_m: 0.05` a bed 3 cm inside the north wall passed. `grade_relative` has always
    // been right; this is the same range.
    let mut passed = (0.0..=w.max_gap_m).contains(&gap);
    if !passed {
        says.push_str(&format!(", and the task wants within {:.3} m", w.max_gap_m));
    }
    if let Some(allowed) = w.centre_within_m {
        let off = centre_offset.abs();
        says.push_str(&format!(" and {off:.3} m off the middle of it"));
        if off > allowed {
            says.push_str(&format!(", and the task wants within {allowed:.3} m"));
            passed = false;
        }
    }
    verdict(passed, says)
}

/// The relation arithmetic: edge-to-edge along one axis, between two pieces' own footprints.
fn grade_relative(
    r: &PlacementRelative,
    map: &Map,
    library: &Library,
    verdict: &impl Fn(bool, String) -> Verdict,
) -> Verdict {
    let mine = subject(&r.id, &r.piece);
    let theirs = r.other.subject();
    for one in [&mine, &theirs] {
        if let Some(wrong) = one.wrong() {
            return verdict(false, format!("the check {wrong}"));
        }
    }
    let found = mine.matches(map);
    let Some(p) = found.first() else {
        return verdict(false, format!("no {} is on the map", mine.shown()));
    };
    let others = theirs.matches(map);
    let Some(q) = others.first() else {
        return verdict(false, format!("no {} is on the map", theirs.shown()));
    };
    let (Some(d_p), Some(d_q)) = (
        bevy_grimoire_core::stack::descriptor_of(library, p),
        bevy_grimoire_core::stack::descriptor_of(library, q),
    ) else {
        return verdict(
            false,
            "one of the two pieces is not in this project's library".to_owned(),
        );
    };
    let (Some((px, pz)), Some((qx, qz))) = (half_extents(&d_p, p), half_extents(&d_q, q)) else {
        return verdict(
            false,
            "one of the two pieces has no measured footprint".to_owned(),
        );
    };
    let (axis, larger) = r.direction.axis();
    // **Edge to edge**, so two pieces touching are 0 m apart however wide either is.
    let gap = match axis {
        Axis::X if larger => (p.at.0 - px) - (q.at.0 + qx),
        Axis::X => (q.at.0 - qx) - (p.at.0 + px),
        Axis::Z if larger => (p.at.1 - pz) - (q.at.1 + qz),
        Axis::Z => (q.at.1 - qz) - (p.at.1 + pz),
    };
    let passed = (0.0..=r.max_gap_m).contains(&gap);
    let where_ = match axis {
        Axis::X => format!("x {:.3}", p.at.0),
        Axis::Z => format!("z {:.3}", p.at.1),
    };
    verdict(
        passed,
        format!(
            "`{}` is {gap:.3} m {} of `{}` along {} (at {where_}), and the task wants 0 to {:.3} m",
            p.id,
            r.direction.name(),
            q.id,
            axis.name(),
            r.max_gap_m
        ),
    )
}

/// **One step along a path**: a key, or a list index.
enum Step<'a> {
    Key(&'a str),
    Index(usize),
}

/// **A path like `gate.health`, `bounds[0]` or `lakes[1].level_m`, as its steps**, or why it does not read.
fn path_steps(path: &str) -> Result<Vec<Step<'_>>, String> {
    let mut steps = Vec::new();
    for part in path.split('.') {
        let (key, mut rest) = match part.find('[') {
            Some(open) => (&part[..open], &part[open..]),
            None => (part, ""),
        };
        if key.is_empty() && rest.is_empty() {
            return Err(format!("`{path}` has an empty step"));
        }
        if !key.is_empty() {
            steps.push(Step::Key(key));
        }
        while let Some(after) = rest.strip_prefix('[') {
            let Some(close) = after.find(']') else {
                return Err(format!("`{path}` opens a `[` and never closes it"));
            };
            let index = after[..close]
                .parse::<usize>()
                .map_err(|_| format!("`{path}`: `{}` is not a list index", &after[..close]))?;
            steps.push(Step::Index(index));
            rest = &after[close + 1..];
        }
        if !rest.is_empty() {
            return Err(format!("`{path}` has `{rest}` after a step it cannot read"));
        }
    }
    Ok(steps)
}

/// **How far apart two numbers may be and still be the same value**, in the number's own unit. A YAML
/// scalar written `40` and one written `40.0` are one number, and the float a parser reads is never
/// exact to the last place; nothing a task asks about is finer than a billionth.
const SCALAR_EPSILON: f64 = 1e-9;

/// **Whether two scalars are the same value**: numbers by value (so `40` and `40.0` agree), everything
/// else as written.
fn same_scalar(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < SCALAR_EPSILON,
        _ => a == b,
    }
}

/// The leaf-of-a-file arithmetic: read the file as data, walk the path, compare.
fn grade_file_value(
    f: &FileValue,
    left: &Snapshot,
    verdict: &impl Fn(bool, String) -> Verdict,
) -> Verdict {
    if f.equals.is_none() && f.min.is_none() && f.max.is_none() && f.contains.is_none() && f.lacks.is_none() {
        return verdict(
            false,
            "the check states no value, `min`, `max`, `contains` or `lacks`, so it would pass on any file".to_owned(),
        );
    }
    // **An empty word is in every string**, so a `contains` of it would pass on any leaf and a `lacks` of it on none.
    if f.contains.as_deref().is_some_and(str::is_empty) || f.lacks.as_deref().is_some_and(str::is_empty) {
        return verdict(false, "the check's `contains` or `lacks` is an empty word, which every string holds".to_owned());
    }
    let Some(text) = left.text(&f.file) else {
        return verdict(false, format!("`{}` is not in the project, so nothing could be read from it", f.file));
    };
    let data: Value = match bevy_grimoire_core::yaml::from_str(&text, &f.file) {
        Ok(data) => data,
        Err(p) => return verdict(false, format!("`{}` did not read: {}", f.file, p.line())),
    };
    let steps = match path_steps(&f.path) {
        Ok(steps) => steps,
        Err(why) => return verdict(false, format!("the check's path does not read: {why}")),
    };
    let mut at = &data;
    for step in &steps {
        let next = match step {
            Step::Key(key) => at.get(*key),
            Step::Index(i) => at.get(*i),
        };
        match next {
            Some(next) => at = next,
            None => {
                return verdict(false, format!("`{}` holds nothing at `{}`", f.file, f.path));
            }
        }
    }
    let mut fails: Vec<String> = Vec::new();
    if let Some(want) = &f.equals
        && !same_scalar(at, want)
    {
        fails.push(format!("it should be {want}"));
    }
    if f.min.is_some() || f.max.is_some() {
        match at.as_f64() {
            Some(n) => {
                if let Some(min) = f.min
                    && n < min
                {
                    fails.push(format!("it should be at least {min}"));
                }
                if let Some(max) = f.max
                    && n > max
                {
                    fails.push(format!("it should be at most {max}"));
                }
            }
            None => fails.push("it is not a number".to_owned()),
        }
    }
    if f.contains.is_some() || f.lacks.is_some() {
        match at.as_str() {
            Some(string) => {
                let lower = string.to_lowercase();
                if let Some(word) = &f.contains
                    && !lower.contains(&word.to_lowercase())
                {
                    fails.push(format!("it should contain `{word}`"));
                }
                if let Some(word) = &f.lacks
                    && lower.contains(&word.to_lowercase())
                {
                    fails.push(format!("it should lack `{word}`"));
                }
            }
            None => fails.push("it is not a string".to_owned()),
        }
    }
    let says = format!("`{}` in `{}` is {at}", f.path, f.file);
    if fails.is_empty() {
        verdict(true, says)
    } else {
        verdict(false, format!("{says}, and {}", fails.join(" and ")))
    }
}

/// The ground arithmetic: the core's own sampling of the saved heightfield.
fn grade_terrain_height(
    t: &TerrainHeight,
    left: &Snapshot,
    verdict: &impl Fn(bool, String) -> Verdict,
) -> Verdict {
    if t.min_m.is_none() && t.max_m.is_none() {
        return verdict(false, "the check states no bound, so it would pass on any ground".to_owned());
    }
    let file = format!("{AUTHORING_KIT}/terrain/{}.yaml", t.map);
    let Some(text) = left.text(&file) else {
        return verdict(
            false,
            format!("`{file}` is not in the project, so the ground of `{}` is not shaped", t.map),
        );
    };
    let field = match bevy_grimoire_core::heightfield::Heightfield::parse(&text) {
        Ok(field) => field,
        Err(why) => return verdict(false, format!("`{file}` did not read: {why}")),
    };
    let height = field.height_at(t.at.0, t.at.1);
    let says = format!("the ground of `{}` at ({:.2}, {:.2}) is {height:.3} m", t.map, t.at.0, t.at.1);
    let mut fails: Vec<String> = Vec::new();
    if let Some(min) = t.min_m
        && height < min
    {
        fails.push(format!("the task wants at least {min:.3} m"));
    }
    if let Some(max) = t.max_m
        && height > max
    {
        fails.push(format!("the task wants at most {max:.3} m"));
    }
    if fails.is_empty() {
        verdict(true, says)
    } else {
        verdict(false, format!("{says}, and {}", fails.join(" and ")))
    }
}

/// The lake arithmetic: the saved water plan's lakes, by centre and radius.
fn grade_water_lake(
    w: &WaterLake,
    left: &Snapshot,
    verdict: &impl Fn(bool, String) -> Verdict,
) -> Verdict {
    let file = format!("{AUTHORING_KIT}/water/{}.yaml", w.map);
    let Some(text) = left.text(&file) else {
        return verdict(false, format!("`{file}` is not in the project, so `{}` has no water", w.map));
    };
    let plan = match bevy_grimoire_core::water::WaterPlan::parse(&text) {
        Ok(plan) => plan,
        Err(why) => return verdict(false, format!("`{file}` did not read: {why}")),
    };
    let mut fails: Vec<String> = Vec::new();
    let mut says: Vec<String> = Vec::new();
    for (i, lake) in plan.lakes.iter().enumerate() {
        says.push(format!(
            "lake {i} at ({:.3}, {:.3}), radius {:.3} m, level {:.3} m",
            lake.x, lake.z, lake.radius_m, lake.level_m
        ));
        if !((w.x.0..=w.x.1).contains(&lake.x) && (w.z.0..=w.z.1).contains(&lake.z)) {
            fails.push(format!("lake {i} is outside x {:.2}..{:.2}, z {:.2}..{:.2}", w.x.0, w.x.1, w.z.0, w.z.1));
        }
        if let Some(min) = w.min_radius_m
            && lake.radius_m < min
        {
            fails.push(format!("lake {i} is under {min:.3} m wide"));
        }
        if let Some((least, most)) = w.level_above_ground_m {
            let terrain = format!("{AUTHORING_KIT}/terrain/{}.yaml", w.map);
            // **A map with no terrain file is flat at 0**, which is what the editor draws; **a terrain file that is there and does not read
            // is not**, and reading it as flat ground would pass or fail a lake against ground that is not the map's. It is a refusal.
            let ground = match left.text(&terrain) {
                Some(text) => match bevy_grimoire_core::heightfield::Heightfield::parse(&text) {
                    Ok(field) => field.height_at(lake.x, lake.z),
                    Err(why) => {
                        fails.push(format!("`{terrain}` did not read, so lake {i} cannot be set against the ground: {why}"));
                        continue;
                    }
                },
                None if left.digest(&terrain).is_some() => {
                    fails.push(format!("`{terrain}` is in the project but cannot be read, so lake {i} cannot be set against the ground"));
                    continue;
                }
                None => 0.0,
            };
            let above = lake.level_m - ground;
            says.push(format!("lake {i} stands {above:.3} m over the ground at its centre"));
            if !(least..=most).contains(&above) {
                fails.push(format!("lake {i} stands {above:.3} m over the ground, and the task wants {least:.2}..{most:.2} m"));
            }
        }
    }
    if plan.lakes.len() != w.count {
        fails.push(format!("the task wants {} lake(s)", w.count));
    }
    let says = if says.is_empty() { format!("`{file}` holds no lake") } else { says.join("; ") };
    if fails.is_empty() {
        verdict(true, says)
    } else {
        verdict(false, format!("{says}, and {}", fails.join(" and ")))
    }
}

/// **A placement's extent along the map's `x` and `z` axes, in metres from its centre** — its footprint
/// turned by its yaw and folded for its tip.
///
/// This is the grader's own arithmetic, not `stack::plan_box`'s: the check exists to be an independent
/// measurement, so it derives the half-extents from the descriptor and the yaw directly.
pub fn half_extents(d: &Descriptor, p: &Placed) -> Option<(f32, f32)> {
    let (w, depth) = bevy_grimoire_core::descriptor::tipped_footprint(d, p.tip)?;
    let (s, c) = p.yaw.to_radians().sin_cos();
    let (hw, hd) = (w * 0.5, depth * 0.5);
    Some((hw * c.abs() + hd * s.abs(), hw * s.abs() + hd * c.abs()))
}
