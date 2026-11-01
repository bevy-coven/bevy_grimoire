//! **Every refusal is a `problem`, and no refusal is a `note`.**
//!
//! The regression test for the 2026-10-21 run's second bug.
//!
//! # What was wrong
//!
//! The crate's `Status` type states the rule in its own doc, and the rule is short:
//!
//! - `Status::note` — *"what just happened, and went as asked"*, for *"a refusal the author is not
//!   owed an alarm about — clicking bare ground with the remove tool armed"*. **An answer.**
//! - `Status::problem` — *"the editor could not do what was asked, **or did only part of it**"*.
//!   **A refusal.** It reaches the toast (a card in the viewport) and the journal (`Cmd+E`).
//!
//! **A survey found twenty-one refusals filed in the note slot.** The fill's three were fixed in
//! the 2026-10-20-07b run; the rest were carried. **This run fixes the twelve that are clean
//! refusals** — the ones where the editor could not do what it was asked.
//!
//! The measured consequence of getting this wrong is the whole reason it matters: a note is **one
//! dim row**, and it is **erased by the next receipt and by a tab switch** (`Status::clear_note`,
//! which exists because a *stale receipt* was the whole of a bug). So a refusal in the note slot is
//! a refusal the author never sees — the editor said nothing where the author is looking.
//!
//! # How the check finds the slot
//!
//! **The enclosing call.** The refusal is written `state.status.problem("…")` — the slot, then the
//! message — so a window that starts at the message and runs forwards never contains the slot it
//! is looking for. **The test therefore finds the `status.<slot>(` call whose *argument list*
//! contains the refusal's text**, and reads the slot off that call. That is exact: it is the call
//! the message is inside, not the nearest call that happens to be nearby.
//!
//! **And the argument list is found by counting parentheses**, so a message that contains a
//! parenthesis, or a call whose argument spans several lines, is still read correctly.
//!
//! # What the check does NOT claim
//!
//! It cannot tell a refusal from an answer by reading the text alone; it says that **these twelve
//! named sites** file their refusal in the problem slot. **A new refusal elsewhere in the crate is
//! not caught by this test** — and that is stated here rather than implied, because a test that
//! reads as a general guarantee and is not one is worse than no test.

use bevy_cauldron::chrome::Status;

/// **A refusal, and the slot it must be filed in.**
struct Site {
    /// The file, relative to the crate's `src/`.
    file: &'static str,
    /// **The text that identifies the refusal.**
    ///
    /// **It has to be present in the fixed code AND the unfixed code**, because the test is
    /// checked against both: a marker that only exists after the fix cannot fail before it, and
    /// a test that cannot fail is not a test. So the marker is the *message* — the refusal's own
    /// wording, or the call that produces it — and never the slot, which is the thing being
    /// changed.
    marker: &'static str,
}

/// **The twelve refusals that were in the receipt slot.**
///
/// Each is a place where **the editor could not do what it was asked**: a name that came out
/// empty, a placement that had nowhere legal to go, a token that had nowhere to land, an id that
/// was already taken, an endpoint that was not configured, a piece with no width to align.
const REFUSALS: &[Site] = &[
    // **The rename's wording is minted in `ops::map_rename`, and `rename_map` gives it its own**
    // frame, "cannot rename the map". That frame is the marker, so this row fails if the refusal
    // is routed anywhere but a problem, and cannot be satisfied by an unrelated `problem(e.0)`.
    Site {
        file: "editor.rs",
        marker: "cannot rename the map: {}",
    },
    Site {
        file: "editor.rs",
        marker: "placement_at(&open.map",
    },
    Site {
        file: "compose.rs",
        marker: "a name cannot be empty; nothing was changed",
    },
    Site {
        file: "compose.rs",
        marker: "NOT RENAMED: {e}",
    },
    Site {
        file: "tiles.rs",
        marker: "if wrote == 0 {",
    },
    Site {
        file: "tiles.rs",
        marker: "an id cannot be empty; nothing was changed",
    },
    Site {
        file: "tiles.rs",
        marker: "give it an id first (I)",
    },
    // **The `let` binding is not the refusal; the call under it is.** Measured 2026-10-25: this
    // marker occurs twice in `tiles.rs`, and only the first — `commit_candidate`'s — has a slot to
    // read. The second is `admit_candidate`'s, which returns `Err` for the operation layer to file,
    // so there is nothing there to check. Naming the refusal it guards keeps the row pointing at
    // the site the rule is about, and leaves the twin out of it.
    Site {
        file: "tiles.rs",
        marker: "is already in the library — select it above to edit that tile",
    },
    Site {
        file: "tiles.rs",
        marker: "it named no mesh, so nothing came back",
    },
    Site {
        file: "labels.rs",
        marker: "if let Err(remedy) = VlmConfig::load(&project.root) {",
    },
    Site {
        file: "labels.rs",
        marker: "let cfg = match VlmConfig::load(&project.root) {",
    },
    Site {
        file: "build.rs",
        marker: "a hole has no width to put flush",
    },
];

/// **The slot of the `status.` call whose argument list contains the marker — or `None`.**
///
/// **The call is found by its parentheses, not by proximity.** A proximity rule is what this test
/// tried first, and it was wrong twice: it reported `status.say` for a call whose own slot was
/// `problem`, and it reported `status.line` for a refusal three lines away. **A test that cannot
/// fail is not a test, and a test that cannot pass is worse: it would have been the reason a
/// correct fix was reverted.**
///
/// **The whitespace between `status` and the slot is not fixed**, because the crate formats a long
/// chain over several lines — `state\n.status\n.problem(...)` — so the search is over the
/// whitespace-stripped text and the offsets are mapped back through it.
fn slot_of_call_containing(src: &str, marker_at: usize, as_call_site: bool) -> Option<String> {
    // **The marker may BE the call.** Some sites are named by their own slot call —
    // `state.status.problem(remedy)` — and for those the slot is the marker's own text, not
    // something to be found around it. **So the marker's own text is read first**, and only if it
    // is not a slot call does the search for the enclosing one begin.
    let own: String = src[marker_at..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    if let Some(rest) = own.strip_prefix("state.status.") {
        let slot: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if !slot.is_empty() {
            return Some(slot);
        }
    }

    // **The flattened text, as CHARS.**
    //
    // **Chars and not bytes, because the two do not agree and the disagreement is silent.** The
    // first version of this helper flattened into a `String` (bytes) but mapped the marker back
    // through a `Vec` with one entry per *char*, and every file here carries em-dashes in its
    // comments — so the two indices drifted apart and the helper found nothing in files whose
    // refusals were all correctly filed. **A `Vec<char>` makes every index the same kind.**
    let chars: Vec<char> = src.chars().collect();
    let flat: Vec<char> = chars
        .iter()
        .copied()
        .filter(|c| !c.is_whitespace())
        .collect();

    // **The marker's position in the flattened text.**
    //
    // **`marker_at` is a BYTE offset** — `match_indices` gives byte offsets — and `chars` is
    // indexed by char. **The two do not agree**, and every file here carries em-dashes in its
    // comments, so the comparison has to be made in bytes: the byte offset is converted to a char
    // index first, and only then compared.
    let marker_char = src[..marker_at].chars().count();
    let mut flat_marker_at = 0usize;
    let mut seen = 0usize;
    for (i, c) in chars.iter().enumerate() {
        if !c.is_whitespace() {
            if i >= marker_char {
                flat_marker_at = seen;
                break;
            }
            seen += 1;
        }
    }

    // **The marker's nesting depth, from the file's own parentheses.**
    //
    // The fallback below has to know whether a `status.` call is in the same block as the marker,
    // and a depth is what "the same block" means here: a refusal filed inside `if ... { }` sits one
    // level deeper than the function body, and a call after the function has closed sits at zero
    // again. Comments and string literals are not parsed — that would be a Rust front end — so this
    // is a heuristic. It is only ever used to REFUSE the fallback (a marker with no depth gets no
    // call site at all), and the exact path above does not use it.
    // **One vector, indexed like the flat text**, because two vectors indexed differently is how
    // the byte/char bug above happened the first time. `flat_depth[i]` is the depth *before* the
    // flat character at `i`, so a marker that IS a call's first token reads the depth it was opened
    // at.
    let mut flat_depth: Vec<i32> = Vec::with_capacity(flat.len() + 1);
    let mut running = 0i32;
    for c in flat.iter() {
        flat_depth.push(running);
        match c {
            '(' => running += 1,
            ')' => running -= 1,
            _ => {}
        }
    }
    flat_depth.push(running);
    let call_depth_at_marker = flat_depth.get(flat_marker_at).copied();

    // **Every `status.` in the file, with the span of its argument list.**
    let needle: Vec<char> = "status.".chars().collect();
    let mut from = 0usize;
    let mut nearest_after: Option<(usize, String)> = None;
    while from + needle.len() <= flat.len() {
        let found = flat[from..].starts_with(&needle);
        if !found {
            from += 1;
            continue;
        }
        let at = from;
        let after = at + needle.len();
        let slot: String = flat[after..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        // **This call's own depth**, read from the same vector as the marker's.
        let call_depth = flat_depth.get(at).copied();

        // **The argument list: from the `(` to its matching `)`.**
        let open = flat[after..]
            .iter()
            .position(|c| *c == '(')
            .map(|p| after + p);
        let mut end = None;
        if let Some(open) = open {
            let mut depth = 0i32;
            for (k, c) in flat[open..].iter().enumerate() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(open + k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        // **The marker inside this call's arguments — the exact answer.**
        if let Some(end) = end
            && !slot.is_empty()
            && flat_marker_at > at
            && flat_marker_at < end
        {
            return Some(slot);
        }
        // **Otherwise the nearest call after the marker**, which is the case where the marker
        // names the *call site* rather than the message: `placement_at(&open.map` is the call,
        // and the slot is a few lines below it.
        //
        // **But only a call in the same BLOCK.** The fallback has no way to tell "a few lines
        // below" from "the next `status.` in the file", and that is not a hypothetical: two rows
        // here name text that occurs twice in `tiles.rs`, the second time inside `admit_candidate`
        // where the refusal is `Err(...)` for the operation layer and no slot exists. Measured
        // 2026-10-25, the fallback attached both to `status.note("select a library mesh to send it
        // back")` — **15 000 characters away**, past the end of the function — and the test
        // reported two refusals in the editor that the editor files as `problem`. **A fallback that
        // can reach any later call is a test that will one day pass for a refusal in the wrong
        // slot**, because some `problem` further down will satisfy it.
        //
        // **`depth_after` is `None` when the marker is not inside any call at all** (a function
        // body, a bare `let`), and then the fallback is off entirely: the marker is not a call site
        // and the search has nothing to say about it.
        let same_block = call_depth.is_some_and(|d| call_depth_at_marker == Some(d));
        if as_call_site && !slot.is_empty() && at > flat_marker_at && same_block {
            let nearer = match &nearest_after {
                Some((best, _)) => at < *best,
                None => true,
            };
            if nearer {
                nearest_after = Some((at, slot));
            }
        }
        from = after;
    }
    nearest_after.map(|(_, slot)| slot)
}

/// **Every refusal is filed in the problem slot.**
///
/// **A refusal handed back by the core is a `problem`,** and the `Status` type's own doc says so:
/// the note slot is for *"a refusal the author is not owed an alarm about"*, and every one of
/// these twelve is a case where **the editor could not do what it was asked**.
#[test]
fn every_refusal_is_a_problem_and_not_a_receipt() {
    let mut wrong_slot = Vec::new();
    let mut not_found = Vec::new();

    for site in REFUSALS {
        let path = format!("src/{}", site.file);
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path} is there: {e}"));
        // **Every occurrence of the marker, checked.** A marker can name two sites — the
        // labeller's config check is in both the `L` key's handler and the script sentinel — and
        // the table lists each of them, so every occurrence has to satisfy the rule.
        // **A marker is checked where it is a REFUSAL, and a file may hold it where it is not.**
        //
        // Two rows here name text that appears twice in `tiles.rs`: once in `commit_candidate`,
        // which refuses with `state.status.problem`, and once in `admit_candidate`, which returns
        // `Err(...)` for the operation layer to file. Both occurrences reach the author as a
        // `problem` — the second because the operation layer files what it returns there — so a
        // second occurrence outside a `status.` call is not a site with a slot, it is the same
        // refusal in its second form.
        //
        // **It used to be read as a site anyway**, and the fallback below — *"the nearest call
        // after the marker"* — attached it to whatever `status.note` came next in the file, here
        // 15 000 characters later. The test then reported two refusals *in the editor* that the
        // editor files correctly: a false positive, and it would have been a false NEGATIVE the day
        // a real refusal moved into the note slot past the end of a function, because any later
        // `note` would have satisfied it.
        let mut checked = 0usize;
        for (at, _) in src.match_indices(site.marker) {
            // **The fallback is only for a marker that is CODE.** The table names two kinds of
            // thing: a **message** — the refusal's own text, which always carries a quote — and a
            // **call site**, which never does (`if wrote == 0 {`, `placement_at(&open.map`,
            // `let mine = project.measured...;`). The call sites are the ones whose slot is a few
            // lines below, and that is the case the fallback was written for.
            //
            // **A message marker gets the exact answer or nothing**, because a message occurs
            // wherever it is minted, including in the operation-layer twin that returns `Err` for a
            // caller to file. Reading a call site as evidence for one of those is what attached
            // `tiles.rs`'s `give it an id first (I)` — and the `let mine = ...;` binding beside it,
            // whose second occurrence has no slot either — to `status.note("select a library mesh
            // to send it back")` **15 000 characters away**, past the end of the function.
            let as_call_site = !site.marker.contains('"') && !site.marker.contains('\'');
            let Some(slot) = slot_of_call_containing(&src, at, as_call_site) else {
                continue;
            };
            checked += 1;
            match Some(slot.as_str()) {
                Some("problem") => {}
                Some(other) => wrong_slot.push(format!(
                    "{}: `{}` files its refusal in the `{other}` slot, not the problem slot. A \
                     refusal the author is not looking at is a refusal the author never sees — a \
                     `note` is one dim row, and it is erased by the next receipt and by a tab \
                     switch.",
                    site.file, site.marker
                )),
                None => unreachable!("the `let Some` above filtered every `None` out"),
            }
        }
        if checked == 0 {
            not_found.push(format!(
                "{}: `{}` is not inside any `status.` call any more — the wording changed, or the \
                 refusal moved, and the table is stale.",
                site.file, site.marker
            ));
        }
    }

    assert!(
        wrong_slot.is_empty(),
        "these refusals are not in the problem slot, so the author never sees them:\n  {}",
        wrong_slot.join("\n  ")
    );
    assert!(
        not_found.is_empty(),
        "these refusals are not where the table says, so the table is stale:\n  {}",
        not_found.join("\n  ")
    );
}

/// **A refusal outlives a receipt and a tab switch; a receipt does not.**
///
/// **The property that makes the channel matter, measured rather than asserted.** A `note` is one
/// `String` that the next receipt overwrites, and `Status::clear_note` erases it on a tab switch —
/// and that method exists because a *stale receipt* was the whole of a bug. **A refusal is not a
/// stale receipt.**
///
/// **And the two are distinguishable**, which is what lets the editor put a refusal where the
/// author will look and a receipt where a receipt belongs.
#[test]
fn a_refusal_outlives_a_receipt_and_a_receipt_does_not() {
    let mut status = Status::default();
    status.problem("there is already something here");
    status.note("filled 12 with wall");
    assert!(
        status.has_problem(),
        "a receipt erased the refusal. The author never learns the editor could not do what they \
         asked."
    );
    assert_eq!(
        status.problem_text(),
        "there is already something here",
        "and the refusal that survives is not the one that was raised"
    );
    status.clear_note();
    assert!(
        status.has_problem(),
        "a tab switch erased the refusal. `clear_note` clears receipts; a refusal is not a receipt."
    );
    assert_eq!(
        status.note_text(),
        "",
        "and the receipt is gone, which is what a tab switch is for"
    );
}
