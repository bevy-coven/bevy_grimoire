//! **The agent's steps, one row each, read from the operation log's own sentences (E-068).**
//!
//! The brief's Collaboration direction asks for this in Mixar's words: *"the agent shows what it is
//! doing as steps narrated by measured effect (the oplog's `result` lines, never raw logs), hangs
//! captures on the step that made them"*. Both halves already existed and neither was on a surface a
//! person watches — the receipt is one row, and the journal held refusals only.
//!
//! # This module decides nothing; it reads
//!
//! Every row's text is [`crate::ops::Entry::result`] **verbatim**. That is the whole point of the
//! beat: the sentence an operation writes about itself is the sentence the person reads, so a step
//! cannot claim more than the log says. A row that re-worded the result would be a second narrator,
//! and two narrators disagree the first time one of them is edited.
//!
//! **A refusal is not a step.** The log records one with `outcome: "refused"` and no `result`,
//! because a refusal is something the editor would *not* do; a row for it would tell the person the
//! agent did something it did not. Refusals reach the person through the journal, which is where they
//! belong.
//!
//! # A capture hangs on its step
//!
//! A capture is not a row. It names the log sequence it was taken at, and it is drawn under that
//! step's row — *"the picture sits on the step that made it"*. A capture naming a sequence the log
//! does not hold hangs on nothing rather than attaching to whichever row is nearest: a capture from
//! another session, or one left behind by a log truncated by hand, is not something anyone did here.

/// How many steps the feed keeps.
///
/// The journal's argument one list over: a list read by scrolling wants more than a panel read at a
/// glance ([`crate::chrome::JOURNAL_CAP`] is 200 for the same reason), and it is capped at all
/// because a session that dispatches one operation per frame is a real thing an agent does.
pub const CAP: usize = 120;

/// **One row of the feed.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The log sequence this row is, which is what a capture is hung on.
    pub seq: u64,
    /// **The operation's name**, so a caller can filter or colour by what was done.
    pub op: String,
    /// **The operation's own sentence**, verbatim from [`crate::ops::Entry::result`].
    pub text: String,
    /// The capture taken at this step, if one was.
    pub capture: Option<Capture>,
}

/// **A capture, as a step shows it**: what it was called, and how many frames it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    /// The sequence of the step that made it.
    pub seq: u64,
    /// The label the capture was given, which is also its directory name.
    pub label: String,
    /// How many frames landed. `0` is a capture whose frames are still being written.
    pub frames: usize,
    /// Where the frames are, for a caller that wants to open one.
    pub path: String,
}

/// **The feed for a log's text**, newest last, capped at [`CAP`].
///
/// `captures` are the captures taken this session; each hangs on the row whose `seq` it names, and one
/// that names no row is dropped (see the module note).
pub fn from_oplog(text: &str, captures: &[Capture]) -> Vec<Step> {
    let mut rows: Vec<Step> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        // **A line that does not parse is a line with no row, and that is silent by default.** This
        // module's own first test wrote `"source": "Agent"` where the log spells `"agent"` and got
        // zero rows with no error — the exact trap. A caller that wants to say so uses
        // [`unreadable_lines`]; the feed itself stays lenient because a log half-written by a crash
        // must still show the steps before the torn line.
        .filter_map(|l| serde_json::from_str::<crate::ops::Entry>(l).ok())
        // **Only what the agent did.** A refused line has no `result`, and an operation that changed
        // no document records one saying so — both are facts, neither is a step.
        .filter_map(|entry| {
            let text = entry.result?;
            Some(Step {
                seq: entry.seq,
                op: entry.op,
                text,
                capture: None,
            })
        })
        .collect();
    // **Hung on the row whose sequence it names**, by a lookup rather than by position: a capture
    // taken at a sequence that has since fallen off the front of the cap has nowhere to go, and
    // `rows.iter().position` is what says so.
    for capture in captures {
        if let Some(row) = rows.iter_mut().find(|r| r.seq == capture.seq) {
            row.capture = Some(capture.clone());
        }
    }
    if rows.len() > CAP {
        let drop = rows.len() - CAP;
        rows.drain(..drop);
    }
    rows
}

/// **How many lines of `text` are not log entries**, so a caller can say so instead of showing a
/// short feed and letting the reader believe that was all of it.
///
/// A torn final line is the ordinary case (a crash mid-write), and it is the one this is for: the
/// steps before it are real and worth showing, and the person is owed the fact that the list stops
/// short of the file.
pub fn unreadable_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| serde_json::from_str::<crate::ops::Entry>(l).is_err())
        .count()
}
