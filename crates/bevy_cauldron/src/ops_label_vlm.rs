//! **VLM labelling as two registered operations** (E-043): `label_suggest` and `label_apply`.
//!
//! The `L` key photographs a piece in the live booth and applies the model's answer on arrival
//! (`tiles::apply_suggestion`). An agent has no booth, so it brings its own two shots (project-relative
//! PNGs, e.g. from `bevy_devshot`) and the two halves of that walk become two operations:
//!
//! - `label_suggest` asks the configured endpoint (`GRIMOIRE_VLM_*`, as the editor does) about one library
//!   piece and **returns the validated proposal without writing anything**, so an agent reads it, edits
//!   it, or throws it away. Undo: none, it only reads.
//! - `label_apply` writes a proposal onto the piece through `labels::apply_fields`, the same field copy
//!   the editor's apply uses, so the provenance says what the model said (`By::Model`) and the pane draws it
//!   as unchecked until a person confirms it. Its inverse restores the whole descriptor (`restore`, which
//!   only an undo may send), because a proposal also moves mount, note, rooms and front, which
//!   `label_set`'s record does not hold.
//!
//! A proposal that wants the mesh turned (`needs_turn`) is refused by `label_apply`: righting rewrites the
//! mesh and re-photographs it, which is the booth's job. Turn it, take new shots, ask again.

use bevy_grimoire_core::descriptor::Descriptor;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};
use crate::project::Project;
use crate::vlm::{Provenance, Suggestion, VlmConfig};

fn piece<'a>(project: &'a Project, id: &str) -> Result<&'a Descriptor, Refusal> {
    project
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| {
            Refusal::new(format!(
                "no library piece is called `{id}`, so nothing was done. Pick one from `grimoire/pieces`."
            ))
        })
}

fn open_project(ctx: &Ctx) -> Result<&Project, Refusal> {
    ctx.project
        .as_ref()
        .ok_or_else(|| Refusal::new("no project is open, so there is no library to label."))
}

// ── label_suggest ────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestParams {
    /// The library piece's id, as `grimoire/pieces` names it.
    pub id: String,
    /// Project-relative path of the first shot (a PNG).
    pub front: String,
    /// Project-relative path of the second shot (a PNG), from another angle.
    pub side: String,
}

fn suggest_params(raw: &serde_json::Value) -> Result<SuggestParams, Refusal> {
    serde_json::from_value(raw.clone()).map_err(|e| {
        Refusal::new(format!(
            "this is not a label request: {e}. The fields are `id`, `front` and `side` (two PNG paths)."
        ))
    })
}

/// **A shot's bytes, from inside the project only.** An absolute path or a `..` would let an agent
/// send any file on the machine to the endpoint.
fn shot(project: &Project, rel: &str) -> Result<Vec<u8>, Refusal> {
    let p = std::path::Path::new(rel);
    let inside = p.is_relative()
        && p.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !inside {
        return Err(Refusal::new(format!(
            "`{rel}` is not a project-relative path, so it was not sent. Name a PNG under the project root."
        )));
    }
    let bytes = std::fs::read(project.root.join(p))
        .map_err(|e| Refusal::new(format!("the shot `{rel}` cannot be read: {e}.")))?;
    if !bytes.starts_with(b"\x89PNG") {
        return Err(Refusal::new(format!("`{rel}` is not a PNG, so it was not sent.")));
    }
    Ok(bytes)
}

/// **Ask the model about one piece's two shots.**
pub struct LabelSuggest;

impl Operation for LabelSuggest {
    fn name(&self) -> &'static str {
        "label_suggest"
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
            "required": ["id", "front", "side"],
            "properties": {
                "id": {"type": "string", "description": "The library piece's id, as grimoire/pieces names it."},
                "front": {"type": "string", "description": "Project-relative path of a PNG shot of the piece."},
                "side": {"type": "string", "description": "Project-relative path of a PNG shot of the piece from another angle."}
            }
        })
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = suggest_params(params)?;
        let project = open_project(ctx)?;
        piece(project, &p.id)?;
        shot(project, &p.front)?;
        shot(project, &p.side)?;
        VlmConfig::load(&project.root).map_err(Refusal::new)?;
        Ok(json!({"id": p.id, "asks": "the configured VLM endpoint", "writes": false}))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = suggest_params(params)?;
        let project = open_project(ctx)?;
        let d = piece(project, &p.id)?;
        let pngs = [shot(project, &p.front)?, shot(project, &p.side)?];
        let cfg = VlmConfig::load(&project.root).map_err(Refusal::new)?;
        let mesh = d.mesh.clone().unwrap_or_default();
        let prompt = crate::labels::prompt_ctx(d, &mesh, project);
        let date: String = crate::ops::now_rfc3339().chars().take(10).collect();
        let (suggestion, provenance) =
            crate::vlm::label_with_retry(&cfg, &pngs, &project.vocab, &prompt, date)
                .map_err(|e| Refusal::new(e.to_string()))?;
        Ok(Outcome {
            summary: format!(
                "`{}` proposed by {}: {}",
                p.id, provenance.model, suggestion.what
            ),
            affected: 0,
            saved: false,
            inverse: None,
            after: Some(json!({
                "id": p.id,
                "suggestion": suggestion,
                "provenance": provenance,
            })),
        })
    }
}

// ── label_apply ──────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    /// The library piece's id.
    pub id: String,
    /// A proposal as `label_suggest` returned it, possibly edited.
    #[serde(default)]
    pub suggestion: Option<Suggestion>,
    /// Who said it: `label_suggest`'s `provenance`, unchanged.
    #[serde(default)]
    pub provenance: Option<Provenance>,
    /// **Undo only**: the whole descriptor to put back.
    #[serde(default)]
    pub restore: Option<Box<Descriptor>>,
}

enum ApplyRequest {
    Apply {
        id: String,
        suggestion: Box<Suggestion>,
        provenance: Provenance,
    },
    Restore {
        id: String,
        descriptor: Box<Descriptor>,
    },
}

fn apply_request(raw: &serde_json::Value) -> Result<ApplyRequest, Refusal> {
    let p: ApplyParams = serde_json::from_value(raw.clone()).map_err(|e| {
        Refusal::new(format!(
            "this is not a proposal to apply: {e}. The fields are `id`, `suggestion` and `provenance`, as `label_suggest` returns them."
        ))
    })?;
    match (p.suggestion, p.provenance, p.restore) {
        (Some(suggestion), Some(provenance), None) => Ok(ApplyRequest::Apply {
            id: p.id,
            suggestion: Box::new(suggestion),
            provenance,
        }),
        (None, None, Some(descriptor)) => Ok(ApplyRequest::Restore {
            id: p.id,
            descriptor,
        }),
        _ => Err(Refusal::new(
            "applying labels needs `id`, `suggestion` and `provenance` together, and nothing else. Nothing was written.",
        )),
    }
}

/// **The edit, on a copy of the measured library**; the commit writes it.
fn apply_edit(
    project: &Project,
    req: &ApplyRequest,
) -> Result<(bevy_grimoire_core::library::Library, Descriptor, String), Refusal> {
    let id = match req {
        ApplyRequest::Apply { id, .. } | ApplyRequest::Restore { id, .. } => id,
    };
    piece(project, id)?;
    let mut measured = (*project.measured).clone();
    let index = measured
        .descriptors
        .iter()
        .position(|d| &d.id == id)
        .ok_or_else(|| Refusal::new(format!("no library piece is called `{id}`.")))?;
    let before = measured.descriptors[index].clone();
    let summary = match req {
        ApplyRequest::Apply {
            suggestion,
            provenance,
            ..
        } => {
            if let Some(turn) = &suggestion.needs_turn {
                return Err(Refusal::new(format!(
                    "the proposal says `{id}` needs turning ({} quarter turn(s) about {}), and the labels were judged from the wrong orientation. Turn the mesh, take new shots and ask again. Nothing was written.",
                    turn.turns,
                    turn.axis.to_uppercase()
                )));
            }
            crate::labels::apply_fields(
                &mut measured.descriptors[index],
                suggestion,
                provenance,
                &project.vocab,
            );
            format!("applied {}'s labels to `{id}`", provenance.model)
        }
        ApplyRequest::Restore { descriptor, .. } => {
            measured.descriptors[index] = (**descriptor).clone();
            format!("restored `{id}`")
        }
    };
    Ok((measured, before, summary))
}

/// **Write a proposal onto one library piece.**
pub struct LabelApply;

impl Operation for LabelApply {
    fn name(&self) -> &'static str {
        "label_apply"
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

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        matches!(apply_request(params), Ok(ApplyRequest::Restore { .. }))
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "suggestion", "provenance"],
            "properties": {
                "id": {"type": "string", "description": "The library piece's id, as grimoire/pieces names it."},
                "suggestion": {"type": "object", "description": "A proposal as label_suggest returned it (`after.suggestion`), possibly edited. Every token must be in the vocabulary; a proposal with `needs_turn` is refused."},
                "provenance": {"type": "object", "description": "label_suggest's `after.provenance`, unchanged: which model answered and when."}
            }
        })
    }

    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let id = after.get("id")?.as_str()?;
        let d = ctx
            .project
            .as_ref()?
            .measured
            .descriptors
            .iter()
            .find(|d| d.id == id)?;
        serde_json::to_value(d).ok()
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let req = apply_request(params)?;
        let (_, _, summary) = apply_edit(open_project(ctx)?, &req)?;
        Ok(json!({"would": summary}))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let req = apply_request(params)?;
        let id = match &req {
            ApplyRequest::Apply { id, .. } | ApplyRequest::Restore { id, .. } => id.clone(),
        };
        let project = ctx
            .project
            .as_mut()
            .ok_or_else(|| Refusal::new("no project is open, so there is no library to label."))?;
        let (measured, before, summary) = apply_edit(project, &req)?;
        if let Err(msg) = crate::tiles::commit_measured(project, measured) {
            return Err(Refusal::new(format!(
                "the file refused the change, so nothing was written and the piece is unchanged: {msg}"
            )));
        }
        let after = piece(project, &id).ok().and_then(|d| serde_json::to_value(d).ok());
        Ok(Outcome {
            summary,
            affected: 1,
            saved: true,
            inverse: Some(json!({"id": id, "restore": before})),
            after,
        })
    }
}
