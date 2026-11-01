//! **An agent shows its plan, as one registered operation** (E-069): `plan_set`.
//!
//! The plan is data beside the map (`bevy_grimoire_core::plan`, the `.plan.ron` next to the `.map.ron`).
//! The agent sends the whole list as it now stands, each item with its status; the list replaces the
//! plan and an empty list clears it. The inverse is the same operation sent the list it replaced, so
//! one undo step puts the previous plan back, and a plan (a preview) is the same code on a copy that is
//! never written.

use bevy_grimoire_core::plan::{self, Plan, PlanItem};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The whole plan, in order. Empty clears it.
    pub items: Vec<PlanItem>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["items"],
            "properties": {
                "items": {
                    "type": "array",
                    "description": "The whole plan, in the order you mean to take the steps; it replaces the plan the person sees. An empty list clears it. Send the list again with a status changed as you work.",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["text"],
                        "properties": {
                            "text": {"type": "string", "description": "What this step does."},
                            "status": {"type": "string", "enum": ["pending", "doing", "done", "skipped"], "description": "Where it stands. Default pending."}
                        }
                    }
                }
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "this is not a plan: {e}. The only field is `items`, a list of {{`text`, `status` (pending, doing, done or skipped)}}."
            ))
        })
    }
}

/// **The plan of the open map**, in the form `grimoire_state` and an undo's check read.
pub fn plan_json(plan: &Plan) -> serde_json::Value {
    let (pending, doing, done, skipped) = plan.counts();
    json!({
        "items": plan.items,
        "pending": pending,
        "doing": doing,
        "done": done,
        "skipped": skipped,
    })
}

/// **The plan of the open map** as `grimoire_state` reports it: an empty list when there is none.
pub fn state_json(open: &crate::project::OpenMap) -> serde_json::Value {
    plan_json(&Plan::load(&plan::path_for_map(&open.map_path)).unwrap_or_default())
}

fn state_of_open(ctx: &Ctx) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    let plan = Plan::load(&plan::path_for_map(&open.map_path)).ok()?;
    Some(json!({"map": open.map.name, "plan": plan_json(&plan)}))
}

fn summary(plan: &Plan) -> String {
    if plan.items.is_empty() {
        return "cleared the plan".to_owned();
    }
    let (pending, doing, done, skipped) = plan.counts();
    format!(
        "plan of {} step(s): {done} done, {doing} doing, {pending} pending, {skipped} skipped",
        plan.items.len()
    )
}

/// **Show the agent's plan beside the open map.**
pub struct PlanSet;

impl PlanSet {
    /// `write` false is the plan: the same code on the same file, and nothing is saved.
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = Params::parse(params)?;
        let Some(open) = ctx.open.as_ref() else {
            return Err(Refusal::new("no map is open, so there is no plan to show beside it."));
        };
        let path = plan::path_for_map(&open.map_path);
        let before = Plan::load(&path).map_err(Refusal::new)?;
        let after = Plan::new(p.items).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
        if !write {
            return Ok(Some(Outcome {
                summary: summary(&after),
                affected: after.items.len(),
                saved: false,
                inverse: None,
                after: None,
            }));
        }
        after
            .save(&path)
            .map_err(|e| Refusal::new(format!("the plan file refused the change, so nothing was written: {e}")))?;
        Ok(Some(Outcome {
            summary: summary(&after),
            affected: after.items.len(),
            saved: true,
            inverse: Some(json!({"items": before.items})),
            after: state_of_open(ctx),
        }))
    }
}

impl Operation for PlanSet {
    fn name(&self) -> &'static str {
        "plan_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["plan"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let out = Self::apply(ctx, params, false)?.expect("a plan returns its outcome");
        Ok(json!({"would": out.summary}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}
