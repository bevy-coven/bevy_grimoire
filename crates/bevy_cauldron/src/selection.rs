//! **The Map's selection: one primary piece, and any number beside it** (UI-50, #236).
//!
//! Until this existed, "the selection" was `TargetLock`'s single id (`editor.rs:7450`: *"the Map has no
//! multi-select, so 'the selection' is that one piece"*), so the inspector had nothing to show a
//! mixed value for and `placement_set_pose`'s `ids` form had no caller. The only multi-piece list was
//! `NoteDraft.pieces`, which is the **note's** subject — binding anything else to it would mean closing
//! a note cleared somebody's selection.
//!
//! # The two answers, and which one is authoritative
//!
//! `TargetLock` stays: the gold marker, the piece-verbs and an agent's `"selection"` frame target all
//! read it, and changing that would change behaviour nobody asked to change. [`Selection`] is the set
//! the *inspector* reads, and it **always contains the lock's piece as its first entry** when the lock
//! holds one — [`Selection::follow`] keeps that true every frame, so a click that moves the lock moves
//! the selection with it, and Shift-click can only ever add pieces to the set the lock anchored.
//!
//! # Session state, deliberately
//!
//! No file, no operation, no undo step: a selection is where a person is looking, not a fact about the
//! map. That is also why `screen::OWNERSHIP` classifies it `Session` rather than `Door` — a door change
//! keeps it, and a piece from another map simply falls out of it (`Selection::prune`).

use bevy::prelude::*;

use crate::editor::TargetLock;
use crate::project::OpenMap;
use crate::screen::Screen;

/// **What the inspector shows, and what a batch edit applies to.** Ordered: the first entry is the
/// primary (the lock's piece), the rest are the pieces Shift-clicked beside it.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct Selection {
    ids: Vec<String>,
    /// The lock's piece at the last `follow`, so a moved lock is told from a Shift-click's addition.
    primary: Option<String>,
}

impl Selection {
    /// The pieces, primary first.
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// How many are selected.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether anything is selected.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// **The set the lock anchors**: the lock's piece first, then whatever else was selected and is
    /// still on the map. Called every frame, and it writes only on a difference.
    ///
    /// **A lock that moved replaces the set**, because a plain click is one piece and it must not leave
    /// the previous primary beside it. A lock that is the same keeps the others, which is what lets
    /// Shift-click add beside it and a frame later still find them there.
    pub fn follow(&mut self, lock: &TargetLock, open: Option<&OpenMap>) -> bool {
        let mut want: Vec<String> = Vec::new();
        match lock.placement_id() {
            Some(id) => {
                if self.primary.as_deref() != Some(id) {
                    // The anchor moved: this is a new click, not an addition to the old set.
                    self.ids = vec![id.to_owned()];
                    self.primary = Some(id.to_owned());
                    return true;
                }
                want.push(id.to_owned());
            }
            // **A released lock is an empty selection** (`Esc`, or a piece that stopped resolving):
            // the set is anchored on it, so it cannot outlive it.
            None => {
                if self.ids.is_empty() && self.primary.is_none() {
                    return false;
                }
                self.ids.clear();
                self.primary = None;
                return true;
            }
        }
        for id in &self.ids {
            if want.iter().any(|w| w == id) {
                continue;
            }
            if open.is_none_or(|o| o.map.placements.iter().any(|p| p.id == *id)) {
                want.push(id.clone());
            }
        }
        if want == self.ids {
            return false;
        }
        self.ids = want;
        true
    }

    /// **Add `id` beside the primary**, or take it out again. The primary — whatever the lock holds —
    /// is never removed by this: Shift-clicking the piece you are standing on should not empty the
    /// selection the inspector is drawn from.
    pub fn toggle(&mut self, id: &str, primary: Option<&str>) -> bool {
        if Some(id) == primary {
            return false;
        }
        match self.ids.iter().position(|p| p == id) {
            Some(at) => {
                self.ids.remove(at);
                true
            }
            None => {
                self.ids.push(id.to_owned());
                true
            }
        }
    }

    /// **A Shift-click on `id`**, which is at `at`. With nothing locked the piece *becomes* the primary
    /// (it takes a held lock, so it stays selected when the cursor walks away); with a primary it joins
    /// the set beside it, or leaves it again. Shift-clicking the primary itself changes nothing. `true`
    /// when the set changed.
    pub(crate) fn shift_click(&mut self, lock: &mut TargetLock, id: &str, at: (f32, f32)) -> bool {
        match lock.placement_id().map(str::to_owned) {
            None => {
                lock.hold(id, at);
                self.only(id)
            }
            Some(primary) => self.toggle(id, Some(&primary)),
        }
    }

    /// **Replace the set with one piece**, as a plain click does.
    pub fn only(&mut self, id: &str) -> bool {
        if self.ids.len() == 1 && self.ids[0] == id {
            return false;
        }
        self.primary = Some(id.to_owned());
        self.ids = vec![id.to_owned()];
        true
    }

    /// Drop ids that are no longer on the map, as a reload or an undo can do.
    pub fn prune(&mut self, open: &OpenMap) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| open.map.placements.iter().any(|p| p.id == *id));
        self.ids.len() != before
    }

    /// For a test or a caller that has the ids already.
    pub fn set(&mut self, ids: Vec<String>) {
        self.primary = ids.first().cloned();
        self.ids = ids;
    }
}

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>().add_systems(
            Update,
            follow_the_lock.run_if(in_state(Screen::Editor)),
        );
    }
}

/// **The selection never drifts from the lock.** One system, so "what is selected" has one answer even
/// though two resources hold it: the lock is the anchor, the selection is the set around it.
fn follow_the_lock(
    lock: Option<Res<TargetLock>>,
    open: Option<Res<OpenMap>>,
    mut selection: ResMut<Selection>,
) {
    let Some(lock) = lock else { return };
    selection.follow(&lock, open.as_deref());
}
