//! **The view an operation changes** (E-002, #129): what a person is *looking through*, not what the
//! project keeps.
//!
//! A view verb — the grid rung, the problems panel, the shortcut overlay — writes no map, no file and no
//! undo step. Before this module an operation had no way to reach one at all, so those keys could only
//! ever be dispatched by a key, and "both front ends dispatch it" was untrue for them.
//!
//! **A value, not a resource.** `Ctx` carries `Option<View>` exactly as it carries `Option<OpenMap>`, and
//! the front ends carry it in and out. That is what lets a test prove a key and an MCP call are one
//! operation without either of them owning a `World`.

use crate::editor::Tool;
use crate::outliner::{Eyes, Locks};
use bevy_grimoire_core::grid::SnapLevel;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// **The rung by name, not by index.** `SnapLevel` lives in the engine-free core and knows nothing about
/// the wire; an operation's parameter is a contract with an agent, so `"fine"` is what an agent writes and
/// a reordering of the enum can never silently change a saved or requested rung.
fn rung_name(rung: SnapLevel) -> &'static str {
    match rung {
        SnapLevel::Tile => "tile",
        SnapLevel::Fine => "fine",
        SnapLevel::Finer => "finer",
    }
}

fn rung_from_name(name: &str) -> Option<SnapLevel> {
    match name {
        "tile" => Some(SnapLevel::Tile),
        "fine" => Some(SnapLevel::Fine),
        "finer" => Some(SnapLevel::Finer),
        _ => None,
    }
}

/// **What a person is looking through**, as much of it as an operation may change.
///
/// Deliberately narrow: a field is added when an operation needs it, not because the editor has one.
/// Everything here is session state — none of it is saved to the project, which is why no operation that
/// writes this declares `Undo::Snapshot`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// The snap ladder's rung: what a piece is nudged to, and what the grid is drawn at.
    #[serde(default, serialize_with = "rung_out", deserialize_with = "rung_in")]
    pub rung: SnapLevel,
    /// **The armed tool** — what a click on the map does. `Tool::Place` is the default and what
    /// every other tool returns to, so this is the same value `EditorState.tool` holds.
    #[serde(default, serialize_with = "tool_out", deserialize_with = "tool_in")]
    pub tool: Tool,
    /// **Whether the snip is armed** — `Cmd+\` arms it, the same key disarms it, and a completed
    /// rectangle disarms it too. A capture mode, not a map change.
    #[serde(default)]
    pub snip_armed: bool,
    /// **Which filter box the keyboard is in**, when the key that moves it is the only way there.
    /// Cleared by the front end when the box loses focus; `None` means "no box is being typed in".
    #[serde(default)]
    pub filter_focus: Option<bool>,
    /// **Whether the journal is up** (`Cmd+E`). Not persisted: a panel a person opened to read one
    /// refusal should not meet them again next launch, so this is session state like the rung.
    #[serde(default)]
    pub journal_open: bool,
    /// **Whether the rig bench's ghost is staged** (`G`): the translucent second figure playing the measured
    /// gait over the declared one. Session state like the journal, and the same `BenchAb` the chip lights.
    #[serde(default)]
    pub ghost: bool,
    /// **What the outliner's eye has hidden** (UI-51, #237): pieces and whole layers, out of the viewport for this
    /// session. Not serialised while empty, so a view that hides nothing is the view it always was.
    #[serde(default, skip_serializing_if = "Eyes::is_empty")]
    pub eyes: Eyes,
    /// **The pieces the outliner has locked** against the viewport's clicks (UI-51, #237), under the same rule.
    #[serde(default, skip_serializing_if = "Locks::is_empty")]
    pub locks: Locks,
    /// **Which composition the picker's cursor is on** (E-002, #129 slice 22): an index into the project's compositions,
    /// the same order the rows are drawn in. Session state like the rung; not serialised at 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub picker_selected: usize,
    /// **Whether the arrows walk the composition picker** rather than the palette (`P`). Not serialised while false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub picker_active: bool,
    /// **The COMPOSE tab's cursors** (E-002, #129 slice 23): which group, which member, and which of the two lists the
    /// arrows walk. Session state like the picker's; not serialised while it is where a session opens.
    #[serde(default, skip_serializing_if = "ComposeCursor::is_default")]
    pub compose: ComposeCursor,
    /// **The rig bench's list and where its cursor is** (E-002, #129 slice 24). Empty until the front end fills it from
    /// the bench it is looking at, so a call with no bench in hand is refused.
    #[serde(default, skip_serializing_if = "RigCursor::is_default")]
    pub rig: RigCursor,
    /// **The Tiles page's row cursor**, when that page is up (E-002, #129 slice 24): `Build::browsing`, and how many rows
    /// the page draws. `None` is the editor not being on the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_page: Option<TilePage>,
    /// **Which packs are folded away, and whether the `EXCLUDED` band is open** on the candidate list (E-002, #129
    /// slice 25): session state like the rest. Not serialised while nothing is folded and the band is closed.
    #[serde(default, skip_serializing_if = "PackFold::is_default")]
    pub packs: PackFold,
    /// **Which filter box has the keyboard**, by the name `pane_name` gives it (E-002, #129 slice 25). `None` is no box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_pane: Option<String>,
    /// **Whether this answer means the named box's search to start fresh** (E-002, #129 review M1). The clear travels
    /// with the answer, so `filter_focus` on the box that already has the keyboard still clears it, as the key and a
    /// click do — the operation's own sentence is "its search starts fresh", and a comparison against which box has
    /// the keyboard cannot say that. Not serialised while false: most answers touch no box.
    #[serde(default, skip_serializing_if = "is_false")]
    pub filter_fresh: bool,
    /// **The rig bench's stage** (E-002, #129 slice 26): whether the figure plays, the phase a scrub holds, and the camera
    /// preset. Session state like the rest; not serialised while it is where a session opens (playing, phase 0, figure).
    #[serde(default, skip_serializing_if = "StageCursor::is_default")]
    pub stage: StageCursor,
    /// **The lattice grid's cursor on the Meshes tab** (E-002, #129 slice 27), when a piece is selected: the grid's divisions
    /// and where the cursor stands. `None` is no grid to step on, so a step is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lattice: Option<LatticeCursor>,
    /// **The tile in hand on the Tiles tab** (E-002, #129 slice 28), when one is open: the draft, which member has the focus,
    /// how deep the arrow ladder is, and whether the piece is taken. `None` is no tile open, so a verb that shapes one is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_hand: Option<TileHand>,
}

/// **The tile being assembled**, as a value an operation can change (E-002, #129 slice 28).
///
/// The same facts `build::Build` holds as `open`, `focus`, `depth` and `placing`. The draft is session state, not the
/// project: nothing is written to `compositions.yaml` until `Cmd+S` saves it (`composition_define`), so an operation on it
/// writes no file and takes no undo step of its own. The front end carries it in and applies what comes back, written only
/// when it moved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileHand {
    /// The tile as it stands: its envelope, its members and their places.
    pub open: bevy_grimoire_core::composition::Composition,
    /// Which member has the focus, as an index into `open`'s members. Out of range reads as none.
    #[serde(default)]
    pub focus: usize,
    /// How deep the arrow ladder is, latched: `0` walks centre to flush in one press.
    #[serde(default)]
    pub depth: u32,
    /// Whether the arrows steer the tile rather than the library list: the piece is taken.
    #[serde(default)]
    pub placing: bool,
}

/// **Where the lattice grid's cursor is**, as a value an operation can move (E-002, #129 slice 27).
///
/// The same facts `tiles::CellEdit` holds as `layer` and `at`, with the divisions of the lattice the focused piece stands on
/// (`tiles::focused_div`), which the front end derives and carries in so the walk stops at the grid's own edges. The rules
/// for moving the cursor are written once, in `ops::lattice_cursor_step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatticeCursor {
    /// How many cells the lattice has along x, y (layers) and z.
    pub div: (u32, u32, u32),
    /// The layer the grid is showing.
    #[serde(default)]
    pub layer: u32,
    /// The selected cell, if any.
    #[serde(default)]
    pub at: Option<(u32, u32, u32)>,
}

/// **Where the rig bench's stage is**, as a value an operation can move (E-002, #129 slice 26).
///
/// The same three facts `anim_stage::BenchScrub` and `BenchCamera` hold as `playing`, `phase` and the preset. The rules for
/// moving them are written once, in `ops::stage`; the front end carries this in and applies what comes back. `View` lost
/// its `Eq` for the phase: a float is the cursor, and a float is only `PartialEq`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageCursor {
    /// **Whether the figure plays**: it advances at the mixture's own authored speed. Paused, the phase is the scrub's.
    #[serde(default = "playing_by_default")]
    pub playing: bool,
    /// The shared phase, `[0, 1)`, which a scrub moves while the figure is paused.
    #[serde(default)]
    pub phase: f32,
    /// The stage camera's framing.
    #[serde(default)]
    pub camera: crate::anim_stage::CamPreset,
}

/// **Alive by default**, as the bench opens: the figure walks in place the moment the tab does.
fn playing_by_default() -> bool {
    true
}

impl Default for StageCursor {
    fn default() -> Self {
        StageCursor { playing: true, phase: 0.0, camera: crate::anim_stage::CamPreset::default() }
    }
}

impl StageCursor {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// **Where the COMPOSE tab's cursors are**, as a value an operation can move (E-002, #129 slice 23).
///
/// The same three facts `compose::ComposeState` holds as `selected`, `member` and `focus`, and the editor's own state
/// is what a front end carries in and applies out. The rules for moving them are written once, in
/// `ops::compose_cursor`, and this is the one place that says choosing a group puts the member cursor back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposeCursor {
    /// Index into the project's compositions, in the order the tab draws them.
    #[serde(default)]
    pub selected: usize,
    /// Index into the selected group's members: the second cursor, under the first.
    #[serde(default)]
    pub member: usize,
    /// **Whether the arrows walk the MEMBERS list** rather than the COMPOSITIONS list.
    #[serde(default)]
    pub members_focused: bool,
}

impl ComposeCursor {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// **The rig bench's list and cursor**, as a value an operation can move (E-002, #129 slice 24).
///
/// The names are the list the bench draws, in manifest order, so the walk goes round the list a person is looking at
/// and not a second reading of the file; the front end carries them in with the cursor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigCursor {
    /// The rig names, in manifest order.
    #[serde(default)]
    pub names: Vec<String>,
    /// Which rig, as an index into `names`.
    #[serde(default)]
    pub selected: usize,
    /// **Whether the bench shows the project-wide summary** rather than one rig. Walking to a rig puts it back on one.
    #[serde(default)]
    pub all: bool,
}

impl RigCursor {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// **The Tiles page's cursor**: which row, of how many the page draws (`+ New Tile`, the kit's tiles and the draft).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TilePage {
    /// The row the cursor is on: `0` is `+ New Tile`.
    pub row: usize,
    /// How many rows the page draws, `+ New Tile` included.
    pub rows: usize,
}

/// **The candidate list's folds**: the packs folded away, by the folder name a heading spells, and whether the
/// `EXCLUDED` band at the bottom is open. The same two facts `tiles::ImportState` holds as `folded_packs` and
/// `excluded_open`; a `BTreeSet` here, so two views of the same folds are equal and print the same.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackFold {
    /// The packs whose rows are folded away.
    #[serde(default)]
    pub folded: std::collections::BTreeSet<String>,
    /// Whether the `EXCLUDED` band is open. Closed by default: the group exists to get its packs out of the way.
    #[serde(default)]
    pub excluded_open: bool,
}

impl PackFold {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// **A filter box by name, not by variant**, for the reason a rung is: an agent writes `"candidates"` and a reordering
/// of the enum can never silently hand it another box.
pub(crate) fn pane_name(pane: crate::filter::Pane) -> &'static str {
    use crate::filter::Pane;
    match pane {
        Pane::Palette => "palette",
        Pane::Candidates => "candidates",
        Pane::Rigs => "rigs",
        Pane::Tags => "tags",
    }
}

pub(crate) fn pane_from_name(name: &str) -> Option<crate::filter::Pane> {
    use crate::filter::Pane;
    Pane::ALL.into_iter().find(|p| pane_name(*p) == name)
}

/// **The filter boxes an agent may name**, in the order `Pane::ALL` lists them, derived so the schema's enum and the parse
/// cannot disagree.
pub(crate) fn pane_names() -> Vec<&'static str> {
    crate::filter::Pane::ALL.into_iter().map(pane_name).collect()
}

/// **Choose a group, and put the member cursor back on the first member.**
///
/// A different group has different members, so leaving the old index would point the seat verbs at whatever
/// happened to sit there. Focus is deliberately not touched: the carousel must move the group cursor without
/// disturbing which list has the arrows. `compose::select_group` (a click on a row or a miniature) and the
/// operations both call this, so the rule is kept once.
pub fn select_group(selected: &mut usize, member: &mut usize, ix: usize) {
    *selected = ix;
    *member = 0;
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// **A tool by name**, for the same reason a rung is: an agent writes `"clone"` and a reordering of the
/// enum can never silently arm the removal tool instead.
pub(crate) fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Place => "place",
        Tool::Remove => "remove",
        Tool::Move => "move",
        Tool::Clone => "clone",
        Tool::Run => "run",
        Tool::Note => "note",
    }
}

pub(crate) fn tool_from_name(name: &str) -> Option<Tool> {
    match name {
        "place" => Some(Tool::Place),
        "remove" => Some(Tool::Remove),
        "move" => Some(Tool::Move),
        "clone" => Some(Tool::Clone),
        "run" => Some(Tool::Run),
        "note" => Some(Tool::Note),
        _ => None,
    }
}

fn rung_out<S: Serializer>(rung: &SnapLevel, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(rung_name(*rung))
}

fn tool_out<S: Serializer>(tool: &Tool, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(tool_name(*tool))
}

fn tool_in<'de, D: Deserializer<'de>>(d: D) -> Result<Tool, D::Error> {
    let name = String::deserialize(d)?;
    tool_from_name(&name).ok_or_else(|| serde::de::Error::custom(format!(
        "`{name}` is not a tool; the tools are place, remove, move, clone, run, note"
    )))
}

fn rung_in<'de, D: Deserializer<'de>>(d: D) -> Result<SnapLevel, D::Error> {
    let name = String::deserialize(d)?;
    rung_from_name(&name).ok_or_else(|| serde::de::Error::custom(format!(
        "`{name}` is not a rung; the ladder is tile, fine, finer"
    )))
}

impl View {
    /// One rung down the ladder, wrapping — the same answer `keys::Action::CycleGrid` gives.
    pub fn cycle_grid(&mut self) -> SnapLevel {
        self.rung = self.rung.next();
        self.rung
    }

    /// **Arm a tool, or put it back if it is already armed** — the answer is what a click now does.
    /// `arm_tool` is the one place this rule lives, and this calls it rather than restating it.
    pub fn arm(&mut self, tool: Tool, chord: &str) -> String {
        crate::editor::arm_tool_in(&mut self.tool, tool, chord)
    }

    /// **Arm the snip, or cancel it** — the answer is whether it is armed now. The same toggle
    /// `snip::snip_key` makes: one key in, one key out.
    pub fn toggle_snip(&mut self) -> bool {
        self.snip_armed = !self.snip_armed;
        self.snip_armed
    }

    /// **Send the keyboard to the filter box.** `tags` picks the tag block over the list's box; the
    /// answer is which one, or `None` when there is no box on this page to move to.
    pub fn focus_filter(&mut self, tags: bool) -> bool {
        self.filter_focus = Some(tags);
        tags
    }

    /// **The journal, up or down**, and the answer is what it is now. A toggle rather than a set,
    /// because `Cmd+E` is the only way in and the only way out.
    pub fn toggle_journal(&mut self) -> bool {
        self.journal_open = !self.journal_open;
        self.journal_open
    }

    /// **The bench's ghost, on or off**, and the answer is what it is now. A toggle, because `G` and the ghost
    /// chip are the only ways in and the only way out.
    pub fn toggle_ghost(&mut self) -> bool {
        self.ghost = !self.ghost;
        self.ghost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The cycle is the ladder's own**, so a key and an operation cannot disagree about it.
    #[test]
    fn the_grid_cycle_wraps_through_the_three_rungs() {
        let mut view = View::default();
        assert_eq!(view.rung, SnapLevel::Tile, "the default is the whole tile");
        assert_eq!(view.cycle_grid(), SnapLevel::Fine);
        assert_eq!(view.cycle_grid(), SnapLevel::Finer);
        assert_eq!(view.cycle_grid(), SnapLevel::Tile, "and it wraps rather than sticking");
    }

    /// **A rung survives a round trip**, which is what lets the front ends carry it as a value.
    #[test]
    fn a_view_round_trips_through_json() {
        let view = View {
            rung: SnapLevel::Finer,
            journal_open: true,
            filter_focus: Some(true),
            tool: Tool::Clone,
            snip_armed: true,
            // Owner exception (2026-11-10): the literal names the new field; the round trip is unchanged.
            ghost: true,
            // Assumed owner exception (2026-11-11): the literal names the two fields #237 adds (empty); the round trip is unchanged.
            eyes: Default::default(),
            locks: Default::default(),
            // Assumed owner exception (2026-11-11): the literal names the two picker fields #129 slice 22 adds (their defaults).
            picker_selected: 0,
            picker_active: false,
            // Assumed owner exception (2026-11-12): the literal names the field #129 slice 23 adds (its default); the round trip is unchanged.
            compose: Default::default(),
            // Assumed owner exception (2026-11-12): the literal names the two fields #129 slice 24 adds (their defaults); the round trip is unchanged.
            rig: Default::default(),
            tile_page: None,
            // Assumed owner exception (2026-11-12): the literal names the fields #129 slice 25 adds (their defaults); the round trip is unchanged.
            packs: Default::default(),
            filter_pane: None,
            filter_fresh: false,
            // Owner exception (2026-11-13): the literal names the field #129 slice 26 adds (its default); the round trip is unchanged.
            stage: Default::default(),
            // Owner exception (2026-11-13): the literal names the field #129 slice 27 adds (its default); the round trip is unchanged.
            lattice: None,
            // Owner exception (2026-11-13): the literal names the field #129 slice 28 adds (its default); the round trip is unchanged.
            tile_hand: None,
        };
        let text = serde_json::to_string(&view).unwrap();
        assert_eq!(serde_json::from_str::<View>(&text).unwrap(), view);
    }

    /// **The journal toggles, and says which way it went.**
    #[test]
    fn the_journal_toggles_both_ways() {
        let mut view = View::default();
        assert!(!view.journal_open, "a session opens with the journal down");
        assert!(view.toggle_journal(), "the first press opens it");
        assert!(!view.toggle_journal(), "and the second closes it");
    }
}
