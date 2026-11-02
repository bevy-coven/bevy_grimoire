//! **Two fields of a library piece that only a person could set, as registered operations** (E-002,
//! #129 slices 17 and 18): `piece_set_note` and `piece_set_width`.
//!
//! The Meshes tab's description field and its SIZE field wrote the library with writes of their own
//! (`tiles::note_keys`, `tiles::scale_keys`), so an agent could label a piece's kind and look but not
//! describe it or resize it, and no log line named either change. Both fields now dispatch these, so a
//! person's edit and an agent's are one validated, saved and logged change with one undo.
//!
//! **Both go through [`crate::tiles::commit_measured`]**, the one door to `library.ron`, after asking
//! [`crate::tiles::check_measured`] so a preview refuses what the commit would. Each inverse puts back
//! the fields the change touched and, for the note, the label record its provenance lives in, and is
//! the writer's own undo only ([`Operation::only_as_undo`]).

use bevy_grimoire_core::descriptor::{Align, Axis, By, Descriptor, Extent, LabelOrigin};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Source, Undo};
use crate::project::Project;

fn by_of(source: Source) -> By {
    match source {
        Source::Human => By::Human,
        Source::Agent => By::Agent,
    }
}

/// The piece, on a copy of the measured library, or the refusal that names what to pick instead.
fn find<'a>(
    project: &Project,
    measured: &'a mut bevy_grimoire_core::library::Library,
    id: &str,
    nothing: &str,
) -> Result<&'a mut Descriptor, Refusal> {
    let Some(index) = measured.descriptors.iter().position(|d| d.id == id) else {
        if let Some(ns) = project.other_kit_of(id) {
            return Err(Refusal::new(format!(
                "`{id}` belongs to kit `{ns}`, not the kit being authored (`{}`), so {nothing}.",
                project.namespace
            )));
        }
        let ids: Vec<&str> = measured.descriptors.iter().map(|d| d.id.as_str()).collect();
        let hint = match crate::ops::nearest_ids(&ids, id) {
            near if near.is_empty() => String::new(),
            near => format!(" Close ids: {}.", near.join(", ")),
        };
        return Err(Refusal::new(format!(
            "no library piece is called `{id}`, so {nothing}.{hint} Pick one from the Meshes shelf."
        )));
    };
    Ok(&mut measured.descriptors[index])
}

fn no_project(what: &str) -> Refusal {
    Refusal::new(format!("no project is open, so there is no library to {what}."))
}

/// **The file first**, and the library adopted only when it wrote: a refused save leaves the piece and
/// the editor as they were.
fn write(project: &mut Project, measured: bevy_grimoire_core::library::Library) -> Result<(), Refusal> {
    crate::tiles::commit_measured(project, measured).map(|_| ()).map_err(|msg| {
        Refusal::new(format!("the file refused the change, so nothing was written and the piece is unchanged: {msg}"))
    })
}

// ── piece_set_note ───────────────────────────────────────────────────────────────────────────────

/// **What an undo puts back**: the description, and the label record its provenance lives in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteRestore {
    pub note: Option<String>,
    pub labels: Option<LabelOrigin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteParams {
    /// The library piece's id, as `grimoire/pieces` names it.
    pub id: String,
    /// The description. Blank clears it, the rule the Meshes field follows. Absent on an undo.
    #[serde(default)]
    pub note: Option<String>,
    /// **Undo only.** The description and label record to put back, exactly as they were.
    #[serde(default)]
    pub restore: Option<NoteRestore>,
}

enum NoteRequest {
    Set { id: String, note: String },
    Restore { id: String, restore: NoteRestore },
}

fn note_request(raw: &serde_json::Value) -> Result<NoteRequest, Refusal> {
    let p: NoteParams = serde_json::from_value(raw.clone()).map_err(|e| {
        Refusal::new(format!("this is not a description to set: {e}. The fields are `id` and `note` (blank clears it)."))
    })?;
    match (p.note, p.restore) {
        (Some(note), None) => Ok(NoteRequest::Set { id: p.id, note: note.trim().to_owned() }),
        (None, Some(restore)) => Ok(NoteRequest::Restore { id: p.id, restore }),
        (Some(_), Some(_)) => Err(Refusal::new("a restore names the whole record, so it takes `id` and `restore` and nothing else.")),
        (None, None) => Err(Refusal::new("describing a piece needs `id` and `note`. Nothing was set.")),
    }
}

struct NoteEdit {
    measured: bevy_grimoire_core::library::Library,
    before: NoteRestore,
    after: serde_json::Value,
    summary: String,
}

fn note_edit(project: &Project, source: Source, req: &NoteRequest) -> Result<NoteEdit, Refusal> {
    let id = match req {
        NoteRequest::Set { id, .. } | NoteRequest::Restore { id, .. } => id,
    };
    let mut measured = (*project.measured).clone();
    let d = find(project, &mut measured, id, "nothing was set")?;
    let before = NoteRestore { note: d.note.clone(), labels: d.labels.clone() };
    let summary = match req {
        NoteRequest::Set { note, .. } => {
            if before.note.as_deref().unwrap_or("") == note {
                return Err(Refusal::new(format!("`{id}` is already described that way, so nothing was set.")));
            }
            d.note = (!note.is_empty()).then(|| note.clone());
            crate::labels::stamp_by(d, Axis::Note, by_of(source));
            if note.is_empty() {
                format!("cleared the description of `{id}`")
            } else {
                format!("described `{id}`: {note}")
            }
        }
        NoteRequest::Restore { restore, .. } => {
            d.note = restore.note.clone();
            d.labels = restore.labels.clone();
            format!("restored the description of `{id}`")
        }
    };
    let after = json!({"id": d.id, "note": d.note});
    crate::tiles::check_measured(project, &measured)
        .map_err(|why| Refusal::new(format!("the project would not take this, so nothing was set: {why}")))?;
    Ok(NoteEdit { measured, before, after, summary })
}

/// **Describe one library piece**, or clear its description.
pub struct PieceSetNote;

impl Operation for PieceSetNote {
    fn name(&self) -> &'static str {
        "piece_set_note"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["library"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn writes_library(&self) -> bool {
        true
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "note"],
            "properties": {
                "id": {"type": "string", "description": "The library piece's id, as grimoire/pieces names it."},
                "note": {"type": "string", "description": "What the piece is, in a sentence. Blank clears it. The write is stamped as the writer's (a person or an agent) on the note axis."}
            }
        })
    }
    /// **An undo's put-back is never a request an agent may send.**
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        matches!(note_request(params), Ok(NoteRequest::Restore { .. }))
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let id = after.get("id")?.as_str()?;
        let d = ctx.project.as_ref()?.measured.descriptors.iter().find(|d| d.id == id)?;
        Some(json!({"id": d.id, "note": d.note}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let req = note_request(params)?;
        let Some(project) = ctx.project.as_ref() else { return Err(no_project("describe a piece in")) };
        Ok(note_edit(project, ctx.source, &req)?.after)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let req = note_request(params)?;
        let source = ctx.source;
        let id = match &req {
            NoteRequest::Set { id, .. } | NoteRequest::Restore { id, .. } => id.clone(),
        };
        let Some(project) = ctx.project.as_mut() else { return Err(no_project("describe a piece in")) };
        let e = note_edit(project, source, &req)?;
        write(project, e.measured)?;
        Ok(Outcome {
            summary: e.summary,
            affected: 1,
            saved: true,
            inverse: Some(json!({"id": id, "restore": e.before})),
            after: Some(e.after),
        })
    }
}

// ── piece_set_width ──────────────────────────────────────────────────────────────────────────────

/// **What an undo puts back**: the extent and alignment a resize rewrites, whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidthRestore {
    pub extent: Extent,
    pub align: Align,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidthParams {
    /// The library piece's id, as `grimoire/pieces` names it.
    pub id: String,
    /// How wide it should stand, in metres. Absent on an undo.
    #[serde(default)]
    pub width: Option<f32>,
    /// **Undo only.** The extent and alignment to put back, exactly as they were.
    #[serde(default)]
    pub restore: Option<WidthRestore>,
}

enum WidthRequest {
    Set { id: String, width: f32 },
    Restore { id: String, restore: WidthRestore },
}

fn width_request(raw: &serde_json::Value) -> Result<WidthRequest, Refusal> {
    let p: WidthParams = serde_json::from_value(raw.clone()).map_err(|e| {
        Refusal::new(format!("this is not a width to set: {e}. The fields are `id` and `width` (metres)."))
    })?;
    match (p.width, p.restore) {
        (Some(width), None) => Ok(WidthRequest::Set { id: p.id, width }),
        (None, Some(restore)) => Ok(WidthRequest::Restore { id: p.id, restore }),
        (Some(_), Some(_)) => Err(Refusal::new("a restore names the whole record, so it takes `id` and `restore` and nothing else.")),
        (None, None) => Err(Refusal::new("resizing a piece needs `id` and `width`. Nothing was set.")),
    }
}

struct WidthEdit {
    measured: bevy_grimoire_core::library::Library,
    before: WidthRestore,
    after: serde_json::Value,
    summary: String,
}

fn width_state(d: &Descriptor) -> serde_json::Value {
    json!({"id": d.id, "extent": d.extent, "align": d.align})
}

fn width_edit(project: &Project, req: &WidthRequest) -> Result<WidthEdit, Refusal> {
    let id = match req {
        WidthRequest::Set { id, .. } | WidthRequest::Restore { id, .. } => id,
    };
    let mut measured = (*project.measured).clone();
    let d = find(project, &mut measured, id, "nothing was resized")?;
    let before = WidthRestore { extent: d.extent, align: d.align };
    let summary = match req {
        WidthRequest::Set { width, .. } => {
            // Refused rather than clamped: a width of zero is a piece that reserves nothing and sits
            // inside a wall with every rule reporting success.
            let ratio = crate::tiles::bake_width(d, *width).map_err(|e| Refusal::new(format!("`{id}`: {e}, so nothing was resized.")))?;
            if ratio == 1.0 {
                return Err(Refusal::new(format!("`{id}` already stands {width:.2} m wide, so nothing was resized.")));
            }
            let (w, dep) = d.extent.footprint.unwrap_or((*width, 0.0));
            match d.align.scale {
                Some(s) => format!("resized `{id}` to {w:.2} x {dep:.2} m (mesh scaled {s:.3}x)"),
                None => format!("resized `{id}` to {w:.2} x {dep:.2} m, back at its authored size"),
            }
        }
        WidthRequest::Restore { restore, .. } => {
            d.extent = restore.extent;
            d.align = restore.align;
            format!("restored the size of `{id}`")
        }
    };
    let after = width_state(d);
    crate::tiles::check_measured(project, &measured)
        .map_err(|why| Refusal::new(format!("the project would not take this, so nothing was resized: {why}")))?;
    Ok(WidthEdit { measured, before, after, summary })
}

/// **Resize one library piece to stand a width**, uniformly, as the Meshes tab's SIZE field does.
pub struct PieceSetWidth;

impl Operation for PieceSetWidth {
    fn name(&self) -> &'static str {
        "piece_set_width"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["library"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn writes_library(&self) -> bool {
        true
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "width"],
            "properties": {
                "id": {"type": "string", "description": "The library piece's id, as grimoire/pieces names it."},
                "width": {"type": "number", "exclusiveMinimum": 0, "description": "How wide the piece should stand, in metres. Its depth, height, pivot and mesh scale follow in proportion. A piece with no measured footprint, and a width it already stands at, are refused."}
            }
        })
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        matches!(width_request(params), Ok(WidthRequest::Restore { .. }))
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let id = after.get("id")?.as_str()?;
        let d = ctx.project.as_ref()?.measured.descriptors.iter().find(|d| d.id == id)?;
        Some(width_state(d))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let req = width_request(params)?;
        let Some(project) = ctx.project.as_ref() else { return Err(no_project("resize a piece in")) };
        Ok(width_edit(project, &req)?.after)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let req = width_request(params)?;
        let id = match &req {
            WidthRequest::Set { id, .. } | WidthRequest::Restore { id, .. } => id.clone(),
        };
        let Some(project) = ctx.project.as_mut() else { return Err(no_project("resize a piece in")) };
        let e = width_edit(project, &req)?;
        write(project, e.measured)?;
        Ok(Outcome {
            summary: e.summary,
            affected: 1,
            saved: true,
            inverse: Some(json!({"id": id, "restore": e.before})),
            after: Some(e.after),
        })
    }
}
