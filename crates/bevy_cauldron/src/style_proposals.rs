//! **A change an agent proposed to a style page, waiting for a person** (SG-07, #406).
//!
//! A level's or an area's style page is changed by the operation `style_edit`. **A game or region page is not an agent's to write**: those
//! files hold the whole game together, so `grimoire_style_propose {path, edits, why}` only puts the change in front of the person — a row in
//! the AGENT block's list beside the wishes and the lessons (`wish_view`) — and **Keep** writes it, **Discard** drops it. The call returns at
//! once, because an MCP call that blocked on a person would deadlock with the editor's own frame; what became of it is read from
//! `grimoire_state.style_proposals`.
//!
//! # The person's Keep is the operation
//!
//! Keep dispatches `style_edit` as the person's own change ([`keep`]), so a kept proposal is one log line and one undo step, and it writes
//! the `TextGrant` for the new text that every edit made in front of the person writes. **A proposal whose page changed after it was made is
//! refused on Keep**, naming the page: a person never keeps a change against text they did not see.
//!
//! # What is held
//!
//! The edits as the agent wrote them, the page's digest when the proposal was made, and the answer. **The state reports `{id, path, state}`
//! and the changed lines' numbers, never a page's text**: the row is for the person, and a page's text is a prompt an agent must not be
//! handed back by the editor's own state.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde_json::json;

use crate::ops_design::TextEdit;

/// **The most edits one proposal may hold.** A proposal is read as a diff, and a person reads a few spans, not a rewrite.
pub const STYLE_EDITS_MAX: usize = 12;

/// **The longest a proposal's `why` may be**, in characters: one sentence a person reads in a row.
pub const STYLE_WHY_MAX_CHARS: usize = 280;

/// **The most proposals that may wait at once.** A person answers them one at a time, and an agent that queued a page of them would be
/// a second job for the person; past this the call is refused until some are answered.
pub const STYLE_WAITING_MAX: usize = 8;

/// **What the person did with a proposal.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StyleState {
    /// Nobody has answered it.
    #[default]
    Waiting,
    /// The person kept it, and the change is in the page: the editor wrote it on the press, so this is a fact about the file.
    Kept,
    /// The person dropped it, and the page is as it was.
    Discarded,
}

impl StyleState {
    /// The word `grimoire_state.style_proposals` reports.
    pub fn as_str(self) -> &'static str {
        match self {
            StyleState::Waiting => "waiting",
            StyleState::Kept => "kept",
            StyleState::Discarded => "discarded",
        }
    }
}

/// **One proposed change, and what the person has done with it.**
#[derive(Clone, Debug, PartialEq)]
pub struct QueuedStyle {
    /// `style-1`, numbered for the life of the process.
    pub id: String,
    /// The page, relative to the project root.
    pub path: String,
    /// The agent's reason, one sentence.
    pub why: String,
    /// The edits, in order, as the agent wrote them.
    pub edits: Vec<TextEdit>,
    /// The project it was made in: a proposal kept in another project would be written into the wrong page.
    pub root: PathBuf,
    /// **The page's digest when the proposal was made**, which Keep compares with the page as it is then.
    pub page_sha256: String,
    /// The lines (1-based, inclusive) the change touches, when it can say.
    pub lines: Option<[u32; 2]>,
    /// Who proposed it, as the client said it was (empty until it said).
    pub client: String,
    pub state: StyleState,
    /// **Why Keep did not write**, when it did not. The row says it and the proposal stays waiting: a page that was not written is not an answer.
    pub note: Option<String>,
}

/// **The style proposals an agent has made this session, in the order they arrived.** `Session`, not `Door`: a proposal is about the project's
/// design, which no door change moves.
#[derive(Resource, Default)]
pub struct StyleProposals {
    pub queued: Vec<QueuedStyle>,
    next: u32,
}

impl StyleProposals {
    /// **The proposals as `grimoire_state` reports them**: `{id, path, state, lines}` each, oldest first. **No text of a page.**
    pub fn state_json(&self) -> serde_json::Value {
        serde_json::Value::Array(self.queued.iter().map(|q| json!({"id": q.id, "path": q.path, "state": q.state.as_str(), "lines": q.lines})).collect())
    }

    /// **How many proposals are waiting for the person.**
    pub fn waiting(&self) -> usize {
        self.queued.iter().filter(|q| q.state == StyleState::Waiting).count()
    }

    /// **The first one still waiting**, the row a chip opens the block on when nothing else waits.
    pub fn first_waiting(&self) -> Option<&str> {
        self.queued.iter().find(|q| q.state == StyleState::Waiting).map(|q| q.id.as_str())
    }

    /// **The waiting proposal for this page**, which a second call about the same page is refused for.
    pub fn waiting_for(&self, root: &Path, path: &str) -> Option<&QueuedStyle> {
        self.queued.iter().find(|q| q.state == StyleState::Waiting && q.root == root && q.path == path)
    }

    /// **Take a proposal**, numbering it and keeping it waiting behind the ones already there.
    pub fn take(&mut self, mut proposal: QueuedStyle) -> String {
        self.next += 1;
        let id = format!("style-{}", self.next);
        proposal.id.clone_from(&id);
        self.queued.push(proposal);
        id
    }
}

/// **The sentence Keep answers when the page is not as the proposal saw it.** Names the page and nothing of its text.
pub fn changed_since(path: &str) -> String {
    format!("`{path}` changed after this was proposed, so nothing was written: a change is not kept against text you did not see. Ask for it again against the page as it is now, or discard this one.")
}

/// **Keep**: the person's own act, which writes the proposed change by dispatching `style_edit` as theirs. Returns what to say, and whether it was written.
///
/// **A page that is not as it was when proposed is refused**, naming the page. Any other refusal of the operation (a limit, a picture that is not
/// there, a page the person has not allowed) is the proposal's note, and the proposal stays waiting.
pub fn keep_this_style_change(queued: &QueuedStyle, project: &crate::project::Project, trust_file: Option<PathBuf>) -> Result<(String, String), String> {
    if queued.root != project.root {
        return Err(format!("`{}` was proposed in another project, so nothing was written.", queued.path));
    }
    let now = std::fs::read_to_string(project.root.join(&queued.path)).ok();
    if now.as_deref().map(|t| bevy_grimoire_core::trust::sha256_hex(t.as_bytes())) != Some(queued.page_sha256.clone()) {
        return Err(changed_since(&queued.path));
    }
    let params = json!({"path": queued.path, "edits": queued.edits});
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        trust_file,
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "style_edit", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => Ok((operation_id, outcome.summary)),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}
