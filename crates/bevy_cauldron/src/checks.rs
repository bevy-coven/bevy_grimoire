//! **What a change broke (E-074).** Run after every operation that changes a document, so the
//! answer an agent reads says what its change introduced, not only that it applied.
//!
//! The brief asks for this in as many words (`emerge-feature-loop.md`:201): *"Relevant validators run
//! after every mutation. Their failures appear in the tool result."* Until now an agent's
//! `grimoire_apply_operation` answer said `"applied"` and nothing else, so a `placement_set_yaw` that
//! opened an edge seam on the far side of a 720-placement map read exactly like one that changed
//! nothing but the facing.
//!
//! # Only two kinds of finding, and that is the scope
//!
//! **Edge faults** — two abutting tiles whose declared face tokens disagree
//! ([`bevy_grimoire_core::adjacency`]). **Overlaps** — two placements whose plans interpenetrate on the
//! same layer ([`bevy_grimoire_core::stack::overlaps`]). **Doorways** — a piece standing in a measured
//! opening ([`bevy_grimoire_core::stack::blocked_openings`]). All three were computed by nothing that
//! ran on the way out of a mutation.
//!
//! One more the scope names is *not* here, filed as its own item rather than half-built:
//!
//! - **How varied a layout is.** Shipped as E-080: `editor::measure_solved` scores a `composed`
//!   proposal's own grid through `range::measure`, and a proposal whose source derives no wall
//!   interfaces answers `null` rather than a scored zero.
//!
//! # What "new" means, and why the diff is the report
//!
//! A finding that was there before is not what this change broke. An agent that reads twenty
//! standing faults every time it moves a crate learns to ignore the field, which is the same as not
//! reporting them. So the answer names the **difference** and states the total beside it, and the
//! reader can tell "I introduced one" from "this map has twenty and I made none".
//!
//! The comparison is on a finding's **identity**, not on its sentence: an edge fault is
//! `(a, b, dir)`, an overlap is its pair of placement ids, a doorway is `(opening id, intruder id)`.
//! A message that gained a word is not a new finding, and an overlap whose two placements were
//! reordered in the file is the same overlap.

use bevy_grimoire_core::adjacency::{self, Fault};
use serde_json::{Value, json};

/// How many new findings of each kind an answer names. The totals are always exact.
///
/// There is a cap because an operation that replaces a whole map — `map_open`, `layout_accept` on a
/// freshly generated layout — can produce hundreds of findings at once, and an answer carrying all of
/// them is a tool result no agent can read. [`NEW_CAP`] is what a person can take in before deciding
/// the change was wrong for a reason the list already shows.
pub const NEW_CAP: usize = 20;

/// The map's findings at one moment.
pub struct Found {
    /// Every place the map disagrees with its tiles' declared edge tokens, in the pass's own order.
    pub faults: Vec<Fault>,
    /// Pairs of placement ids, each pair sorted (smaller id first) so a reorder is not a change.
    pub overlaps: Vec<(String, String)>,
    /// `(opening id, intruder id)` — a piece whose plan reaches into a measured doorway at a height
    /// the hole spans. **Not sorted as a pair**: which one is the hole is the finding, so a doorway
    /// whose two ids swap order is still `("door@0", "crate@1")`.
    pub doorways: Vec<(String, String)>,
    /// `(keeper id, intruder id)` — a piece standing in the space another piece's `Descriptor::clearance`
    /// reserves, at a height their spans share (E-079). Name order is the finding here too: the keeper
    /// is the piece that asked for the space.
    pub clearances: Vec<(String, String)>,
}

/// Whether `operation` gets checks: every operation that changes a document (`Undo` is not `None`),
/// plus `map_open`, which replaces the whole map and declares `Undo::None`.
///
/// # The registry is the list, not a second one
///
/// "Which operations change a document" is already answered by [`crate::ops::Operation::undo`], and a
/// second list here would be the kind of two-answer problem this crate keeps meeting. An operation
/// that grows an inverse is checked the day it grows one, with nothing to remember.
///
/// **`map_open` is the exception and it has to be named.** It declares no inverse — the map it loaded
/// had no counterpart in the map it replaced — and it is exactly the operation whose findings a reader
/// most needs, because it is the one that brings a whole file's worth onto the screen. The name is a
/// literal rather than a lookup because the exception is the point: an operation that gains
/// `Undo::Inverse` is checked by the rule, and this line is the one operation the rule cannot cover.
pub fn wanted(operation: &str) -> bool {
    crate::ops::find(operation).is_some_and(|o| o.undo() != crate::ops::Undo::None)
        || operation == "map_open"
}

/// **The snapshot a change is measured against, or `None` when there is nothing to compare.**
///
/// `None` has two causes, and both mean the same thing to a reader: nothing in this answer's map was
/// already there. Either no document exists yet (the menu has no map), or the operation **replaced**
/// the document wholesale.
///
/// # Why `map_open` is not compared, and a path could not decide it
///
/// Findings are matched by placement **id**, and every map of a kit holds `wall@0`, `pane@1`,
/// `crate@0`. Opening map B therefore cannot be diffed against map A: B's fault on `pane@1` would be
/// matched against A's unrelated piece of the same id and reported as *nothing new* — a silent
/// under-report, the one failure mode a checks field cannot afford.
///
/// The obvious guard, comparing the open map's file path, is wrong in the other direction: `map_rename`
/// keeps the same document and points `OpenMap::map_path` at the new file, so a path comparison would
/// call an unchanged map a different one and report every standing finding on it as introduced —
/// renaming Sanctuary's `town` would answer `overlaps.new_total: 781`, "your rename broke 781
/// things". Measured on this crate: `open.save` sets `self.map_path = path` (`project.rs`).
///
/// So the rule is the operation's own: **`map_open` replaces the map; nothing else does.** One
/// function answers it, and both front ends call this rather than repeating the name.
pub fn before(operation: &str, ctx: &crate::ops::Ctx) -> Option<Found> {
    if !wanted(operation) || operation == "map_open" {
        return None;
    }
    find(ctx)
}

/// The findings for the map in `ctx`, or `None` when `ctx.project` or `ctx.open` is `None`.
///
/// Both are needed and neither is optional: a check compares placements against the library that
/// gives them their extents and tokens, and there is no map on the menu screen. The menu is not a
/// failure — `map_create` runs there and changes no map — so this answers `None` rather than
/// refusing.
pub fn find(ctx: &crate::ops::Ctx) -> Option<Found> {
    let project = ctx.project.as_ref()?;
    let open = ctx.open.as_ref()?;
    let faults = adjacency::faults(&open.map, &project.library, project.lattice.face_bands);
    let overlaps = bevy_grimoire_core::stack::overlaps(&open.map, &project.library)
        .into_iter()
        .map(|(i, j)| {
            let a = open.map.placements[i].id.clone();
            let b = open.map.placements[j].id.clone();
            // **Sorted by id, not by index**, so the identity of an overlap survives the two
            // placements changing order in the file — which a save, a patch or a generator's output
            // can do without the map meaning anything different.
            if a <= b { (a, b) } else { (b, a) }
        })
        .collect();
    let doorways = bevy_grimoire_core::stack::blocked_openings(&open.map, &project.library)
        .into_iter()
        .map(|(i, j)| {
            (
                open.map.placements[i].id.clone(),
                open.map.placements[j].id.clone(),
            )
        })
        .collect();
    let clearances = bevy_grimoire_core::stack::blocked_clearances(&open.map, &project.library)
        .into_iter()
        .map(|(i, j)| {
            (
                open.map.placements[i].id.clone(),
                open.map.placements[j].id.clone(),
            )
        })
        .collect();
    Some(Found { faults, overlaps, doorways, clearances })
}

/// The `checks` value of an answer: what `after` has that `before` did not.
///
/// `before == None` means everything in `after` is new — which is what [`before`] answers for an
/// operation that **replaced** the map rather than editing it.
pub fn report(before: Option<&Found>, after: &Found) -> Value {
    let is_new_fault = |f: &Fault| {
        !before.is_some_and(|b| {
            b.faults
                .iter()
                .any(|o| o.a == f.a && o.b == f.b && o.dir == f.dir)
        })
    };
    let is_new_overlap = |o: &(String, String)| {
        !before.is_some_and(|b| b.overlaps.iter().any(|p| p == o))
    };
    let is_new_doorway = |o: &(String, String)| {
        !before.is_some_and(|b| b.doorways.iter().any(|p| p == o))
    };
    let is_new_clearance = |o: &(String, String)| {
        !before.is_some_and(|b| b.clearances.iter().any(|p| p == o))
    };

    let new_faults: Vec<&Fault> = after.faults.iter().filter(|f| is_new_fault(f)).collect();
    let new_overlaps: Vec<&(String, String)> = after
        .overlaps
        .iter()
        .filter(|o| is_new_overlap(o))
        .collect();
    let new_doorways: Vec<&(String, String)> = after
        .doorways
        .iter()
        .filter(|o| is_new_doorway(o))
        .collect();
    let new_clearances: Vec<&(String, String)> = after
        .clearances
        .iter()
        .filter(|o| is_new_clearance(o))
        .collect();

    json!({
        "edge_faults": {
            "new": new_faults
                .iter()
                .take(NEW_CAP)
                .map(|f| json!({
                    "a": f.a,
                    "b": f.b,
                    "dir": adjacency::dir_name(f.dir),
                    "message": f.message,
                }))
                .collect::<Vec<_>>(),
            "new_total": new_faults.len(),
            "total": after.faults.len(),
        },
        "overlaps": {
            "new": new_overlaps
                .iter()
                .take(NEW_CAP)
                .map(|(a, b)| json!({"a": a, "b": b}))
                .collect::<Vec<_>>(),
            "new_total": new_overlaps.len(),
            "total": after.overlaps.len(),
        },
        // **Named `opening`/`inside`, not `a`/`b`.** Which id is the doorway is the whole finding: a
        // reader told `{"a": "crate@1", "b": "door@0"}` has to work out the direction, and the
        // sentence it would have to construct is the one field names exist to save.
        "doorways_in_the_way": {
            "new": new_doorways
                .iter()
                .take(NEW_CAP)
                .map(|(opening, inside)| json!({"opening": opening, "inside": inside}))
                .collect::<Vec<_>>(),
            "new_total": new_doorways.len(),
            "total": after.doorways.len(),
        },
        // **`keeper`/`intruder`, for the same reason `opening`/`inside` is spelled out above**: which
        // id reserved the space is half the finding, and a reader given `{"a": ..., "b": ...}` has to
        // reconstruct a sentence field names exist to save.
        "clearances_in_the_way": {
            "new": new_clearances
                .iter()
                .take(NEW_CAP)
                .map(|(keeper, intruder)| json!({"keeper": keeper, "intruder": intruder}))
                .collect::<Vec<_>>(),
            "new_total": new_clearances.len(),
            "total": after.clearances.len(),
        },
    })
}

#[cfg(test)]
mod tests {
    use bevy_grimoire_core::wfc::E;

    use super::*;

    /// **The registry decides, and `map_open` is the one exception.** Pin both halves: the rule (an
    /// operation with an inverse is checked, a read-only one is not) and the named exception, so a
    /// `map_open` that was quietly dropped from the list fails here.
    #[test]
    fn wanted_follows_the_registry() {
        assert!(wanted("placement_add"), "a placement changes the map");
        assert!(wanted("map_open"), "opening a map replaces it whole");
        assert!(!wanted("brush_arm"), "arming a brush changes nothing");
        assert!(!wanted("layout_propose"), "a proposal is not map data");
        assert!(!wanted("no_such_op"), "an unknown name has no undo to ask");
    }

    /// **A finding already there is not new, and the total still counts it.** The two numbers answer
    /// two different questions — "did I break this" and "how bad is this map" — and a change that
    /// touched neither must leave both alone.
    #[test]
    fn a_finding_already_there_is_not_new() {
        let fault = |b: &str, dir| Fault {
            a: "wall@0".into(),
            b: b.into(),
            dir,
            a_face: vec![None],
            b_face: vec![None],
            message: format!("wall@0 and {b} disagree"),
        };
        let before = Found {
            faults: vec![fault("pane@1", E)],
            overlaps: vec![("a".into(), "b".into())],
            doorways: Vec::new(),
            clearances: Vec::new(),
        };
        let after = Found {
            faults: vec![fault("pane@1", E)],
            overlaps: vec![("a".into(), "b".into()), ("c".into(), "d".into())],
            doorways: Vec::new(),
            clearances: Vec::new(),
        };
        let r = report(Some(&before), &after);
        assert_eq!(r["overlaps"]["new_total"], 1);
        assert_eq!(r["overlaps"]["total"], 2);
        assert_eq!(r["overlaps"]["new"][0], json!({"a": "c", "b": "d"}));
        assert_eq!(r["edge_faults"]["new_total"], 0, "the standing fault is not new");
        assert_eq!(r["edge_faults"]["total"], 1);
    }

    /// **`before` decides comparability from the operation, not from a file path.**
    ///
    /// Two rules that look right and are each wrong:
    ///
    /// - Comparing `map_open` against the map it replaces under-reports, because findings match by
    ///   placement id and every map of a kit shares them (`wall@0`, `pane@1`).
    /// - Comparing a file path over-reports on `map_rename`, which keeps the document and points
    ///   `OpenMap::map_path` at the new file — the same unchanged map would come back with every
    ///   standing finding called new.
    ///
    /// So the answer is the operation's own: `map_open` replaces the map and has nothing to compare
    /// against; every other checked operation edits the map it was given.
    #[test]
    fn before_is_the_operation_s_question_not_a_path_s_question() {
        let ctx = crate::ops::Ctx::default();
        assert!(
            before("map_open", &ctx).is_none(),
            "a replaced map has nothing in it that was already there"
        );
        assert!(
            before("brush_arm", &ctx).is_none(),
            "an operation that changes no document has nothing to compare"
        );
        // **A rename keeps its document**, so it is comparable and must be: `wanted` says yes, and
        // it is not `map_open`. With no project loaded `find` answers `None`, which is the only
        // reason this is `None` here — the guard is what `wanted` and the `map_open` test above pin.
        assert!(before("map_rename", &ctx).is_none(), "no project is loaded in this context");
        assert!(wanted("map_rename"), "a rename changes a document and must be checked");
    }
}
