//! **A refusal reaches the author, and no message promises what the tool cannot do.**
//!
//! The regression tests for the 2026-10-20-07b run.
//!
//! # What was wrong
//!
//! **The editor's `Status` has two slots, and its own doc says which belongs where:**
//!
//! - `Status::note` — *"what just happened, and went as asked"*, for *"a refusal the author is not
//!   owed an alarm about — clicking bare ground with the remove tool armed"*. **An answer.**
//! - `Status::problem` — *"the editor could not do what was asked, **or did only part of it**"*.
//!   **A refusal.**
//!
//! **The fill's three refusals were in the receipt slot.** So:
//!
//! - **a fill that could not be done said nothing where the author looks** — measured on a
//!   capture, the INSPECTOR's receipt row was empty while the ghost's refusal for the same piece
//!   was plainly visible in the amber row;
//! - **and a receipt is erased** by the next receipt and by a tab switch, so the refusal was gone
//!   before the author looked.
//!
//! **And the cap's message promised a route that does not exist.** The flood said *"fill again to
//!   continue"*; the second fill **cannot** continue, because the flood will not start on a cell
//!   that already holds a piece — and the cell the author clicks is the cell the first fill filled.
//!
//! # What these tests pin
//!
//! - **A refusal is a `problem`**, and a problem survives a receipt and a tab switch.
//! - **The cap's message names a route that works**, and the route is followed in a test.
//! - **The two fills say the same kind of thing**, and that a stopped fill does not look like a
//!   finished one — both live in `a_fill_tells_the_truth.rs`, which reads `editor.rs` *and*
//!   `ops.rs`. They were duplicated here reading `editor.rs` alone and **passed** while the
//!   flood's sentence moved into `ops::fill`, then failed on `main` once it had. Removed rather
//!   than re-pinned: a copy of a test is a place for two answers to disagree.

/// **The fill's refusals are problems, and the author can see them.**
///
/// The `Status` type's own contract is the rule: a refusal handed back by the core is a `problem`.
#[test]
fn a_refused_fill_is_a_problem_and_not_a_receipt() {
    let src = std::fs::read_to_string("src/editor.rs").expect("editor.rs is there");
    // **The three fill sites.** Each is a refusal the core handed back, and each must be a
    // `problem`.
    let refusals = [
        ("the box fill", "box_fill_between"),
        ("the riders", "riders_refuse(&open.map, &doomed)"),
        ("the flood", "flood_from_cursor"),
    ];
    for (what, marker) in refusals {
        let at = src
            .find(marker)
            .unwrap_or_else(|| panic!("the {what} is not in editor.rs"));
        // --- The refusal arm that follows it, and only that arm: from the call to the end of the
        // --- `Err` arm, which is where the slot is decided.
        let rest = &src[at..];
        let err_at = rest
            .find("Err(e) =>")
            .or_else(|| rest.find("if let Err(e) = riders_refuse"))
            .unwrap_or_else(|| panic!("the {what}'s refusal arm is not there"));
        let arm: String = rest[err_at..].chars().take(400).collect();
        assert!(
            arm.contains("state.status.problem(e)"),
            "the {what}'s refusal goes to the receipt slot, so the author never sees it. A refusal \\
             handed back by the core is a `problem` — see `Status::problem`. The arm:\\n{arm}"
        );
    }
}

/// **A refusal survives a receipt and a tab switch.**
///
/// **This is the property that makes the channel matter.** A `note` is one `String` that the next
/// receipt overwrites, and [`Status::clear_note`] erases it on a tab switch — and that method
/// exists because a *stale receipt* was the whole of a bug. **A refusal is not a stale receipt.**
#[test]
fn a_refusal_survives_a_receipt_and_a_tab_switch() {
    let mut status = bevy_cauldron::chrome::Status::default();
    status.problem("there is already something here");
    // --- A receipt, written after the refusal.
    status.note("filled 12 with wall");
    assert!(
        status.has_problem(),
        "a receipt erased the refusal. The author never learns the fill did not happen."
    );
    assert_eq!(
        status.problem_text(),
        "there is already something here",
        "and the refusal that survives is not the one that was raised"
    );
    // --- And a tab switch, which is `clear_note`.
    status.clear_note();
    assert!(
        status.has_problem(),
        "a tab switch erased the refusal. `clear_note` clears receipts; a refusal is not a receipt."
    );
}

/// **The cap's message names a route that works, and the route is the one the fill offers.**
///
/// The research's criterion 1: *any message that tells the author to take an action must be
/// followed, in a test, by performing that action and asserting the stated result.* The failing
/// form of it is the test this run added to `fill.rs`:
/// `the_caps_promise_cannot_be_kept_by_a_second_fill`.
#[test]
fn the_caps_message_names_a_route_that_works() {
    let route = bevy_cauldron::fill::CAP_ROUTE;
    assert!(
        route.contains("another empty cell"),
        "the cap's message must name the route that works — a fill from a cell the first fill did \\
         not reach — because the route that does NOT work is filling the same cell again. It said \\
         `{route}`"
    );
    assert!(
        !route.to_lowercase().contains("fill again to continue"),
        "the cap's message must not promise 'fill again to continue', because the second fill in \\
         the same cell is refused: the flood will not start on a cell that already holds a piece. \\
         It said `{route}`"
    );
}
