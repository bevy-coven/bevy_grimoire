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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// The snap ladder's rung: what a piece is nudged to, and what the grid is drawn at.
    #[serde(default, serialize_with = "rung_out", deserialize_with = "rung_in")]
    pub rung: SnapLevel,
}

fn rung_out<S: Serializer>(rung: &SnapLevel, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(rung_name(*rung))
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
        let view = View { rung: SnapLevel::Finer };
        let text = serde_json::to_string(&view).unwrap();
        assert_eq!(serde_json::from_str::<View>(&text).unwrap(), view);
    }
}
