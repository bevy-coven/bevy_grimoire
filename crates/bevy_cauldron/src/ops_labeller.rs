//! **The labeller's three requests as registered operations** (E-002, #129 slice 34): `labels_suggest`, `labels_walk` and
//! `labels_abandon`.
//!
//! `L` photographs the piece in focus and asks the model for its labels, `Shift+L` walks everything in scope (or holds the walk
//! that is running, or lets a held one go on) and `Shift+Y` drops everything the labeller holds. The labeller is a background
//! worker (the photo booth, a queue of pieces and the model's endpoint), so each is a **request** and not a document change:
//! the operation reads the labeller's state off the view it is carried ([`crate::ops_view::LabelsCursor`]), decides what is
//! wanted and says so ([`crate::ops_view::LabelsAnswer`]), and the front end carries the answer out
//! ([`crate::labels::Labeller::take`]), for a key and for an agent's call alike. The labels that arrive land through
//! `label_apply`'s own field copy with their own undo (`Cmd+Z` per piece); nothing here writes a file.
//!
//! `label_suggest` (E-043) is the other way to ask: an agent that brings its own two shots asks the endpoint for one library
//! piece and reads the proposal back. These three drive the editor's booth and queue, which is what `L`, `Shift+L` and `Shift+Y` do.

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};
use crate::ops_view::{LabelsAnswer, LabelsCursor};
use serde::Deserialize;
use serde_json::json;

/// **The labeller's cursor, from the view in hand**, or the refusal that says no front end carried one.
fn labels<'a>(ctx: &'a Ctx, what: &str) -> Result<&'a LabelsCursor, Refusal> {
    ctx.view.as_ref().map(|v| &v.labels).ok_or_else(|| {
        Refusal::new(format!("no view is in hand, so there is nothing to {what}. An editor front end carries one in `Ctx::view`."))
    })
}

/// **An operation that takes no fields**, or the refusal that says what it takes instead.
fn no_fields(params: &serde_json::Value, takes: &str) -> Result<(), Refusal> {
    match params {
        serde_json::Value::Null => Ok(()),
        serde_json::Value::Object(o) if o.is_empty() => Ok(()),
        _ => Err(Refusal::new(format!("these are not fields it takes: {takes}"))),
    }
}

/// **What abandoning the labeller says it dropped**: the sentence the key has always said, written once for the operation's answer
/// and the front end that does the dropping.
pub(crate) fn cleared_says(proposals: usize, queued: usize, in_flight: usize) -> String {
    format!("cleared {proposals} proposal(s), {queued} queued, {in_flight} in flight — applied labels are undone per piece with Cmd+Z")
}

/// **What starting a walk says**: how many pieces, and the filter that scoped them when one did.
pub(crate) fn labeling_says(total: usize, scope: &str) -> String {
    if scope.is_empty() {
        format!("labeling {total} piece(s) — readying the endpoint first... Shift+L holds it")
    } else {
        format!("labeling {total} piece(s) matching `{scope}` — readying the endpoint first... Shift+L holds it")
    }
}

// ── labels_suggest ───────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestParams {
    /// A library piece's id, or a candidate's mesh path (what the Meshes tab's list shows for a mesh not yet in the library).
    piece: String,
}

fn suggest_params(raw: &serde_json::Value) -> Result<SuggestParams, Refusal> {
    serde_json::from_value(raw.clone()).map_err(|e| {
        Refusal::new(format!("this is not a piece to label: {e}. The field is `piece`: a library piece's id or a candidate's mesh path."))
    })
}

/// **Photograph a piece in the editor's booth and ask the model for its labels**, as `L` does on the Meshes tab.
pub struct LabelsSuggest;

impl LabelsSuggest {
    /// What is asked of the front end and the sentence for it: the rule, one copy for plan and commit.
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(LabelsAnswer, String), Refusal> {
        let p = suggest_params(params)?;
        let cursor = labels(ctx, "label")?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there is no piece to label."));
        };
        let (candidate, has_mesh) = match project.library.get(&p.piece) {
            Some(d) => (false, d.mesh.is_some()),
            None if cursor.candidates.contains(&p.piece) => (true, true),
            None => {
                return Err(Refusal::new(format!(
                    "no piece is called `{}`: name a library piece's id (see `grimoire/pieces`) or a candidate's mesh path from the Meshes tab.",
                    p.piece
                )));
            }
        };
        if cursor.busy.contains(&p.piece) {
            return Ok((LabelsAnswer::Nothing, format!("`{}` is already at the model", p.piece)));
        }
        if !has_mesh {
            return Err(Refusal::new("this piece has no mesh — nothing to photograph"));
        }
        let said = format!("photographing `{}` for labels...", p.piece);
        Ok((LabelsAnswer::Photograph { piece: p.piece, candidate }, said))
    }
}

impl Operation for LabelsSuggest {
    fn name(&self) -> &'static str {
        "labels_suggest"
    }
    fn changes_view(&self) -> bool {
        true
    }
    /// **The piece the labels will land on**, when it is in the library; a candidate has no row to show yet.
    fn shows(&self, _ctx: &Ctx, params: &serde_json::Value) -> Shown {
        match params.get("piece").and_then(serde_json::Value::as_str) {
            Some(id) if !id.contains('/') => Shown::Piece { id: id.to_owned() },
            _ => Shown::Nowhere,
        }
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["piece"],
            "properties": {
                "piece": {"type": "string", "description": "A library piece's id, or a candidate's mesh path as the Meshes tab lists it."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "answer": answer, "says": said }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        if let Some(view) = ctx.view.as_mut() {
            view.labels.answer = answer.clone();
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "answer": answer })) })
    }
}

// ── labels_walk ──────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct WalkParams {
    /// `true` walks every piece in scope, `false` only the pieces with no judgement yet; absent asks the person when some in
    /// scope are labelled already.
    #[serde(default)]
    relabel: Option<bool>,
}

fn walk_params(raw: &serde_json::Value) -> Result<WalkParams, Refusal> {
    if raw.is_null() {
        return Ok(WalkParams::default());
    }
    serde_json::from_value(raw.clone()).map_err(|e| {
        Refusal::new(format!("these are not fields a walk takes: {e}. The only field is `relabel` (true, or false), and it is optional."))
    })
}

/// **Walk what is in scope through the labeller, or hold the walk that is running, or let a held one go on**, as `Shift+L` does.
pub struct LabelsWalk;

impl LabelsWalk {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(LabelsAnswer, String), Refusal> {
        let relabel = walk_params(params)?.relabel;
        let l = labels(ctx, "walk")?;
        if l.asking {
            return Err(Refusal::new("a question about which pieces to label is already on screen: answer it first."));
        }
        // **A second call HOLDS the walk; it does not throw it away.** Abandoning is `labels_abandon`.
        if l.running {
            if relabel.is_some() {
                return Err(Refusal::new(format!(
                    "a walk is already running ({}/{}): a call now holds it, or lets it go on if it is held, and takes no `relabel`.",
                    l.done, l.total
                )));
            }
            return Ok(if l.paused {
                (LabelsAnswer::Resume, format!("batch resumed at {}/{}", l.done, l.total))
            } else {
                (
                    LabelsAnswer::Hold,
                    format!(
                        "batch held at {}/{} — Shift+L resumes, Shift+Y abandons it. Nothing already proposed is affected",
                        l.done, l.total
                    ),
                )
            });
        }
        if ctx.project.is_none() {
            return Err(Refusal::new("no project is open, so there is no library to walk."));
        }
        // **Ask whether anybody is home before committing to a walk**: with the endpoint down, a walk queued every mesh and
        // reported one identical failure per mesh.
        if let Some(remedy) = &l.unreachable {
            return Err(Refusal::new(format!("no batch: {remedy}")));
        }
        let judged = l.everything.saturating_sub(l.unjudged);
        let all = match relabel {
            Some(all) => all,
            None if judged > 0 => {
                return Ok((
                    LabelsAnswer::Ask,
                    format!("{judged} of {} in scope are judged: asking which pieces to label", l.everything),
                ));
            }
            None => false,
        };
        let total = if all { l.everything } else { l.unjudged };
        if total == 0 {
            // **Say which set was empty.** "Nothing is missing labels" is a lie when a filter is on and the unfiltered
            // library is full of unjudged meshes.
            let said = match (all, l.scope.is_empty()) {
                (true, _) => "nothing is in scope to label".to_owned(),
                (false, true) => "nothing is missing labels — L re-asks for a single piece".to_owned(),
                (false, false) => format!("nothing matching `{}` is missing labels — Backspace in the filter widens it", l.scope),
            };
            return Ok((LabelsAnswer::Nothing, said));
        }
        Ok((LabelsAnswer::Start { all }, labeling_says(total, &l.scope)))
    }
}

impl Operation for LabelsWalk {
    fn name(&self) -> &'static str {
        "labels_walk"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "relabel": {
                    "type": "boolean",
                    "description": "true walks every piece in scope, false only the pieces with no labels yet. Absent asks the person at the editor when some pieces in scope are labelled already. Not accepted while a walk runs."
                }
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "answer": answer, "says": said }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        if let Some(view) = ctx.view.as_mut() {
            view.labels.answer = answer.clone();
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "answer": answer })) })
    }
}

// ── labels_abandon ───────────────────────────────────────────────────────────────────────────────

/// **Drop everything the labeller holds**, as `Shift+Y` does: the proposals, the queue, the booth's shots and the requests in flight.
pub struct LabelsAbandon;

impl LabelsAbandon {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<String, Refusal> {
        no_fields(params, "it takes none. Everything the labeller holds is what it drops.")?;
        let l = labels(ctx, "abandon")?;
        Ok(cleared_says(l.proposals, l.queued, l.in_flight))
    }
}

impl Operation for LabelsAbandon {
    fn name(&self) -> &'static str {
        "labels_abandon"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({"type": "object", "additionalProperties": false, "properties": {}})
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Ok(json!({ "says": Self::decide(ctx, params)? }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let said = Self::decide(ctx, params)?;
        if let Some(view) = ctx.view.as_mut() {
            view.labels.answer = LabelsAnswer::Abandon;
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "answer": LabelsAnswer::Abandon })) })
    }
}
