//! **Reference images as a registered operation** (#124, E-075): `reference_set`.
//!
//! A reference is a project-relative image the person pointed at with "like this"
//! (`bevy_grimoire_core::references`, the `.references.ron` beside the `.map.ron`). This operation is the
//! only way the editor changes that file, so the person's line in the terrain field and an agent's
//! `apply` are one change with one log line and one undo. The agent reads the list in `grimoire_state`
//! (`references`) and names one back with its id, which `References::cite` words.
//!
//! Its inverse is the same operation sent a parameter only an undo may send (`take_back`, `put_back`).

use bevy_grimoire_core::references::{self, Reference, References};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// **A reference put back where it stood**, for the undo of a removal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub index: usize,
    pub reference: Reference,
    /// The id counter before the removal, so an undo leaves the file as it was.
    pub issued: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Add: a project-relative image path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caption: String,
    /// Remove the reference with this id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<String>,
    /// Undo only: take an added reference back off, and the counter back to `issued`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take_back: Option<TakeBack>,
    /// Undo only: put a removed reference back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeBack {
    pub id: String,
    pub issued: u32,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {"type": "string", "description": "Add a reference: the image's path inside the project (png, jpg, jpeg, webp or gif). The same path twice is refused."},
                "caption": {"type": "string", "description": "What the picture is for, in a line. Optional."},
                "remove": {"type": "string", "description": "Remove the reference with this id (r1, r2, ...). Give this or `path`."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a reference: {e}. The fields are `path` and `caption`, or `remove`, and nothing else."))
        })?;
        let named = [p.path.is_some(), p.remove.is_some(), p.take_back.is_some(), p.put_back.is_some()];
        if named.iter().filter(|b| **b).count() != 1 {
            return Err(Refusal::new("a reference change gives a `path` to add or an id to `remove`, and not both. Nothing was changed."));
        }
        Ok(p)
    }

    fn undo_only(&self) -> bool {
        self.take_back.is_some() || self.put_back.is_some()
    }
}

fn file_of(ctx: &Ctx) -> Result<std::path::PathBuf, Refusal> {
    let Some(open) = ctx.open.as_ref() else {
        return Err(Refusal::new("no map is open, so there is no map to keep a reference beside."));
    };
    Ok(references::path_for_map(&open.map_path))
}

/// **The references of the open map**, which is the state an undo checks.
fn state_of_open(ctx: &Ctx) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    let refs = References::load(&references::path_for_map(&open.map_path)).ok()?;
    Some(json!({"map": open.map.name, "references": serde_json::to_value(&refs).ok()?}))
}

/// **What an agent reads in `grimoire_state`**: each reference, and whether its file is still there.
pub fn state_json(open: &crate::project::OpenMap, root: Option<&std::path::Path>) -> serde_json::Value {
    let refs = match References::load(&references::path_for_map(&open.map_path)) {
        Ok(r) => r,
        Err(e) => return json!({"problem": e}),
    };
    let missing: Vec<String> = root.map(|r| refs.missing(r).into_iter().map(|(id, _)| id).collect()).unwrap_or_default();
    json!({
        "references": refs.refs.iter().map(|r| json!({
            "id": r.id, "path": r.path, "caption": r.caption, "missing": missing.contains(&r.id),
        })).collect::<Vec<_>>(),
        "cite_as": "name a reference by its id in a note's summary or assumptions, e.g. `reference r1 (path): caption`",
    })
}

fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
    let p = Params::parse(params)?;
    let path = file_of(ctx)?;
    let mut refs = References::load(&path).map_err(Refusal::new)?;
    let (summary, inverse) = if let Some(tb) = &p.take_back {
        let index = refs.refs.iter().position(|r| r.id == tb.id).unwrap_or(0);
        let issued = refs.issued;
        let reference = refs.remove(&tb.id).map_err(Refusal::new)?;
        refs.issued = tb.issued;
        (format!("took back reference {}", tb.id), json!({"put_back": PutBack { index, reference, issued }}))
    } else if let Some(pb) = &p.put_back {
        if refs.get(&pb.reference.id).is_some() {
            return Err(Refusal::new(format!("reference {} is already there, so it was not put back.", pb.reference.id)));
        }
        let id = pb.reference.id.clone();
        let before = refs.issued;
        refs.refs.insert(pb.index.min(refs.refs.len()), pb.reference.clone());
        refs.issued = refs.issued.max(pb.issued);
        (format!("put back reference {id}"), json!({"take_back": TakeBack { id, issued: before }}))
    } else if let Some(id) = &p.remove {
        let index = refs.refs.iter().position(|r| &r.id == id).unwrap_or(0);
        let issued = refs.issued;
        let reference = refs.remove(id).map_err(Refusal::new)?;
        (format!("removed reference {id} ({})", reference.path), json!({"put_back": PutBack { index, reference, issued }}))
    } else {
        let issued = refs.issued;
        let id = refs.add(p.path.as_deref().unwrap_or_default(), &p.caption).map_err(Refusal::new)?;
        let at = refs.get(&id).map(|r| r.path.clone()).unwrap_or_default();
        (format!("added reference {id} ({at})"), json!({"take_back": TakeBack { id, issued }}))
    };
    if !write {
        return Ok(None);
    }
    refs.save(&path).map_err(|e| Refusal::new(format!("the references file refused the change, so nothing was written: {e}")))?;
    Ok(Some(Outcome { summary, affected: 1, saved: true, inverse: Some(inverse), after: state_of_open(ctx) }))
}

/// **Add a reference image to the open map, or remove one.**
pub struct ReferenceSet;

impl Operation for ReferenceSet {
    fn name(&self) -> &'static str {
        "reference_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["references"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).is_ok_and(|p| p.undo_only())
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), ..Ctx::default() };
        apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}
