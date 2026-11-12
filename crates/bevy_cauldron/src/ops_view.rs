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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
