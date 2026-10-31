//! **An agent stamps a composition onto the open map, as one registered operation** (E-072):
//! `stamp_add`, the counterpart of `stamp_remove`.
//!
//! The stamp click (`stamp_here`) and this operation share [`plan_stamp`]: the same id rule
//! (`<short>@<n>`, past every minted id the map carries), the same trial expansion before anything is
//! kept, and so the same refusals (an unknown composition, a member that cannot resolve, more members
//! than the cap).
//!
//! Undo is the operation's own undo-only forms: `remove` takes the stamp this made out by id, and
//! `restore` puts a removed stamp back at the index it held, so a redo of the add lands where it was.

use serde::{Deserialize, Serialize};
use serde_json::json;

use bevy_grimoire_core::composition::Stamped;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};
use crate::project::Project;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The composition to stamp, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
    /// Where the anchor goes, floor plan metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f32; 2]>,
    /// Turn about the anchor, degrees, as every yaw on the map and in every other operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaw: Option<f32>,
    /// Name the stamp. Minted as `<composition>@<n>` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Undo only: take this stamp off by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<String>,
    /// Undo only: put a removed stamp back at its index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Restore>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    pub index: usize,
    pub stamp: Stamped,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["of", "at"],
            "properties": {
                "of": {"type": "string", "description": "The composition to stamp, by id (see composition_define)."},
                "at": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "Where the anchor goes, [x, z] in metres."},
                "yaw": {"type": "number", "description": "Turn about the anchor, in degrees (90 is a quarter turn). Default 0."},
                "id": {"type": "string", "description": "A name for the stamp, unique on the map. Minted as `<composition>@<n>` when absent."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("this is not a stamp: {e}. The fields are `of`, `at`, `yaw` and `id`."))
        })?;
        let undo = p.remove.is_some() || p.restore.is_some();
        if undo && (p.of.is_some() || p.at.is_some() || p.yaw.is_some() || p.id.is_some()) {
            return Err(Refusal::new("an undo removes or restores a stamp and names nothing else."));
        }
        if p.remove.is_some() && p.restore.is_some() {
            return Err(Refusal::new("an undo either removes a stamp or restores one, not both."));
        }
        Ok(p)
    }

    fn undo_only(&self) -> bool {
        self.remove.is_some() || self.restore.is_some()
    }
}

/// **The stamp a click or a request would make, tried before it is kept.** `n` is the counter value
/// the minted id uses; an explicit `id` ignores it. Pure: the map is only cloned.
pub fn plan_stamp(
    project: &Project,
    map: &bevy_grimoire_core::map::Map,
    of: &str,
    at: (f32, f32),
    yaw: f32,
    id: Option<&str>,
    n: u32,
) -> Result<Stamped, String> {
    let comps = &project.compositions.compositions;
    if !comps.iter().any(|c| c.id == of) {
        return Err(format!("the project defines no composition called `{of}`, so there is nothing to stamp."));
    }
    let id = match id {
        Some(id) => id.to_owned(),
        None => format!("{}@{n}", crate::editor::short_id(of)),
    };
    if map.stamps.iter().any(|s| s.id == id) || map.placements.iter().any(|p| p.id == id) {
        return Err(format!("`{id}` is already on this map, and a stamp's id is shared with no other row."));
    }
    let stamped = Stamped { id, of: of.to_owned(), at, yaw, ..Default::default() };
    let mut trial = map.clone();
    trial.stamps.push(stamped.clone());
    bevy_grimoire_core::composition::expand(&trial, &trial.stamps, comps, &project.library)
        .map_err(|e| format!("cannot stamp `{of}` here: {e}"))?;
    Ok(stamped)
}

fn state_json(ctx: &Ctx, ids: &[String]) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    let on_map = ids.iter().filter(|id| open.map.stamps.iter().any(|s| &s.id == *id)).count();
    Some(json!({"map": open.map.name, "ids": ids, "on_map": on_map}))
}

/// **Put one composition on the open map.**
pub struct StampAdd;

impl StampAdd {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_mut()) else {
            return Err(Refusal::new("no map is open, so there is nowhere to stamp."));
        };
        let (summary, index, stamp, inverse) = if let Some(id) = &p.remove {
            let Some(index) = open.map.stamps.iter().position(|s| &s.id == id) else {
                return Err(Refusal::new(format!("no stamp is called `{id}` on this map, so nothing was removed.")));
            };
            let stamp = open.map.stamps[index].clone();
            (format!("took stamp {id} back off"), Some(index), stamp.clone(), json!({"restore": Restore { index, stamp }}))
        } else if let Some(r) = &p.restore {
            if (r.index > 0 && open.map.stamps.get(r.index - 1).is_none()) || open.map.stamps.iter().any(|s| s.id == r.stamp.id) {
                return Err(Refusal::new(format!(
                    "stamp `{}` cannot go back at index {}: it is already there, or the index is past the end.",
                    r.stamp.id, r.index
                )));
            }
            (format!("put stamp {} back", r.stamp.id), Some(r.index), r.stamp.clone(), json!({"remove": r.stamp.id}))
        } else {
            let (Some(of), Some(at)) = (p.of.as_deref(), p.at) else {
                return Err(Refusal::new("a stamp needs `of` (a composition id) and `at` ([x, z])."));
            };
            let n = crate::editor::next_id_after(&open.map) + 1;
            let stamp = plan_stamp(project, &open.map, of, (at[0], at[1]), p.yaw.unwrap_or(0.0), p.id.as_deref(), n)
                .map_err(Refusal::new)?;
            (format!("stamped {of} as {}", stamp.id), None, stamp.clone(), json!({"remove": stamp.id}))
        };
        let ids = vec![stamp.id.clone()];
        if write {
            // `None` is an add, which goes on the end; `Some` is where an undo takes from or puts back.
            let at = match (p.remove.is_some(), index) {
                (true, Some(i)) => {
                    open.map.stamps.remove(i);
                    i
                }
                (false, Some(i)) => {
                    open.map.stamps.insert(i, stamp.clone());
                    i
                }
                (_, None) => {
                    open.map.stamps.push(stamp.clone());
                    open.map.stamps.iter().position(|s| s.id == stamp.id).unwrap_or_default()
                }
            };
            if let Err(e) = open.save(project) {
                if p.remove.is_some() {
                    open.map.stamps.insert(at, stamp);
                } else {
                    open.map.stamps.remove(at);
                }
                return Err(Refusal::new(format!("the file refused the change, so nothing was written: {e}")));
            }
        }
        let after = state_json(ctx, &ids);
        // A dry run has not changed the map, so its `on_map` is the before-state; the plan only needs
        // the verdict.
        Ok(Outcome { summary, affected: 1, saved: write, inverse: write.then_some(inverse), after })
    }
}

impl Operation for StampAdd {
    fn name(&self) -> &'static str {
        "stamp_add"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
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
        let ids: Vec<String> =
            after.get("ids")?.as_array()?.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
        state_json(ctx, &ids)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { project: ctx.project.clone(), open: ctx.open.clone(), ..Ctx::default() };
        let out = Self::go(&mut scratch, params, false)?;
        Ok(json!({"would": out.summary}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::go(ctx, params, true)
    }
}
