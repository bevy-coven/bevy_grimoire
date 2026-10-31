//! **Notes for agents, as registered operations** (#89): `note_add`, `note_edit`, `note_remove`.
//!
//! A note is data beside the map it marks (`bevy_grimoire_core::notes`, the `.notes.ron` next to the
//! `.map.ron`). These three operations are the only way the editor changes it, so a person's click
//! and an agent's `apply` are one change with one log line and one undo.
//!
//! Each operation's inverse is the same operation sent a parameter only an undo may send
//! (`take_back`, `put_back`), the way `placement_remove` puts a piece back. A plan is the commit run
//! on a copy that is never written, so a preview cannot disagree with what a dispatch does.

use bevy_grimoire_core::notes::{self, Note, Notes};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// **A note put back where it stood**, for the undo of a removal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub index: usize,
    pub note: Note,
}

fn notes_path(ctx: &Ctx) -> Result<std::path::PathBuf, Refusal> {
    let Some(open) = ctx.open.as_ref() else {
        return Err(Refusal::new("no map is open, so there is nowhere to leave a note."));
    };
    Ok(notes::path_for_map(&open.map_path))
}

/// **The notes of the open map and the map's name**, which is the state an undo checks.
fn state_of_open(ctx: &Ctx) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    let notes = Notes::load(&notes::path_for_map(&open.map_path)).ok()?;
    Some(json!({"map": open.map.name, "notes": serde_json::to_value(&notes).ok()?}))
}

/// What one edit to the notes gives back: the receipt line, the inverse, and how many notes it touched.
struct Change {
    summary: String,
    inverse: serde_json::Value,
}

/// **Run one change on the open map's notes.** `write` false is the plan: the same code on the same
/// notes, and nothing is saved.
fn run(
    ctx: &mut Ctx,
    write: bool,
    change: impl FnOnce(&mut Notes, &bevy_grimoire_core::map::Map) -> Result<Change, Refusal>,
) -> Result<Option<Outcome>, Refusal> {
    let path = notes_path(ctx)?;
    let mut notes = Notes::load(&path).map_err(Refusal::new)?;
    let open = ctx.open.as_ref().expect("notes_path checked the map");
    let done = change(&mut notes, &open.map)?;
    if !write {
        return Ok(None);
    }
    notes.save(&path).map_err(|e| Refusal::new(format!("the notes file refused the change, so nothing was written: {e}")))?;
    Ok(Some(Outcome {
        summary: done.summary,
        affected: 1,
        saved: true,
        inverse: Some(done.inverse),
        after: state_of_open(ctx),
    }))
}

/// `by_add` says which operation is putting it back, because an inverse is dispatched under the
/// operation that logged it: `note_add` is undone with `take_back`, `note_remove` with `id`.
fn put_back(notes: &mut Notes, pb: &PutBack, by_add: bool) -> Result<Change, Refusal> {
    if notes.get(&pb.note.id).is_some() {
        return Err(Refusal::new(format!("note {} is already on the map, so it was not put back.", pb.note.id)));
    }
    let at = pb.index.min(notes.notes.len());
    let id = pb.note.id.clone();
    notes.notes.insert(at, pb.note.clone());
    let inverse = if by_add { json!({"take_back": id}) } else { json!({"id": id}) };
    Ok(Change { summary: format!("put back note {id}"), inverse })
}

fn take_out(notes: &mut Notes, id: &str, what: &str) -> Result<Change, Refusal> {
    let index = notes
        .notes
        .iter()
        .position(|n| n.id == id)
        .ok_or_else(|| Refusal::new(format!("no note {id} on this map, so nothing was {what}.")))?;
    let note = notes.remove(id).map_err(Refusal::new)?;
    Ok(Change { summary: format!("{what} note {id}"), inverse: json!({"put_back": PutBack { index, note }}) })
}

fn bad_params(what: &str, fields: &str, e: serde_json::Error) -> Refusal {
    Refusal::new(format!("these are not {what}: {e}. The fields are {fields}, and nothing else."))
}

// ---------------------------------------------------------------- note_add

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddParams {
    /// Inclusive cell rectangle `[min_x, min_z, max_x, max_z]` the note is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cells: Option<[i32; 4]>,
    /// Placement ids the note is about, instead of a rectangle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// The camera pose when written: `[x, y, z, yaw, pitch]`. Zeros when the writer has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<[f32; 5]>,
    /// Decide open choices yourself: the agent that takes the note may not ask about them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto: bool,
    /// Undo only: take a note this operation added back off the map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take_back: Option<String>,
    /// Undo only: put a note this operation's undo took off back where it stood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl AddParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["text"],
            "properties": {
                "cells": {
                    "type": "array", "items": {"type": "integer"}, "minItems": 4, "maxItems": 4,
                    "description": "The cell rectangle the note is about, inclusive: min_x, min_z, max_x, max_z. Give this or `pieces`."
                },
                "pieces": {
                    "type": "array", "items": {"type": "string"},
                    "description": "Ids of the placements the note is about. Give this or `cells`."
                },
                "text": {"type": "string", "description": "The instruction, for an agent, about that place."},
                "camera": {
                    "type": "array", "items": {"type": "number"}, "minItems": 5, "maxItems": 5,
                    "description": "The camera pose when written: x, y, z, yaw, pitch. Optional."
                },
                "auto": {
                    "type": "boolean",
                    "description": "Decide open choices yourself this turn: the agent that takes the note may not ask the person about them and records its assumptions on resolving. Fenced changes are still refused. Omitted is false."
                }
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<AddParams, Refusal> {
        serde_json::from_value(raw.clone())
            .map_err(|e| bad_params("a note", "`cells` or `pieces`, `text` and `camera`", e))
    }

    fn undo_only(&self) -> bool {
        self.take_back.is_some() || self.put_back.is_some()
    }
}

/// **Leave a draft note on a cell rectangle or on pieces.**
pub struct NoteAdd;

impl NoteAdd {
    fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = AddParams::parse(params)?;
        run(ctx, write, |notes, map| {
            if let Some(id) = &p.take_back {
                return take_out(notes, id, "took back");
            }
            if let Some(pb) = &p.put_back {
                return put_back(notes, pb, true);
            }
            let camera = p.camera.map_or((0.0, 0.0, 0.0, 0.0, 0.0), |c| (c[0], c[1], c[2], c[3], c[4]));
            let cell = bevy_grimoire_core::grid::TILE;
            let id = match (&p.cells, &p.pieces) {
                (Some(c), None) => notes.add_area(map, (c[0], c[1], c[2], c[3]), cell, camera, &p.text),
                (None, Some(ids)) => notes.add_pieces(map, ids, cell, camera, &p.text),
                _ => return Err(Refusal::new("a note is about `cells` or about `pieces`, and not both. Nothing was added.")),
            }
            .map_err(Refusal::new)?;
            if p.auto {
                notes.set_auto(&id, true).map_err(Refusal::new)?;
            }
            Ok(Change { summary: format!("left note {id}"), inverse: json!({"take_back": id}) })
        })
    }
}

impl Operation for NoteAdd {
    fn name(&self) -> &'static str {
        "note_add"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["notes"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        AddParams::parse(params).is_ok_and(|p| p.undo_only())
    }
    fn schema(&self) -> serde_json::Value {
        AddParams::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), ..Ctx::default() };
        Self::apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// --------------------------------------------------------------- note_edit

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditParams {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// Undo only: the note as it stood before the edit, put back whole so its history is not
    /// lengthened by the undo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Note>,
}

impl EditParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "text"],
            "properties": {
                "id": {"type": "string", "description": "The note's id."},
                "text": {"type": "string", "description": "The new text. A resolved note is a record and is not edited."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<EditParams, Refusal> {
        let p: EditParams = serde_json::from_value(raw.clone()).map_err(|e| bad_params("a note edit", "`id` and `text`", e))?;
        if p.restore.is_none() && (p.id.is_empty() || p.text.is_empty()) {
            return Err(Refusal::new("a note edit needs the note's `id` and its new `text`. Nothing was edited."));
        }
        Ok(p)
    }
}

/// **Change a draft or open note's text.**
pub struct NoteEdit;

impl NoteEdit {
    fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = EditParams::parse(params)?;
        run(ctx, write, |notes, _| {
            if let Some(old) = &p.restore {
                let at = notes
                    .notes
                    .iter()
                    .position(|n| n.id == old.id)
                    .ok_or_else(|| Refusal::new(format!("note {} is gone, so the edit was not taken back.", old.id)))?;
                notes.notes[at] = old.clone();
                return Ok(Change { summary: format!("took back the edit of note {}", old.id), inverse: json!({}) });
            }
            let before = notes.get(&p.id).ok_or_else(|| Refusal::new(format!("no note {} on this map, so nothing was edited.", p.id)))?.clone();
            notes.edit(&p.id, &p.text).map_err(Refusal::new)?;
            Ok(Change { summary: format!("edited note {}", p.id), inverse: json!({"restore": before}) })
        })
    }
}

impl Operation for NoteEdit {
    fn name(&self) -> &'static str {
        "note_edit"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["notes"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        EditParams::parse(params).is_ok_and(|p| p.restore.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        EditParams::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), ..Ctx::default() };
        Self::apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// ------------------------------------------------------------- note_remove

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Undo only: the note a removal took, and the place it held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl RemoveParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id"],
            "properties": {"id": {"type": "string", "description": "The note's id. Its sign comes off the map."}}
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<RemoveParams, Refusal> {
        let p: RemoveParams = serde_json::from_value(raw.clone())
            .map_err(|e| bad_params("a note removal", "`id`", e))?;
        if p.id.is_some() == p.put_back.is_some() {
            return Err(Refusal::new("a removal names one note by `id`. Nothing was removed."));
        }
        Ok(p)
    }
}

/// **Take a note off the map.**
pub struct NoteRemove;

impl NoteRemove {
    fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = RemoveParams::parse(params)?;
        run(ctx, write, |notes, _| match (&p.id, &p.put_back) {
            (Some(id), _) => take_out(notes, id, "removed"),
            (None, Some(pb)) => put_back(notes, pb, false),
            (None, None) => unreachable!("parse refused it"),
        })
    }
}

impl Operation for NoteRemove {
    fn name(&self) -> &'static str {
        "note_remove"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["notes"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        RemoveParams::parse(params).is_ok_and(|p| p.put_back.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        RemoveParams::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), ..Ctx::default() };
        Self::apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// --------------------------------------------------------------- note_send

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Undo only: the notes as they stood before the send, put back whole, history included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Vec<Note>>,
}

impl SendParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {"id": {"type": "string", "description": "The draft to send. Omit it to send every draft on the map."}}
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<SendParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| bad_params("a note send", "`id`, or nothing to send every draft", e))
    }
}

/// **Send one draft, or every draft, so an agent may claim them.** One undo step returns them to drafts.
pub struct NoteSend;

impl NoteSend {
    fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = SendParams::parse(params)?;
        run(ctx, write, |notes, _| {
            if let Some(before) = &p.restore {
                for old in before {
                    let at = notes
                        .notes
                        .iter()
                        .position(|n| n.id == old.id)
                        .ok_or_else(|| Refusal::new(format!("note {} is gone, so the send was not taken back.", old.id)))?;
                    notes.notes[at] = old.clone();
                }
                return Ok(Change { summary: "took back a send".to_owned(), inverse: json!({}) });
            }
            let sent = match &p.id {
                Some(id) => {
                    let before = notes.get(id).ok_or_else(|| Refusal::new(format!("no note {id} on this map, so nothing was sent.")))?.clone();
                    notes.send(id).map_err(Refusal::new)?;
                    (vec![before], vec![id.clone()])
                }
                None => {
                    let before: Vec<Note> = notes.notes.iter().filter(|n| n.is_draft()).cloned().collect();
                    let ids = notes.send_all().map_err(Refusal::new)?;
                    (before, ids)
                }
            };
            Ok(Change {
                summary: format!("sent {}", sent.1.join(", ")),
                inverse: json!({"restore": sent.0}),
            })
        })
    }
}

impl Operation for NoteSend {
    fn name(&self) -> &'static str {
        "note_send"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["notes"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        SendParams::parse(params).is_ok_and(|p| p.restore.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        SendParams::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), ..Ctx::default() };
        Self::apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// ------------------------------------------------- note_claim, note_renew, note_release (#91)

/// **What a lease operation is asked**, shared by `note_claim`, `note_renew` and `note_release`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Who holds, or is to hold, the lease. A person releasing a claim from the editor omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The call's time, in seconds since the Unix epoch. Omit it to use the clock; a replay or a
    /// test that must not wait out a lease names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
    /// Undo only: the notes as they stood before, put back whole, history included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Vec<Note>>,
}

impl LeaseParams {
    pub fn schema(what: &str) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "agent"],
            "properties": {
                "id": {"type": "string", "description": format!("The note to {what}.")},
                "agent": {"type": "string", "description": "Your name, which shows on the note while you hold it."},
                "at": {"type": "integer", "minimum": 0, "description": "The call's time in seconds since the Unix epoch. Omit it: the editor's clock is used."}
            }
        })
    }

    fn parse(raw: &serde_json::Value, what: &str) -> Result<LeaseParams, Refusal> {
        let p: LeaseParams = serde_json::from_value(raw.clone()).map_err(|e| bad_params(what, "`id`, `agent`, and `at`", e))?;
        if p.restore.is_none() && p.id.is_none() {
            return Err(Refusal::new(format!("{what} names the note by `id`. Nothing was changed.")));
        }
        Ok(p)
    }

    fn clock(&self) -> u64 {
        self.at.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()))
    }
}

#[derive(Clone, Copy)]
enum Lease {
    Claim,
    Renew,
    Release,
}

impl Lease {
    fn name(self) -> &'static str {
        match self {
            Lease::Claim => "note_claim",
            Lease::Renew => "note_renew",
            Lease::Release => "note_release",
        }
    }
    fn what(self) -> &'static str {
        match self {
            Lease::Claim => "a note claim",
            Lease::Renew => "a note renewal",
            Lease::Release => "a note release",
        }
    }

    fn apply(self, ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = LeaseParams::parse(params, self.what())?;
        let source = ctx.source;
        run(ctx, write, |notes, _| {
            if let Some(before) = &p.restore {
                *notes = Notes { notes: before.clone(), ..notes.clone() };
                return Ok(Change { summary: format!("took back {}", self.what()), inverse: json!({}) });
            }
            let id = p.id.as_deref().expect("parse checked it");
            let now = p.clock();
            let before = notes.notes.clone();
            let agent = p.agent.as_deref();
            let said = match self {
                Lease::Claim => {
                    let by = agent.ok_or_else(|| Refusal::new("a claim names the agent making it in `agent`. Nothing was claimed."))?;
                    let until = notes.claim(id, by, now).map_err(Refusal::new)?;
                    format!("{by} claimed note {id} until {until}")
                }
                Lease::Renew => {
                    let by = agent.ok_or_else(|| Refusal::new("a renewal names the holder in `agent`. Nothing was renewed."))?;
                    let until = notes.renew(id, by, now).map_err(Refusal::new)?;
                    format!("{by} renewed note {id} until {until}")
                }
                Lease::Release => {
                    if agent.is_none() && source == crate::ops::Source::Agent {
                        return Err(Refusal::new("an agent releases a note it holds by naming itself in `agent`. Nothing was released."));
                    }
                    notes.release(id, agent, now).map_err(Refusal::new)?;
                    format!("released note {id}")
                }
            };
            Ok(Change { summary: said, inverse: json!({"restore": before}) })
        })
    }
}

macro_rules! lease_operation {
    ($(#[$doc:meta])* $ty:ident, $lease:expr, $what:literal) => {
        $(#[$doc])*
        pub struct $ty;

        impl Operation for $ty {
            fn name(&self) -> &'static str {
                $lease.name()
            }
            fn writes(&self) -> &'static [&'static str] {
                &["notes"]
            }
            fn undo(&self) -> Undo {
                Undo::Inverse
            }
            fn only_as_undo(&self, params: &serde_json::Value) -> bool {
                LeaseParams::parse(params, $lease.what()).is_ok_and(|p| p.restore.is_some())
            }
            fn schema(&self) -> serde_json::Value {
                LeaseParams::schema($what)
            }
            fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
                state_of_open(ctx)
            }
            fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
                let mut scratch = Ctx { open: ctx.open.clone(), source: ctx.source, ..Ctx::default() };
                $lease.apply(&mut scratch, params, false)?;
                Ok(params.clone())
            }
            fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
                Ok($lease.apply(ctx, params, true)?.expect("a write returns its outcome"))
            }
        }
    };
}

lease_operation!(
    /// **Take the lease on a sent note.** Refused for a held note (naming the holder), one overlapping
    /// a held note, and one overlapping a note sent first. A lease not renewed in the project's lease
    /// length returns the note to the queue.
    NoteClaim, Lease::Claim, "claim"
);
lease_operation!(
    /// **Keep the lease while working**: it runs another lease length from now.
    NoteRenew, Lease::Renew, "renew"
);
lease_operation!(
    /// **Give a claimed note back to the queue**, the agent that holds it or the person.
    NoteRelease, Lease::Release, "release"
);

/// **What `note_ask`, `note_answer` and `note_resolve` are asked** (#92).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TalkParams {
    pub id: Option<String>,
    /// `note_ask`, `note_resolve`: the holder's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// `note_ask`: the question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// `note_answer`: the person's answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    /// `note_resolve`: the one-line summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// `note_resolve`: ids of the operations the agent ran under the note.
    #[serde(default)]
    pub ops: Vec<String>,
    /// `note_resolve`: `[before, after]` capture paths.
    #[serde(default)]
    pub captures: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
    /// Undo only: the notes as they stood before, put back whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Vec<Note>>,
}

#[derive(Clone, Copy)]
enum Talk {
    Ask,
    Answer,
    Resolve,
    Keep,
}

impl Talk {
    fn name(self) -> &'static str {
        match self {
            Talk::Ask => "note_ask",
            Talk::Answer => "note_answer",
            Talk::Resolve => "note_resolve",
            Talk::Keep => "note_keep",
        }
    }

    fn schema(self) -> serde_json::Value {
        let (required, extra) = match self {
            Talk::Keep => (json!(["id"]), json!({})),
            Talk::Ask => (json!(["id", "agent", "question"]), json!({
                "agent": {"type": "string", "description": "Your name: the holder of the note."},
                "question": {"type": "string", "description": "What you need the person to decide. The note shows it as waiting for them."}
            })),
            Talk::Answer => (json!(["id", "answer"]), json!({
                "answer": {"type": "string", "description": "The person's answer to the question standing on the note."}
            })),
            Talk::Resolve => (json!(["id", "agent", "summary", "captures"]), json!({
                "agent": {"type": "string", "description": "Your name: the holder of the note."},
                "summary": {"type": "string", "description": "One line: what you did."},
                "assumptions": {"type": "array", "items": {"type": "string"}, "description": "What you assumed."},
                "ops": {"type": "array", "items": {"type": "string"}, "description": "Ids of the operations you ran under the note."},
                "captures": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 2, "description": "[before, after]: paths of two pictures of the area."}
            })),
        };
        let mut props = json!({
            "id": {"type": "string", "description": "The note."},
            "at": {"type": "integer", "minimum": 0, "description": "The call's time in seconds since the Unix epoch. Omit it."}
        });
        props.as_object_mut().expect("object").extend(extra.as_object().expect("object").clone());
        json!({"type": "object", "additionalProperties": false, "required": required, "properties": props})
    }

    fn apply(self, ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p: TalkParams = serde_json::from_value(params.clone())
            .map_err(|e| bad_params(self.name(), "`id`, `agent`, `question`, `answer`, `summary`, `assumptions`, `ops`, `captures`", e))?;
        let source = ctx.source;
        let refs = ctx.open.as_ref().and_then(|o| bevy_grimoire_core::references::References::load(&bevy_grimoire_core::references::path_for_map(&o.map_path)).ok()).unwrap_or_default();
        run(ctx, write, |notes, _| {
            let before = notes.notes.clone();
            if let Some(before) = &p.restore {
                *notes = Notes { notes: before.clone(), ..notes.clone() };
                return Ok(Change { summary: format!("took back {}", self.name()), inverse: json!({}) });
            }
            let id = p.id.as_deref().ok_or_else(|| Refusal::new(format!("{} names the note by `id`. Nothing was changed.", self.name())))?;
            let now = p.at.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
            let need = |v: &Option<String>, what: &str| -> Result<String, Refusal> {
                v.clone().ok_or_else(|| Refusal::new(format!("{} needs `{what}`. Nothing was changed.", self.name())))
            };
            let said = match self {
                Talk::Keep => {
                    notes.keep(id, now).map_err(Refusal::new)?;
                    format!("kept note {id}: its sign is gone")
                }
                Talk::Ask => {
                    let by = need(&p.agent, "agent")?;
                    notes.ask(id, &by, &need(&p.question, "question")?, now).map_err(Refusal::new)?;
                    format!("{by} asked on note {id}")
                }
                Talk::Answer => {
                    notes.answer(id, &need(&p.answer, "answer")?, now).map_err(Refusal::new)?;
                    format!("answered the question on note {id}")
                }
                Talk::Resolve => {
                    let by = need(&p.agent, "agent")?;
                    let mut captures = p.captures.clone();
                    if captures.is_empty() && source != crate::ops::Source::Agent {
                        // A person finishing a note by hand has no pictures to name; the record says so.
                        captures = vec!["(none: resolved by a person)".to_owned(); 2];
                    }
                    let cited = refs.unknown_citations(p.summary.iter().map(String::as_str).chain(p.assumptions.iter().map(String::as_str)));
                    if !cited.is_empty() {
                        return Err(Refusal::new(format!("{} cites {} but this map has no such reference (see `references` in grimoire_state). Nothing was changed.", self.name(), cited.join(", "))));
                    }
                    let record = notes::Resolution { summary: need(&p.summary, "summary")?, assumptions: p.assumptions.clone(), ops: p.ops.clone(), captures };
                    notes.resolve_record(id, &by, record, now).map_err(Refusal::new)?;
                    format!("{by} resolved note {id}")
                }
            };
            Ok(Change { summary: said, inverse: json!({"restore": before}) })
        })
    }
}

macro_rules! talk_operation {
    ($(#[$doc:meta])* $ty:ident, $talk:expr) => {
        $(#[$doc])*
        pub struct $ty;

        impl Operation for $ty {
            fn name(&self) -> &'static str {
                $talk.name()
            }
            fn writes(&self) -> &'static [&'static str] {
                &["notes"]
            }
            fn undo(&self) -> Undo {
                Undo::Inverse
            }
            fn only_as_undo(&self, params: &serde_json::Value) -> bool {
                params.get("restore").is_some()
            }
            fn schema(&self) -> serde_json::Value {
                $talk.schema()
            }
            fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
                state_of_open(ctx)
            }
            fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
                let mut scratch = Ctx { open: ctx.open.clone(), source: ctx.source, ..Ctx::default() };
                $talk.apply(&mut scratch, params, false)?;
                Ok(params.clone())
            }
            fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
                Ok($talk.apply(ctx, params, true)?.expect("a write returns its outcome"))
            }
        }
    };
}

talk_operation!(
    /// **The holder asks the person a question** and waits; the note shows it (#92).
    NoteAsk, Talk::Ask
);
talk_operation!(
    /// **The person answers the question standing on a note** (#92).
    NoteAnswer, Talk::Answer
);
talk_operation!(
    /// **The holder finishes a note** with summary, assumptions, operation ids and two captures (#92).
    NoteResolve, Talk::Resolve
);

talk_operation!(
    /// **The person keeps a done note's work**: the note is archived and its sign goes away. Cmd+Z
    /// brings it back to done (#93).
    NoteKeep, Talk::Keep
);

/// **What `note_undo_work` is asked** (#93).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UndoWorkParams {
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
    /// Undo only: the notes as they were, and the row changes that put the map back, in order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redo: Option<Redo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Redo {
    pub notes: Vec<Note>,
    pub deltas: Vec<crate::ops::RowDelta>,
}

/// The placement ids and cells one logged operation touched, read from what it recorded. Cells come
/// from rows the operation names, else from where the piece stands on `map` now.
pub(crate) fn footprint(e: &crate::ops::Entry, map: &bevy_grimoire_core::map::Map) -> notes::Footprint {
    let cell = |at: (f32, f32)| ((at.0 / bevy_grimoire_core::grid::TILE).floor() as i32, (at.1 / bevy_grimoire_core::grid::TILE).floor() as i32);
    let mut ids: Vec<String> = Vec::new();
    let mut cells: Vec<(i32, i32)> = Vec::new();
    if let Some(d) = e.inverse.as_ref().and_then(crate::ops::RowDelta::undoing) {
        ids.extend(d.remove_placements.iter().cloned());
        for a in &d.add_placements {
            ids.push(a.row.id.clone());
            cells.push(cell(a.row.at));
        }
    }
    for v in [Some(&e.params), e.inverse.as_ref(), e.after.as_ref()].into_iter().flatten() {
        if let Some(s) = v.get("id").and_then(|s| s.as_str()) {
            ids.push(s.to_owned());
        }
    }
    ids.sort();
    ids.dedup();
    for id in &ids {
        if let Some(p) = map.placements.iter().find(|p| &p.id == id) {
            cells.push(cell(p.at));
        }
    }
    cells.sort();
    cells.dedup();
    notes::Footprint { op: e.operation_id.clone(), placements: ids, cells }
}

fn state_with_rows(ctx: &Ctx) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    let notes = Notes::load(&notes::path_for_map(&open.map_path)).ok()?;
    let ids: Vec<&String> = open.map.placements.iter().map(|p| &p.id).collect();
    Some(json!({"map": open.map.name, "notes": serde_json::to_value(&notes).ok()?, "placements": ids}))
}

pub struct NoteUndoWork;

impl NoteUndoWork {
    fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p: UndoWorkParams = serde_json::from_value(params.clone()).map_err(|e| bad_params("a note's work undone", "`id` and `at`", e))?;
        let path = notes_path(ctx)?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there is no operation log to undo from."));
        };
        let open = ctx.open.as_ref().expect("notes_path checked the map");
        let mut notes = Notes::load(&path).map_err(Refusal::new)?;
        let before_notes = notes.notes.clone();
        let mut map = open.map.clone();
        let mut inverses: Vec<crate::ops::RowDelta> = Vec::new();
        let summary;
        if let Some(redo) = &p.redo {
            notes.notes = redo.notes.clone();
            for d in &redo.deltas {
                inverses.push(d.apply(&mut map)?);
            }
            summary = "took back a note's undo".to_owned();
        } else {
            let id = p.id.as_deref().ok_or_else(|| Refusal::new("note_undo_work names the note by `id`. Nothing was changed."))?;
            let now = p.at.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
            let ops: Vec<String> = match notes.get(id).map(|n| &n.state) {
                Some(notes::State::Resolved { ops, .. }) => ops.clone(),
                Some(_) => return Err(Refusal::new(format!("note {id} is not done, so there is no work to undo. Nothing was changed."))),
                None => return Err(Refusal::new(format!("no note {id} on this map. Nothing was changed."))),
            };
            let entries = crate::ops::read_entries(&crate::ops::oplog_path(project));
            let undone: Vec<&str> = entries.iter().filter(|e| e.outcome == "applied").filter_map(|e| e.undoes.as_deref()).collect();
            let mut mine: Vec<&crate::ops::Entry> = Vec::new();
            for op in &ops {
                let e = entries
                    .iter()
                    .find(|e| &e.operation_id == op && e.outcome == "applied")
                    .ok_or_else(|| Refusal::new(format!("operation {op}, which note {id} ran, is not in the log, so its work cannot be undone. Nothing was changed.")))?;
                if !undone.contains(&e.operation_id.as_str()) {
                    mine.push(e);
                }
            }
            let first = mine.iter().map(|e| e.seq).min();
            let note_fps: Vec<notes::Footprint> = mine.iter().map(|e| footprint(e, &map)).collect();
            let later_fps: Vec<notes::Footprint> = entries
                .iter()
                .filter(|e| e.outcome == "applied" && first.is_some_and(|f| e.seq > f))
                .filter(|e| !ops.contains(&e.operation_id) && !e.op.starts_with("note_"))
                .filter(|e| e.undoes.as_deref().is_none_or(|u| !ops.iter().any(|o| o == u)))
                .map(|e| footprint(e, &map))
                .collect();
            notes.undo_work(id, &note_fps, &later_fps, now).map_err(Refusal::new)?;
            for e in mine.iter().rev() {
                let delta = e
                    .inverse
                    .as_ref()
                    .and_then(crate::ops::RowDelta::undoing)
                    .ok_or_else(|| Refusal::new(format!("{} ({}) changed more than pieces, so note {id}'s work cannot be undone in one step. Undo it by hand.", e.operation_id, e.op)))?;
                inverses.push(delta.apply(&mut map)?);
            }
            summary = format!("undid note {id}'s work ({} operation(s)); the note is sent again", mine.len());
        }
        if !write {
            return Ok(None);
        }
        inverses.reverse();
        let inverse = json!({"redo": Redo { notes: before_notes, deltas: inverses }});
        let affected = map.placements.len().abs_diff(open.map.placements.len());
        let open = ctx.open.as_mut().expect("checked");
        let old = std::mem::replace(&mut open.map, map);
        if let Err(e) = open.save(ctx.project.as_ref().expect("checked")) {
            open.map = old;
            return Err(Refusal::new(format!("the map file refused the change, so nothing was written: {e}")));
        }
        if let Err(e) = notes.save(&path) {
            open.map = old;
            let _ = open.save(ctx.project.as_ref().expect("checked"));
            return Err(Refusal::new(format!("the notes file refused the change, so nothing was written: {e}")));
        }
        Ok(Some(Outcome { summary, affected, saved: true, inverse: Some(inverse), after: state_with_rows(ctx) }))
    }
}

impl Operation for NoteUndoWork {
    fn name(&self) -> &'static str {
        "note_undo_work"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map", "notes"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        params.get("redo").is_some()
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id"],
            "properties": {
                "id": {"type": "string", "description": "A done note whose work to undo: every operation it ran is reverted as one step and the note is sent again with its history. Refused, naming the later operation, when one touched the same placements or cells."},
                "at": {"type": "integer", "minimum": 0, "description": "The call's time in seconds since the Unix epoch. Omit it."}
            }
        })
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_with_rows(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), project: ctx.project.clone(), source: ctx.source, ..Ctx::default() };
        Self::apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

/// **What an agent reads to work a note** (#91): the text, the area plus its margin, the pieces in and
/// around it, the kit's palette, the camera the person had, how to get a picture of the area, and the
/// earlier notes on this map with how they turned out. `None` when no such note stands.
pub fn bundle(notes: &Notes, id: &str, map: &bevy_grimoire_core::map::Map, library: Option<&bevy_grimoire_core::library::Library>) -> Option<serde_json::Value> {
    let note = notes.get(id)?;
    let reach = note.reach_rect();
    let near = |at: (f32, f32)| reach.is_some_and(|r| at.0 >= r[0] && at.0 < r[2] && at.1 >= r[1] && at.1 < r[3]);
    let pieces: Vec<_> = map
        .placements
        .iter()
        .filter(|p| near(p.at))
        .map(|p| json!({"id": p.id, "kind": p.descriptor, "at": [p.at.0, p.at.1], "yaw": p.yaw, "lift": p.lift}))
        .collect();
    let palette: Vec<_> = library.map_or_else(Vec::new, |l| {
        l.descriptors.iter().map(|d| json!({"id": d.id, "extent": d.extent})).collect()
    });
    let earlier: Vec<_> = notes
        .notes
        .iter()
        .filter(|n| n.id != id)
        .map(|n| json!({"id": n.id, "text": n.text, "status": n.status(), "result": match &n.state {
            notes::State::Resolved { ops, summary, assumptions, captures }
            | notes::State::Kept { ops, summary, assumptions, captures } => {
                json!({"ops": ops, "summary": summary, "assumptions": assumptions, "captures": captures})
            }
            _ => serde_json::Value::Null,
        }}))
        .collect();
    Some(json!({
        "id": note.id,
        "text": note.text,
        "status": note.status(),
        "holder": note.holder(),
        "area_cells": note.site.as_ref().map(|s| [s.cells.0, s.cells.1, s.cells.2, s.cells.3]),
        "reach_cells": note.site.as_ref().map(|s| { let r = s.reach(); [r.0, r.1, r.2, r.3] }),
        "reach_world": reach,
        "margin": note.site.as_ref().map_or(notes::DEFAULT_MARGIN, |s| s.margin),
        "pieces": pieces,
        "palette": palette,
        "camera": note.site.as_ref().map(|s| json!({"x": s.camera.0, "y": s.camera.1, "z": s.camera.2, "yaw": s.camera.3, "pitch": s.camera.4})),
        "picture": reach.map(|r| json!({"tool": "grimoire_frame_view", "rect": r, "then": "grimoire_capture_still"})),
        "earlier_notes": earlier,
    }))
}

/// **The notes an agent can act on, for `grimoire/state`**: sent and claimed notes with their holder
/// and lease, oldest first, with `note` naming one to get its [`bundle`]. Leases that ran out by
/// `now` show as sent again, as a claim would find them.
pub fn state_json<L: std::ops::Deref<Target = bevy_grimoire_core::library::Library>>(open: &crate::project::OpenMap, library: Option<&L>, want: Option<&str>, now: u64) -> serde_json::Value {
    let Ok(mut notes) = Notes::load(&notes::path_for_map(&open.map_path)) else {
        return json!({"error": "the notes file could not be read"});
    };
    notes.expire(now);
    let list: Vec<_> = notes
        .notes
        .iter()
        .filter(|n| matches!(n.state, notes::State::Open | notes::State::Claimed { .. }))
        .map(|n| {
            let until = if let notes::State::Claimed { until, .. } = n.state { Some(until) } else { None };
            json!({"id": n.id, "text": n.text, "status": n.status(), "holder": n.holder(), "lease_until": until})
        })
        .collect();
    json!({
        "lease_secs": notes.lease_secs,
        "sent": list,
        "bundle": want.map(|id| bundle(&notes, id, &open.map, library.map(|l| &**l)).unwrap_or_else(|| json!({"error": format!("no note {id}")}))),
    })
}

/// **The operations that change pieces on the open map**, which the fence measures (#92). Every one of
/// them writes only the map file, so a trial of it can run against a copy of the map in a scratch
/// directory and leave the project untouched.
const FENCED: &[&str] = &[
    "placement_add",
    "placement_set_pin",
    "placement_set_yaw",
    "placement_set_tip",
    "placement_set_pose",
    "placement_set_lift",
    "placement_set_family",
    "placement_remove",
    "stamp_remove",
    "stamp_add",
    "fill",
    "layout_accept",
    "run_add",
    "run_set",
    "run_remove",
];

/// **What a trial of a fenced operation changed on the map**, for [`notes::Notes::fence`].
///
/// `agent_made` is the set of ids the operation log credits to an agent; a removed id outside it is a
/// piece a person placed (or one the file had before the log began, which is a person's too).
fn measure(before: &bevy_grimoire_core::map::Map, after: &bevy_grimoire_core::map::Map, agent_made: &std::collections::HashSet<String>) -> notes::Change {
    use std::collections::HashMap;
    let cell = |v: f32| (v / bevy_grimoire_core::grid::TILE).floor() as i32;
    let mut change = notes::Change::default();
    let touch = |at: (f32, f32), change: &mut notes::Change| {
        let (x, z) = (cell(at.0), cell(at.1));
        change.cells = Some(change.cells.map_or((x, z, x, z), |r| (r.0.min(x), r.1.min(z), r.2.max(x), r.3.max(z))));
    };
    fn diff<'a, T: PartialEq>(
        before: &'a [T],
        after: &'a [T],
        id: impl Fn(&T) -> &str,
    ) -> (Vec<&'a T>, Vec<&'a T>) {
        let old: HashMap<&str, &T> = before.iter().map(|r| (id(r), r)).collect();
        let new: HashMap<&str, &T> = after.iter().map(|r| (id(r), r)).collect();
        let gone = before.iter().filter(|r| new.get(id(r)) != Some(r)).collect();
        let came = after.iter().filter(|r| old.get(id(r)) != Some(r)).collect();
        (gone, came)
    }
    let mut removed_ids: Vec<&str> = Vec::new();
    let (gone, came) = diff(&before.placements, &after.placements, |r| r.id.as_str());
    let still: std::collections::HashSet<&str> = after.placements.iter().map(|r| r.id.as_str()).collect();
    for r in &gone {
        touch(r.at, &mut change);
        if !still.contains(r.id.as_str()) {
            removed_ids.push(&r.id);
        }
    }
    for r in &came {
        touch(r.at, &mut change);
    }
    change.pieces += gone.len().max(came.len());
    let (gone, came) = diff(&before.stamps, &after.stamps, |r| r.id.as_str());
    let still: std::collections::HashSet<&str> = after.stamps.iter().map(|r| r.id.as_str()).collect();
    for r in &gone {
        touch(r.at, &mut change);
        if !still.contains(r.id.as_str()) {
            removed_ids.push(&r.id);
        }
    }
    for r in &came {
        touch(r.at, &mut change);
    }
    change.pieces += gone.len().max(came.len());
    let (gone, came) = diff(&before.runs, &after.runs, |r| r.id.as_str());
    let still: std::collections::HashSet<&str> = after.runs.iter().map(|r| r.id.as_str()).collect();
    for r in gone.iter().chain(came.iter()) {
        touch(r.run.from, &mut change);
        touch(r.run.to, &mut change);
        for c in &r.run.corners {
            touch(c.at, &mut change);
        }
    }
    for r in &gone {
        if !still.contains(r.id.as_str()) {
            removed_ids.push(&r.id);
        }
    }
    change.pieces += gone.len().max(came.len());
    change.person_placed = removed_ids.into_iter().filter(|id| !agent_made.contains(*id)).map(str::to_owned).collect();
    change
}

/// **The ids the operation log credits to an agent**: what its applied, not-undone changes added,
/// read from each line's inverse (`delta.remove_placements`, `delta.remove_stamps`, `take_back`).
fn agent_made_ids(ctx: &Ctx) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    let Some(project) = ctx.project.as_ref() else { return ids };
    for e in crate::ops::read_entries(&crate::ops::oplog_path(project)) {
        if e.source != crate::ops::Source::Agent || e.outcome != "applied" || e.undoes.is_some() {
            continue;
        }
        let Some(inv) = e.inverse.as_ref() else { continue };
        for key in ["remove_placements", "remove_stamps"] {
            if let Some(list) = inv.pointer(&format!("/delta/{key}")).and_then(|v| v.as_array()) {
                ids.extend(list.iter().filter_map(|v| v.as_str().map(str::to_owned)));
            }
        }
        if let Some(id) = inv.get("take_back").and_then(|v| v.as_str()) {
            ids.insert(id.to_owned());
        }
    }
    ids
}

/// **The fence on an agent's operation** (#92): `Some(why)` when `agent` holds a note and the
/// operation would change something outside the note's area plus its margin, or something that
/// needs a question answered first. Nothing has been changed when this answers.
///
/// The trial runs the operation's own `commit` on a copy of the project whose `maps_dir` and the
/// copy's `map_path` point into a scratch directory, so the real map file, the log and the editor's
/// state are untouched; the two maps are then compared. An operation the trial itself refuses is
/// left to the real dispatch, which gives its own reason.
pub fn fence_refusal(ctx: &Ctx, agent: &str, op: &str, params: &serde_json::Value) -> Option<String> {
    if !FENCED.contains(&op) {
        return None;
    }
    let open = ctx.open.as_ref()?;
    let project = ctx.project.as_ref()?;
    let mut notes = Notes::load(&notes::path_for_map(&open.map_path)).ok()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    notes.expire(now);
    let held: Vec<String> = notes.notes.iter().filter(|n| n.holder() == Some(agent)).map(|n| n.id.clone()).collect();
    if held.is_empty() {
        return None;
    }
    let operation = crate::ops::find(op)?;
    // A counter, not the clock: two fences in the same second in one process shared a directory, and the first to finish deleted the other's.
    static FENCES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = FENCES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let scratch_dir = std::env::temp_dir().join(format!("grimoire-fence-{}-{}-{}", std::process::id(), now, n));
    std::fs::create_dir_all(&scratch_dir).ok()?;
    let mut trial_project = project.clone();
    trial_project.maps_dir = scratch_dir.clone();
    let mut trial_open = open.clone();
    trial_open.map_path = scratch_dir.join(bevy_grimoire_core::naming::map_file_name(&trial_open.map.name));
    let mut trial = Ctx {
        project: Some(trial_project),
        root: None,
        open: Some(trial_open),
        source: ctx.source,
        session: ctx.session.clone(),
        proposal: ctx.proposal.clone(),
    };
    let ran = operation.commit(&mut trial, params);
    let _ = std::fs::remove_dir_all(&scratch_dir);
    ran.ok()?;
    let change = measure(&open.map, &trial.open.as_ref()?.map, &agent_made_ids(ctx));
    if change == notes::Change::default() {
        return None;
    }
    let mut first = None;
    for id in &held {
        match notes.fence(id, agent, &change) {
            Ok(()) => return None,
            Err(why) => {
                first.get_or_insert(why);
            }
        }
    }
    first
}
