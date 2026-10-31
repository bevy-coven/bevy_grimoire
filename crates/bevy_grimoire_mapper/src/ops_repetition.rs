//! **Turn the copies a player would see as stamped, as one registered operation** (#143):
//! `repetition_vary`.
//!
//! `grimoire_state.repetition` and the REPEATS line name the visible repeats of the open map (#136,
//! #139); this ends the clutter ones a turn can end. `bevy_grimoire_core::repetition::vary` turns each
//! clutter copy that looks like an earlier one in range, under the project's `repetition.ron`, and
//! the turned rows are written as a [`RowDelta`], so one undo puts every one back. Structure is never
//! turned, and neither is a round piece (#184), which looks the same at any yaw; a long piece (a
//! bench, a cart) takes only the half turn, the one that keeps it on its cells (#186). A
//! preview is the same plan with nothing written.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::repetition;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Indexed, Operation, Outcome, Refusal, RowDelta, Undo};
use crate::project::{OpenMap, Project};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// **The undo's own form**: the rows to put back. Only an undo sets it.
    #[serde(default)]
    pub delta: Option<RowDelta>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "delta": {
                    "description": "The rows an undo puts back. Only an undo may set this: to turn the repeats, call with no parameters.",
                    "type": "object"
                }
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "these are not the parameters for repetition_vary: {e}. Call it with no parameters."
            ))
        })
    }
}

/// **What turning the open map's clutter repeats would do**: the delta of turned rows, their ids,
/// and the clutter copies still alike after it.
struct Plan {
    delta: RowDelta,
    turned: Vec<String>,
    left: usize,
}

fn plan(project: &Project, open: &OpenMap) -> Result<Plan, Refusal> {
    // **A rules file that does not read is refused**, never replaced by the defaults: the defaults
    // could call a project's walls clutter and turn them.
    let rules = crate::ops_brp::load_repetition(&project.project_dir)
        .map_err(|e| Refusal::new(format!("{e}; nothing was turned.")))?;
    let v = repetition::vary(&open.map.placements, &project.library, &rules);
    let add_placements: Vec<Indexed<Placed>> = v
        .turned
        .iter()
        .map(|&(index, yaw)| {
            let mut row = open.map.placements[index].clone();
            row.yaw = yaw;
            Indexed { index, row }
        })
        .collect();
    let turned: Vec<String> = add_placements.iter().map(|a| a.row.id.clone()).collect();
    Ok(Plan {
        delta: RowDelta {
            remove_placements: turned.clone(),
            add_placements,
            ..RowDelta::default()
        },
        turned,
        left: v.left,
    })
}

/// **The rows a change touched, as they stand now**: what an undo checks before it puts them back.
fn rows_of(open: &OpenMap, ids: &[String]) -> serde_json::Value {
    let rows: Vec<&Placed> = ids
        .iter()
        .filter_map(|id| open.map.placements.iter().find(|p| &p.id == id))
        .collect();
    json!({"map": open.map.name, "ids": ids, "rows": rows})
}

fn summary(p: &Plan) -> String {
    let left = match p.left {
        0 => String::new(),
        n => format!("; {n} clutter piece(s) still have a look-alike in range (more copies close together than their turns can tell apart: four for a square piece, two for a long one, which only a half turn keeps on its cells; or round pieces no turn changes)"),
    };
    if p.turned.is_empty() {
        return format!("nothing to turn: no clutter copy can be turned out of a repeat{left}");
    }
    format!("turned {} ({}){left}", p.turned.len(), p.turned.join(", "))
}

/// **Turn the clutter copies of the open map that repeat visibly**, never structure.
pub struct RepetitionVary;

impl Operation for RepetitionVary {
    fn name(&self) -> &'static str {
        "repetition_vary"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
    }

    fn undo(&self) -> Undo {
        Undo::Inverse
    }

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).is_ok_and(|p| p.delta.is_some())
    }

    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }

    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        let open = ctx.open.as_ref()?;
        Some(json!({"map": open.map.name, "rows": open.map.placements}))
    }

    /// **Only the rows the change touched**, so an unrelated edit made by hand cannot block the undo.
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let Some(ids) = after.get("ids").and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok()) else {
            return self.state(ctx);
        };
        Some(rows_of(ctx.open.as_ref()?, &ids))
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = Params::parse(params)?;
        let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_ref()) else {
            return Err(Refusal::new("no map is open, so there are no repeats to turn."));
        };
        if let Some(delta) = &p.delta {
            let mut trial = open.map.clone();
            delta.apply(&mut trial)?;
            return Ok(json!({"map": open.map.name, "would_restore": delta.add_placements.len()}));
        }
        let plan = plan(project, open)?;
        Ok(json!({
            "map": open.map.name,
            "would": summary(&plan),
            "would_turn": plan.turned,
            "left": plan.left,
        }))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open."));
        };
        let Some(open) = ctx.open.as_mut() else {
            return Err(Refusal::new("no map is open, so there are no repeats to turn."));
        };
        let (delta, text) = match p.delta {
            Some(delta) => {
                if delta == RowDelta::default() {
                    // **The undo of a vary that turned nothing** changes nothing, so writes nothing.
                    return Ok(Outcome {
                        summary: "nothing to put back: repetition_vary turned nothing".to_owned(),
                        affected: 0,
                        saved: false,
                        inverse: Some(json!({"delta": delta})),
                        after: Some(rows_of(open, &[])),
                    });
                }
                let text = format!("put back {} row(s) repetition_vary turned", delta.add_placements.len());
                (delta, text)
            }
            None => {
                let plan = plan(project, open)?;
                let text = summary(&plan);
                if plan.turned.is_empty() {
                    // **Nothing turned, so nothing is written** and the undo has nothing to take back.
                    return Ok(Outcome {
                        summary: text,
                        affected: 0,
                        saved: false,
                        inverse: Some(json!({"delta": RowDelta::default()})),
                        after: Some(rows_of(open, &[])),
                    });
                }
                (plan.delta, text)
            }
        };
        let n = delta.add_placements.len();
        let before = open.map.clone();
        let inverse = delta.apply(&mut open.map)?;
        if let Err(e) = open.save(project) {
            open.map = before;
            return Err(Refusal::new(format!(
                "the file refused the turned rows, so nothing was written and nothing turned: {e}"
            )));
        }
        let ids: Vec<String> = delta.add_placements.iter().map(|a| a.row.id.clone()).collect();
        Ok(Outcome {
            summary: text,
            affected: n,
            saved: true,
            inverse: Some(json!({"delta": inverse})),
            after: Some(rows_of(open, &ids)),
        })
    }
}
