//! **An agent defines a composition from placements on the map, as one registered operation**
//! (E-072): `composition_define`.
//!
//! The Map tab's `Shift+B` box and `keep_as_group` already turn a set of pieces into a reusable
//! group; this is the same conversion with the set named by placement ids instead of drawn. It goes
//! through [`crate::editor::set_of_placements`], [`crate::editor::composition_from_set`] and
//! [`crate::project::Project::commit_composition`], so the group an agent defines is the group a
//! person would have kept, with the same refusals (unmeasured piece, support left outside, hung
//! from the ceiling) and the same replace rule: an existing id is redefined only when `replace` is
//! true, and never over a stamp override it would strand.
//!
//! Undo puts `compositions.ron` back: the previous definition, or removal when the id was new.

use serde::{Deserialize, Serialize};
use serde_json::json;

use bevy_grimoire_core::composition::Composition;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The group's name; forced to snake_case, as the Compose tab forces it.
    pub id: String,
    /// Ids of the open map's placements that become its members.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<String>,
    /// The members as a set in hand, instead of placements on the map: what a person's `M` sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<crate::editor::CloneSet>,
    /// Redefine a composition that already has this id. Every stamp of it then expands to the new body.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replace: bool,
    /// Undo only: put this definition back whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Composition>,
    /// Undo only: take the composition this operation made out again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remove: bool,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id"],
            "properties": {
                "id": {"type": "string", "description": "The group's name, snake_case. It is the id stamps refer to."},
                "from": {"type": "array", "items": {"type": "string"}, "minItems": 1, "description": "Ids of placements on the open map that become the group's members. A piece resting on another in the list stays on it; one resting on something outside the list is refused. Name `from` or `set`, not both."},
                "set": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["pieces"],
                    "description": "The members themselves, for a group whose pieces are not (or are no longer) placements on the open map: the set a person holds after a box capture. Offsets are metres from the group's centre. Name `set` or `from`, not both.",
                    "properties": {
                        "pieces": {"type": "array", "minItems": 1, "description": "Each is {descriptor, offset: [x, z], yaw?, tip?: [u8, u8], lift?, note?, owned?, owned_because?, patch?, on?}. `on` is \"layer\" (default), \"outside\" (refused: a group cannot carry a piece whose support stayed behind) or {\"in_set\": index into this list}."},
                        "stamps": {"type": "array", "description": "Stamps carried whole, nested by reference: {of: composition id, offset: [x, z], yaw?, overrides?, owned?, owned_because?, note?}."},
                        "centre_off": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "Where the set's footprint centre is, from the origin the offsets use. Default [0, 0]: the offsets are already from the centre."},
                        "half": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "Half the footprint's width and depth in metres, [x, z]. It becomes the group's bounded envelope, so it must enclose the pieces; a zero is refused (a bounded group has to enclose something)."}
                    }
                },
                "replace": {"type": "boolean", "description": "Redefine a group that already has this id. Without it an existing id is refused and nothing is written."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("this is not a composition definition: {e}. The fields are `id`, `from` (or `set`) and `replace`."))
        })
    }

    fn undo_only(&self) -> bool {
        self.restore.is_some() || self.remove
    }
}

/// **Make a composition from the open map's placements.**
pub struct CompositionDefine;

fn state_json(ctx: &Ctx, id: &str) -> Option<serde_json::Value> {
    let project = ctx.project.as_ref()?;
    let def = project.compositions.compositions.iter().find(|c| c.id == id);
    Some(json!({"id": id, "def": def}))
}

impl CompositionDefine {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = Params::parse(params)?;
        let id = bevy_grimoire_core::naming::to_snake_case(&p.id);
        let (Some(project), Some(open)) = (ctx.project.as_mut(), ctx.open.as_ref()) else {
            return Err(Refusal::new("no map is open, so there are no placements to make a group from."));
        };
        let prior = project.compositions.compositions.iter().find(|c| c.id == id).cloned();

        let (summary, members) = if p.undo_only() {
            if !p.from.is_empty() || p.set.is_some() || p.restore.is_some() && p.remove {
                return Err(Refusal::new("an undo either restores a definition or removes one, and names no placements."));
            }
            if let Some(c) = &p.restore {
                if c.id != id {
                    return Err(Refusal::new(format!("the definition to restore is `{}`, not `{id}`.", c.id)));
                }
                if write {
                    project.commit_composition(c.clone()).map_err(Refusal::new)?;
                }
                (format!("put group {id} back as it was"), c.members.len())
            } else {
                if write {
                    project.remove_composition(&id).map_err(Refusal::new)?;
                }
                (format!("took group {id} back out"), 0)
            }
        } else {
            let set = match (&p.set, p.from.is_empty()) {
                (Some(_), false) => {
                    return Err(Refusal::new("name the members as `from` (placement ids) or as `set`, not both."));
                }
                (Some(set), true) => set.clone(),
                (None, _) => crate::editor::set_of_placements(project, &open.map, &p.from).map_err(Refusal::new)?,
            };
            if prior.is_some() && !p.replace {
                return Err(Refusal::new(format!(
                    "a group named `{id}` already exists. Pass `replace: true` to redefine it, or pick another name. Nothing was written."
                )));
            }
            if write {
                crate::editor::keep_as_group(project, open, &set, &id, true).map_err(Refusal::new)?;
            } else {
                let comp = crate::editor::composition_from_set(&set, &id, &project.library, &project.compositions)
                    .map_err(Refusal::new)?;
                if prior.is_some() {
                    crate::editor::strand_check(open, &comp).map_err(Refusal::new)?;
                }
                return Ok(Some(Outcome {
                    summary: format!("would define group {id} from {} piece(s)", comp.members.len()),
                    affected: comp.members.len(),
                    saved: false,
                    inverse: None,
                    after: None,
                }));
            }
            let n = set.pieces.len() + set.stamps.len();
            (format!("defined group {id} from {n} piece(s)"), n)
        };

        if !write {
            return Ok(None);
        }
        let inverse = match prior {
            Some(c) => json!({"id": id, "restore": c}),
            None => json!({"id": id, "remove": true}),
        };
        let after = state_json(ctx, &id);
        Ok(Some(Outcome { summary, affected: members, saved: true, inverse: Some(inverse), after }))
    }
}

impl Operation for CompositionDefine {
    fn name(&self) -> &'static str {
        "composition_define"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["compositions"]
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
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        state_json(ctx, after.get("id")?.as_str()?)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { project: ctx.project.clone(), open: ctx.open.clone(), ..Ctx::default() };
        Self::go(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::go(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

/// **Change one composition's envelope** (E-072 slice 3): `composition_set_envelope`.
///
/// `anchored` claims no tile; `bounded` fills a tile-shaped box. Without `size`, a bounded
/// envelope is read off the members the way the Tiles tab fits one ([`crate::build::fit_envelope`],
/// at the project's cell height). The write goes through [`crate::project::Project::commit_composition`],
/// so a change that strands a parent which nests this group, or a member outside the box, is refused
/// with nothing written. Undo calls the operation again with the previous envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeParams {
    pub id: String,
    /// `anchored` or `bounded`.
    pub envelope: String,
    /// Bounded only: `[width, height, depth]` in metres. Omitted: fitted to the members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 3]>,
}

pub struct CompositionSetEnvelope;

impl CompositionSetEnvelope {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p: EnvelopeParams = serde_json::from_value(params.clone()).map_err(|e| {
            Refusal::new(format!("this is not an envelope change: {e}. The fields are `id`, `envelope` and `size`."))
        })?;
        let id = bevy_grimoire_core::naming::to_snake_case(&p.id);
        let Some(project) = ctx.project.as_mut() else {
            return Err(Refusal::new("no project is open."));
        };
        let Some(mut comp) = project.compositions.compositions.iter().find(|c| c.id == id).cloned() else {
            return Err(Refusal::new(format!("the project has no composition `{id}`.")));
        };
        let prior = comp.envelope;
        comp.envelope = match p.envelope.as_str() {
            "anchored" => {
                if p.size.is_some() {
                    return Err(Refusal::new("an anchored envelope has no size."));
                }
                bevy_grimoire_core::composition::Envelope::Anchored
            }
            "bounded" => {
                let size = match p.size {
                    Some([w, h, d]) => (w, h, d),
                    None => crate::build::fit_envelope(&comp.members, &project.library, project.lattice.cell_height),
                };
                if ![size.0, size.1, size.2].iter().all(|v| v.is_finite() && *v > 0.0) {
                    return Err(Refusal::new("a bounded envelope's size must be three positive numbers."));
                }
                bevy_grimoire_core::composition::Envelope::Bounded { size }
            }
            other => return Err(Refusal::new(format!("`{other}` is not an envelope: use `anchored` or `bounded`."))),
        };
        let summary = format!("group {id} is now {}", match comp.envelope {
            bevy_grimoire_core::composition::Envelope::Anchored => "anchored".to_owned(),
            bevy_grimoire_core::composition::Envelope::Bounded { size } =>
                format!("bounded {:.2}x{:.2}x{:.2}", size.0, size.1, size.2),
        });
        if !write {
            let mut trial = project.clone();
            trial.compositions.compositions.iter_mut().for_each(|c| if c.id == id { *c = comp.clone() });
            bevy_grimoire_core::composition::validate(&trial.compositions.compositions, &trial.library).map_err(Refusal::new)?;
            return Ok(Outcome { summary: format!("would make {summary}"), affected: 1, saved: false, inverse: None, after: None });
        }
        project.commit_composition(comp).map_err(Refusal::new)?;
        let inverse = match prior {
            bevy_grimoire_core::composition::Envelope::Anchored => json!({"id": id, "envelope": "anchored"}),
            bevy_grimoire_core::composition::Envelope::Bounded { size } =>
                json!({"id": id, "envelope": "bounded", "size": [size.0, size.1, size.2]}),
        };
        let after = state_json(ctx, &id);
        Ok(Outcome { summary, affected: 1, saved: true, inverse: Some(inverse), after })
    }
}

impl Operation for CompositionSetEnvelope {
    fn name(&self) -> &'static str {
        "composition_set_envelope"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["compositions"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "envelope"],
            "properties": {
                "id": {"type": "string", "description": "The group to change, by id."},
                "envelope": {"type": "string", "enum": ["anchored", "bounded"], "description": "`anchored` claims no tile; `bounded` fills a tile-shaped box (and is the only kind a solver can place)."},
                "size": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Bounded only: [width, height, depth] in metres. Omitted, it is fitted to the members."}
            }
        })
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        state_json(ctx, after.get("id")?.as_str()?)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { project: ctx.project.clone(), open: ctx.open.clone(), ..Ctx::default() };
        Self::go(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::go(ctx, params, true)
    }
}

/// **Rename one composition, and every map that stamps it** (E-002, #129 slice 2):
/// `composition_rename`.
///
/// The COMPOSE tab's `I` field dispatches it through [`crate::editor::rename_composition`], and an
/// agent calls the same name. The name is forced the way the field forces it
/// ([`bevy_grimoire_core::naming::to_id`], which keeps a `/` namespace). The write is
/// [`crate::project::Project::rename_composition`]: the compositions file and every map naming the
/// old id, all or none. Undo renames it back the same way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameParams {
    /// The composition's current id.
    pub composition: String,
    /// Its new name.
    pub name: String,
}

pub struct CompositionRename;

impl CompositionRename {
    /// **The old id and the new one, or the refusal**, checked against the project before a byte is
    /// written: the composition exists, the new name is not empty, not the same, and not taken.
    fn checked(ctx: &Ctx, params: &serde_json::Value) -> Result<(String, String), Refusal> {
        let p: RenameParams = serde_json::from_value(params.clone()).map_err(|e| {
            Refusal::new(format!(
                "this is not a composition rename: {e}. The fields are `composition` and `name`."
            ))
        })?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there is no composition to rename."));
        };
        let comps = &project.compositions.compositions;
        if !comps.iter().any(|c| c.id == p.composition) {
            let known: Vec<&str> = comps.iter().map(|c| c.id.as_str()).collect();
            return Err(Refusal::new(if known.is_empty() {
                format!("`{}` is not a composition, and the project has none, so nothing was renamed.", p.composition)
            } else {
                format!(
                    "`{}` is not a composition, so nothing was renamed. The project has: {}.",
                    p.composition,
                    known.join(", ")
                )
            }));
        }
        let name = bevy_grimoire_core::naming::to_id(&p.name);
        if name.is_empty() {
            return Err(Refusal::new("a name cannot be empty; nothing was changed."));
        }
        if name == p.composition {
            return Err(Refusal::new(format!("the composition is already `{name}`, so nothing changes.")));
        }
        if comps.iter().any(|c| c.id == name) {
            return Err(Refusal::new(format!(
                "`{name}` is already a composition, so `{}` cannot take that name; nothing was changed.",
                p.composition
            )));
        }
        Ok((p.composition, name))
    }
}

impl Operation for CompositionRename {
    fn name(&self) -> &'static str {
        "composition_rename"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["compositions", "map"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["composition", "name"],
            "properties": {
                "composition": {"type": "string", "description": "The id of the composition to rename."},
                "name": {"type": "string", "description": "Its new name, forced to snake_case per `/` segment (`Town/Big Hall` becomes `town/big_hall`). It must not be empty or another composition's. Every map that stamps the composition follows."}
            }
        })
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        state_json(ctx, after.get("id")?.as_str()?)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (from, to) = Self::checked(ctx, params)?;
        Ok(json!({"composition": from, "name": to}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (from, to) = Self::checked(ctx, params)?;
        let Some(project) = ctx.project.as_mut() else {
            return Err(Refusal::new("no project is open, so there is no composition to rename."));
        };
        project
            .rename_composition(&from, &to, ctx.open.as_mut())
            .map_err(Refusal::new)?;
        let after = state_json(ctx, &to);
        Ok(Outcome {
            summary: format!("`{from}` is now `{to}`, and every map that stamped it follows"),
            affected: 1,
            saved: true,
            inverse: Some(json!({"composition": to, "name": from})),
            after,
        })
    }
}
