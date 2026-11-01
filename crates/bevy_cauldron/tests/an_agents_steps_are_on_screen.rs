//! **The agent's steps are on screen, one row each, with the capture under its step (E-068).**
//!
//! The brief's Collaboration direction, in Mixar's words: *"the agent shows what it is doing as steps
//! narrated by measured effect (the oplog's `result` lines, never raw logs), hangs captures on the
//! step that made them"*. bevy_grimoire had both halves and neither was on a surface the person
//! watches: the oplog carries one `result` line per operation and capture sequences exist, but the
//! receipt is ONE row and the journal held refusals only.
//!
//! # What is being proved
//!
//! 1. **Every operation in the log becomes a step row**, in log order, read from its `result` line —
//!    the line the operation already writes about itself, never a summary reconstructed here.
//! 2. **A step that made a capture says so on its own row.** The capture is not a row of its own; it
//!    belongs to the step, which is the whole of "hangs captures on the step that made them".
//! 3. **The feed is bounded and says what it dropped**, like the journal's own cap: a long session
//!    must not grow a list without limit.
//! 4. **A step's own words are the operation log's**, so a row cannot claim something the log does not
//!    say. An operation with no `result` — a refusal — is not a step, because a refusal is not
//!    something the agent did.
//!
//! # Why this is a data test and not a screenshot test
//!
//! The panel is one column of text; a still of it would show a list I could caption no better than
//! the list itself, and the properties that matter — order, one row per operation, the capture on the
//! right row, the cap — are assertions, not pixels. The report says the same thing rather than
//! implying a picture was checked.

use bevy_cauldron::steps::{self, Step};

fn log_line(seq: u64, op: &str, result: Option<&str>, refused: bool) -> String {
    let outcome = if refused { "refused" } else { "applied" };
    let mut entry = serde_json::json!({
        "seq": seq,
        "operation_id": format!("0000-{seq}"),
        "at": "2026-10-26T00:00:00Z",
        "source": "agent",
        "session": "s",
        "op": op,
        "params": {},
        "outcome": outcome,
    });
    if let Some(result) = result {
        entry["result"] = serde_json::json!(result);
    }
    if refused {
        entry["refusal"] = serde_json::json!("no");
    }
    format!("{entry}\n")
}

/// **One row per applied operation, in log order, reading its own `result` line.**
///
/// And **a refusal is not a step**: the log records one with `outcome: "refused"` and no `result`,
/// because a refusal is something the editor would not do. A row for it would tell the person the
/// agent did something it did not.
#[test]
fn every_applied_operation_becomes_one_row_in_log_order() {
    let text = [
        log_line(0, "placement_add", Some("placed crate@1 at (2.00, 2.00)"), false),
        log_line(1, "placement_add", None, true),
        log_line(2, "map_save", Some("saved"), false),
    ]
    .concat();

    let rows = steps::from_oplog(&text, &[]);
    assert_eq!(rows.len(), 2, "two applied operations, two rows: {rows:#?}");
    assert_eq!(rows[0].text, "placed crate@1 at (2.00, 2.00)");
    assert_eq!(rows[0].op, "placement_add");
    assert_eq!(rows[1].text, "saved");
    assert_eq!(rows[1].op, "map_save");
    assert_eq!(
        rows[0].seq, 0,
        "a row carries its log sequence, which is what a capture is hung on"
    );
}

/// **A capture hangs on the step that made it**, not on a row of its own.
#[test]
fn a_capture_belongs_to_the_step_that_made_it() {
    let text = [
        log_line(0, "placement_add", Some("placed crate@1"), false),
        log_line(1, "layout_propose", Some("proposed: 12 piece(s)"), false),
    ]
    .concat();
    // A capture at sequence 1 — the propose — with its frames.
    let captures = vec![steps::Capture {
        seq: 1,
        label: "proposal".to_owned(),
        frames: 8,
        path: "target/loop/captures/proposal".to_owned(),
    }];

    let rows = steps::from_oplog(&text, &captures);
    assert_eq!(rows.len(), 2, "a capture is not a row: {rows:#?}");
    assert!(
        rows[0].capture.is_none(),
        "the first step made no capture: {rows:#?}"
    );
    let hung = rows[1]
        .capture
        .as_ref()
        .unwrap_or_else(|| panic!("the propose's capture is not on its row: {rows:#?}"));
    assert_eq!(hung.label, "proposal");
    assert_eq!(hung.frames, 8);
}

/// **A capture naming a sequence the log does not have hangs on nothing**, rather than inventing a
/// row: a capture from another session (or a log truncated by hand) is not a step anyone took.
#[test]
fn a_capture_with_no_step_is_dropped() {
    let text = log_line(0, "placement_add", Some("placed crate@1"), false);
    let captures = vec![steps::Capture {
        seq: 9,
        label: "orphan".to_owned(),
        frames: 4,
        path: "target/loop/captures/orphan".to_owned(),
    }];
    let rows = steps::from_oplog(&text, &captures);
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].capture.is_none(),
        "a capture for a sequence the log does not hold must not attach to another row: {rows:#?}"
    );
}

/// **The feed is bounded, keeps the newest, and counts what it dropped** — the journal's own rule,
/// one list over.
#[test]
fn the_feed_keeps_the_newest_and_counts_what_it_dropped() {
    let mut text = String::new();
    for seq in 0..(steps::CAP + 5) {
        text.push_str(&log_line(
            seq as u64,
            "placement_add",
            Some(&format!("placed {seq}")),
            false,
        ));
    }
    let rows = steps::from_oplog(&text, &[]);
    assert_eq!(rows.len(), steps::CAP, "the feed is capped: {}", rows.len());
    assert_eq!(
        rows.last().map(|r| r.text.as_str()),
        Some(&*format!("placed {}", steps::CAP + 4)),
        "the newest step is the one kept"
    );
    assert_eq!(
        rows.first().map(|r| r.text.as_str()),
        Some(&*format!("placed {}", 5)),
        "the oldest five fell off the front"
    );
}

/// **A step's row is the log's own words.** Nothing in this module writes prose of its own: the row
/// text is `Entry::result` verbatim, so the panel cannot say something the operation log does not.
#[test]
fn a_row_is_the_logs_own_sentence() {
    let phrase = "proposed: 5776 piece(s) replacing 720, around 0 pinned cell(s), from 8 prototype(s)";
    let text = log_line(0, "layout_propose", Some(phrase), false);
    let rows: Vec<Step> = steps::from_oplog(&text, &[]);
    assert_eq!(rows[0].text, phrase);
}

/// **A line that is not a log entry is counted, not silently skipped.**
///
/// Found on this module's own first run: the test wrote `"source": "Agent"` where the log spells
/// `"agent"`, and `from_oplog` answered zero rows with no error at all. The feed stays lenient — a log
/// torn by a crash must still show the steps before the tear — so the count is how a caller says the
/// list stops short of the file.
#[test]
fn a_line_that_is_not_a_log_entry_is_counted() {
    let torn = format!(
        "{}{}",
        log_line(0, "placement_add", Some("placed crate@1"), false),
        "{\"seq\": 1, \"source\": \"Agent\"}\n"
    );
    let rows = steps::from_oplog(&torn, &[]);
    assert_eq!(rows.len(), 1, "the good line is still a step");
    assert_eq!(
        steps::unreadable_lines(&torn),
        1,
        "the torn line must be countable, or a short feed looks like a complete one"
    );
}
