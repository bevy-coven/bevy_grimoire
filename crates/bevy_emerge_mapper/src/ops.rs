//! **The one place an edit is defined** — and the one place an edit is applied.
//!
//! # Why this module exists
//!
//! The editor had **two ways to change the same thing**, and they were not the same way:
//!
//! - a **key handler** in `editor.rs` / `tiles.rs` / `compose.rs`, which read the keyboard, read
//!   some state, wrote some state, and put a line in the status bar;
//! - a **file on disk**, which the editor read once, at open, and never again.
//!
//! So an **agent** — which reaches the editor over BRP — could not make an edit at all, because
//! the only door was a key handler and an agent has no keyboard. And a **developer** who edited a
//! data file in their editor saw nothing happen until they restarted the editor, which is the
//! slowest possible feedback loop for the person who is trying to see what a change looks like.
//!
//! **This module is the third door, and the only one that is the definition.** An operation is
//! named once, validated once, applied once, and undone once — and every front end (a key
//! handler, a BRP command, an MCP tool) is a *thin* front end that builds the operation's
//! parameters and dispatches it. **No front end changes editor state any other way**, and
//! `tests/operations_have_two_front_ends.rs` says so.
//!
//! # The shape
//!
//! An operation is three things:
//!
//! | | what it is | why it is separate |
//! |---|---|---|
//! | **parameters** | a typed struct, with a JSON schema | so a key handler and an agent send the *same* thing, and a typo is a refusal rather than a silent no-op |
//! | **plan** | a pure function: what would change | so **preview** is free — a plan that changes nothing is the preview |
//! | **commit** | the one function that writes | so there is exactly one place a change is made, and it is not in a front end |
//!
//! **And the undo is declared, not inferred.** Each operation says how it is undone: an inverse
//! delta for a cheap edit, a snapshot for a procedural one. The editor's own `Undo` stack is a
//! *different* thing — it is the author's undo, at the keyboard — and the operation's undo
//! strategy is what an agent's `emerge_undo` uses.
//!
//! # The operation log
//!
//! **One append-only file, and it is the source of truth.** Every operation — from a key handler
//! or from an agent — appends one line to `assets/emerge/oplog.jsonl`. The line records who did
//! it (a human or an agent, and which session), what it was, what it was called with, what came
//! back, and — where the operation is procedural — the seed and the rules it ran under.
//!
//! **Undo is a new line, never a deletion.** The log is append-only, so the fact that something
//! was undone is itself recorded; an agent can read what the human just did, and a session can be
//! replayed.
//!
//! # What is not here yet
//!
//! **This is the first slice, and the slice is small.** One workflow — a map's own settings —
//! goes through the registry, with both front ends. The rest of the editor's verbs still change
//! state directly, and `tests/operations_have_two_front_ends.rs` carries an explicit allowlist of
//! them, which later iterations shrink. **The allowlist is the work list**, and a verb leaving it
//! is the point of the exercise.

use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::project::{OpenMap, Project};

/// **A metre count with no trailing `.0`** — `32` rather than `32.0`, `3.5` as itself.
///
/// **It lives here, and not in the editor**, because the receipt that uses it is written by the
/// operation, and a receipt's wording is part of the operation's contract.
pub fn trim_metres(m: f32) -> String {
    if (m - m.round()).abs() < 1e-4 {
        format!("{m:.0}")
    } else {
        format!("{m:.1}")
    }
}

/// **Where the operation log lives** — in the project's own directory.
///
/// **At the project level, not inside a kit's.** The log records what was done to *the project* —
/// to its maps, its compositions, its vocabulary — and a project binds several kits. **A log
/// inside the authoring kit's directory would be lost when the authoring kit changed**, and a
/// project whose log moves when the author switches kits is not a log the author can rely on.
///
/// **In the project directory, not the user's home.** The log is about *this* project: an agent
/// working on a game needs to know what was done to that game's map, and a log in a shared
/// location would answer a different question.
pub fn oplog_path(project: &Project) -> PathBuf {
    oplog_path_at(&project.root)
}

/// **The log's path from a project root alone**, so a change made before a door is open — the menu's
/// `map_create` — records itself in the same file the editor's changes do.
pub fn oplog_path_at(root: &std::path::Path) -> PathBuf {
    root.join("assets/emerge").join("oplog.jsonl")
}

/// **Who did it.**
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A person at the keyboard.
    #[default]
    Human,
    /// An agent, over BRP or MCP.
    Agent,
}

/// **One line of the operation log.**
///
/// **Every field earns its place, and the two that look like bookkeeping are the ones an agent
/// cannot work without.** `operation_id` is what the agent cites when it presents a change and
/// what the human reverts by; `session` is what tells two agents' work apart in a log they both
/// read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// **Monotonic, from the log itself.** The count of lines before this one, so a reader can
    /// ask for "everything after what I saw" without a timestamp comparison.
    pub seq: u64,
    /// **What the agent cites and the human reverts by.** Unique within a project.
    pub operation_id: String,
    /// **When it happened**, RFC 3339, UTC.
    pub at: String,
    /// **Who did it.**
    pub source: Source,
    /// **Which session.** The editor's own session, or the agent's.
    pub session: String,
    /// **The operation's name** — the same name the registry and the MCP tool use.
    pub op: String,
    /// **What it was called with**, verbatim. So a replay can send the same call.
    pub params: serde_json::Value,
    /// **What came back**: `applied` or `refused`. An undo is an `applied` line whose `undoes`
    /// names the line it took back.
    pub outcome: String,
    /// **The refusal's text**, when the outcome is `refused`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// **A one-line summary of the result**, for a reader who does not want to replay it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// **The seed, where the operation was procedural.** `None` for a deterministic edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rng_seed: Option<u64>,
    /// **The rules hash, where the operation ran under rules.** `None` for a deterministic edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_hash: Option<String>,
    /// **The operation this one undid**, when it is an undo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undoes: Option<String>,
    /// **The parameters that put this line's change back**, for the same operation.
    ///
    /// **Recorded with the change, because only the change knows what it replaced.** An undo
    /// read from the log afterwards cannot reconstruct the old values from the new ones. Lines
    /// written before this field existed have none, and [`undo`] refuses them by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse: Option<serde_json::Value>,
    /// **The state this line left behind**, for the guard [`undo`] runs. See [`Outcome::after`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<serde_json::Value>,
}

/// **The session this editor is running under.**
///
/// **One per launch, and it is what tells two agents' work apart in a log they both read.** The
/// id is the process id and the start time, which is enough to be unique among the editors
/// running on one machine and is free — no dependency, no clock call at every operation.
///
/// **It lives here, and not in the BRP bridge**, because the keyboard's own edits are in the log
/// too, and a session id that only the agent's door could produce would leave the person's
/// work without one.
pub fn session_id() -> String {
    format!("{}-{}", std::process::id(), editor_start())
}

/// **When this editor started, in seconds since the epoch.**
///
/// **Computed once and kept**, because the session id has to be stable for the editor's life and
/// a per-call clock read would make it a different id every time.
fn editor_start() -> u64 {
    use std::sync::LazyLock;
    static START: LazyLock<u64> = LazyLock::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    *START
}

/// **How an operation is undone.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undo {
    /// **Nothing to undo** — the operation only read.
    None,
    /// **The inverse of the edit**, which is enough to put it back.
    ///
    /// For an edit that changed one field of one thing, the inverse is the field's old value.
    /// Cheap, exact, and it does not need the whole map.
    Inverse,
    /// **A snapshot of what was there**, put back whole.
    ///
    /// For an operation whose effect cannot be expressed as a delta — a generator that laid a
    /// hundred pieces, say. The snapshot is the map as it was.
    Snapshot,
}

/// **A refusal — the editor could not do what was asked.**
///
/// **One type, for both front ends.** A key handler and an agent get the *same* refusal, from
/// the same place, with the same text — which is what makes the two front ends interchangeable
/// rather than merely similar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub String);

impl Refusal {
    pub fn new(text: impl Into<String>) -> Refusal {
        Refusal(text.into())
    }
}

/// **One row of a map, with where it sits** — so a removal can be put back exactly where it was.
///
/// An index and not just the row, because a map's order is meaningful: `at` decides nothing about
/// precedence, but the file is a list an author reads, and a restoration that appended would move a
/// piece from the middle of the file to its end on every undo/redo round trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Indexed<T> {
    pub index: usize,
    pub row: T,
}

/// **A change to a map's rows that can be undone by applying its own inverse.**
///
/// The shape every row-changing operation in this series shares, because the undo path re-dispatches
/// an operation with its logged `inverse` as the parameters — so the inverse has to be expressible
/// as *this operation's own parameters*. `RowDelta` is that vocabulary: what to take away, and what
/// to put back, each with its index.
///
/// **It is pure and it does not save.** Applying it edits the map in memory and hands back the delta
/// that would undo it; the caller decides when that map is written, which is what lets a save failure
/// restore both.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowDelta {
    #[serde(default)]
    pub remove_placements: Vec<String>,
    #[serde(default)]
    pub remove_stamps: Vec<String>,
    #[serde(default)]
    pub add_placements: Vec<Indexed<bevy_emerge_core::map::Placed>>,
    #[serde(default)]
    pub add_stamps: Vec<Indexed<bevy_emerge_core::composition::Stamped>>,
}

impl RowDelta {
    /// **Apply this delta to `map`, and return the delta that undoes it.**
    ///
    /// Three steps, in this order and for stated reasons:
    ///
    /// 1. **Find every row to remove before removing anything.** A missing id is refused with the id
    ///    named — it means something else has changed the map since this delta was recorded, and a
    ///    partial application followed by a refusal is worse than no application at all.
    /// 2. **Record each removed row with its index, then remove in descending order**, so removing an
    ///    earlier row cannot shift a later one out from under the loop.
    /// 3. **Insert the additions in ascending order**, at `index.min(len)` — clamped because a delta
    ///    recorded against a map that has since shrunk is still answerable: the row goes at the end
    ///    rather than vanishing.
    ///
    /// An added id that is already on the map is refused: two rows with one id is a map the editor
    /// cannot address, and nothing downstream would say so.
    pub fn apply(&self, map: &mut bevy_emerge_core::map::Map) -> Result<RowDelta, Refusal> {
        // 1 — find, and refuse before touching anything.
        let mut doomed_placements: Vec<(usize, bevy_emerge_core::map::Placed)> = Vec::new();
        for id in &self.remove_placements {
            let at = map
                .placements
                .iter()
                .position(|p| &p.id == id)
                .ok_or_else(|| {
                    Refusal::new(format!(
                        "`{id}` is not on the map any more, so this cannot be taken back — \
                         something changed it since"
                    ))
                })?;
            doomed_placements.push((at, map.placements[at].clone()));
        }
        let mut doomed_stamps: Vec<(usize, bevy_emerge_core::composition::Stamped)> = Vec::new();
        for id in &self.remove_stamps {
            let at = map
                .stamps
                .iter()
                .position(|s| &s.id == id)
                .ok_or_else(|| {
                    Refusal::new(format!(
                        "`{id}` is not on the map any more, so this cannot be taken back — \
                         something changed it since"
                    ))
                })?;
            doomed_stamps.push((at, map.stamps[at].clone()));
        }
        for add in &self.add_placements {
            if map.placements.iter().any(|p| p.id == add.row.id) {
                return Err(Refusal::new(format!("`{}` is already on the map", add.row.id)));
            }
        }
        for add in &self.add_stamps {
            if map.stamps.iter().any(|s| s.id == add.row.id) {
                return Err(Refusal::new(format!("`{}` is already on the map", add.row.id)));
            }
        }

        // 2 — remove descending, so indices stay valid.
        doomed_placements.sort_by_key(|(i, _)| *i);
        doomed_stamps.sort_by_key(|(i, _)| *i);
        for (i, _) in doomed_placements.iter().rev() {
            map.placements.remove(*i);
        }
        for (i, _) in doomed_stamps.iter().rev() {
            map.stamps.remove(*i);
        }

        // 3 — add ascending, clamping to the map that is there now.
        let mut added_placements = self.add_placements.clone();
        added_placements.sort_by_key(|a| a.index);
        for add in &added_placements {
            let at = add.index.min(map.placements.len());
            map.placements.insert(at, add.row.clone());
        }
        let mut added_stamps = self.add_stamps.clone();
        added_stamps.sort_by_key(|a| a.index);
        for add in &added_stamps {
            let at = add.index.min(map.stamps.len());
            map.stamps.insert(at, add.row.clone());
        }

        // **The inverse: what was added comes out, what was removed goes back at its index.**
        Ok(RowDelta {
            remove_placements: added_placements.iter().map(|a| a.row.id.clone()).collect(),
            remove_stamps: added_stamps.iter().map(|a| a.row.id.clone()).collect(),
            add_placements: doomed_placements
                .into_iter()
                .map(|(index, row)| Indexed { index, row })
                .collect(),
            add_stamps: doomed_stamps
                .into_iter()
                .map(|(index, row)| Indexed { index, row })
                .collect(),
        })
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// **What an operation would change, and what it changed.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// **A one-line summary for a receipt.**
    pub summary: String,
    /// **What the operation would place or change**, as a count. `0` for a read.
    pub affected: usize,
    /// **Whether the map on disk changed.** The one thing a caller always wants to know.
    pub saved: bool,
    /// **The parameters that put this change back**, for the same operation — what
    /// [`undo`] dispatches. `None` for an operation that declares no inverse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse: Option<serde_json::Value>,
    /// **The part of the editor's state this change left behind**, as [`Operation::state`] reads
    /// it. [`undo`] refuses when the state is no longer this, because something outside the log
    /// changed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<serde_json::Value>,
}

/// **The result of a dispatch.**
#[derive(Debug, Clone, PartialEq)]
pub enum Applied {
    /// **It happened.** The summary says what, and the id is what the agent cites.
    Done {
        /// **What happened.**
        outcome: Outcome,
        /// **The id the agent cites and the human reverts by.**
        operation_id: String,
        /// **The log's own record of it** — the same line that was appended.
        ///
        /// **Boxed, because the entry is a big datum and the refusal arm is not.** An enum whose
        /// variants differ by four times their size costs every match on it the size of the
        /// larger arm, and the refusal arm is on the hot path of every refused call.
        entry: Box<Entry>,
    },
    /// **It could not.** The refusal says why.
    Refused(Refusal),
}

/// **The context an operation runs in.**
///
/// **One context, both front ends.** A key handler and an agent build the same `Ctx` and call the
/// same `dispatch`, so "the agent's undo works" and "the author's undo works" are the same
/// statement about the same code.
///
/// **It owns what it carries, and that is what makes the operation trait dyn-compatible.** A
/// context that borrowed the project and the map would put a lifetime on every method, and a
/// trait with a lifetime on its methods cannot be a `dyn` — and the registry has to be a `dyn`,
/// because a list of operations is looked up by name. **So the context takes them by value and
/// gives them back.** One copy of a map's settings, and a file written once, is a price worth
/// paying for a registry that can hold more than one operation.
#[derive(Default)]
pub struct Ctx {
    /// **The project**, for the library and the paths.
    pub project: Option<Project>,
    /// **The project's root directory, when there is no loaded `Project` in hand.**
    ///
    /// The menu is the screen you are on *before* a door: it has a root and an unloaded library, and
    /// opening the whole `Project` to make one map would parse every kit and require `vocab.ron`.
    /// What a *change made from the menu* needs is the root — the log line goes in it, and a map goes
    /// in its `maps/`. An operation that needs the library says so, which is the refusal it already
    /// has; nothing dispatched from the menu does.
    pub root: Option<PathBuf>,
    /// **The map being edited**, if there is one.
    pub open: Option<OpenMap>,
    /// **Who is doing it.**
    pub source: Source,
    /// **Which session.**
    pub session: String,
    /// **The layout standing on screen, when an operation needs to see it.**
    ///
    /// A proposal is not map data — nothing is written until the person or an agent keeps it — so it
    /// travels in the context rather than in `OpenMap`. `layout_propose` writes one here, and
    /// `layout_accept` / `layout_discard` take it away; every other operation leaves it alone, which
    /// is why it defaults to `None` and why the front ends carry it across as a value rather than
    /// reaching into a resource.
    pub proposal: Option<crate::editor::Proposed>,
}

impl Ctx {
    /// **Where a map goes** — the project's own directory when one is loaded, else the menu's.
    ///
    /// One question with one answer, so `map_create` does not have to know which screen called it.
    pub fn maps_dir_or_project(&self) -> Option<PathBuf> {
        self.project
            .as_ref()
            .map(|p| p.maps_dir.clone())
            .or_else(|| self.root.as_ref().map(|r| r.join("assets/emerge/maps")))
    }

    /// **The project's root**, loaded or not, so a log line has somewhere to go.
    pub fn root_or_project(&self) -> Option<PathBuf> {
        self.project
            .as_ref()
            .map(|p| p.root.clone())
            .or_else(|| self.root.clone())
    }
}

/// **An operation, as the registry knows it.**
///
/// **A trait object, not a generic.** The registry is a list of operations that different front
/// ends look up by name, so the list has to be one type — and the parameters have to be erased,
/// because a list of `dyn` operations cannot be generic over its own parameter type.
///
/// **The name is a method, not an associated constant.** A `const` cannot be read off a
/// `dyn Operation`, and the registry's whole job is to look an operation up by name.
pub trait Operation: 'static {
    /// **The name the registry, the log and the MCP tool all use.**
    ///
    /// **Snake_case, and the same word the UI uses.** `map_set_settings` and `emerge_map_set_settings`
    /// are one operation, and the parity test says so.
    fn name(&self) -> &'static str;

    /// **How it is undone.**
    fn undo(&self) -> Undo;

    /// **The JSON schema of its parameters.**
    ///
    /// **A function, because a schema is built at run time** and a `const` cannot be. It is what
    /// `docs/mcp-tools.md` is generated from and what an agent reads.
    fn schema(&self) -> serde_json::Value;

    /// **What would change, and whether it is allowed to.**
    ///
    /// **Pure: it changes nothing.** A plan that says "no" is a refusal; a plan that says "yes" is
    /// what `commit` will do. **Preview is this function called and the result thrown away** —
    /// which is why preview is free and cannot half-apply.
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal>;

    /// **Do it.**
    ///
    /// **The only function in the editor that writes an operation's effect.** A front end calls
    /// `dispatch`, never this.
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal>;

    /// **The part of the editor's state this operation's inverse restores**, named so it can be
    /// compared: an undo is safe only while the state is still what the change left. `None` for an
    /// operation with no inverse.
    fn state(&self, _ctx: &Ctx) -> Option<serde_json::Value> {
        None
    }

    /// **The same part, as the change recorded it in `after`**, for an operation whose change
    /// touches one part of the map. An undo compares only that part, so an unrelated edit made
    /// by hand does not block it. Defaults to the whole [`Operation::state`].
    fn state_of(&self, ctx: &Ctx, _after: &serde_json::Value) -> Option<serde_json::Value> {
        self.state(ctx)
    }

    /// **Whether these parameters are a change only an undo may make**, so a dispatch that is not an
    /// undo refuses them. `false` for every operation but a removal's put-back (gap 2).
    fn only_as_undo(&self, _params: &serde_json::Value) -> bool {
        false
    }

    /// **Whether this operation writes the library** (`measured`, the Meshes door's data). Its logged
    /// change joins the Meshes history, so one Cmd+Z there takes back the newest library change,
    /// whoever made it (cycle 9 gap 1). `false` unless it does.
    fn writes_library(&self) -> bool {
        false
    }

    /// **The RNG seed this operation ran under, for the log.** `None` unless the operation's result
    /// depends on one — a proposal without its seed is a layout nobody can reproduce, which is the
    /// complaint the brief already makes about a WFC run recorded without its RNG.
    fn rng_seed(&self, _params: &serde_json::Value) -> Option<u64> {
        None
    }
}

/// **Make a map** — the operation a developer's *"make me a courtyard"* becomes (E-008).
///
/// # Why this is an operation and not just the chooser's function
///
/// `chooser::create_map` writes a map's file, and it was the only way to make one: a person, at the
/// keyboard, on the menu screen. The brief's second step is *"the in-editor agent turns that into
/// typed editor operations"*, and the first thing anyone asks for is a place to work — so the
/// simplest request in the whole loop had no tool behind it.
///
/// **The write itself is not duplicated.** This calls the same `chooser::create_map`, so the naming
/// rule, the refusal for a name already in use, and the validate-then-atomic write are one
/// implementation with two front ends rather than two that agree by habit.
///
/// # Undo
///
/// `Undo::Inverse`, and the inverse is the map's own **removal**: the delta a create leaves behind is
/// a file that was not there. Undo does not restore anything — there was nothing before — it takes
/// the new file away, which is the only truthful inverse of "this did not exist".
pub mod map_create {
    use super::*;
    use serde_json::json;

    /// **The map to make.**
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// **What it is called**, forced to snake_case by the same rule a file name uses.
        pub name: String,
        /// **How big, in metres on each axis.** Absent means a new map's default — 10 x 4 x 10 —
        /// so an agent that only knows a name does not have to state a size it has no opinion about.
        #[serde(default)]
        pub bounds: Option<(f32, f32, f32)>,
        /// **Where it sits in the world.** Absent means the origin.
        #[serde(default)]
        pub origin: Option<(f32, f32, f32)>,
        /// **The header prose** — what the developer said they wanted, kept as data.
        #[serde(default)]
        pub note: Option<String>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name"],
                "properties": {
                    "name": {
                        "description": "What the map is called. Forced to snake_case: lowercase letters, digits and single underscores, starting with a letter.",
                        "type": "string"
                    },
                    "bounds": {
                        "description": "How big the map is, in metres on each axis, as [x, y, z]. Every axis is positive. Omit for a new map's default of 10 x 4 x 10.",
                        "type": "array",
                        "items": {"type": "number"},
                        "minItems": 3,
                        "maxItems": 3
                    },
                    "origin": {
                        "description": "Where the map sits in the world, in metres, as [x, y, z]. Omit for the origin.",
                        "type": "array",
                        "items": {"type": "number"},
                        "minItems": 3,
                        "maxItems": 3
                    },
                    "note": {
                        "description": "The map's note: what it is and why it is laid out this way, as data. This is where a request like 'a mossy ruined courtyard with two exits' is worth keeping.",
                        "type": "string"
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for making a map: {e}. The fields are `name`,                      `bounds`, `origin` and `note`, and `name` is required."
                ))
            })
        }
    }

    pub struct MapCreate;

    impl Operation for MapCreate {
        fn name(&self) -> &'static str {
            "map_create"
        }

        /// **No inverse, and the reason is a gap rather than a choice.**
        ///
        /// The truthful inverse of a create is the file's removal, and `take_back` applies an inverse
        /// by calling **this same operation's `commit` with it** — so the inverse has to be this
        /// operation's own parameters. There is no `map_remove` operation to carry that, and writing
        /// one is a separate verb with its own refusals (what happens to the map another session has
        /// open?), so this declares `Undo::None` the way `door_open` does and says so here rather
        /// than inventing a parameter shape that would be refused on the way back.
        ///
        /// **What that costs, stated plainly:** Cmd+Z does not take a map away. The map is a file,
        /// and deleting it is the chooser's `Delete` row. Logged as E-066.
        fn undo(&self) -> Undo {
            Undo::None
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(maps_dir) = ctx.maps_dir_or_project() else {
                return Err(Refusal::new("no project is open, so there is nowhere to put a map."));
            };
            let name = bevy_emerge_core::naming::to_snake_case(&p.name);
            if name.is_empty() {
                return Err(Refusal::new(format!(
                    "`{}` leaves nothing usable as a name. Names are snake_case — lowercase                      letters, digits and single underscores, starting with a letter.",
                    p.name
                )));
            }
            // **The same refusal `create_map` makes, said before anything is written.** A preview
            // that would fail at commit is not a preview of the change; it is a preview of a
            // refusal, and saying so here is what makes `dry_run` worth calling.
            let path = maps_dir.join(bevy_emerge_core::naming::map_file_name(&name));
            if path.exists() {
                return Err(Refusal::new(format!(
                    "`{name}` already exists in this project. Pick another name, or open the one                      that is there."
                )));
            }
            if let Some(b) = p.bounds
                && (b.0 <= 0.0 || b.1 <= 0.0 || b.2 <= 0.0)
            {
                return Err(Refusal::new(format!(
                    "a map's bounds are metres on each axis, and every one has to be positive —                      `{b:?}` is not."
                )));
            }
            let default = bevy_emerge_core::map::Map::default();
            let bounds = p.bounds.unwrap_or(default.bounds);
            let origin = p.origin.unwrap_or(default.origin);
            Ok(json!({
                "name": name,
                "map": name,
                "bounds": [bounds.0, bounds.1, bounds.2],
                "origin": [origin.0, origin.1, origin.2],
                "note": p.note,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let plan = self.plan(ctx, params)?;
            let name = plan["name"].as_str().unwrap_or_default().to_owned();
            let read = |key: &str| -> (f32, f32, f32) {
                let v = &plan[key];
                (
                    v[0].as_f64().unwrap_or(0.0) as f32,
                    v[1].as_f64().unwrap_or(0.0) as f32,
                    v[2].as_f64().unwrap_or(0.0) as f32,
                )
            };
            let bounds = read("bounds");
            let origin = read("origin");
            let note = plan["note"].as_str().map(str::to_owned);

            let Some(maps_dir) = ctx.maps_dir_or_project() else {
                return Err(Refusal::new("no project is open, so there is nowhere to put a map."));
            };
            // **The chooser's own function, so the two front ends cannot drift.** Its refusals are
            // already sentences an author can act on, and it is the write every other path uses.
            let path = crate::chooser::create_map(&maps_dir, &name, bounds, origin, note.clone())
                .map_err(Refusal::new)?;
            let (x, y, z) = bounds;
            Ok(Outcome {
                summary: format!(
                    "made map `{name}` — {} x {} x {} m, and the file was written",
                    trim_metres(x),
                    trim_metres(y),
                    trim_metres(z)
                ),
                affected: 1,
                saved: true,
                // **No inverse**, for the reason `undo` gives: the faithful one is a `map_remove` that
                // does not exist yet.
                inverse: None,
                after: Some(json!({
                    "name": name,
                    "map": name,
                    "bounds": [x, y, z],
                    "origin": [origin.0, origin.1, origin.2],
                    "note": note,
                    "path": path.to_string_lossy(),
                })),
            })
        }
    }
}

/// **Propose a layout, change nothing, and let the person or the agent decide.**
///
/// The generate keys (`G`, `Shift+G`, `Cmd+G`) have always worked this way for a person: a layout is
/// *offered*, drawn as outlines, and kept with `Enter` or thrown away with `Esc`. But the offer was
/// `editor`'s own private act — five functions taking `&mut EditorState` — so an agent could not ask
/// for one at all, and an agent is exactly who the brief's *"a mossy ruined courtyard with two
/// exits"* is supposed to reach.
///
/// This is that offer with a name in the registry. It calls [`crate::editor::propose`], which reads a
/// `Project` and an `OpenMap` and nothing else.
///
/// # It writes nothing, and says so
///
/// `undo()` is [`Undo::None`] because there is no change to take back: the map, the file and the
/// library are untouched, and `after` carries `"preview_only": true` beside the rows so a caller
/// cannot mistake the offer for the change. A standing proposal is taken away by
/// [`layout_discard`], which is a different verb because it is a different act.
pub mod layout_propose {
    use super::*;
    use serde_json::json;

    /// **Where the rules come from, and the seed.**
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// Which of the three sources lays the layout.
        pub source: crate::editor::LayoutSource,
        /// **The solver's seed.** Absent means the editor picks a fresh one per call.
        ///
        /// Named rather than hidden so the same seed on the same map lays the same layout — which is
        /// what makes a proposal something an agent can talk about, and what a test can assert.
        #[serde(default)]
        pub seed: Option<u64>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["source"],
                "properties": {
                    "source": {
                        "description": "Where the rules come from. `learned` continues what is already placed on the map (refuses an empty map); `declared` reads the kit's declared edge tokens (works on an empty map); `composed` builds from the kit's compositions and is the only source that lays stamps rather than pieces.",
                        "type": "string",
                        "enum": ["learned", "declared", "composed"]
                    },
                    "seed": {
                        "description": "The solver's seed. Omit it and the editor picks a fresh one per call; the same seed on the same map lays the same layout.",
                        "type": "integer",
                        "minimum": 0
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for proposing a layout: {e}. The fields are \
                     `source` (learned, declared or composed) and `seed`, and `source` is required."
                ))
            })
        }
    }

    pub struct LayoutPropose;

    impl Operation for LayoutPropose {
        fn name(&self) -> &'static str {
            "layout_propose"
        }

        /// **Nothing to undo**: a proposal is an offer, and the map is untouched. Taking a standing
        /// proposal away is [`super::layout_discard`], which is its own verb.
        fn undo(&self) -> Undo {
            Undo::None
        }

        /// **The seed goes in the log.** Without it an agent's proposal is a layout nobody can
        /// reproduce — the same complaint the brief makes about a WFC run recorded without its RNG.
        fn rng_seed(&self, params: &serde_json::Value) -> Option<u64> {
            serde_json::from_value::<Params>(params.clone())
                .ok()
                .and_then(|p| p.seed)
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_ref()) else {
                return Err(Refusal::new(
                    "no map is open, so there is nowhere to propose a layout.",
                ));
            };
            let laid = crate::editor::propose(project, open, p.source, p.seed.unwrap_or(1))
                .map_err(Refusal::new)?;
            Ok(json!({
                "map": open.map.name,
                "source": p.source,
                "seed": p.seed,
                "laid": laid.proposed.placements.len() + laid.proposed.stamps.len(),
                "placements": laid.proposed.placements.len(),
                "stamps": laid.proposed.stamps.len(),
                "composes": laid.proposed.composes,
                "preview_only": laid.proposed.preview_only,
                "note": laid.note,
                "warnings": laid.warnings,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let after = self.plan(ctx, params)?;
            let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_ref()) else {
                return Err(Refusal::new(
                    "no map is open, so there is nowhere to propose a layout.",
                ));
            };
            // **The same computation the plan made**, so what was previewed is what stands — the
            // preview is this operation's own plan thrown away, and it must not be a second opinion.
            let laid = crate::editor::propose(project, open, p.source, p.seed.unwrap_or(1))
                .map_err(Refusal::new)?;
            let note = laid.note.clone();
            ctx.proposal = Some(laid.proposed);
            Ok(Outcome {
                summary: note,
                affected: 0,
                saved: false,
                inverse: None,
                after: Some(after),
            })
        }
    }
}

/// **Keep the layout on screen** — `Enter`, and the agent's half of the same door.
///
/// The person's arm used to live inline in `apply_proposal`; this is the same write with a name, so
/// an agent that proposed a layout can finish the job instead of leaving it on screen for somebody
/// to press a key at.
///
/// # What it clears, and when that is decided
///
/// A grammar over descriptors lays pieces, so it clears the **unpinned placements**; a grammar over
/// compositions lays stamps, so it clears the **unpinned stamps** and leaves the author's pieces
/// alone. Which rows those are is read from the **live map at the moment of commitment**, never
/// captured when the layout was proposed: an index held across the gap is a stale-index bug waiting
/// for the first author who places something while a proposal is up.
///
/// The removed rows are recorded with their indices in the inverse, so `Cmd+Z` puts the sketch back
/// where it was rather than at the end of the file.
///
/// # Undo
///
/// `Undo::Inverse`, and the inverse is this operation's own `delta` parameter — the vocabulary
/// [`RowDelta`] gives every row-changing operation here. The `delta` form is marked
/// [`Operation::only_as_undo`], so a bare `layout_accept {"delta": ...}` from an agent is refused:
/// putting rows back is the undo stack's job, not something an agent may do directly.
pub mod layout_accept {
    use super::*;
    use serde_json::json;

    /// **Keep the proposal, or (as an undo) put rows back.**
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// **The undo's own form.** Present only when this is being called to take a keep back.
        #[serde(default)]
        pub delta: Option<RowDelta>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "delta": {
                        "description": "The rows an undo puts back. Only an undo may set this: to keep the layout on screen, call with no parameters.",
                        "type": "object"
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for keeping a layout: {e}. Call with no \
                     parameters to keep the proposal on screen."
                ))
            })
        }
    }

    /// **The preview-only sentence, in one place.** The key path and the operation both say it, and a
    /// message that differs between them is a message one of them got wrong.
    pub const PREVIEW_ONLY: &str = "preview only — the game lays these blocks out itself. Esc clears \
                                    it; to fix a cell, place a tile there and pin it with O.";

    pub struct LayoutAccept;

    impl Operation for LayoutAccept {
        fn name(&self) -> &'static str {
            "layout_accept"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            Params::parse(params).map(|p| p.delta.is_some()).unwrap_or(false)
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new("no map is open, so there is nothing to lay a layout on."));
            };
            if let Some(delta) = &p.delta {
                // **Planned against a copy**, so the plan stays pure: the rows are counted, not moved.
                let mut trial = open.map.clone();
                delta.apply(&mut trial)?;
                return Ok(json!({
                    "map": open.map.name,
                    "placements": trial.placements.len(),
                    "stamps": trial.stamps.len(),
                }));
            }
            let Some(proposed) = ctx.proposal.as_ref() else {
                return Err(Refusal::new(
                    "there is no layout to keep — propose one first (layout_propose)",
                ));
            };
            if proposed.preview_only {
                return Err(Refusal::new(PREVIEW_ONLY));
            }
            // **What it would clear**, read now. The count is the whole of `docs/ui.md` §3.2's delta:
            // what changes, beside what is there.
            let clears = if proposed.composes {
                open.map.stamps.iter().filter(|s| !s.owned).count()
            } else {
                open.map.placements.iter().filter(|p| !p.owned).count()
            };
            Ok(json!({
                "map": open.map.name,
                "laid": if proposed.composes {
                    json!({"stamps": proposed.stamps.len()})
                } else {
                    json!({"placements": proposed.placements.len()})
                },
                "clears": clears,
                "composes": proposed.composes,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_mut() else {
                return Err(Refusal::new("no map is open, so there is nothing to lay a layout on."));
            };
            let before = open.map.clone();

            // **The undo's form: put the rows back, save, and hand back the delta that undoes it.**
            if let Some(delta) = &p.delta {
                let inverse = delta.apply(&mut open.map)?;
                let (some, taken) = (
                    inverse.add_placements.len() + inverse.add_stamps.len(),
                    delta.remove_placements.len(),
                );
                let Some(project) = ctx.project.as_ref() else {
                    return Err(Refusal::new("no project is open."));
                };
                let saved = match open.save(project) {
                    Ok(()) => true,
                    Err(e) => {
                        // **The rows go back before the refusal**: the in-memory map and the file
                        // must not disagree, or the dirty flag says "unsaved" about a change nobody
                        // made.
                        open.map = before;
                        return Err(Refusal::new(format!(
                            "the rows were put back but the file refused them, so nothing was \
                             written: {e}"
                        )));
                    }
                };
                return Ok(Outcome {
                    summary: format!("put back {some} row(s) and took away {taken}"),
                    affected: some + delta.remove_placements.len(),
                    saved,
                    inverse: Some(json!({ "delta": inverse })),
                    after: Some(json!({
                        "map": open.map.name,
                        "placements": open.map.placements.len(),
                        "stamps": open.map.stamps.len(),
                    })),
                });
            }

            // **The keep.** Take the proposal, then reproduce the map edit it describes.
            let Some(taken) = ctx.proposal.take() else {
                return Err(Refusal::new(
                    "there is no layout to keep — propose one first (layout_propose)",
                ));
            };
            if taken.preview_only {
                // **Put it back**, so the screen still has something to clear with Esc: the refusal
                // must not throw away the very thing it refused to keep.
                ctx.proposal = Some(taken);
                return Err(Refusal::new(PREVIEW_ONLY));
            }

            // **Which rows go, read from the live map.** See the type's documentation for why this is
            // not captured at propose time.
            let removed: Vec<Indexed<bevy_emerge_core::map::Placed>> = if taken.composes {
                Vec::new()
            } else {
                open.map
                    .placements
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| !p.owned)
                    .map(|(index, row)| Indexed { index, row: row.clone() })
                    .collect()
            };
            let unstamped: Vec<Indexed<bevy_emerge_core::composition::Stamped>> = if taken.composes {
                open.map
                    .stamps
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| !s.owned)
                    .map(|(index, row)| Indexed { index, row: row.clone() })
                    .collect()
            } else {
                Vec::new()
            };
            // Descending, so removing an earlier row cannot shift a later one out from under us.
            for row in removed.iter().rev() {
                open.map.placements.remove(row.index);
            }
            for row in unstamped.iter().rev() {
                open.map.stamps.remove(row.index);
            }

            let count = taken.placements.len();
            let stamped = taken.stamps.len();
            let added: Vec<String> = taken.placements.iter().map(|p| p.id.clone()).collect();
            let added_stamps: Vec<String> = taken.stamps.iter().map(|s| s.id.clone()).collect();
            open.map.placements.extend(taken.placements);
            open.map.stamps.extend(taken.stamps);

            // **A save failure restores the map AND the proposal**, so a refusal leaves the screen
            // exactly as it was and the person can try again or fix the file.
            let Some(project) = ctx.project.as_ref() else {
                open.map = before;
                return Err(Refusal::new("no project is open."));
            };
            if let Err(e) = open.save(project) {
                open.map = before;
                // The rows are back in the proposal, rebuilt from what was just written into it.
                ctx.proposal = Some(crate::editor::Proposed {
                    composes: taken.composes,
                    placements: added
                        .iter()
                        .filter_map(|id| open.map.placements.iter().find(|p| &p.id == id).cloned())
                        .collect(),
                    stamps: added_stamps
                        .iter()
                        .filter_map(|id| open.map.stamps.iter().find(|s| &s.id == id).cloned())
                        .collect(),
                    cell: taken.cell,
                    preview_only: taken.preview_only,
                    generation: taken.generation,
                });
                return Err(Refusal::new(format!(
                    "the layout was laid but the file refused it, so nothing was written: {e}"
                )));
            }

            let inverse = RowDelta {
                remove_placements: added,
                remove_stamps: added_stamps,
                add_placements: removed,
                add_stamps: unstamped,
            };
            Ok(Outcome {
                summary: if taken.composes {
                    format!("kept the layout: {stamped} stamped")
                } else {
                    format!("kept the layout: {count} placed")
                },
                affected: count + stamped,
                saved: true,
                inverse: Some(json!({ "delta": inverse })),
                after: Some(json!({
                    "map": open.map.name,
                    "placements": open.map.placements.len(),
                    "stamps": open.map.stamps.len(),
                })),
            })
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            let open = ctx.open.as_ref()?;
            Some(json!({
                "map": open.map.name,
                "placements": open.map.placements.len(),
                "stamps": open.map.stamps.len(),
            }))
        }
    }
}

/// **Throw the layout away** — `Esc`, and the agent's half of the same door.
///
/// The map is untouched by a proposal, so throwing one away is not an undo: it writes nothing and
/// records nothing to take back. It is a verb because *dropping the offer* is an act with a receipt,
/// and because an agent that proposed a layout and thought better of it needs a way to say so.
pub mod layout_discard {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {}

    pub struct LayoutDiscard;

    impl Operation for LayoutDiscard {
        fn name(&self) -> &'static str {
            "layout_discard"
        }

        /// Nothing is written, so there is nothing to take back.
        fn undo(&self) -> Undo {
            Undo::None
        }

        fn schema(&self) -> serde_json::Value {
            json!({"type": "object", "additionalProperties": false, "properties": {}})
        }

        fn plan(&self, ctx: &Ctx, _params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            if ctx.proposal.is_none() {
                return Err(Refusal::new("there is no layout to throw away"));
            }
            Ok(json!({"map": ctx.open.as_ref().map(|o| o.map.name.clone())}))
        }

        fn commit(&self, ctx: &mut Ctx, _params: &serde_json::Value) -> Result<Outcome, Refusal> {
            if ctx.proposal.take().is_none() {
                return Err(Refusal::new("there is no layout to throw away"));
            }
            Ok(Outcome {
                summary: "layout thrown away — the map is as it was".to_owned(),
                affected: 0,
                saved: false,
                inverse: None,
                after: Some(json!({"map": ctx.open.as_ref().map(|o| o.map.name.clone())})),
            })
        }
    }
}

/// **Flood fill from a cell** — `F`, and the agent's half of it.
///
/// The fill itself is `fill::flood`, unchanged: it refuses a piece that mounts on something, it stops
/// at anything already placed, and it stops at `MAX_CELLS` with a route to carry on. What was missing
/// was a name in the registry, so an agent could not lay a floor.
///
/// # `at` is a cell, not a pixel
///
/// The start point is **the same value the click passes to `flood`** — `open.map.to_map_space((x, z))`
/// from the cursor — so a caller is speaking map space in metres. A generator that reads placements
/// sits on the 1 m grid; a person's brush may be on a finer one, and the pitch argument is theirs.
pub mod fill {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// **What to lay.** A descriptor id from the project's library.
        ///
        /// **Optional in the type and required in practice**, because the undo's form carries only
        /// `delta` — `take_back` re-dispatches an operation with the inverse its log line recorded,
        /// and a `required` field would make the undo unparseable. The refusal is made by `plan`, by
        /// name, when a fill is asked for without a piece.
        #[serde(default)]
        pub piece: String,
        /// **Where the flood starts**, in map space in metres. Absent only in the undo's form.
        #[serde(default)]
        pub at: (f32, f32),
        #[serde(default)]
        pub yaw: f32,
        #[serde(default)]
        pub tip: (u8, u8),
        /// The cell pitch. Omitted means the project's default snap.
        #[serde(default)]
        pub pitch: Option<f32>,
        /// **The undo's own form**: the placements a fill added, to be taken away again.
        #[serde(default)]
        pub delta: Option<RowDelta>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["piece", "at"],
                "properties": {
                    "piece": {
                        "description": "The piece to lay, by descriptor id. emerge_list_pieces lists them. A piece that mounts on a surface or a wall cannot be filled with, because a fill covers open floor.",
                        "type": "string"
                    },
                    "at": {
                        "description": "Where the flood starts, in map space in metres, as [x, z]. The same value a click passes: not a screen position.",
                        "type": "array",
                        "items": {"type": "number"},
                        "minItems": 2,
                        "maxItems": 2
                    },
                    "yaw": {
                        "description": "The facing every filled piece takes, in degrees. Omit for 0.",
                        "type": "number"
                    },
                    "tip": {
                        "description": "Which variant of a tipped piece to lay, as [a, b]. Omit for [0, 0].",
                        "type": "array",
                        "items": {"type": "integer", "minimum": 0},
                        "minItems": 2,
                        "maxItems": 2
                    },
                    "pitch": {
                        "description": "The cell pitch in metres. Omit for the project's default snap.",
                        "type": "number"
                    },
                    "delta": {
                        "description": "The rows an undo takes away. Only an undo may set this: to fill, call with `piece` and `at`.",
                        "type": "object"
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for a fill: {e}. The fields are `piece`, `at`, \
                     `yaw`, `tip`, `pitch` and `delta`, and `piece` and `at` are required."
                ))
            })
        }
    }

    pub struct Fill;

    impl Operation for Fill {
        fn name(&self) -> &'static str {
            "fill"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            Params::parse(params).map(|p| p.delta.is_some()).unwrap_or(false)
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_ref()) else {
                return Err(Refusal::new("no map is open, so there is nowhere to fill."));
            };
            if let Some(delta) = &p.delta {
                let mut trial = open.map.clone();
                delta.apply(&mut trial)?;
                return Ok(json!({
                    "map": open.map.name,
                    "placements": trial.placements.len(),
                }));
            }
            let Some(descriptor) = project.library.get(&p.piece) else {
                return Err(Refusal::new(format!(
                    "`{}` is not a piece in this project — emerge_list_pieces lists them",
                    p.piece
                )));
            };
            let pitch = p
                .pitch
                .unwrap_or_else(|| default_pitch(project.lattice.snap_divisor));
            // **A dry run counts the cells and writes nothing.** The mint is the same shape the click
            // uses, so the count is the count the fill would really lay.
            let short = crate::editor::short_id(&descriptor.id).to_owned();
            let mut n = crate::editor::next_id_after(&open.map);
            let filled = crate::fill::flood(
                &open.map,
                descriptor,
                p.at,
                p.yaw,
                p.tip,
                pitch,
                || {
                    n += 1;
                    format!("{short}@{n}")
                },
            )
            .map_err(Refusal::new)?;
            Ok(json!({
                "map": open.map.name,
                "would_fill": filled.placements.len(),
                "truncated": filled.truncated,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_mut() else {
                return Err(Refusal::new("no map is open, so there is nowhere to fill."));
            };
            let before = open.map.clone();

            if let Some(delta) = &p.delta {
                let inverse = delta.apply(&mut open.map)?;
                let taken = inverse.add_placements.len();
                let Some(project) = ctx.project.as_ref() else {
                    return Err(Refusal::new("no project is open."));
                };
                let saved = match open.save(project) {
                    Ok(()) => true,
                    Err(e) => {
                        open.map = before;
                        return Err(Refusal::new(format!(
                            "the fill was taken away but the file refused it, so nothing was \
                             written: {e}"
                        )));
                    }
                };
                return Ok(Outcome {
                    summary: format!("took the fill away: {taken} piece(s)"),
                    affected: taken,
                    saved,
                    inverse: Some(json!({ "delta": inverse })),
                    after: Some(json!({
                        "map": open.map.name,
                        "placements": open.map.placements.len(),
                        "truncated": false,
                    })),
                });
            }

            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open."));
            };
            let Some(descriptor) = project.library.get(&p.piece).cloned() else {
                return Err(Refusal::new(format!(
                    "`{}` is not a piece in this project — emerge_list_pieces lists them",
                    p.piece
                )));
            };
            let pitch = p
                .pitch
                .unwrap_or_else(|| default_pitch(project.lattice.snap_divisor));
            let short = crate::editor::short_id(&descriptor.id).to_owned();
            let mut n = crate::editor::next_id_after(&open.map);
            let filled = crate::fill::flood(
                &open.map,
                &descriptor,
                p.at,
                p.yaw,
                p.tip,
                pitch,
                || {
                    n += 1;
                    format!("{short}@{n}")
                },
            )
            .map_err(Refusal::new)?;
            let count = filled.placements.len();
            let added: Vec<String> = filled.placements.iter().map(|p| p.id.clone()).collect();
            open.map.placements.extend(filled.placements);

            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open."));
            };
            if let Err(e) = open.save(project) {
                open.map = before;
                return Err(Refusal::new(format!(
                    "the fill was laid but the file refused it, so nothing was written: {e}"
                )));
            }
            // **The cap's route is carried in the summary**, because a truncated fill looks exactly
            // like a finished one and the message that says so is the only thing that distinguishes
            // them. `CAP_ROUTE` names a route that works: a fill from a *new* cell.
            let summary = if filled.truncated {
                format!(
                    "filled {count} and STOPPED at the {} cell cap. {}",
                    crate::fill::MAX_CELLS,
                    crate::fill::CAP_ROUTE
                )
            } else {
                format!("filled {count} with {}", p.piece)
            };
            Ok(Outcome {
                summary,
                affected: count,
                saved: true,
                inverse: Some(json!({ "delta": { "remove_placements": added } })),
                after: Some(json!({
                    "map": open.map.name,
                    "placements": open.map.placements.len(),
                    "truncated": filled.truncated,
                })),
            })
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            let open = ctx.open.as_ref()?;
            Some(json!({
                "map": open.map.name,
                "placements": open.map.placements.len(),
            }))
        }

    }

    /// The project's own default cell pitch, so an omitted `pitch` fills on the grid the kits are
    /// authored on rather than on whatever the last brush used.
    fn default_pitch(snap_divisor: u32) -> f32 {
        bevy_emerge_core::grid::SnapLevel::default().pitch(snap_divisor)
    }
}

/// **Save the open map** — `Cmd+S` on the Map door, and the agent's half of it.
///
/// The save is `OpenMap::save`, unchanged: it validates, expands the stamps the way the game does,
/// and refuses rather than writing a file the game cannot load. The value of the operation is that an
/// agent can *finish*: every mutating operation already writes, but a proposal that was kept and a
/// brush that was armed are different acts, and "save now" is what a person presses when they are
/// done.
///
/// # Undo
///
/// `Undo::None`: a save changes no document. There is nothing to take back, and taking it back would
/// mean restoring a file the author had already replaced.
pub mod map_save {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {}

    pub struct MapSave;

    impl Operation for MapSave {
        fn name(&self) -> &'static str {
            "map_save"
        }

        fn undo(&self) -> Undo {
            Undo::None
        }

        fn schema(&self) -> serde_json::Value {
            json!({"type": "object", "additionalProperties": false, "properties": {}})
        }

        fn plan(&self, ctx: &Ctx, _params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open, so there is nothing to save."));
            };
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new("no map is open, so there is nothing to save."));
            };
            let path = project
                .maps_dir
                .join(bevy_emerge_core::naming::map_file_name(&open.map.name));
            Ok(json!({
                "map": open.map.name,
                "path": path.to_string_lossy(),
                "dirty": open.dirty(),
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let plan = self.plan(ctx, params)?;
            let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_mut()) else {
                return Err(Refusal::new("no map is open, so there is nothing to save."));
            };
            match open.save(project) {
                Ok(()) => Ok(Outcome {
                    summary: format!("saved {}", plan["path"].as_str().unwrap_or_default()),
                    affected: 0,
                    saved: true,
                    inverse: None,
                    after: Some(json!({
                        "map": open.map.name,
                        "path": plan["path"],
                        "dirty": false,
                    })),
                }),
                // **A refusal of the whole operation**, not a half-applied change that reports
                // success: the map is unchanged and the file is unchanged.
                Err(e) => Err(Refusal::new(format!("NOT SAVED: {e}"))),
            }
        }
    }
}

/// **Open a map by name** — the verb an agent needs the moment it has made one (E-067).
///
/// `map_create` could make a map and then the agent had no way in: `door_open` takes a door and a
/// tab, and nothing took a map's name. The acceptance script's own third step failed on exactly this
/// and had to relaunch the editor as a workaround — which is not something an agent can to do to a
/// person's session.
///
/// # What it does, and what it does not
///
/// It reads the file through `OpenMap::open` — the same door the Map tab uses, so the naming rule and
/// the refusal for a file that calls itself something else are one implementation — puts it in the
/// context, and asks for the Map door. The crossing itself is the editor's, in the next frame, for
/// the same reason `door_open` waits: a map with unsaved edits asks the person before it is replaced.
///
/// **It is not `door_open` with a parameter.** Crossing doors and choosing a map are two questions,
/// and folding them would make a caller state a door it has no opinion about.
pub mod map_open {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The map's name, as `emerge_list_pieces`-style listings and `map_create` spell it.
        pub name: String,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name"],
                "properties": {
                    "name": {
                        "description": "The map to open, by name. The name IS its file (maps/<name>.map.ron), so a name that does not exist is refused rather than started.",
                        "type": "string"
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for opening a map: {e}. The field is `name`, and \
                     it is required."
                ))
            })
        }
    }

    pub struct MapOpen;

    impl Operation for MapOpen {
        fn name(&self) -> &'static str {
            "map_open"
        }

        /// **`Undo::None`, and the honest reason is that opening a map is not an edit.** It changes
        /// which document the editor is holding, not the document — the same argument `door_open`
        /// makes. Undoing it would mean putting the previous map back, which is `map_open` with the
        /// other name.
        fn undo(&self) -> Undo {
            Undo::None
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            // **The menu's root is enough, and that is the whole point of this verb.** An agent makes
            // a map from the menu (where there is a root and no loaded `Project`) and then wants in —
            // so this loads the project from the root when nothing is loaded, exactly as the editor
            // does on a door crossing. Measured: without this, `map_open` right after `map_create`
            // refused with *"no project is open"*, which is the same dead end E-067 described.
            let loaded;
            let project = match (ctx.project.as_ref(), ctx.root.as_ref()) {
                (Some(p), _) => p,
                (None, Some(root)) => {
                    loaded = crate::project::Project::open(root, None).map_err(Refusal::new)?;
                    &loaded
                }
                (None, None) => {
                    return Err(Refusal::new(
                        "no project is open, so there is no map to open.",
                    ));
                }
            };
            // **A map that does not exist is refused, and this is the one place that differs from
            // the editor's own "new map" path.** `OpenMap::open` *starts* a map when the file is
            // absent, which is exactly right for `map_create`'s caller and an author pressing `+ new
            // map` — and wrong here: an agent asking to open a map it has not made would silently
            // get an empty one under a name nobody chose, and would then work in the wrong document
            // without ever being told. Measured: the first version of this operation did exactly
            // that and answered `opened `no_such_map``.
            let name = bevy_emerge_core::naming::to_snake_case(&p.name);
            let path = project
                .maps_dir
                .join(bevy_emerge_core::naming::map_file_name(&name));
            if !path.is_file() {
                return Err(Refusal::new(format!(
                    "`{}` is not a map in this project, so there is nothing to open. `map_create` \
                     makes one; the maps directory holds the rest.",
                    p.name
                )));
            }
            // **The same reader the Map tab uses**, so a file that calls itself something else is
            // refused here exactly as it is there. A refusal changes nothing.
            let open = OpenMap::open(project, &p.name).map_err(Refusal::new)?;
            Ok(json!({
                "map": open.map.name,
                "bounds": [open.map.bounds.0, open.map.bounds.1, open.map.bounds.2],
                "placements": open.map.placements.len(),
                "stamps": open.map.stamps.len(),
                "dirty": open.dirty(),
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let plan = self.plan(ctx, params)?;
            let p = Params::parse(params)?;
            // **The project is loaded here too, and KEPT.** The plan may have loaded it from the
            // menu's root; a commit that then refused because `ctx.project` was still `None` was the
            // same dead end one step later — measured: `map_create` applied, `map_open` refused with
            // "no project is open". Loading it into the context is what a crossing needs anyway: the
            // editor cannot show a map without the project it belongs to.
            if ctx.project.is_none()
                && let Some(root) = ctx.root.clone()
            {
                ctx.project = Some(crate::project::Project::open(&root, None).map_err(Refusal::new)?);
            }
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new(
                    "no project is open, so there is no map to open.",
                ));
            };
            // **A map with unsaved edits is not silently replaced.** The same guard the rest of the
            // editor keeps: the person's work outranks an agent's convenience, and the route is to
            // save it or open it again.
            if let Some(previous) = ctx.open.as_ref()
                && previous.dirty()
                && previous.map.name != p.name
            {
                return Err(Refusal::new(format!(
                    "`{}` has unsaved edits, so it was not replaced. Save it (map_save) and ask \
                     again, or open it again to carry on.",
                    previous.map.name
                )));
            }
            let fresh = OpenMap::open(project, &p.name).map_err(Refusal::new)?;
            let name = fresh.map.name.clone();
            let placements = fresh.map.placements.len();
            ctx.open = Some(fresh);
            Ok(Outcome {
                summary: format!("opened `{name}`"),
                affected: 0,
                saved: false,
                inverse: None,
                after: Some(json!({
                    "map": name,
                    "placements": placements,
                    "bounds": plan["bounds"],
                })),
            })
        }
    }
}

/// **Place one piece** — the click, and the agent's half of it (S2.1).
///
/// **No operation added a row.** Every push of a placement was in the key/mouse path, so an agent
/// could turn a piece, pin it, or remove it, and could not put one down — which is the first thing
/// any request needs. The acceptance script's `place` step failed on exactly this.
///
/// The work is `editor::plan_placement`, which is the click's own code split at the point where it
/// stops describing and starts writing. One function, so an agent's placement and a person's cannot
/// disagree about what is already there, what a piece mounts to, or which rows it replaces.
///
/// # Undo
///
/// `Undo::Inverse`: the inverse removes the row it added and puts back the rows it replaced, at their
/// indices — the same [`RowDelta`] vocabulary every row-changing operation in this series uses.
pub mod placement_add {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The piece, by descriptor id.
        #[serde(default)]
        pub piece: String,
        /// **The piece's centre in map space, in metres**, used exactly as given — so a caller that
        /// wants the 1 m grid must snap for itself. An absent `at` is the undo's form only.
        #[serde(default)]
        pub at: (f32, f32),
        #[serde(default)]
        pub yaw: f32,
        #[serde(default)]
        pub tip: (u8, u8),
        /// **The undo's own form**: what to take away, and what to put back.
        #[serde(default)]
        pub delta: Option<RowDelta>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["piece", "at"],
                "properties": {
                    "piece": {
                        "description": "The piece to place, by descriptor id. emerge_list_pieces lists them.",
                        "type": "string"
                    },
                    "at": {
                        "description": "The piece's centre in map space, in metres, as [x, z]. Used exactly as given, so snap it yourself if you want the 1 m grid pieces a generator reads sit on.",
                        "type": "array",
                        "items": {"type": "number"},
                        "minItems": 2,
                        "maxItems": 2
                    },
                    "yaw": {"type": "number", "description": "Facing in degrees. Omit for 0."},
                    "tip": {
                        "description": "Which variant of a tipped piece, as [a, b]. Omit for [0, 0].",
                        "type": "array",
                        "items": {"type": "integer", "minimum": 0},
                        "minItems": 2,
                        "maxItems": 2
                    },
                    "delta": {
                        "description": "The rows an undo moves. Only an undo may set this.",
                        "type": "object"
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for placing a piece: {e}. The fields are `piece`, \
                     `at`, `yaw`, `tip` and `delta`, and `piece` and `at` are required."
                ))
            })
        }
    }

    pub struct PlacementAdd;

    impl Operation for PlacementAdd {
        fn name(&self) -> &'static str {
            "placement_add"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            Params::parse(params).map(|p| p.delta.is_some()).unwrap_or(false)
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_ref()) else {
                return Err(Refusal::new("no map is open, so there is nowhere to place a piece."));
            };
            if let Some(delta) = &p.delta {
                let mut trial = open.map.clone();
                delta.apply(&mut trial)?;
                return Ok(json!({
                    "map": open.map.name,
                    "placements": trial.placements.len(),
                }));
            }
            let Some(descriptor) = project.library.get(&p.piece) else {
                return Err(Refusal::new(format!(
                    "`{}` is not a piece in this project — emerge_list_pieces lists them",
                    p.piece
                )));
            };
            let planned = crate::editor::plan_placement(
                project,
                open,
                descriptor,
                p.at,
                p.yaw,
                p.tip,
                crate::editor::next_id_after(&open.map) + 1,
            )
            .map_err(Refusal::new)?;
            let replaced: Vec<&String> = planned
                .replaces
                .iter()
                .filter_map(|i| open.map.placements.get(*i).map(|row| &row.id))
                .collect();
            Ok(json!({
                "id": planned.placed.id,
                "map": open.map.name,
                "on": planned.placed.on,
                "replaces": replaced,
                "y": planned.y,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let plan = self.plan(ctx, params)?;
            let Some(open) = ctx.open.as_mut() else {
                return Err(Refusal::new("no map is open, so there is nowhere to place a piece."));
            };
            let before = open.map.clone();

            if let Some(delta) = &p.delta {
                let inverse = delta.apply(&mut open.map)?;
                let taken = inverse.add_placements.len();
                let Some(project) = ctx.project.as_ref() else {
                    return Err(Refusal::new("no project is open."));
                };
                let saved = match open.save(project) {
                    Ok(()) => true,
                    Err(e) => {
                        open.map = before;
                        return Err(Refusal::new(format!(
                            "the rows were put back but the file refused them, so nothing was \
                             written: {e}"
                        )));
                    }
                };
                return Ok(Outcome {
                    summary: format!("took away {taken} row(s)"),
                    affected: taken,
                    saved,
                    inverse: Some(json!({ "delta": inverse })),
                    after: Some(json!({
                        "map": open.map.name,
                        "placements": open.map.placements.len(),
                    })),
                });
            }

            let (Some(project), Some(descriptor)) = (
                ctx.project.as_ref(),
                ctx.project
                    .as_ref()
                    .and_then(|pr| pr.library.get(&p.piece).cloned()),
            ) else {
                return Err(Refusal::new(format!(
                    "`{}` is not a piece in this project — emerge_list_pieces lists them",
                    p.piece
                )));
            };
            let planned = crate::editor::plan_placement(
                project,
                open,
                &descriptor,
                p.at,
                p.yaw,
                p.tip,
                crate::editor::next_id_after(&open.map) + 1,
            )
            .map_err(Refusal::new)?;

            // **The rows it replaces come out first, recorded with their indices** so the inverse can
            // put each one back where it was rather than at the end of the file.
            let replaced: Vec<Indexed<bevy_emerge_core::map::Placed>> = planned
                .replaces
                .iter()
                .filter_map(|i| {
                    open.map
                        .placements
                        .get(*i)
                        .map(|row| Indexed { index: *i, row: row.clone() })
                })
                .collect();
            for row in replaced.iter().rev() {
                open.map.placements.remove(row.index);
            }
            let id = planned.placed.id.clone();
            let on = planned.placed.on.clone();
            let y = planned.y;
            open.map.placements.push(planned.placed);

            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open."));
            };
            if let Err(e) = open.save(project) {
                open.map = before;
                return Err(Refusal::new(format!(
                    "the piece was placed but the file refused it, so nothing was written: {e}"
                )));
            }

            // **The receipt is the click's own sentence**, word for word: a person and an agent
            // reading the same act must read the same words, or one of the two is a different
            // feature.
            let summary = match (on.clone(), replaced.is_empty()) {
                (_, false) => format!(
                    "replaced {} with {id}",
                    replaced
                        .iter()
                        .map(|r| format!("`{}`", r.row.id))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                (Some(host), true) => format!("placed {id} on {host}"),
                (None, true) => format!("placed {id} at ({:.2}, {:.2})", p.at.0, p.at.1),
            };
            Ok(Outcome {
                summary,
                affected: 1,
                saved: true,
                inverse: Some(json!({
                    "delta": {
                        "remove_placements": [id],
                        "add_placements": replaced,
                    }
                })),
                after: Some(json!({
                    "map": plan["map"],
                    "id": id,
                    "removed": replaced.iter().map(|r| r.row.id.clone()).collect::<Vec<_>>(),
                    "placements": open.map.placements.len(),
                    "y": y,
                })),
            })
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            let open = ctx.open.as_ref()?;
            Some(json!({
                "map": open.map.name,
                "placements": open.map.placements.len(),
            }))
        }
    }
}

/// **Import a `.glb` inside the project as a kit piece** — the bridge from golem to the palette (S2.2).
///
/// # This is a bridge, not a second importer
///
/// **golem is ours and it makes the mesh** (`AGENTS.md`, "Blender is a tool we own"). What this
/// operation does is the step after that: take a `.glb` that is already inside the project, measure it
/// with `import::measure`, run the Meshes tab's own four admissions (`tiles::admit_candidate`), and
/// write it into the library. It shares no code with golem and neither project knows the other's
/// schema — golem writes `kit.yaml` for kitbashing in Blender, and this writes a `library.ron`
/// `Descriptor` for placing on a map. The duplication to refuse is a second *generator*, not a bridge.
///
/// # The path is confined to the project
///
/// The brief's rule for a file parameter, and the only shape it can take here: the `.glb` must resolve
/// inside `<root>/assets`, which is the same directory `Descriptor::mesh` is relative to. A path that
/// escapes is refused by name rather than canonicalized into somewhere unexpected.
pub mod piece_import {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The `.glb`, relative to the project root — `assets/inbox/box.glb`.
        #[serde(default)]
        pub glb: Option<String>,
        /// The id to import it under. Absent means the one the measurement proposes (the file stem).
        #[serde(default)]
        pub id: Option<String>,
        /// **The undo's own form**: take this id out of the library.
        #[serde(default)]
        pub remove: Option<String>,
        /// **The undo's own form**: put this descriptor back.
        #[serde(default)]
        pub restore: Option<bevy_emerge_core::descriptor::Descriptor>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "glb": {
                        "description": "The mesh to import, as a path relative to the project root. It must be inside the project's assets folder and must be a .glb: a .gltf is refused, because a .gltf is a file plus its .bin and golem exports .glb.",
                        "type": "string"
                    },
                    "id": {
                        "description": "The id to import it under. Omit and the file's stem is used, in the project's snake_case. A kit's ids are one namespace, so an id already in the library is refused rather than replaced.",
                        "type": "string"
                    },
                    "remove": {
                        "description": "The id an undo takes out of the library. Only an undo may set this.",
                        "type": "string"
                    },
                    "restore": {
                        "description": "The descriptor an undo puts back. Only an undo may set this.",
                        "type": "object"
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for importing a mesh: {e}. The fields are `glb`, \
                     `id`, `remove` and `restore`; give `glb` to import, or `remove`/`restore` as an \
                     undo."
                ))
            })
        }
    }

    pub struct PieceImport;

    impl Operation for PieceImport {
        fn name(&self) -> &'static str {
            "piece_import"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            Params::parse(params)
                .map(|p| p.remove.is_some() || p.restore.is_some())
                .unwrap_or(false)
        }

        /// **It writes the library**, so its logged change joins the Meshes history — the same rule
        /// `label_set` follows, and the reason one Cmd+Z on that tab takes back the newest library
        /// change whoever made it.
        fn writes_library(&self) -> bool {
            true
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new(
                    "no project is open, so there is no library to import into.",
                ));
            };
            if let Some(id) = &p.remove {
                // **A preview of a removal asks the same question the removal does**, through the
                // project rather than by writing: what places this, what names it in a composition.
                let used = project.maps_that_place(id).map_err(Refusal::new)?;
                if !used.is_empty() {
                    return Err(Refusal::new(format!(
                        "`{id}` is placed by {} — remove those placements first",
                        used.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
                    )));
                }
                return Ok(json!({"remove": id, "would": "take it out of the library"}));
            }
            if let Some(d) = &p.restore {
                return Ok(json!({"restore": d.id, "would": "put it back"}));
            }
            let (abs, rel) = resolve(project, p.glb.as_deref())?;
            let candidate = bevy_emerge_core::import::measure(&abs, &rel, &project.measured);
            if candidate.blocked() {
                return Err(Refusal::new(format!(
                    "this mesh cannot be measured, so there is nothing to add: {}",
                    candidate
                        .findings
                        .iter()
                        .map(|f| f.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                )));
            }
            let id = p.id.clone().unwrap_or_else(|| candidate.proposed.id.clone());
            let descriptor = crate::tiles::admit_candidate(project, &candidate, &id)
                .map_err(Refusal::new)?;
            Ok(json!({
                "id": descriptor.id,
                "mesh": descriptor.mesh,
                "triangles": candidate.triangles,
                "kit": project.namespace,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let plan = self.plan(ctx, params)?;
            let Some(project) = ctx.project.as_mut() else {
                return Err(Refusal::new(
                    "no project is open, so there is no library to import into.",
                ));
            };

            // **The undo's two forms.** A removal hands back the descriptor it took out, so the
            // inverse is this operation again with `restore`.
            if let Some(id) = &p.remove {
                let before = project
                    .measured
                    .descriptors
                    .iter()
                    .find(|d| &d.id == id)
                    .cloned()
                    .ok_or_else(|| Refusal::new(format!("`{id}` is not in the library")))?;
                crate::tiles::take_out_of_library(id, project).map_err(Refusal::new)?;
                return Ok(Outcome {
                    summary: format!("took `{id}` out of the library"),
                    affected: 1,
                    saved: true,
                    inverse: Some(json!({"restore": before})),
                    after: Some(json!({"id": id})),
                });
            }
            if let Some(d) = &p.restore {
                let mut trial = project.measured.clone();
                trial.descriptors.push(d.clone());
                crate::tiles::commit_measured(project, trial).map_err(Refusal::new)?;
                return Ok(Outcome {
                    summary: format!("put `{}` back in the library", d.id),
                    affected: 1,
                    saved: true,
                    inverse: Some(json!({"remove": d.id})),
                    after: Some(json!({"id": d.id})),
                });
            }

            let (abs, rel) = resolve(project, p.glb.as_deref())?;
            let candidate = bevy_emerge_core::import::measure(&abs, &rel, &project.measured);
            if candidate.blocked() {
                return Err(Refusal::new(format!(
                    "this mesh cannot be measured, so there is nothing to add: {}",
                    candidate
                        .findings
                        .iter()
                        .map(|f| f.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                )));
            }
            let id = p.id.clone().unwrap_or_else(|| candidate.proposed.id.clone());
            let descriptor = crate::tiles::admit_candidate(project, &candidate, &id)
                .map_err(Refusal::new)?;
            let imported = descriptor.id.clone();
            let mut trial = project.measured.clone();
            trial.descriptors.push(descriptor);
            crate::tiles::commit_measured(project, trial).map_err(Refusal::new)?;
            Ok(Outcome {
                summary: format!("added `{imported}` — it is in the palette now"),
                affected: 1,
                saved: true,
                inverse: Some(json!({"remove": imported})),
                after: Some(json!({
                    "id": plan["id"],
                    "kit": plan["kit"],
                    "mesh": rel,
                })),
            })
        }
    }

    /// **The mesh's absolute path and its project-relative one**, or a refusal.
    ///
    /// Two questions in one place: is the file inside `<root>/assets` (the brief's rule for a path
    /// parameter), and is it a `.glb` (the only container `import::measure` reads — a `.gltf` is a
    /// file plus its `.bin`, and golem exports `.glb`).
    fn resolve(
        project: &Project,
        glb: Option<&str>,
    ) -> Result<(std::path::PathBuf, String), Refusal> {
        let Some(glb) = glb else {
            return Err(Refusal::new(
                "no mesh to import — give `glb`, a path to a .glb inside this project's assets folder",
            ));
        };
        let assets = project.root.join("assets");
        let joined = project.root.join(glb);
        let abs = joined
            .canonicalize()
            .map_err(|e| Refusal::new(format!("`{glb}`: {e}")))?;
        let canonical_assets = assets.canonicalize().unwrap_or(assets);
        if !abs.starts_with(&canonical_assets) {
            return Err(Refusal::new(format!(
                "`{glb}` is not inside this project's assets folder"
            )));
        }
        if abs.extension().is_none_or(|e| e != "glb") {
            return Err(Refusal::new(format!("`{glb}` is not a .glb file")));
        }
        let rel = abs
            .strip_prefix(&canonical_assets)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| glb.to_owned());
        Ok((abs, rel))
    }
}

/// **Every operation the editor knows, in one list.**
///
/// **The one list.** The MCP tool surface, the parity test and the log's vocabulary are all read
/// from it, so they cannot disagree about what exists.
pub fn registry() -> Vec<Box<dyn Operation>> {
    vec![
        Box::new(map_create::MapCreate),
        Box::new(map_set_settings::MapSetSettings),
        Box::new(layout_propose::LayoutPropose),
        Box::new(layout_accept::LayoutAccept),
        Box::new(layout_discard::LayoutDiscard),
        Box::new(fill::Fill),
        Box::new(map_save::MapSave),
        Box::new(map_open::MapOpen),
        Box::new(placement_add::PlacementAdd),
        Box::new(piece_import::PieceImport),
        Box::new(placement_set_pin::PlacementSetPin),
        Box::new(placement_set_yaw::PlacementSetYaw),
        Box::new(placement_remove::PlacementRemove),
        Box::new(stamp_remove::StampRemove),
        Box::new(map_rename::MapRename),
        Box::new(brush_arm::BrushArm),
        Box::new(door_open::DoorOpen),
        Box::new(piece_select::PieceSelect),
        Box::new(label_set::LabelSet),
    ]
}

/// **The operation with this name.**
pub fn find(name: &str) -> Option<Box<dyn Operation>> {
    registry().into_iter().find(|op| op.name() == name)
}

/// **Apply an operation, and record what happened.**
///
/// **The one path.** A key handler calls this; a BRP command calls this; an MCP tool calls this.
/// **Nobody calls `commit` directly**, and `tests/operations_have_two_front_ends.rs` holds that.
///
/// **And the log is written here, once**, so an operation cannot be applied without being
/// recorded — which is what makes the log a source of truth rather than a report.
pub fn dispatch(ctx: &mut Ctx, name: &str, params: &serde_json::Value) -> Result<Applied, Refusal> {
    dispatch_as(ctx, name, params, None)
}

/// **The refusal for a change only an undo may make**, the same wherever it is found: [`dispatch`]
/// and [`preview`] both give it.
fn undo_only_refusal(name: &str) -> Refusal {
    Refusal::new(format!(
        "`{name}` with `put_back` is the undo of a logged removal, not a change. Take that removal \
         back with `emerge/operation_undo`, or Cmd+Z in the editor. Nothing was put back."
    ))
}

/// **What [`dispatch`] would do, without doing it.** A preview goes through this, so it refuses
/// what dispatch refuses: a preview never says "would change" for a change dispatch would not make.
pub fn preview(
    ctx: &Ctx,
    op: &dyn Operation,
    params: &serde_json::Value,
) -> Result<serde_json::Value, Refusal> {
    if op.only_as_undo(params) {
        return Err(undo_only_refusal(op.name()));
    }
    op.plan(ctx, params)
}

/// **[`dispatch`], with the line it undoes named** — the one path for a change and for its undo,
/// so an undo is validated, written and logged exactly as the change was.
fn dispatch_as(
    ctx: &mut Ctx,
    name: &str,
    params: &serde_json::Value,
    undoes: Option<&str>,
) -> Result<Applied, Refusal> {
    let Some(op) = find(name) else {
        return Err(Refusal::new(format!(
            "`{name}` is not an operation this editor knows. The known ones are: {}.",
            registry()
                .iter()
                .map(|o| o.name())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    };
    // **A change only an undo may make is refused from anywhere else** (gap 2). Only [`undo`] passes
    // `undoes`, and it dispatches the line's own recorded inverse, so this is the one door a put-back
    // comes through. Applied on its own, it would bring back a piece the person may have removed again.
    if undoes.is_none() && op.only_as_undo(params) {
        return Err(undo_only_refusal(name));
    }
    // **The plan runs first, always.** A refusal found by the plan is found before anything is
    // written, so a refused operation changes nothing — not the map, not the file, not the log
    // (beyond the record that it was refused, which is itself worth having).
    op.plan(ctx, params)?;
    let outcome = op.commit(ctx, params)?;
    let entry = Entry {
        seq: next_seq(ctx),
        operation_id: operation_id(ctx),
        at: now_rfc3339(),
        source: ctx.source,
        session: ctx.session.clone(),
        op: name.to_owned(),
        params: params.clone(),
        outcome: "applied".to_owned(),
        refusal: None,
        result: Some(outcome.summary.clone()),
        rng_seed: op.rng_seed(params),
        rules_hash: None,
        undoes: undoes.map(str::to_owned),
        inverse: outcome.inverse.clone(),
        after: outcome.after.clone(),
    };
    // **A log that could not be written is a refusal, and the change does not stay.**
    //
    // The log is the source of truth. An operation applied without a record of it is an operation
    // that happened and cannot be told about or undone, so a failed append takes the change back
    // through its recorded inverse and says so. The take-back itself is not logged: nothing the
    // log should describe happened. See [`take_back`].
    append(ctx, &entry).map_err(|e| take_back(ctx, &*op, &outcome, e))?;
    Ok(Applied::Done {
        outcome,
        operation_id: entry.operation_id.clone(),
        entry: Box::new(entry),
    })
}

/// **A change whose log line cannot be written is taken back, or the refusal says it stayed.**
///
/// The change is already in the map and the file when the append fails. Its recorded inverse is
/// the put-back, so it is committed through the same operation, with no log line. The refusal
/// then says the change was taken back. When there is no inverse, the refusal says either that
/// nothing was changed (the operation changed nothing) or that the change is still in place, and
/// it never claims the change was undone. A failed take-back also says the change is still in place.
fn take_back(ctx: &mut Ctx, op: &dyn Operation, outcome: &Outcome, log_error: String) -> Refusal {
    let Some(inverse) = outcome.inverse.as_ref() else {
        // **An operation that changed nothing has nothing to keep in place.** Saying "still in
        // place" would tell the author a change exists on the map when none does.
        if !outcome.saved && outcome.affected == 0 {
            return Refusal::new(format!(
                "the operation could not be logged, so nothing was changed: {log_error}"
            ));
        }
        return Refusal::new(format!(
            "the operation was applied but could not be logged, and it records no inverse to take \
             it back, so the change is still in place: {log_error}"
        ));
    };
    match op.commit(ctx, inverse) {
        Ok(_) => Refusal::new(format!(
            "the operation could not be logged, so it was taken back and nothing was changed: \
             {log_error}"
        )),
        Err(Refusal(why)) => Refusal::new(format!(
            "the operation was applied but could not be logged, and taking it back failed, so the \
             change is still in place: {log_error}. Taking it back said: {why}"
        )),
    }
}

/// **What an undo resolved to**: the change being taken back or put back, the log line whose
/// inverse does it, and that inverse.
#[derive(Debug, Clone)]
pub struct UndoPlan {
    /// **The change**, by the id of the line that first made it.
    pub change: String,
    /// **The line whose inverse is dispatched**: the change itself, or the newest undo or redo of
    /// it. The new line's `undoes` names this one.
    pub line: Entry,
    /// **The parameters dispatched.**
    pub inverse: serde_json::Value,
    /// **Whether this puts the change back into effect** (a redo) rather than taking it back. The
    /// editor's history records a put-back as its newest change, exactly as it records a new one.
    pub restores: bool,
}

/// **Which change an undo takes back, and the parameters that do it.**
///
/// **Pure: it reads the log and changes nothing**, so a preview and an undo resolve the same way.
///
/// **A change is a chain**: the line that made it, then any undo of it, then any undo of that (a
/// redo), and so on. Each chain is in effect or not, and its newest line is the one whose inverse
/// flips it. With no `target`, the newest change in effect is taken back. A `target` names any line
/// of a chain: in effect, the chain is taken back; out of effect, naming the chain's newest line
/// puts it back (a redo), and naming an older line is refused, pointing at the newest.
///
/// **Two guards, because a newer change must never be overwritten:**
/// - a change still in effect that is newer than this chain's newest line blocks it, named;
/// - the state must still be what the chain's newest line left ([`Operation::state`]), or
///   something changed it outside the log (a hand-edited file the watcher adopted, the menu, or a
///   different map open), and the undo is refused, naming what differs.
pub fn undo_target(ctx: &Ctx, target: Option<&str>) -> Result<UndoPlan, Refusal> {
    use std::collections::{HashMap, HashSet};
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new(
            "no project is open, so there is no log to undo from.",
        ));
    };
    let path = oplog_path(project);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    // **A change that records no undo, by id and op name.** It stays out of the undo lines, so an
    // undo of its id can say what it is rather than that it is not in the log.
    let mut silent: HashMap<String, String> = HashMap::new();
    let mut lines = Vec::new();
    for (n, raw) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let entry: Entry = serde_json::from_str(raw).map_err(|e| {
            Refusal::new(format!(
                "line {} of {} is not a log entry ({e}), so the log cannot say what the newest \
                 change is. Nothing was changed.",
                n + 1,
                path.display()
            ))
        })?;
        let records_no_undo = find(&entry.op).is_some_and(|op| op.undo() == Undo::None);
        if entry.outcome == "applied" {
            if records_no_undo {
                silent.insert(entry.operation_id.clone(), entry.op.clone());
            } else {
                lines.push(entry);
            }
        }
    }
    // **Each line's chain, whether each chain is in effect, and each chain's newest line.**
    let mut chain_of: HashMap<&str, &str> = HashMap::new();
    let mut in_effect: HashSet<&str> = HashSet::new();
    let mut newest: HashMap<&str, usize> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let chain = match line.undoes.as_deref() {
            None => line.operation_id.as_str(),
            Some(undone) => chain_of.get(undone).copied().unwrap_or(undone),
        };
        chain_of.insert(line.operation_id.as_str(), chain);
        // A new change starts in effect; an undo line flips its chain.
        if line.undoes.is_none() || !in_effect.remove(chain) {
            in_effect.insert(chain);
        }
        newest.insert(chain, i);
    }
    // **The log is the project's; a change belongs to one map.** A line's map is the one its
    // recorded state names, and only changes to the open map are picked, or can block.
    let here = ctx.open.as_ref().map(|o| o.map.name.as_str());
    let map_of = |l: &Entry| {
        l.after
            .as_ref()
            .and_then(|a| a.get("map"))
            .and_then(|m| m.as_str())
            .map(str::to_owned)
    };
    // **The piece a library change set**, named by the `id` its recorded state carries. A map's
    // changes have no piece, and a library change shares a scope only with its own piece.
    let piece_of = |l: &Entry| {
        l.after
            .as_ref()
            .and_then(|a| a.get("id"))
            .and_then(|i| i.as_str())
            .map(str::to_owned)
    };
    let chain_map = |c: &str| map_of(&lines[newest[c]]);
    let chain: &str = match target {
        None => {
            let Some(chain) = in_effect
                .iter()
                .copied()
                .filter(|c| chain_map(c).as_deref() == here)
                .max_by_key(|c| newest[c])
            else {
                return Err(Refusal::new(
                    "nothing to undo: no change to the open map is still in effect.",
                ));
            };
            chain
        }
        Some(id) => {
            if let Some(op) = silent.get(id) {
                return Err(Refusal::new(format!(
                    "`{id}` is a {op} and changes nothing, so there is nothing to undo. \
                     emerge_read_oplog lists the changes that can be undone."
                )));
            }
            let Some(chain) = chain_of.get(id).copied() else {
                return Err(Refusal::new(format!(
                    "no operation `{id}` is in the log. emerge_read_oplog lists the ids."
                )));
            };
            let head = &lines[newest[chain]].operation_id;
            if !in_effect.contains(chain) && head != id {
                return Err(Refusal::new(format!(
                    "`{id}` is not in effect: `{head}` took it back. Undo `{head}` to put it back."
                )));
            }
            chain
        }
    };
    let line = &lines[newest[chain]];
    let line_map = map_of(line);
    if let Some(m) = line_map.as_deref()
        && Some(m) != here
    {
        return Err(Refusal::new(format!(
            "`{}` changed map `{m}`, and the open map is `{}`. Open `{m}` to take it back.",
            line.operation_id,
            here.unwrap_or("none")
        )));
    }
    // **Guard 1: a newer change in the same scope, logged, still in effect.** A map's changes share
    // the map, and a library change shares only its piece, so two label sets on different pieces
    // never refuse each other (cycle 9 gap 3).
    let same_scope = |other: &Entry| {
        map_of(other) == line_map && (line_map.is_some() || piece_of(other) == piece_of(line))
    };
    let changed = match line_map {
        Some(_) => "the map".to_owned(),
        None => format!("`{}`", piece_of(line).unwrap_or_else(|| "the library".to_owned())),
    };
    if let Some(blocker) = in_effect
        .iter()
        .filter(|c| **c != chain && newest[*c] > newest[chain] && same_scope(&lines[newest[*c]]))
        .map(|c| &lines[newest[c]])
        .max_by_key(|l| l.seq)
    {
        return Err(Refusal::new(format!(
            "`{}` ({} by {:?}) changed {} after `{}`, so putting the old values back would \
             overwrite it. Undo `{}` first, or leave `{}` as it is.",
            blocker.operation_id,
            blocker.op,
            blocker.source,
            changed,
            line.operation_id,
            blocker.operation_id,
            line.operation_id
        )));
    }
    let Some(op) = find(&line.op) else {
        return Err(Refusal::new(format!(
            "`{}` is not an operation this editor knows.",
            line.op
        )));
    };
    // **Guard 2: the state is what the line left, or something changed it since.**
    //
    // **Compared on the keys `state_of` reports, on both sides.** The right side is the map now; the
    // left is the logged `after`, and that carries fields the projection does not — a fill's
    // `truncated`, a rename's `path`. Comparing the whole logged `after` against a projection
    // refused an undo over a field that is a report and not a fact: measured on S1.5's own fill,
    // `truncated is null and 0000-1764 left false`, which reads to an author as "somebody edited
    // your map" when nobody did. What this guard means is *has the part of the map this change
    // touched moved*, and that is a comparison of two projections.
    if let (Some(now), Some(left)) = (
        line.after.as_ref().and_then(|a| op.state_of(ctx, a)),
        line.after.as_ref(),
    ) {
        // The logged row, cut down to the fields this operation's projection describes.
        let left = match now.as_object() {
            Some(keys) => {
                let mut projected = serde_json::Map::new();
                for key in keys.keys() {
                    projected.insert(
                        key.clone(),
                        left.get(key).cloned().unwrap_or(serde_json::Value::Null),
                    );
                }
                serde_json::Value::Object(projected)
            }
            // A projection that is not an object is the whole map, which is what it guarded before.
            None => left.clone(),
        };
        if left != now {
            // **A logged change is named first**: an undo or redo of another change to this map can
            // be what moved the state, and blaming an edit outside the log would be false.
            if let Some(later) = lines[newest[chain] + 1..]
                .iter()
                .rev()
                .find(|l| same_scope(l))
            {
                return Err(Refusal::new(format!(
                    "`{}` ({} by {:?}) changed {} after `{}`, so putting the old values back \
                     would overwrite it. Take `{}` back, or put it back, first.",
                    later.operation_id,
                    later.op,
                    later.source,
                    changed,
                    line.operation_id,
                    later.operation_id
                )));
            }
            let differs: Vec<String> = match (left.as_object(), now.as_object()) {
                (Some(l), Some(n)) => l
                    .keys()
                    .filter(|k| l.get(*k) != n.get(*k))
                    .map(|k| {
                        format!(
                            "{k} is {} and `{}` left {}",
                            n.get(k.as_str()).unwrap_or(&serde_json::Value::Null),
                            line.operation_id,
                            l[k.as_str()]
                        )
                    })
                    .collect(),
                _ => vec![format!(
                    "the state is {now} and `{}` left {left}",
                    line.operation_id
                )],
            };
            return Err(Refusal::new(format!(
                "the map has changed outside the operation log since `{}` ({}): a file edited by \
                 hand, the menu, or a different map open. Putting the old values back would \
                 overwrite that, so nothing was changed.",
                line.operation_id,
                differs.join("; ")
            )));
        }
    }
    let Some(inverse) = line.inverse.clone() else {
        return Err(Refusal::new(format!(
            "`{}` recorded no inverse (it was logged before undo existed), so it cannot be taken \
             back from the log.",
            line.operation_id
        )));
    };
    Ok(UndoPlan {
        change: chain.to_owned(),
        line: line.clone(),
        inverse,
        restores: !in_effect.contains(chain),
    })
}

/// **Take one change back, or put it back, by dispatching the inverse its newest line recorded.**
///
/// **The undo is an operation like any other**: the inverse goes through [`dispatch`]'s own path
/// (plan, commit, the file written, a new log line) and the new line's `undoes` names the line it
/// inverted. **Nothing is deleted from the log.**
pub fn undo(ctx: &mut Ctx, target: Option<&str>) -> Result<(UndoPlan, Applied), Refusal> {
    let plan = undo_target(ctx, target)?;
    let applied = dispatch_as(
        ctx,
        &plan.line.op,
        &plan.inverse,
        Some(&plan.line.operation_id),
    )?;
    Ok((plan, applied))
}

/// **Every line of the operation log, parsed** — or an empty list when there is no log yet.
///
/// **One reader**, because the undo walk and `emerge/present` both need to know what the log holds
/// and two parses would be two answers to "is this id real". A line that does not parse is skipped
/// rather than failing the whole read: a log is append-only text, and one damaged line must not make
/// every entry after it unreachable.
pub fn read_entries(path: &std::path::Path) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .collect()
}

/// **The next sequence number** — the count of lines already in the log.
///
/// **Counted, not tracked**, so a log edited by hand does not desynchronise the count.
fn next_seq(ctx: &Ctx) -> u64 {
    let Some(project) = ctx.project.as_ref() else {
        return 0;
    };
    std::fs::read_to_string(oplog_path(project))
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count() as u64)
        .unwrap_or(0)
}

/// **An operation id** — unique within a project, and short enough to read aloud.
///
/// **The sequence and the session's first four characters.** Two agents in one project therefore
/// get different ids without a clock or a random source, and the id says which run made the
/// change.
fn operation_id(ctx: &Ctx) -> String {
    let seq = next_seq(ctx);
    let tag: String = ctx
        .session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(4)
        .collect();
    format!("{seq:04}-{tag}")
}

/// **When it happened, as RFC 3339 UTC.**
///
/// **Computed here, not passed in**, so a caller cannot record a time that is not the time.
fn now_rfc3339() -> String {
    // **`SystemTime` and a hand-rolled formatter**, because the crate has no date dependency and
    // adding one for a timestamp is not worth it. The format is the RFC 3339 subset that matters:
    // UTC, seconds precision, a `Z` suffix.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// **The calendar date and time of a Unix timestamp, in UTC.**
///
/// **Howard Hinnant's `civil_from_days`**, which is the standard conversion and is exact for every
/// date this century. It is here because the alternative is a dependency, and the alternative to
/// *that* is a wrong timestamp.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    );
    // Shift the epoch to 0000-03-01 so the leap day is the last day of the cycle.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, h, mi, s)
}

/// **Append one entry to the log.**
///
/// **An append, not a replace.** The file is opened with `append(true)`, so the log cannot lose
/// an earlier line by being rewritten — and the crate's own `ron_surgery::save_atomic` rules do
/// not apply to it, because it is not a document anybody edits.
fn append(ctx: &Ctx, entry: &Entry) -> Result<(), String> {
    // **A root is enough.** The log lives at a fixed place under it, and a change made from the menu
    // has a root but no loaded `Project` — see `Ctx::root`.
    let Some(root) = ctx.root_or_project() else {
        return Err("no project is open, so there is nowhere to write the log.".to_owned());
    };
    let path = oplog_path_at(&root);
    // **A line that cannot be serialised is a failure, not a substitute.** A log line that says
    // "unloggable" is a line about the log, in a log about the work, and a reader cannot tell
    // the two apart.
    let line = serde_json::to_string(entry)
        .map_err(|e| format!("the log entry could not be written: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    use std::io::Write;
    writeln!(f, "{line}").map_err(|e| format!("{}: {e}", path.display()))?;
    f.sync_data()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

/// Edit distance between two ids. Core's `vocab::edit_distance` is private, and the ops that
/// suggest ids need only a short list, so they share this one.
fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = if ca == *cb {
                prev
            } else {
                1 + prev.min(row[j]).min(old)
            };
            prev = old;
        }
    }
    row[b.len()]
}

/// The three ids closest to `bad`, nearest first.
fn nearest_ids(ids: &[&str], bad: &str) -> Vec<String> {
    let mut scored: Vec<(usize, &str)> =
        ids.iter().map(|id| (levenshtein(id, bad), *id)).collect();
    scored.sort();
    scored
        .into_iter()
        .take(3)
        .map(|(_, id)| id.to_string())
        .collect()
}

/// **The operations a map's own settings are changed by.**
pub mod map_set_settings {
    use super::*;
    use serde_json::json;

    /// **A map's own settings** — the fields an author tunes while looking at a result.
    ///
    /// **A partial update, and every field is optional.** "Change the map's bounds" is one call
    /// with one field, and the fields not named keep their values. **A full replacement would be
    /// a different operation**, and a worse one: an agent that wanted to change the bounds would
    /// have to know and restate the origin and the note, and a restatement that got one field
    /// wrong would silently rewrite it.
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// **Where the map sits in the world**, in metres. A triple.
        #[serde(default)]
        pub origin: Option<(f32, f32, f32)>,
        /// **How big the map is**, in metres on each axis. A triple.
        #[serde(default)]
        pub bounds: Option<(f32, f32, f32)>,
        /// **One axis of the bounds, in metres.**
        ///
        /// **The UI's own shape, and it is here for the UI's reason.** The editor's size field
        /// edits **one** axis at a time — X, then Y, then Z — and the author sees the other two
        /// on screen while they type. **So the operation takes an axis as well as a whole
        /// triple**, and the UI's field and an agent's call are the same call.
        ///
        /// **`None` means "the whole triple", and `bounds` and `axis` together are a refusal** —
        /// two ways of saying the same thing is not a thing an operation should guess about.
        #[serde(default)]
        pub axis: Option<Axis>,
        /// **The value for `axis`, in metres.**
        ///
        /// **Required when `axis` is given, and refused without it.** An axis with no value is a
        /// half-said thing, and the operation says so rather than guessing.
        #[serde(default)]
        pub axis_value: Option<f32>,
        /// **The named combination of kits the map offers**, or `None` for every bound kit.
        ///
        /// **Absent leaves it alone; `null` clears it.** See [`present`].
        #[serde(default, deserialize_with = "present")]
        pub bash: Option<Option<String>>,
        /// **The map's note** — the header prose, as data. Absent leaves it; `null` clears it.
        #[serde(default, deserialize_with = "present")]
        pub note: Option<Option<String>>,
        /// **The fraction of a generated region that should close into rooms**, `0.0..=1.0`.
        ///
        /// Tuning this is why the field exists: what an author means by "rooms" differs per map, and
        /// the editor held one number for every map until E-005. Absent leaves it alone.
        #[serde(default)]
        pub enclosure_wish: Option<f32>,
        /// **What that wish is worth** against the other terms of the solve. Absent leaves it alone.
        #[serde(default)]
        pub enclosure_weight: Option<u32>,
    }

    /// **A field that is present is `Some`, even when it is `null`.**
    ///
    /// serde reads an explicit `null` into `Option<Option<T>>` as the outer `None` — the same as an
    /// absent field — so `"note": null` meant "leave the note alone", the operation answered
    /// `applied`, and nothing changed, while the schema says null clears it. **Called only when the
    /// field is present** (`#[serde(default)]` covers absence), so wrapping in `Some` is exact:
    /// absent is `None`, `null` is `Some(None)`, a value is `Some(Some(v))`.
    fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }

    /// **Which axis of the map's bounds.**
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Axis {
        X,
        Y,
        Z,
    }

    impl Axis {
        /// **The axis's own letter**, for a receipt.
        pub fn label(self) -> &'static str {
            match self {
                Axis::X => "X",
                Axis::Y => "Y",
                Axis::Z => "Z",
            }
        }

        /// **The axis's place in a bounds triple.**
        pub fn index(self) -> usize {
            match self {
                Axis::X => 0,
                Axis::Y => 1,
                Axis::Z => 2,
            }
        }
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "origin": {
                        "type": "array",
                        "items": {"type": "number"},
                        "minItems": 3,
                        "maxItems": 3,
                        "description": "Where the map sits in the world, in metres, as [x, y, z]."
                    },
                    "bounds": {
                        "type": "array",
                        "items": {"type": "number"},
                        "minItems": 3,
                        "maxItems": 3,
                        "description": "How big the map is, in metres on each axis, as [x, y, z]."
                    },
                    "axis": {
                        "description": "One axis of the bounds, for an edit that changes one axis and leaves the others. Use `axis_value` for its size.",
                        "type": ["string", "null"],
                        "enum": ["x", "y", "z", null]
                    },
                    "axis_value": {
                        "description": "The size of `axis`, in metres. Required when `axis` is given.",
                        "type": "number"
                    },
                    "bash": {
                        "description": "The named combination of kits the map offers. Null means every bound kit.",
                        "type": ["string", "null"]
                    },
                    "note": {
                        "description": "The map's note: the header prose, as data.",
                        "type": ["string", "null"]
                    },
                    "enclosure_wish": {
                        "description": "The fraction of a generated region that should close into rooms, 0.0 to 1.0. Used by the composition source, whose grammar has walls to close; the learned and declared sources derive no interfaces and ignore it. Soft: a region that cannot reach it returns its best arrangement and says so.",
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0
                    },
                    "enclosure_weight": {
                        "description": "What the enclosure wish is worth against the other terms of the solve. Any positive value makes it decisive, because it is the only soft constraint in play.",
                        "type": "integer",
                        "minimum": 1
                    }
                }
            })
        }

        /// **The parameters, parsed.**
        ///
        /// **`deny_unknown_fields`, so a typo is a refusal** rather than a field silently ignored
        /// and an agent left to believe it changed something.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not a map's settings: {e}. The fields are `origin`, `bounds`, `axis`, \
                     `axis_value`, `bash`, `note`, `enclosure_wish` and `enclosure_weight`, and \
                     nothing else."
                ))
            })
        }
    }

    /// **A map's own settings, changed.**
    pub struct MapSetSettings;

    impl Operation for MapSetSettings {
        fn name(&self) -> &'static str {
            "map_set_settings"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            ctx.open.as_ref().map(settings_of)
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            // **A map has to be open.** A settings change with no map has no subject, and saying
            // so is the whole refusal.
            if ctx.open.is_none() {
                return Err(Refusal::new(
                    "no map is open, so there are no settings to change.",
                ));
            }
            // **The bounds have to be positive.** A map with a zero or negative extent is a map
            // whose edges are inside it, and the validators refuse it — so the refusal comes here,
            // where the author is, rather than at the save.
            if let Some(b) = p.bounds
                && (b.0 <= 0.0 || b.1 <= 0.0 || b.2 <= 0.0)
            {
                return Err(Refusal::new(format!(
                    "a map's bounds are metres on each axis, and every one has to be positive —                      `{b:?}` is not."
                )));
            }
            // **An axis needs its value, and only its value.**
            //
            // **Two ways of saying the same thing is a refusal, not a guess.** A caller that
            // sends both `bounds` and `axis` has said two things about the bounds, and the
            // operation will not pick one — because picking wrong writes a size the author did
            // not ask for.
            if p.bounds.is_some() && p.axis.is_some() {
                return Err(Refusal::new(
                    "say the bounds one way or the other: `bounds` for all three axes, or `axis`                      and `axis_value` for one. Sending both is two answers to one question.",
                ));
            }
            if p.axis.is_some() && p.axis_value.is_none() {
                return Err(Refusal::new(
                    "an axis needs its size: send `axis` and `axis_value` together.",
                ));
            }
            if p.axis_value.is_some() && p.axis.is_none() {
                return Err(Refusal::new(
                    "`axis_value` is a size for an axis, and no axis was named.",
                ));
            }
            // **And the axis's size has to be a size.** A map's axis runs `MIN_BOUND..MAX_BOUND`
            // metres, and a value outside that is a map whose edges are inside it.
            if let (Some(axis), Some(value)) = (p.axis, p.axis_value)
                && !(value.is_finite() && value > 0.0)
            {
                return Err(Refusal::new(format!(
                    "the {} axis is metres across the map, and it has to be a positive number — \
                     `{value}` is not.",
                    axis.label()
                )));
            }
            // **The wish is a fraction, and a weight has to weigh something.**
            //
            // A wish outside `0.0..=1.0` is not a bigger ask, it is a number with no meaning — the
            // solver compares it against the fraction of a region that closed. A weight of zero is
            // the author switching the wish off while saying they want it, which is two answers to
            // one question and is refused rather than silently guessed.
            if let Some(wish) = p.enclosure_wish
                && !(wish.is_finite() && (0.0..=1.0).contains(&wish))
            {
                return Err(Refusal::new(format!(
                    "the enclosure wish is the fraction of a region that should close into rooms, \
                     from 0 to 1 — `{wish}` is not."
                )));
            }
            if let Some(weight) = p.enclosure_weight
                && weight < 1
            {
                return Err(Refusal::new(
                    "the enclosure weight is what the wish is worth, and it has to be at least 1 — \
                     a wish worth nothing is a wish not being asked for.",
                ));
            }
            // **The preview is the change, stated.**
            Ok(json!({
                "origin": p.origin,
                "bounds": p.bounds,
                "bash": p.bash,
                "note": p.note,
                "enclosure_wish": p.enclosure_wish,
                "enclosure_weight": p.enclosure_weight,
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let Some(mut open) = ctx.open.take() else {
                return Err(Refusal::new(
                    "no map is open, so there are no settings to change.",
                ));
            };
            // **The old values, for the undo.**
            let old = Settings {
                origin: open.map.origin,
                bounds: open.map.bounds,
                bash: open.map.bash.clone(),
                note: open.map.note.clone(),
                enclosure: open.map.enclosure,
            };
            if let Some(v) = p.origin {
                open.map.origin = v;
            }
            if let Some(v) = p.bounds {
                open.map.bounds = v;
            }
            // **One axis, and only that one.** The editor's size field edits X, then Y, then Z,
            // and the author sees the other two on screen while they type — so an axis edit
            // changes one number and leaves the other two exactly as they were.
            if let (Some(axis), Some(value)) = (p.axis, p.axis_value) {
                let (x, y, z) = open.map.bounds;
                open.map.bounds = match axis {
                    Axis::X => (value, y, z),
                    Axis::Y => (x, value, z),
                    Axis::Z => (x, y, value),
                };
            }
            if let Some(v) = p.bash {
                open.map.bash = v;
            }
            if let Some(v) = p.note {
                open.map.note = v;
            }
            // **The wish, one field at a time**, the same partial-update rule as every other
            // setting: an author tuning the weight keeps the fraction they already chose.
            if let Some(v) = p.enclosure_wish {
                open.map.enclosure.wish = v;
            }
            if let Some(v) = p.enclosure_weight {
                open.map.enclosure.weight = v;
            }
            // **And the file, so the change is not only in memory.** The brief's rule: a live
            // change is written back to the data file, so nothing exists only in memory.
            //
            // **The save is the map's own gate** — `OpenMap::save` validates, expands the stamps
            // the way the game does, and refuses rather than writing a file the game cannot load.
            // **A refusal to save is a refusal of the whole operation**, not a half-applied change
            // that reports success.
            let Some(project) = ctx.project.as_ref() else {
                // **The map goes back before the refusal.** The commit took it out of the
                // context, and a refusal that dropped it would leave the editor holding nothing
                // — the map would come back as a default, and the author's work with it.
                ctx.open = Some(open);
                return Err(Refusal::new("no project is open."));
            };
            let saved = match open.save(project) {
                Ok(()) => true,
                Err(e) => {
                    // **Put the map back.** The settings were changed in memory and the file
                    // refused them, so the in-memory map and the file now disagree — and the
                    // editor's own dirty flag would say the map is unsaved, which is true and
                    // useless. **The old values go back, so the editor and the file agree.**
                    open.map.origin = old.origin;
                    open.map.bounds = old.bounds;
                    open.map.bash = old.bash;
                    open.map.note = old.note;
                    open.map.enclosure = old.enclosure;
                    ctx.open = Some(open);
                    return Err(Refusal::new(format!(
                        "the settings were changed but the file refused them, so nothing was \
                         written: {e}"
                    )));
                }
            };
            // **The map goes back into the context, changed or not.** The refusal arm restores
            // the old values *before* this, so the editor and the file always agree.
            //
            // **The receipt is written before the move**, because the map is the context's after
            // this line and the summary is about the map.
            let (x, y, z) = open.map.bounds;
            let summary = format!(
                "map is {} x {} x {} m, and the file was written",
                trim_metres(x),
                trim_metres(y),
                trim_metres(z)
            );
            ctx.open = Some(open);
            // **The inverse is the settings this change replaced, all four of them.** Naming every
            // field, rather than only the ones that changed, keeps the undo exact when a field was
            // absent (`null` clears it) and keeps it one call.
            let inverse = serde_json::json!({
                "origin": [old.origin.0, old.origin.1, old.origin.2],
                "bounds": [old.bounds.0, old.bounds.1, old.bounds.2],
                "bash": old.bash,
                "note": old.note,
                "enclosure_wish": old.enclosure.wish,
                "enclosure_weight": old.enclosure.weight,
            });
            let after = ctx.open.as_ref().map(settings_of);
            Ok(Outcome {
                summary,
                affected: 1,
                saved,
                inverse: Some(inverse),
                after,
            })
        }
    }

    /// **Which map, and its four settings** — what an undo of a settings change compares before it
    /// puts old values back. **The map's name is part of it**, because the log is the project's and
    /// the operation acts on whichever map is open.
    pub fn settings_of(open: &OpenMap) -> serde_json::Value {
        json!({
            "map": open.map.name,
            "origin": [open.map.origin.0, open.map.origin.1, open.map.origin.2],
            "bounds": [open.map.bounds.0, open.map.bounds.1, open.map.bounds.2],
            "bash": open.map.bash,
            "note": open.map.note,
            "enclosure_wish": open.map.enclosure.wish,
            "enclosure_weight": open.map.enclosure.weight,
        })
    }

    /// **A map's settings, as they were.**
    ///
    /// **The inverse delta.** The undo of a settings change is the settings it replaced, and
    /// that is a small thing to keep.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Settings {
        pub origin: (f32, f32, f32),
        pub bounds: (f32, f32, f32),
        pub bash: Option<String>,
        pub note: Option<String>,
        pub enclosure: bevy_emerge_core::map::Enclosure,
    }
}

/// **One placement's pin, set or cleared** — the operation behind the O key and the pin-reason field.
///
/// **Addressed by the placement's id, not its row.** A row is where the piece sits in the list
/// this minute; the id is what the piece is called, and it does not change when another piece is
/// removed. The undo names the id for the same reason.
pub mod placement_set_pin {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The placement's id, as the map file and `emerge/state` name it.
        pub id: String,
        /// `true` pins the piece; `false` unpins it.
        pub pinned: bool,
        /// **Why the piece is held.** Required when `pinned` is true, and refused when it is not:
        /// an unpin takes no reason, and sending one is two answers to one question.
        #[serde(default)]
        pub because: Option<String>,
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "pinned"],
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The placement's id, as the map file names it."
                    },
                    "pinned": {
                        "type": "boolean",
                        "description": "true pins the piece so the solver routes around it; false unpins it."
                    },
                    "because": {
                        "type": ["string", "null"],
                        "description": "Why the piece is held. Required when pinned is true; refused otherwise."
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not a placement's pin: {e}. The fields are `id`, `pinned` and \
                     `because`, and nothing else."
                ))
            })
        }
    }

    /// **The one rule for a pin's reason**, in one place: a pin has a reason, an unpin has none.
    /// Returns the reason as it will be stored, trimmed.
    fn reason_of(p: &Params) -> Result<Option<String>, Refusal> {
        match (p.pinned, p.because.as_deref().map(str::trim)) {
            (true, Some(r)) if !r.is_empty() => Ok(Some(r.to_owned())),
            (true, _) => Err(Refusal::new(
                "a pin needs a reason, and it is what the author will read when they ask why the \
                 piece is held. Nothing was pinned.",
            )),
            (false, None) => Ok(None),
            (false, Some(_)) => Err(Refusal::new(
                "an unpin takes no reason, so `because` has to be left out. Nothing was unpinned.",
            )),
        }
    }

    /// **The pins a map holds, as its id and reason**, which is the state an undo checks.
    fn pins_of(open: &OpenMap) -> serde_json::Value {
        let pins: Vec<serde_json::Value> = open
            .map
            .placements
            .iter()
            .filter(|p| p.owned)
            .map(|p| json!([p.id, p.owned_because]))
            .collect();
        json!({"map": open.map.name, "pins": pins})
    }

    /// **Set or clear one placement's pin.**
    pub struct PlacementSetPin;

    impl Operation for PlacementSetPin {
        fn name(&self) -> &'static str {
            "placement_set_pin"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            ctx.open.as_ref().map(pins_of)
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new(
                    "no map is open, so there are no placements to pin.",
                ));
            };
            if !open.map.placements.iter().any(|q| q.id == p.id) {
                return Err(Refusal::new(format!(
                    "no placement is called `{}` on this map, so nothing was pinned.",
                    p.id
                )));
            }
            let because = reason_of(&p)?;
            Ok(json!({"id": p.id, "pinned": p.pinned, "because": because}))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let because = reason_of(&p)?;
            let Some(mut open) = ctx.open.take() else {
                return Err(Refusal::new(
                    "no map is open, so there are no placements to pin.",
                ));
            };
            let Some(index) = open.map.placements.iter().position(|q| q.id == p.id) else {
                ctx.open = Some(open);
                return Err(Refusal::new(format!(
                    "no placement is called `{}` on this map, so nothing was pinned.",
                    p.id
                )));
            };
            // **The old pin, for the undo.** Its inverse is the same operation with these values.
            let old_owned = open.map.placements[index].owned;
            let old_because = open.map.placements[index].owned_because.clone();
            open.map.placements[index].owned = p.pinned;
            open.map.placements[index].owned_because = because.clone();
            let summary = match &because {
                Some(reason) => format!("pinned {}: {reason}", p.id),
                None => format!("unpinned {}", p.id),
            };
            // **And the file, so the pin is not only in memory.** The save is the map's own gate;
            // a refusal to save puts the old pin back, so the editor and the file agree.
            let Some(project) = ctx.project.as_ref() else {
                ctx.open = Some(open);
                return Err(Refusal::new("no project is open."));
            };
            let saved = match open.save(project) {
                Ok(()) => true,
                Err(e) => {
                    open.map.placements[index].owned = old_owned;
                    open.map.placements[index].owned_because = old_because;
                    ctx.open = Some(open);
                    return Err(Refusal::new(format!(
                        "the pin was changed but the file refused it, so nothing was written: {e}"
                    )));
                }
            };
            let inverse = json!({"id": p.id, "pinned": old_owned, "because": old_because});
            ctx.open = Some(open);
            let after = ctx.open.as_ref().map(pins_of);
            Ok(Outcome {
                summary,
                affected: 1,
                saved,
                inverse: Some(inverse),
                after,
            })
        }
    }
}

/// **Turn one placement to a facing**, which is what the turn keys do and what an agent asks for.
pub mod placement_set_yaw {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The placement's id, as the map file and `emerge/state` name it.
        pub id: String,
        /// The facing in degrees. Wrapped into `[0, 360)` before it is stored, so `-15` is `345`.
        pub yaw: f32,
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "yaw"],
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The placement's id, as the map file names it."
                    },
                    "yaw": {
                        "type": "number",
                        "description": "The facing in degrees. Wrapped into 0 to 360, so -15 is 345."
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not a placement's facing: {e}. The fields are `id` and `yaw`, and \
                     nothing else."
                ))
            })?;
            if !p.yaw.is_finite() {
                return Err(Refusal::new(
                    "a facing has to be a finite number of degrees. Nothing was turned.",
                ));
            }
            Ok(p)
        }
    }

    /// **The facing a placement is stored with**: wrapped into `[0, 360)`, as the keys wrap it.
    fn wrapped(yaw: f32) -> f32 {
        yaw.rem_euclid(360.0)
    }

    /// **The facings a map holds, as id and degrees**, which is the state an undo checks.
    fn yaws_of(open: &OpenMap) -> serde_json::Value {
        let yaws: Vec<serde_json::Value> = open
            .map
            .placements
            .iter()
            .map(|p| json!([p.id, p.yaw]))
            .collect();
        json!({"map": open.map.name, "yaws": yaws})
    }

    /// **Turn one placement to a facing.**
    pub struct PlacementSetYaw;

    impl Operation for PlacementSetYaw {
        fn name(&self) -> &'static str {
            "placement_set_yaw"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            ctx.open.as_ref().map(yaws_of)
        }

        /// **The turned piece's facing now**, against the one its change recorded. A log line
        /// without a piece id (written before this was scoped) falls back to the whole map.
        fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
            let Some(id) = after.get("id").and_then(serde_json::Value::as_str) else {
                return self.state(ctx);
            };
            let open = ctx.open.as_ref()?;
            let p = open.map.placements.iter().find(|q| q.id == id)?;
            Some(json!({"id": p.id, "yaw": p.yaw}))
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new(
                    "no map is open, so there are no placements to turn.",
                ));
            };
            if !open.map.placements.iter().any(|q| q.id == p.id) {
                return Err(Refusal::new(format!(
                    "no placement is called `{}` on this map, so nothing was turned.",
                    p.id
                )));
            }
            Ok(json!({"id": p.id, "yaw": wrapped(p.yaw)}))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let yaw = wrapped(p.yaw);
            let Some(mut open) = ctx.open.take() else {
                return Err(Refusal::new(
                    "no map is open, so there are no placements to turn.",
                ));
            };
            let Some(index) = open.map.placements.iter().position(|q| q.id == p.id) else {
                ctx.open = Some(open);
                return Err(Refusal::new(format!(
                    "no placement is called `{}` on this map, so nothing was turned.",
                    p.id
                )));
            };
            // **The old facing, for the undo.** Its inverse is the same operation with this value.
            let old_yaw = open.map.placements[index].yaw;
            open.map.placements[index].yaw = yaw;
            let summary = format!("turned {} to {yaw:.0} deg", p.id);
            // **And the file, so the turn is not only in memory.** A refusal to save puts the old
            // facing back, so the editor and the file agree.
            let Some(project) = ctx.project.as_ref() else {
                ctx.open = Some(open);
                return Err(Refusal::new("no project is open."));
            };
            let saved = match open.save(project) {
                Ok(()) => true,
                Err(e) => {
                    open.map.placements[index].yaw = old_yaw;
                    ctx.open = Some(open);
                    return Err(Refusal::new(format!(
                        "the file refused the turn, so nothing was written and the facing is unchanged: {e}"
                    )));
                }
            };
            let inverse = json!({"id": p.id, "yaw": old_yaw});
            ctx.open = Some(open);
            // **Only the turned piece is recorded**: an undo checks that piece, not the whole map,
            // so an unrelated edit made by hand cannot block it.
            let after = Some(json!({"id": p.id, "yaw": yaw}));
            Ok(Outcome {
                summary,
                affected: 1,
                saved,
                inverse: Some(inverse),
                after,
            })
        }
    }
}

/// **Set one library piece's label axis to a list of vocabulary tokens** (E-042). An agent calls
/// this, and so does a person through the chip; the write is the same and the provenance is the
/// caller's `Source`, so the label says who put the tokens there. Mount and note are not vocabulary
/// lists, so they are not here.
pub mod label_set {
    use super::*;
    use bevy_emerge_core::descriptor::{Axis, By, Descriptor, LabelOrigin};
    use bevy_emerge_core::vocab::{Vocabularies, Vocabulary};
    use serde_json::json;

    /// **The axes an agent can set.** The order is the order the schema lists them in.
    const AXES: [&str; 4] = ["kind", "effects", "look", "surfaces"];

    /// **What a piece carries on the axes and its label**, which an undo puts back exactly.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Record {
        pub kind: Vec<String>,
        pub effects: Vec<String>,
        pub look: Vec<String>,
        pub surfaces: Vec<String>,
        pub labels: Option<LabelOrigin>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The library piece's id, as `emerge/pieces` names it.
        pub id: String,
        /// Which axis to set. Absent on an undo's put-back.
        #[serde(default)]
        pub axis: Option<String>,
        /// The tokens the axis should hold, which replace it. Absent on an undo's put-back.
        #[serde(default)]
        pub tokens: Option<Vec<String>>,
        /// **Undo only.** The whole record to put back, exactly as it was.
        #[serde(default)]
        pub restore: Option<Record>,
    }

    /// **What a request asks for**: a set of tokens, or an undo's exact put-back. Never both.
    enum Request {
        Set {
            id: String,
            axis: String,
            tokens: Vec<String>,
        },
        Restore {
            id: String,
            record: Record,
        },
    }

    /// **The parameters, parsed and shaped.** `deny_unknown_fields`, so a typo is a refusal.
    fn request(raw: &serde_json::Value) -> Result<Request, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "this is not a label to set: {e}. The fields are `id`, `axis` (kind, effects, look \
                 or surfaces) and `tokens`."
            ))
        })?;
        match (p.restore, p.axis, p.tokens) {
            (Some(record), None, None) => Ok(Request::Restore { id: p.id, record }),
            (Some(_), _, _) => Err(Refusal::new(
                "a restore names the whole record, so it takes `id` and `restore` and nothing else.",
            )),
            (None, Some(axis), Some(tokens)) => {
                if !AXES.contains(&axis.as_str()) {
                    return Err(Refusal::new(format!(
                        "`{axis}` is not a label axis. The axes are kind, effects, look and \
                         surfaces. Nothing was set."
                    )));
                }
                Ok(Request::Set {
                    id: p.id,
                    axis,
                    tokens,
                })
            }
            (None, _, _) => Err(Refusal::new(
                "setting a label needs `id`, `axis` and `tokens`. Nothing was set.",
            )),
        }
    }

    /// **The vocabulary one axis draws its tokens from.**
    fn vocab_for<'a>(v: &'a Vocabularies, axis: &str) -> &'a Vocabulary {
        match axis {
            "kind" => &v.kind,
            "effects" => &v.effects,
            "look" => &v.look,
            _ => &v.surfaces,
        }
    }

    /// **The token list one axis holds on a piece.**
    fn list_of<'a>(d: &'a mut Descriptor, axis: &str) -> &'a mut Vec<String> {
        match axis {
            "kind" => &mut d.kind,
            "effects" => &mut d.effects,
            "look" => &mut d.look,
            _ => &mut d.offers.surfaces,
        }
    }

    /// **The axis, read.** [`list_of`]'s mutable twin, for a caller that needs the value before a
    /// write through `list_of` — one `&mut` borrow cannot be held across the call that uses it.
    fn read_list<'a>(d: &'a Descriptor, axis: &str) -> &'a [String] {
        match axis {
            "kind" => &d.kind,
            "effects" => &d.effects,
            "look" => &d.look,
            _ => &d.offers.surfaces,
        }
    }

    /// **What a label write changed, as a phrase or nothing.** Only the part that moved: a receipt
    /// repeating a value the author already had is noise, and one that names nothing is what this
    /// exists to stop.
    fn changed_from(was: &[String], now: &[String]) -> String {
        let added: Vec<&str> = now
            .iter()
            .filter(|t| !was.contains(t))
            .map(String::as_str)
            .collect();
        let removed: Vec<&str> = was
            .iter()
            .filter(|t| !now.contains(t))
            .map(String::as_str)
            .collect();
        let say = |sign: &str, toks: &[&str]| {
            if toks.is_empty() {
                String::new()
            } else {
                format!(" {sign}{}", toks.join(", "))
            }
        };
        let out = format!("{}{}", say("+", &added), say(" -", &removed));
        if out.is_empty() {
            // **Not "unchanged", and the difference is real.** Re-asserting the same tokens still
            // moves the axis's provenance: `stamp_by` flips it to this writer, and a chip that was
            // `TEXT` (a person confirmed it) paints `UNCHECKED` afterwards. So the receipt says what
            // actually happened — the claim was re-stated by this writer — rather than reporting a
            // no-op the screen contradicts.
            if was.is_empty() {
                " — nothing".to_owned()
            } else {
                " — same tokens, claim restated".to_owned()
            }
        } else {
            out
        }
    }

    fn axis_of(axis: &str) -> Axis {
        match axis {
            "kind" => Axis::Kind,
            "effects" => Axis::Effects,
            "look" => Axis::Look,
            _ => Axis::Surfaces,
        }
    }

    /// **The tokens, checked against the axis's vocabulary and put in its order.** A token the
    /// vocabulary does not hold is refused with the nearest one, and a token listed twice is
    /// refused, because a set that names a thing twice is not a set. The vocabulary order means
    /// the same set always writes the same line.
    fn checked(vocab: &Vocabulary, axis: &str, tokens: &[String]) -> Result<Vec<String>, Refusal> {
        let mut seen: Vec<&str> = Vec::new();
        for t in tokens {
            if !vocab.contains(t) {
                let hint = bevy_emerge_core::vocab::nearest(vocab, t)
                    .map(|n| format!(" Did you mean `{n}`?"))
                    .unwrap_or_default();
                return Err(Refusal::new(format!(
                    "`{t}` is not a {axis} token.{hint} Nothing was set."
                )));
            }
            if seen.contains(&t.as_str()) {
                return Err(Refusal::new(format!(
                    "`{t}` is listed twice on {axis}. Nothing was set."
                )));
            }
            seen.push(t);
        }
        Ok(vocab
            .names()
            .filter(|n| seen.contains(n))
            .map(str::to_string)
            .collect())
    }

    /// **The record a piece has now**, which an undo checks and a put-back restores.
    fn record_of(d: &Descriptor) -> Record {
        Record {
            kind: d.kind.clone(),
            effects: d.effects.clone(),
            look: d.look.clone(),
            surfaces: d.offers.surfaces.clone(),
            labels: d.labels.clone(),
        }
    }

    /// **The state an agent reads back**: the axes and the label, by piece id.
    fn state_json(d: &Descriptor) -> serde_json::Value {
        json!({
            "id": d.id,
            "kind": d.kind,
            "effects": d.effects,
            "look": d.look,
            "surfaces": d.offers.surfaces,
            "labels": d.labels,
        })
    }

    /// **Write one axis and stamp it with the caller's provenance.** A kind change settles the
    /// effects it implies, and a changed effects list is stamped too, so the label never claims
    /// an effect it did not put there.
    fn set_axis(d: &mut Descriptor, vocab: &Vocabularies, axis: &str, tokens: Vec<String>, by: By) {
        let effects_before = d.effects.clone();
        *list_of(d, axis) = tokens;
        crate::labels::stamp_by(d, axis_of(axis), by);
        if axis == "kind" || axis == "effects" {
            crate::labels::settle_implied_effects(d, vocab);
            if d.effects != effects_before {
                crate::labels::stamp_by(d, Axis::Effects, by);
            }
        }
    }

    /// **A derived effect belongs to the kind that implies it, and a write that says otherwise is
    /// refused.** The next kind write would settle it away, so keeping it on a piece whose kind
    /// does not imply it is a claim the library cannot keep. Refused, not settled away: the author
    /// asked for it, and the answer has to say why nothing was written.
    fn derived_effect_refusal(
        vocab: &Vocabularies,
        id: &str,
        kind: &[String],
        tokens: &[String],
    ) -> Option<Refusal> {
        let implied = vocab.implied_effects(kind);
        let token = tokens
            .iter()
            .find(|t| crate::labels::is_derived(vocab, t) && !implied.contains(t))?;
        let of = if kind.is_empty() {
            "has no kind".to_string()
        } else {
            format!("is `{}`", kind.join("`, `"))
        };
        Some(Refusal::new(format!(
            "`{token}` is an effect a kind implies, and `{id}` {of}, which does not imply it, so \
             nothing was set. Set a kind that implies `{token}`, or leave the effect off."
        )))
    }

    /// **The edit, computed on a copy of the measured library.** Nothing is written here; the
    /// commit does that, and a refused write then leaves the piece as it was.
    struct Edit {
        measured: bevy_emerge_core::library::Library,
        before: Record,
        after: serde_json::Value,
        summary: String,
    }

    fn edit(project: &Project, source: Source, req: &Request) -> Result<Edit, Refusal> {
        let id = match req {
            Request::Set { id, .. } | Request::Restore { id, .. } => id,
        };
        let mut measured = project.measured.clone();
        let index = measured
            .descriptors
            .iter()
            .position(|d| &d.id == id)
            .ok_or_else(|| {
                if let Some(ns) = project.other_kit_of(id) {
                    return Refusal::new(format!(
                        "`{id}` belongs to kit `{ns}`, not the kit being authored (`{}`), so nothing was set.",
                        project.namespace
                    ));
                }
                let ids: Vec<&str> = measured.descriptors.iter().map(|d| d.id.as_str()).collect();
                let hint = match nearest_ids(&ids, id) {
                    near if near.is_empty() => String::new(),
                    near => format!(" Close ids: {}.", near.join(", ")),
                };
                Refusal::new(format!(
                    "no library piece is called `{id}`, so nothing was set.{hint} Pick one from the \
                     Meshes shelf."
                ))
            })?;
        let d = &mut measured.descriptors[index];
        let before = record_of(d);
        let (summary, after) = match req {
            Request::Set { axis, tokens, .. } => {
                let tokens = checked(vocab_for(&project.vocab, axis), axis, tokens)?;
                if axis == "effects"
                    && let Some(refusal) =
                        derived_effect_refusal(&project.vocab, id, &d.kind, &tokens)
                {
                    return Err(refusal);
                }
                let by = match source {
                    Source::Human => By::Human,
                    Source::Agent => By::Agent,
                };
                // **The derived axis, before the write** — E-063. `set_axis` re-settles `effects`
                // from `kind`, so a `kind` write can change tokens the author did not type.
                //
                // **Settled first, and that is what keeps the receipt honest.** The row on disk can
                // be behind its own `kind`: a library written before an `implies` rule was added
                // stores effects that the rule now contradicts, and `Project::open` settles only the
                // in-memory `library` — not `measured`, the copy this write edits. Comparing the raw
                // stored list would then tell the author the vocabulary had *just* added an effect
                // the editor has been showing since open (found by review, on this very test's
                // fixture: `kind` `prop` before and after, and a receipt claiming `+electricity`).
                // Settling both sides compares what this write moved against what the screen said.
                let effects_were =
                    crate::labels::settled_effects(&d.kind, &d.effects, &project.vocab);
                // **The axis as it was, before the write** — the receipt names a transition, and
                // "set kind on `barrel`" is a sentence that leaves out the only part a person wants:
                // what it became. Measured on the running editor, an agent's `label_set` on a piece
                // was `0.116%` of the window and its receipt said only `set kind on town/shrub`.
                //
                // **For the effects axis, "as it was" means as the screen showed it**, which is the
                // settled list — the same argument the paragraph above makes. Reading the raw row
                // here made a stale file report the vocabulary's own effect as the author's
                // addition: `+powered -inert` where `powered` had never left.
                let was = if axis == "effects" {
                    effects_were.clone()
                } else {
                    read_list(d, axis).to_vec()
                };
                // **What the author asked for**, kept because `set_axis` takes the list by value and
                // the effects clause needs to compare the settled result against the request. Only
                // that clause reads it, so only that axis pays for the copy (review, P3).
                let asked = (axis == "effects").then(|| tokens.clone());
                set_axis(d, &project.vocab, axis, tokens, by);
                let now = read_list(d, axis).to_vec();
                // **And said out loud, when the vocabulary moved them.** The write is right; a
                // receipt that names only the axis the author typed is not. The two are reported
                // separately because they are two decisions: one a person made, one the vocabulary
                // made from it — and only the second is the one nobody expected.
                let derived = if axis == "effects" {
                    // **The vocabulary can overrule an effects write too** — E-064.
                    //
                    // `set_axis` settles `effects` after either axis, because the derived half is
                    // the kind's to state: a piece whose kind implies `powered` cannot be left
                    // without it. An author who removes it has it put back — and the receipt used to
                    // say only `set effects on `bench`  -powered`, naming a removal that did not
                    // happen. A receipt that reports a change the library refused is worse than none,
                    // because it is believed.
                    //
                    // The net phrase cannot carry this: `-inert` is true whether or not `powered`
                    // survived, so the fact that the author's request was overruled has to be said.
                    // The tokens named are the ones *asked for and kept*, which is the part nobody
                    // expected.
                    // **Read against what the author ASKED for, not against the settled pair.**
                    // `was` and `now` are the same list here in the one case that matters — the
                    // effect never left — so a comparison of them cannot see the override. The
                    // request is `tokens`, which is the axis's whole new value.
                    let kept: Vec<&str> = was
                        .iter()
                        .filter(|t| !asked.as_ref().is_some_and(|a| a.contains(t)))
                        .filter(|t| d.effects.iter().any(|e| e == *t))
                        .map(String::as_str)
                        .collect();
                    if kept.is_empty() {
                        String::new()
                    } else {
                        format!(" — kept {} (the kind implies it)", kept.join(", "))
                    }
                } else if effects_were == d.effects {
                    // A `kind` write that left the effects alone has nothing to add. Compared as
                    // lists, not by parsing the phrase: `changed_from` also speaks for a no-op
                    // ("same tokens, claim restated"), and a string test here would be a second
                    // rule that could drift from the first.
                    String::new()
                } else {
                    format!("; effects{}", changed_from(&effects_were, &d.effects))
                };
                // **The axis phrase, and the derived one.** When the axis did not move but the
                // vocabulary did, "same tokens, claim restated" is not the news — the effects are —
                // so the restated clause is dropped rather than leading the sentence.
                // **Structural, not a string match.** The axis moved iff the two lists differ;
                // that is the same fact `changed_from` reads, asked here directly so the two cannot
                // drift on what "moved" means.
                let axis_phrase = if !derived.is_empty() && was == now {
                    String::new()
                } else {
                    changed_from(&was, &now)
                };
                (
                    format!("set {axis} on `{id}`{axis_phrase}{derived}"),
                    state_json(d),
                )
            }
            Request::Restore { record, .. } => {
                d.kind = record.kind.clone();
                d.effects = record.effects.clone();
                d.look = record.look.clone();
                d.offers.surfaces = record.surfaces.clone();
                d.labels = record.labels.clone();
                (format!("restored `{id}`"), state_json(d))
            }
        };
        Ok(Edit {
            measured,
            before,
            after,
            summary,
        })
    }

    /// **Set one piece's label axis.**
    pub struct LabelSet;

    impl Operation for LabelSet {
        fn name(&self) -> &'static str {
            "label_set"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn writes_library(&self) -> bool {
            true
        }

        fn schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "axis", "tokens"],
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The library piece's id, as emerge/pieces names it."
                    },
                    "axis": {
                        "type": "string",
                        "enum": AXES,
                        "description": "Which label axis to set: kind, effects, look or surfaces."
                    },
                    "tokens": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "The vocabulary tokens the axis should hold. Replaces the axis; a token the vocabulary does not hold, or one listed twice, is refused."
                    }
                }
            })
        }

        /// **An undo's put-back is never a request an agent may send.**
        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            matches!(request(params), Ok(Request::Restore { .. }))
        }

        /// **The piece's state now**, against the one the change recorded. A piece that has gone
        /// missing answers nothing, so an undo cannot match it.
        fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
            let id = after.get("id")?.as_str()?;
            let project = ctx.project.as_ref()?;
            let d = project.measured.descriptors.iter().find(|d| d.id == id)?;
            Some(state_json(d))
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let req = request(params)?;
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new(
                    "no project is open, so there is no library to set a label in.",
                ));
            };
            Ok(edit(project, ctx.source, &req)?.after)
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let req = request(params)?;
            let source = ctx.source;
            let id = match &req {
                Request::Set { id, .. } | Request::Restore { id, .. } => id.clone(),
            };
            let Some(project) = ctx.project.as_mut() else {
                return Err(Refusal::new(
                    "no project is open, so there is no library to set a label in.",
                ));
            };
            let e = edit(project, source, &req)?;
            // **The file first.** `commit_measured` writes and only then adopts the library, so a
            // refused save leaves the piece and the editor as they were.
            if let Err(msg) = crate::tiles::commit_measured(project, e.measured) {
                return Err(Refusal::new(format!(
                    "the file refused the change, so nothing was written and the piece is unchanged: {msg}"
                )));
            }
            Ok(Outcome {
                summary: e.summary,
                affected: 1,
                saved: true,
                inverse: Some(json!({"id": id, "restore": e.before})),
                after: Some(e.after),
            })
        }
    }
}

/// **Arm a piece, so the next placement puts it down.**
///
/// E-037: the palette click did this in the UI, so an agent could place pieces but could not choose
/// one. Arming changes no map, so it records no undo: its `after` is the piece the editor now holds,
/// and the editor applies it (`editor::arm_piece`, `editor::apply_armed`).
pub mod brush_arm {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The library id of the piece to arm, as the palette names it, e.g. `site/wall`. `null`
        /// puts the brush down, as Esc does for an armed piece.
        #[serde(default, deserialize_with = "present")]
        pub descriptor: Option<Option<String>>,
    }

    /// **Present, possibly null.** Serde's own `Option` reads an absent key as `None` too, so a
    /// typo would look like a request to put the brush down. Here the outer `None` is an absent
    /// key (refused) and the inner `None` is an explicit null (put down).
    fn present<'de, D>(d: D) -> Result<Option<Option<String>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <Option<String> as serde::Deserialize>::deserialize(d).map(Some)
    }

    /// How many ids a refusal lists. The closest first, so a typo's intended piece is among them.
    const LISTED_ON_REFUSAL: usize = 20;

    /// Levenshtein distance, two rows at a time. The ranking needs no more than this, so no crate.
    fn distance(a: &str, b: &str) -> usize {
        let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        let mut cur = vec![0usize; b.len() + 1];
        for (i, ca) in a.iter().enumerate() {
            cur[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let sub = prev[j] + usize::from(ca != cb);
                cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
            }
            std::mem::swap(&mut prev, &mut cur);
        }
        prev[b.len()]
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["descriptor"],
                "properties": {
                    "descriptor": {
                        "type": ["string", "null"],
                        "description": "The library id of the piece to arm, as the palette names it, e.g. `site/wall`. null puts the brush down."
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "this is not a piece to arm: {e}. The one field is `descriptor`, and nothing else."
                ))
            })
        }
    }

    /// **Arm one piece from the library.**
    pub struct BrushArm;

    impl Operation for BrushArm {
        fn name(&self) -> &'static str {
            "brush_arm"
        }

        fn undo(&self) -> Undo {
            Undo::None
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        /// **A piece the library holds, or a refusal.** Nothing is armed by a refused request.
        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let descriptor = match p.descriptor {
                None => {
                    return Err(Refusal::new(
                        "this is not a piece to arm: `descriptor` is missing. Name a piece, or \
                         give null to put the brush down.",
                    ));
                }
                Some(None) => return Ok(json!({"armed": null})),
                Some(Some(id)) => id,
            };
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new(
                    "no project is open, so there is no library to arm a piece from.",
                ));
            };
            if !project.library.descriptors.iter().any(|d| d.id == descriptor) {
                // **The refusal names what exists** (gap 2): an agent cannot retry with an id it cannot
                // see, and "the palette lists the pieces" sent it to a palette it has no tool to read.
                let mut ids: Vec<&str> = project
                    .library
                    .descriptors
                    .iter()
                    .map(|d| d.id.as_str())
                    .collect();
                if ids.is_empty() {
                    return Err(Refusal::new(format!(
                        "`{}` is not in the library, and the library holds no pieces, so nothing was armed.",
                        descriptor
                    )));
                }
                ids.sort_by_key(|id| (distance(id, &descriptor), id.to_string()));
                let shown = ids
                    .iter()
                    .take(LISTED_ON_REFUSAL)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Refusal::new(format!(
                    "`{}` is not in the library, so nothing was armed. The library holds {} pieces; \
                     the closest are: {shown}.",
                    descriptor,
                    ids.len()
                )));
            }
            // **A piece from a kit the map's bash leaves out is refused** (gap 5). The palette filters
            // by the same `palette_namespaces` rule, so an agent armed a piece a person could not
            // see. Without an open map there is no bash to apply, so nothing is refused.
            if let Some(open) = ctx.open.as_ref()
                && let Some(kit) = project.kit_of(&descriptor)
                && let Some(bash) = open.map.bash.as_deref()
                && !open.palette_namespaces(project).contains(kit)
            {
                return Err(Refusal::new(format!(
                    "`{descriptor}` is in kit `{kit}`, which this map's bash `{bash}` leaves out, so \
                     nothing was armed. The palette does not offer it either. Name a piece from a kit \
                     `{bash}` includes, or change the bash."
                )));
            }
            Ok(json!({"armed": descriptor}))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let plan = self.plan(ctx, params)?;
            let summary = match plan["armed"].as_str() {
                Some(id) => format!("armed {id}"),
                None => "put the brush down".to_owned(),
            };
            Ok(Outcome {
                summary,
                affected: 0,
                saved: false,
                inverse: None,
                after: Some(plan),
            })
        }
    }
}

/// **Open a door, or a panel of one (cycle 8, item 1).**
///
/// **The answer is the target; the editor does the crossing.** The keys and an agent's request reach
/// the same operation, so a door is crossed by one path whoever asks. `door` is `kit`, `map` or
/// `rigs`, and `tab`, when given, is a panel that door shows. Nothing changes on disk: the answer is
/// read by [`crate::editor::request_door`], and the crossing is run by `editor::apply_door_request`.
///
/// **The kit is not a parameter.** A project opens with one kit, and no operation switches kits, so a
/// request that named one would have nothing to switch to.
pub mod door_open {
    use super::*;
    use crate::tiles::{Door, Mode};
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// `kit`, `map` or `rigs`.
        pub door: String,
        /// A panel that door shows, by its label (e.g. `meshes`). Absent opens the door on its first
        /// panel.
        #[serde(default)]
        pub tab: Option<String>,
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            let doors: Vec<String> = Door::ALL.iter().map(|d| d.label().to_lowercase()).collect();
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["door"],
                "properties": {
                    "door": {
                        "type": "string",
                        "enum": doors,
                        "description": "The door to cross to. kit makes reusable content; map builds the level; rigs is the rig bench."
                    },
                    "tab": {
                        "type": "string",
                        "description": "A panel that door shows, e.g. meshes. Absent opens the door on its first panel."
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "this is not a door to open: {e}. The fields are `door` and `tab`, and nothing else."
                ))
            })
        }
    }

    pub struct DoorOpen;

    /// **The answer to a door request, validated.** `plan` and `commit` share it.
    ///
    /// `read_map` is the Map door's read. `plan` does it, so a map that will not open is refused
    /// before the answer says "applied". `commit` does not read it again: `dispatch_as` has just
    /// planned, and the crossing reads the map once more. The same file was parsed three times per
    /// request before this (cycle 8, gap 12).
    fn door_answer(
        ctx: &Ctx,
        params: &serde_json::Value,
        read_map: bool,
    ) -> Result<serde_json::Value, Refusal> {
        let p = Params::parse(params)?;
        let Some(door) = Door::ALL
            .into_iter()
            .find(|d| d.label().eq_ignore_ascii_case(&p.door))
        else {
            let names: Vec<String> = Door::ALL.iter().map(|d| d.label().to_lowercase()).collect();
            return Err(Refusal::new(format!(
                "`{}` is not a door. The doors are {}.",
                p.door,
                names.join(", ")
            )));
        };
        let tab = match p.tab.as_deref() {
            None => None,
            Some(name) => {
                let Some(mode) = Mode::ALL
                    .iter()
                    .copied()
                    .find(|m| m.label().eq_ignore_ascii_case(name))
                else {
                    let all: Vec<String> =
                        Mode::ALL.iter().map(|m| m.label().to_lowercase()).collect();
                    return Err(Refusal::new(format!(
                        "`{name}` is not a panel. The panels are {}.",
                        all.join(", ")
                    )));
                };
                if !door.tabs().contains(&mode) {
                    let shown: Vec<String> =
                        door.tabs().iter().map(|m| m.label().to_lowercase()).collect();
                    return Err(Refusal::new(format!(
                        "the {} door does not show `{name}`. It shows {}.",
                        door.label().to_lowercase(),
                        shown.join(", ")
                    )));
                }
                Some(mode)
            }
        };
        // **The Map door needs a map to show:** with no map open it would have nothing to cross onto,
        // and the crossing would be a silent no-op.
        let map_open = ctx.open.is_some()
            || ctx
                .project
                .as_ref()
                .and_then(|project| project.open_map_name.as_ref())
                .is_some();
        if door == Door::Map && !map_open {
            return Err(Refusal::new(
                "the Map door needs a map, and no map is open. Open a map first, then the Map door \
                 has something to show.",
            ));
        }
        // **The Map door crosses onto the map on disk, and a map that will not open cannot be crossed
        // to.** The crossing reads it through this same `OpenMap::open`, so the read happens here, before
        // the answer says "applied". It writes nothing.
        if read_map
            && door == Door::Map
            && ctx.open.is_none()
            && let Some(project) = ctx.project.as_ref()
            && let Some(name) = project.open_map_name.as_deref()
        {
            crate::project::OpenMap::open(project, name).map_err(Refusal::new)?;
        }
        // **Leaving a map with unsaved edits is the person's question.** The request is accepted and
        // the answer names what it waits on: the editor asks the leaving question, and this operation
        // cannot answer it for them.
        let waiting = door != Door::Map && ctx.open.as_ref().is_some_and(|open| open.dirty());
        let mut answer = json!({
            "door": door.label().to_lowercase(),
            "tab": tab.map(|m| m.label().to_lowercase()),
        });
        if waiting {
            answer["waiting"] = json!("the map has unsaved changes");
        }
        Ok(answer)
    }

    impl Operation for DoorOpen {
        fn name(&self) -> &'static str {
            "door_open"
        }

        fn undo(&self) -> Undo {
            Undo::None
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            door_answer(ctx, params, true)
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let plan = door_answer(ctx, params, false)?;
            let door = plan["door"].as_str().unwrap_or("");
            let target = match plan["tab"].as_str() {
                Some(tab) => format!("the {door} door on {tab}"),
                None => format!("the {door} door"),
            };
            let summary = if plan.get("waiting").is_some() {
                format!(
                    "asked for {target}; the map has unsaved changes, so the leaving question comes first"
                )
            } else {
                format!("opened {target}")
            };
            Ok(Outcome {
                summary,
                affected: 0,
                saved: false,
                inverse: None,
                after: Some(plan),
            })
        }
    }
}

/// **`piece_select`: point the kit door's Meshes shelf at one library piece.**
///
/// The shelf row click and an agent's request are this one operation (E-044). It changes no
/// document, so `Undo::None`. The plan validates only what the library knows: the id names a piece.
/// Whether the Meshes shelf is on screen, and whether the filter shows the piece, are editor state,
/// so `ops_brp::operation_apply` checks them for agents before dispatch.
pub mod piece_select {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The id of a library piece, e.g. `site/wall`.
        pub id: String,
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id"],
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The id of a library piece on the Meshes shelf, e.g. site/wall. A piece the shelf filter hides is refused."
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "this is not a piece to select: {e}. The one field is `id`."
                ))
            })
        }
    }

    pub struct PieceSelect;

    /// **The answer to a selection, validated.** `plan` and `commit` share it.
    fn selection_answer(ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = Params::parse(params)?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new(
                "no map or kit is open, so there is no library to select a piece from.",
            ));
        };
        let ids: Vec<&str> = project
            .library
            .descriptors
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        if !ids.contains(&p.id.as_str()) {
            let hint = match nearest_ids(&ids, &p.id) {
                near if near.is_empty() => "The library has no pieces.".to_string(),
                near => format!("Close ids: {}.", near.join(", ")),
            };
            return Err(Refusal::new(format!(
                "`{}` is not a library piece. {hint}",
                p.id
            )));
        }
        Ok(json!({ "selected": p.id }))
    }

    impl Operation for PieceSelect {
        fn name(&self) -> &'static str {
            "piece_select"
        }
        fn undo(&self) -> Undo {
            Undo::None
        }
        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }
        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            selection_answer(ctx, params)
        }
        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let after = selection_answer(ctx, params)?;
            let id = after["selected"].as_str().unwrap_or_default().to_string();
            Ok(Outcome {
                summary: format!("selected `{id}`"),
                affected: 0,
                saved: false,
                inverse: None,
                after: Some(after),
            })
        }
    }
}

/// **Remove one placement, or put back what a logged removal took.**
///
/// **A removal takes the piece and what stands on it**, because a stack goes whole
/// (`stack::group_of`): removing a host alone would leave its riders pointing at a placement that
/// no longer exists, and the map would refuse to load. So the undo is the group, put back at the
/// rows it held.
///
/// **A put-back is accepted only against the log.** The registry has no other way to add a
/// placement, so a put-back the log did not record would be one.
pub mod placement_remove {
    use super::*;
    use bevy_emerge_core::map::Placed;
    use serde_json::json;

    /// **One row a removal took**: the index it held and the placement as the file wrote it.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Row {
        pub index: usize,
        pub placed: serde_json::Value,
    }

    /// **What an undo puts back**: the piece the removal was asked for, and the rows it took.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct PutBack {
        pub head: String,
        pub rows: Vec<Row>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The placement to remove. What stands on it goes too.
        #[serde(default)]
        pub id: Option<String>,
        /// **Only an undo sends this**: puts back the rows a logged removal took.
        #[serde(default)]
        pub put_back: Option<PutBack>,
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The placement to remove. A piece standing on it goes too."
                    },
                    "put_back": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["head", "rows"],
                        "description": "Only an undo sends this: it restores the pieces a logged removal took. It is not a way to add a piece.",
                        "properties": {
                            "head": {"type": "string"},
                            "rows": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "required": ["index", "placed"],
                                    "properties": {
                                        "index": {"type": "integer", "minimum": 0},
                                        "placed": {"type": "object"}
                                    }
                                }
                            }
                        }
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal; and exactly one
        /// of `id` and `put_back`, so a request cannot say both.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not a removal: {e}. The fields are `id`, or `put_back` for an undo, and \
                     nothing else."
                ))
            })?;
            match (&p.id, &p.put_back) {
                (Some(_), None) | (None, Some(_)) => Ok(p),
                _ => Err(Refusal::new(
                    "a removal names one placement by `id`, or puts back what a removal took with \
                     `put_back`, and not both. Nothing was removed.",
                )),
            }
        }
    }

    /// **The position of a placement by id**, or `None` when this map does not hold it.
    fn index_of(open: &OpenMap, id: &str) -> Option<usize> {
        open.map.placements.iter().position(|q| q.id == id)
    }

    /// **Whether the log holds a removal that took exactly these rows.** A put-back's parameters are
    /// the removal's recorded inverse, so equality with that inverse is the proof.
    fn logged_removal(project: &Project, pb: &PutBack) -> bool {
        let Ok(want) = serde_json::to_value(pb) else {
            return false;
        };
        let text = std::fs::read_to_string(oplog_path(project)).unwrap_or_default();
        text.lines()
            .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
            .any(|e| {
                e.op == "placement_remove"
                    && e.outcome == "applied"
                    && e.inverse.as_ref().and_then(|i| i.get("put_back")) == Some(&want)
            })
    }

    /// **The rows a put-back restores, checked before anything is written.** Each must be a piece
    /// this map does not hold, the rows must be what a logged removal took, and each must land at an
    /// index the map can take once the earlier rows are in.
    fn restorable(
        project: &Project,
        open: &OpenMap,
        pb: &PutBack,
    ) -> Result<Vec<(usize, Placed)>, Refusal> {
        if pb.rows.is_empty() {
            return Err(Refusal::new(
                "a put-back has no rows, so nothing was put back.",
            ));
        }
        if !logged_removal(project, pb) {
            return Err(Refusal::new(
                "no logged removal took these pieces, so they cannot be put back. An undo restores \
                 only what a removal in the log took. Nothing was changed.",
            ));
        }
        let mut rows: Vec<(usize, Placed)> = Vec::with_capacity(pb.rows.len());
        for row in &pb.rows {
            let placed: Placed = serde_json::from_value(row.placed.clone()).map_err(|e| {
                Refusal::new(format!(
                    "a restored row is not a placement: {e}. Nothing was put back."
                ))
            })?;
            if index_of(open, &placed.id).is_some() || rows.iter().any(|(_, q)| q.id == placed.id) {
                return Err(Refusal::new(format!(
                    "`{}` is already on this map, so putting it back would make two. Nothing was put \
                     back.",
                    placed.id
                )));
            }
            rows.push((row.index, placed));
        }
        rows.sort_by_key(|(i, _)| *i);
        for (n, (i, _)) in rows.iter().enumerate() {
            if *i > open.map.placements.len() + n {
                return Err(Refusal::new(format!(
                    "row {i} is past the end of the map, so it cannot go back there. Nothing was put \
                     back."
                )));
            }
        }
        Ok(rows)
    }

    /// **Take a removal out**, and the group it took with it, then save. A refused save puts every
    /// row back, so the editor and the file agree.
    fn remove(project: &Project, open: &mut OpenMap, id: &str) -> Result<Outcome, Refusal> {
        let Some(index) = index_of(open, id) else {
            return Err(Refusal::new(format!(
                "no placement is called `{id}` on this map, so nothing was removed."
            )));
        };
        // **A stack goes whole**, by the same function the editor's delete has always used.
        let mut group = bevy_emerge_core::stack::group_of(&open.map, index);
        group.sort_unstable();
        let mut rows: Vec<Row> = Vec::with_capacity(group.len());
        for &i in &group {
            let placed = serde_json::to_value(&open.map.placements[i]).map_err(|e| {
                Refusal::new(format!(
                    "cannot record `{}` for the undo: {e}. Nothing was removed.",
                    open.map.placements[i].id
                ))
            })?;
            rows.push(Row { index: i, placed });
        }
        let ids: Vec<String> = group
            .iter()
            .map(|&i| open.map.placements[i].id.clone())
            .collect();
        // **Back to front**: removing an earlier row shifts every later one down.
        let mut taken: Vec<(usize, Placed)> = Vec::with_capacity(group.len());
        for &i in group.iter().rev() {
            taken.push((i, open.map.placements.remove(i)));
        }
        if let Err(e) = open.save(project) {
            // **Ascending again**, the order the rows went back in.
            for (i, placed) in taken.into_iter().rev() {
                open.map.placements.insert(i, placed);
            }
            return Err(Refusal::new(format!(
                "the file refused the removal, so nothing was written and the piece is still there: {e}"
            )));
        }
        let summary = match ids.len() {
            1 => format!("removed {id}"),
            n => format!("removed {id} and {} on it", n - 1),
        };
        let inverse = json!({"put_back": PutBack { head: id.to_owned(), rows }});
        Ok(Outcome {
            summary,
            affected: ids.len(),
            saved: true,
            inverse: Some(inverse),
            // **How many of the pieces are still on the map**: none, after a removal. An undo checks
            // that count, so a piece put back or taken away by hand blocks it.
            after: Some(json!({"map": open.map.name, "ids": ids, "on_map": 0})),
        })
    }

    /// **Put a logged removal's pieces back**, at the rows they held, then save.
    fn put_back(project: &Project, open: &mut OpenMap, pb: &PutBack) -> Result<Outcome, Refusal> {
        let rows = restorable(project, open, pb)?;
        let ids: Vec<String> = rows.iter().map(|(_, p)| p.id.clone()).collect();
        // **Ascending**: each row lands at its own index once the earlier rows are back.
        for (i, placed) in rows {
            open.map.placements.insert(i, placed);
        }
        if let Err(e) = open.save(project) {
            for id in &ids {
                if let Some(i) = index_of(open, id) {
                    open.map.placements.remove(i);
                }
            }
            return Err(Refusal::new(format!(
                "the file refused the put-back, so nothing was written and the pieces are not back: {e}"
            )));
        }
        let summary = match ids.len() {
            1 => format!("put back {}", ids[0]),
            n => format!("put back {} and {} on it", pb.head, n - 1),
        };
        Ok(Outcome {
            summary,
            affected: ids.len(),
            saved: true,
            // **Its inverse removes the piece it was put back for**: the same group, now the map's again.
            inverse: Some(json!({"id": pb.head})),
            after: Some(json!({"map": open.map.name, "ids": ids, "on_map": ids.len()})),
        })
    }

    /// **Remove one placement, and what stands on it.**
    pub struct PlacementRemove;

    impl Operation for PlacementRemove {
        fn name(&self) -> &'static str {
            "placement_remove"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        /// **A put-back is the undo of a logged removal and nothing else** (gap 2), so only [`undo`]
        /// may send one. `parse` refuses a request that names both `id` and `put_back`.
        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            Params::parse(params).is_ok_and(|p| p.put_back.is_some())
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        /// **The pieces a change names, and how many the map still holds** — the part an undo checks.
        /// A removal's undo is safe only while none of them is back on the map; a put-back's only while
        /// all of them are.
        fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
            let open = ctx.open.as_ref()?;
            let ids: Vec<String> = after
                .get("ids")?
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let on_map = ids.iter().filter(|id| index_of(open, id).is_some()).count();
            Some(json!({"map": open.map.name, "ids": ids, "on_map": on_map}))
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new(
                    "no map is open, so there is no placement to remove.",
                ));
            };
            if let Some(pb) = &p.put_back {
                let Some(project) = ctx.project.as_ref() else {
                    return Err(Refusal::new("no project is open."));
                };
                restorable(project, open, pb)?;
                return Ok(json!({"put_back": pb.head, "rows": pb.rows.len()}));
            }
            let Some(id) = p.id.as_deref() else {
                return Err(Refusal::new("a removal names one placement by `id`."));
            };
            let Some(index) = index_of(open, id) else {
                return Err(Refusal::new(format!(
                    "no placement is called `{id}` on this map, so nothing was removed."
                )));
            };
            let group = bevy_emerge_core::stack::group_of(&open.map, index);
            Ok(json!({"id": id, "removes": group.len()}))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open."));
            };
            let Some(mut open) = ctx.open.take() else {
                return Err(Refusal::new(
                    "no map is open, so there is no placement to remove.",
                ));
            };
            let done = match (&p.put_back, p.id.as_deref()) {
                (Some(pb), _) => put_back(project, &mut open, pb),
                (None, Some(id)) => remove(project, &mut open, id),
                (None, None) => Err(Refusal::new("a removal names one placement by `id`.")),
            };
            ctx.open = Some(open);
            done
        }
    }
}

/// **Remove one stamp**: a composition placed on the map, taken off `map.stamps` by its id.
///
/// The shape of [`placement_remove`], one row wide: a stamp is one entry in the file, and what it
/// expands to is not written, so a removal takes exactly the one entry and its undo puts that entry
/// back at the index it held.
pub mod stamp_remove {
    use super::*;
    use bevy_emerge_core::composition::Stamped;
    use serde_json::json;

    /// **What an undo puts back**: the stamp the removal took, and the index it held.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct PutBack {
        pub index: usize,
        pub stamp: serde_json::Value,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// The stamp to remove, by its id.
        #[serde(default)]
        pub id: Option<String>,
        /// **Only an undo sends this**: puts back the stamp a logged removal took.
        #[serde(default)]
        pub put_back: Option<PutBack>,
    }

    impl Params {
        /// **The JSON schema**, which is what an agent reads.
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The stamp to remove, by its id, as the map's stamps list names it."
                    },
                    "put_back": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["index", "stamp"],
                        "description": "Only an undo sends this: it restores the stamp a logged removal took. It is not a way to add a stamp.",
                        "properties": {
                            "index": {"type": "integer", "minimum": 0},
                            "stamp": {"type": "object"}
                        }
                    }
                }
            })
        }

        /// **The parameters, parsed.** `deny_unknown_fields`, so a typo is a refusal; and exactly one
        /// of `id` and `put_back`, so a request cannot say both.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not a stamp removal: {e}. The fields are `id`, or `put_back` for an undo, \
                     and nothing else."
                ))
            })?;
            match (&p.id, &p.put_back) {
                (Some(_), None) | (None, Some(_)) => Ok(p),
                _ => Err(Refusal::new(
                    "a stamp removal names one stamp by `id`, or puts back what a removal took with \
                     `put_back`, and not both. Nothing was removed.",
                )),
            }
        }
    }

    /// **The position of a stamp by id**, or `None` when this map does not hold it.
    fn index_of(open: &OpenMap, id: &str) -> Option<usize> {
        open.map.stamps.iter().position(|s| s.id == id)
    }

    /// **Whether the log holds a removal that took exactly this stamp at this index.** A put-back's
    /// parameters are the removal's recorded inverse, so equality with that inverse is the proof.
    fn logged_removal(project: &Project, pb: &PutBack) -> bool {
        let Ok(want) = serde_json::to_value(pb) else {
            return false;
        };
        let text = std::fs::read_to_string(oplog_path(project)).unwrap_or_default();
        text.lines()
            .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
            .any(|e| {
                e.op == "stamp_remove"
                    && e.outcome == "applied"
                    && e.inverse.as_ref().and_then(|i| i.get("put_back")) == Some(&want)
            })
    }

    /// **The stamp a put-back restores, checked before anything is written.** The log must hold the
    /// removal, the stamp must not already be on the map, and its index must be one the map can take.
    fn restorable(project: &Project, open: &OpenMap, pb: &PutBack) -> Result<Stamped, Refusal> {
        if !logged_removal(project, pb) {
            return Err(Refusal::new(
                "no logged removal took this stamp, so it cannot be put back. An undo restores only \
                 what a removal in the log took. Nothing was changed.",
            ));
        }
        let stamp: Stamped = serde_json::from_value(pb.stamp.clone()).map_err(|e| {
            Refusal::new(format!(
                "the restored stamp is not a stamp: {e}. Nothing was put back."
            ))
        })?;
        if index_of(open, &stamp.id).is_some() {
            return Err(Refusal::new(format!(
                "`{}` is already on this map, so putting it back would make two. Nothing was put back.",
                stamp.id
            )));
        }
        if pb.index > open.map.stamps.len() {
            return Err(Refusal::new(format!(
                "index {} is past the end of the map's stamps, so it cannot go back there. Nothing was \
                 put back.",
                pb.index
            )));
        }
        Ok(stamp)
    }

    /// **Take a stamp out**, then save. A refused save puts it back where it was, so the editor and
    /// the file agree.
    fn remove(project: &Project, open: &mut OpenMap, id: &str) -> Result<Outcome, Refusal> {
        let Some(index) = index_of(open, id) else {
            return Err(Refusal::new(format!(
                "no stamp is called `{id}` on this map, so nothing was removed."
            )));
        };
        let taken = open.map.stamps.remove(index);
        let stamp = match serde_json::to_value(&taken) {
            Ok(v) => v,
            Err(e) => {
                open.map.stamps.insert(index, taken);
                return Err(Refusal::new(format!(
                    "cannot record `{id}` for the undo: {e}. Nothing was removed."
                )));
            }
        };
        if let Err(e) = open.save(project) {
            open.map.stamps.insert(index, taken);
            return Err(Refusal::new(format!(
                "the file refused the removal, so nothing was written and the stamp is still there: {e}"
            )));
        }
        Ok(Outcome {
            summary: format!("removed stamp {id}"),
            affected: 1,
            saved: true,
            inverse: Some(json!({"put_back": PutBack { index, stamp }})),
            // **How many of the stamps are still on the map**: none, after a removal.
            after: Some(json!({"map": open.map.name, "ids": [id], "on_map": 0})),
        })
    }

    /// **Put a logged removal's stamp back**, at the index it held, then save.
    fn put_back(project: &Project, open: &mut OpenMap, pb: &PutBack) -> Result<Outcome, Refusal> {
        let stamp = restorable(project, open, pb)?;
        let id = stamp.id.clone();
        open.map.stamps.insert(pb.index, stamp);
        if let Err(e) = open.save(project) {
            open.map.stamps.remove(pb.index);
            return Err(Refusal::new(format!(
                "the file refused the put-back, so nothing was written and the stamp is not back: {e}"
            )));
        }
        Ok(Outcome {
            summary: format!("put back stamp {id}"),
            affected: 1,
            saved: true,
            // **Its inverse removes the stamp it was put back for**, as a fresh logged removal.
            inverse: Some(json!({"id": id})),
            after: Some(json!({"map": open.map.name, "ids": [id], "on_map": 1})),
        })
    }

    /// **Remove one stamp.**
    pub struct StampRemove;

    impl Operation for StampRemove {
        fn name(&self) -> &'static str {
            "stamp_remove"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        /// **A put-back is the undo of a logged removal and nothing else**, so only [`undo`] may send
        /// one. `parse` refuses a request that names both `id` and `put_back`.
        fn only_as_undo(&self, params: &serde_json::Value) -> bool {
            Params::parse(params).is_ok_and(|p| p.put_back.is_some())
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        /// **The stamps a change names, and how many the map still holds**: the part an undo checks.
        fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
            let open = ctx.open.as_ref()?;
            let ids: Vec<String> = after
                .get("ids")?
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let on_map = ids.iter().filter(|id| index_of(open, id).is_some()).count();
            Some(json!({"map": open.map.name, "ids": ids, "on_map": on_map}))
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new(
                    "no map is open, so there is no stamp to remove.",
                ));
            };
            if let Some(pb) = &p.put_back {
                let Some(project) = ctx.project.as_ref() else {
                    return Err(Refusal::new("no project is open."));
                };
                restorable(project, open, pb)?;
                return Ok(json!({"put_back": pb.stamp.get("id"), "index": pb.index}));
            }
            let Some(id) = p.id.as_deref() else {
                return Err(Refusal::new("a stamp removal names one stamp by `id`."));
            };
            if index_of(open, id).is_none() {
                return Err(Refusal::new(format!(
                    "no stamp is called `{id}` on this map, so nothing was removed."
                )));
            }
            Ok(json!({"id": id, "removes": 1}))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let p = Params::parse(params)?;
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open."));
            };
            let Some(mut open) = ctx.open.take() else {
                return Err(Refusal::new(
                    "no map is open, so there is no stamp to remove.",
                ));
            };
            let done = match (&p.put_back, p.id.as_deref()) {
                (Some(pb), _) => put_back(project, &mut open, pb),
                (None, Some(id)) => remove(project, &mut open, id),
                (None, None) => Err(Refusal::new("a stamp removal names one stamp by `id`.")),
            };
            ctx.open = Some(open);
            done
        }
    }
}

/// **A map's name, changed, and the file moves with it.**
///
/// **The name becomes the file**, so the name is forced the way every map name is, and a name
/// another map in the project has is refused before anything is written. The N key and an agent
/// both come here, so the rule has one answer.
pub mod map_rename {
    use serde_json::json;

    use super::map_set_settings::settings_of;
    use super::{Ctx, Operation, Outcome, Refusal, Undo};
    use bevy_emerge_core::naming::{map_file_name, to_snake_case};

    /// **What the operation takes.** One field, the new name.
    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        pub name: String,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name"],
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The map's new name. It is forced to snake_case, the way every map name is, because the name becomes the file: `Town Two` is written as `town_two.map.ron`. It must not be empty, and no other map in the project may have it."
                    }
                }
            })
        }

        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "this is not a map's new name: {e}. The one field is `name`, and nothing else."
                ))
            })
        }
    }

    /// **The name, forced the way every map name is.** A name that is empty once forced is a
    /// refusal, because a map with no name has no file to be.
    fn checked_name(raw: &str) -> Result<String, Refusal> {
        let name = to_snake_case(raw.trim());
        if name.is_empty() {
            return Err(Refusal::new("a map needs a name; nothing was changed"));
        }
        Ok(name)
    }

    /// **The rules a rename rewrites**: every `example` that names `was` becomes `name`, in the
    /// file's own text, so its comments survive. `Ok(None)` when no rule names the map.
    ///
    /// **The rewrite is checked before anything is written.** The number of `example: "was"`
    /// strings must equal the rules that name it, and the new text must parse to the in-memory
    /// rules with that one name changed. A rewrite that misses a rule is the dangling example this
    /// operation exists to prevent, so a mismatch is a refusal, not a best effort.
    fn rules_renamed(
        project: &crate::project::Project,
        was: &str,
        name: &str,
    ) -> Result<Option<(String, String)>, Refusal> {
        let named = project.rules.blocks.iter().filter(|b| b.example == was).count();
        if named == 0 {
            return Ok(None);
        }
        let file = project.project_dir.join(bevy_emerge_core::rules::FILE);
        let refuse = |why: String| {
            Refusal::new(format!("the map was not renamed, and nothing was written: {why}"))
        };
        let old = std::fs::read_to_string(&file)
            .map_err(|e| refuse(format!("{}: {e}", file.display())))?;
        let needle = format!("example: \"{was}\"");
        if old.matches(&needle).count() != named {
            return Err(refuse(format!(
                "{} names `{was}` in a form this rename cannot rewrite; change its example by hand first",
                file.display()
            )));
        }
        let new = old.replace(&needle, &format!("example: \"{name}\""));
        let parsed = bevy_emerge_core::rules::Rules::parse(&new)
            .map_err(|e| refuse(format!("{}: {e}", file.display())))?;
        let expected = project
            .rules
            .blocks
            .iter()
            .map(|b| if b.example == was { name } else { b.example.as_str() });
        if !parsed.blocks.iter().map(|b| b.example.as_str()).eq(expected) {
            return Err(refuse(format!(
                "the rewrite of {} does not match the rules the editor holds",
                file.display()
            )));
        }
        Ok(Some((old, new)))
    }

    /// **The in-memory rules follow the rename**, so generate reads the example the file now names.
    fn set_example(project: &mut crate::project::Project, was: &str, name: &str) {
        for block in project.rules.blocks.iter_mut().filter(|b| b.example == was) {
            block.example = name.to_owned();
        }
    }

    /// **A map's name, changed.**
    pub struct MapRename;

    impl Operation for MapRename {
        fn name(&self) -> &'static str {
            "map_rename"
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
            ctx.open.as_ref().map(settings_of)
        }

        fn plan(
            &self,
            ctx: &Ctx,
            params: &serde_json::Value,
        ) -> Result<serde_json::Value, Refusal> {
            let p = Params::parse(params)?;
            let Some(open) = ctx.open.as_ref() else {
                return Err(Refusal::new(
                    "no map is open, so there is no name to change.",
                ));
            };
            let name = checked_name(&p.name)?;
            if name == open.map.name {
                return Err(Refusal::new(format!(
                    "the map is already `{name}`, so nothing changes."
                )));
            }
            let Some(project) = ctx.project.as_ref() else {
                return Err(Refusal::new("no project is open."));
            };
            // **A taken name is refused here**, before a byte is written. `OpenMap::save` would
            // refuse it too, but the refusal belongs where the author asked, with nothing moved.
            let target = project.maps_dir.join(map_file_name(&name));
            if target.exists() {
                return Err(Refusal::new(format!(
                    "`{name}` is already a map in this project, so this map cannot take that name. \
                     Rename this map to something else."
                )));
            }
            Ok(json!({"name": name, "was": open.map.name}))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            let name = checked_name(&Params::parse(params)?.name)?;
            let Some(mut open) = ctx.open.take() else {
                return Err(Refusal::new(
                    "no map is open, so there is no name to change.",
                ));
            };
            let was = open.map.name.clone();
            let Some(project) = ctx.project.as_mut() else {
                // **The map goes back before the refusal**, as the settings commit does: a
                // refusal that dropped it would leave the editor holding nothing.
                ctx.open = Some(open);
                return Err(Refusal::new("no project is open."));
            };
            // **The rules are part of the rename.** A rule names the map its grammar is learned
            // from, so a rename that moved the file and left that name behind would leave the block
            // unreadable. The rules file is written first; a refusal here changes nothing.
            let rules = match rules_renamed(project, &was, &name) {
                Ok(rules) => rules,
                Err(refusal) => {
                    ctx.open = Some(open);
                    return Err(refusal);
                }
            };
            let file = project.project_dir.join(bevy_emerge_core::rules::FILE);
            if let Some((_, new)) = &rules
                && let Err(e) = bevy_emerge_core::ron_surgery::save_atomic(&file, new)
            {
                ctx.open = Some(open);
                return Err(Refusal::new(format!(
                    "the map was not renamed, and nothing was written: {e}"
                )));
            }
            // **The save is the map's own gate.** It moves the file and removes the old one, and
            // it refuses a taken name rather than replacing it.
            open.map.name = name.clone();
            if let Err(e) = open.save(project) {
                // **The write landed; only the old file's removal failed.** Save has moved the map's
                // path to the new file, so the name stays with it, and the refusal tells the author
                // which file to delete. Putting the old name back here would leave a name no file holds.
                if open.map_path == project.maps_dir.join(map_file_name(&name)) {
                    set_example(project, &was, &name);
                    ctx.open = Some(open);
                    return Err(Refusal::new(e));
                }
                // **The map did not move, so the rules go back too.** If that restore itself fails,
                // the refusal says so: the rules file then names the new map, and the author must fix it.
                let mut message = format!("the map was not renamed, and nothing was written: {e}");
                if let Some((old, _)) = &rules
                    && let Err(restore) = bevy_emerge_core::ron_surgery::save_atomic(&file, old)
                {
                    message.push_str(&format!(
                        "; {} could not be put back ({restore}), and it now names `{name}`, so change its example by hand",
                        file.display()
                    ));
                }
                open.map.name = was;
                ctx.open = Some(open);
                return Err(Refusal::new(message));
            }
            set_example(project, &was, &name);
            let summary = if rules.is_some() {
                format!("renamed `{was}` to `{name}`, and the file moved with it; the rules' example follows")
            } else {
                format!("renamed `{was}` to `{name}`, and the file moved with it")
            };
            // **The inverse is the name it replaced**, and undo takes the same path: a rename back,
            // refused if that name is taken by now. The rules follow it back the same way.
            let inverse = json!({"name": was});
            ctx.open = Some(open);
            let after = ctx.open.as_ref().map(settings_of);
            Ok(Outcome {
                summary,
                affected: 1,
                saved: true,
                inverse: Some(inverse),
                after,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_emerge_core::composition::Stamped;
    use bevy_emerge_core::map::{Map, Placed};

    /// A map with three rows, so an index in the middle and an index at the end both exist.
    fn three() -> Map {
        let mut map = Map {
            name: "t".to_owned(),
            ..Map::default()
        };
        for (i, x) in [0.0, 3.0, 6.0].into_iter().enumerate() {
            map.placements.push(Placed {
                id: format!("wall@{i}"),
                descriptor: "wall".to_owned(),
                at: (x, 0.0),
                ..Placed::default()
            });
        }
        map
    }

    fn stamp(id: &str, x: f32) -> Stamped {
        Stamped {
            id: id.to_owned(),
            of: "tile_floor".to_owned(),
            at: (x, 0.0),
            yaw: 0.0,
            ..Stamped::default()
        }
    }

    /// **Applying a delta and then its inverse restores the exact map**, order included.
    ///
    /// The order is the half that is easy to get wrong: a restoration that appended would move a row
    /// from the middle of the file to its end on every undo/redo round trip, so `assert_eq` on the
    /// whole `Map` is the assertion — comparing only the ids would pass while the file churned.
    #[test]
    fn a_placement_delta_undoes_itself_exactly() {
        let mut map = three();
        let before = map.clone();

        // Remove the middle row, and add a new one at the end.
        let delta = RowDelta {
            remove_placements: vec!["wall@1".to_owned()],
            add_placements: vec![Indexed {
                index: 2,
                row: Placed {
                    id: "gen@1".to_owned(),
                    descriptor: "wall".to_owned(),
                    at: (9.0, 0.0),
                    ..Placed::default()
                },
            }],
            ..RowDelta::default()
        };
        let inverse = delta.apply(&mut map).expect("the delta applies");
        assert_eq!(
            map.placements.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            vec!["wall@0", "wall@2", "gen@1"],
            "the removal and the addition did not land where they were asked to"
        );

        inverse.apply(&mut map).expect("the inverse applies");
        assert_eq!(
            map, before,
            "applying a delta and then its inverse did not restore the map exactly — the rows are \
             equal but their order is not, which moves a piece in the file on every round trip"
        );
    }

    /// The same for stamps, which are a separate list and a separate bug surface.
    #[test]
    fn a_stamp_delta_undoes_itself_exactly() {
        let mut map = Map {
            name: "t".to_owned(),
            stamps: vec![stamp("s@0", 0.0), stamp("s@1", 3.0), stamp("s@2", 6.0)],
            ..Map::default()
        };
        let before = map.clone();

        let delta = RowDelta {
            remove_stamps: vec!["s@0".to_owned(), "s@2".to_owned()],
            add_stamps: vec![Indexed {
                index: 0,
                row: stamp("s@9", 9.0),
            }],
            ..RowDelta::default()
        };
        let inverse = delta.apply(&mut map).expect("the delta applies");
        assert_eq!(
            map.stamps.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["s@9", "s@1"],
            "two removals and one addition did not land as asked"
        );

        inverse.apply(&mut map).expect("the inverse applies");
        assert_eq!(map, before, "the stamp round trip did not restore the map exactly");
    }

    /// **A row that is no longer there is refused, and nothing is changed.**
    ///
    /// The refusal is the whole point: a delta recorded against a map that has since lost a row
    /// cannot be honoured, and half-applying it would leave the map in a state neither the undo stack
    /// nor the author describes.
    #[test]
    fn a_missing_row_is_refused_and_the_map_is_untouched() {
        let mut map = three();
        let before = map.clone();
        let delta = RowDelta {
            remove_placements: vec!["wall@0".to_owned(), "wall@99".to_owned()],
            ..RowDelta::default()
        };
        let refused = delta.apply(&mut map).expect_err("a missing row must be refused");
        assert!(
            refused.0.contains("wall@99"),
            "the refusal does not name the row it could not find: {refused}"
        );
        assert_eq!(
            map, before,
            "the delta was applied in part before it was refused, so a refused undo changed the map"
        );
    }

    /// An id that is already on the map is refused rather than duplicated.
    #[test]
    fn an_id_already_on_the_map_is_refused() {
        let mut map = three();
        let delta = RowDelta {
            add_placements: vec![Indexed {
                index: 0,
                row: Placed {
                    id: "wall@1".to_owned(),
                    ..Placed::default()
                },
            }],
            ..RowDelta::default()
        };
        let refused = delta.apply(&mut map).expect_err("a duplicate id must be refused");
        assert!(
            refused.0.contains("wall@1"),
            "the refusal does not name the duplicate: {refused}"
        );
        assert_eq!(map.placements.len(), 3, "the duplicate was added anyway");
    }
}
