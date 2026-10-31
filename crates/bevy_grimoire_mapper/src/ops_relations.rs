//! **Mend the open map's broken relations, as one registered operation** (#137): `relations_solve`.
//!
//! The relations are data beside the map (`bevy_grimoire_core::relations`, the `.relations.ron` next to
//! the `.map.ron`), and `relations::solve` nudges the pieces of every unmet one. This runs it on the
//! open map, writes the rows it moved, and records them as a [`RowDelta`], so one undo puts every
//! moved row back where it was. A preview is the same solve on the same rows, with nothing written.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{self, Relations};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Indexed, Operation, Outcome, Refusal, RowDelta, Undo};
use crate::project::OpenMap;

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
                    "description": "The rows an undo puts back. Only an undo may set this: to mend the map, call with no parameters.",
                    "type": "object"
                }
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "these are not the parameters for a relations solve: {e}. Call it with no parameters."
            ))
        })
    }
}

/// **The relations beside the open map**, or a refusal naming the file that is missing or bad.
fn load(open: &OpenMap) -> Result<Relations, Refusal> {
    let path = relations::path_for_map(&open.map_path);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        Refusal::new(format!(
            "there is no relations file to mend the map against ({}: {e}); write the relations an agent \
             or a person means this map to keep there first.",
            path.display()
        ))
    })?;
    Relations::parse(&text)
        .map_err(|e| Refusal::new(format!("{} does not read, so nothing was moved: {e}", path.display())))
}

/// **What a solve would do to the open map**: the delta that moves the changed rows, the ids the
/// solver moved, the unmet count before and after, and the relations it left unmet (their piece,
/// and whether each is hard).
struct Mend {
    delta: RowDelta,
    moved: Vec<String>,
    unmet_before: usize,
    unmet_after: usize,
    unsatisfied: Vec<(String, bool)>,
}

fn mend(open: &OpenMap) -> Result<Mend, Refusal> {
    let relations = load(open)?;
    let solved = relations::solve_map(&open.map.placements, &relations);
    // **Every row that changed, carried riders included**: `moved` lists only the pieces the solver
    // chose, but a rider moved with its host must be put back by the same undo.
    let add_placements: Vec<Indexed<Placed>> = solved
        .placements
        .into_iter()
        .zip(&open.map.placements)
        .enumerate()
        .filter(|(_, (new, old))| new != *old)
        .map(|(index, (row, _))| Indexed { index, row })
        .collect();
    let unsatisfied = solved
        .unsatisfied
        .iter()
        .map(|&i| (relations.relations[i].of.clone(), relations.relations[i].hard))
        .collect();
    Ok(Mend {
        delta: RowDelta {
            remove_placements: add_placements.iter().map(|a| a.row.id.clone()).collect(),
            add_placements,
            ..RowDelta::default()
        },
        moved: solved.moved,
        unmet_before: solved.unmet_before,
        unmet_after: solved.unmet_after,
        unsatisfied,
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

/// The relations a solve left unmet, as `"; still unmet: door (hard), lamp"`, or nothing.
fn still_unmet(m: &Mend) -> String {
    if m.unsatisfied.is_empty() {
        return String::new();
    }
    let names: Vec<String> = m
        .unsatisfied
        .iter()
        .map(|(of, hard)| if *hard { format!("{of} (hard)") } else { of.clone() })
        .collect();
    format!("; still unmet: {}", names.join(", "))
}

fn summary(m: &Mend) -> String {
    if m.delta.add_placements.is_empty() {
        return format!(
            "nothing to mend: {} relation(s) unmet, and no move mends one{}",
            m.unmet_before,
            still_unmet(m)
        );
    }
    format!(
        "moved {} ({}): unmet relations {} -> {}{}",
        m.moved.len(),
        m.moved.join(", "),
        m.unmet_before,
        m.unmet_after,
        still_unmet(m)
    )
}

/// **Nudge the open map's pieces until its relations hold**, never for the worse.
pub struct RelationsSolve;

impl Operation for RelationsSolve {
    fn name(&self) -> &'static str {
        "relations_solve"
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
        let Some(open) = ctx.open.as_ref() else {
            return Err(Refusal::new("no map is open, so there are no relations to mend."));
        };
        if let Some(delta) = &p.delta {
            let mut trial = open.map.clone();
            delta.apply(&mut trial)?;
            return Ok(json!({"map": open.map.name, "would_restore": delta.add_placements.len()}));
        }
        let m = mend(open)?;
        Ok(json!({
            "map": open.map.name,
            "would": summary(&m),
            "would_move": m.moved,
            "unmet_before": m.unmet_before,
            "unmet_after": m.unmet_after,
            "unsatisfied": m.unsatisfied.iter().map(|(of, hard)| json!({"of": of, "hard": hard})).collect::<Vec<_>>(),
        }))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let Some(open) = ctx.open.as_mut() else {
            return Err(Refusal::new("no map is open, so there are no relations to mend."));
        };
        let (delta, text, moved) = match p.delta {
            Some(delta) => {
                let n = delta.add_placements.len();
                if delta == RowDelta::default() {
                    // **The undo of a solve that moved nothing** changes nothing, so writes nothing.
                    return Ok(Outcome {
                        summary: "nothing to put back: the relations solve moved nothing".to_owned(),
                        affected: 0,
                        saved: false,
                        inverse: Some(json!({"delta": delta})),
                        after: Some(rows_of(open, &[])),
                    });
                }
                (delta, format!("put back {n} row(s) the relations solve moved"), n)
            }
            None => {
                let m = mend(open)?;
                let text = summary(&m);
                if m.delta.add_placements.is_empty() {
                    // **Nothing moved, so nothing is written** and the undo has nothing to take back.
                    return Ok(Outcome {
                        summary: text,
                        affected: 0,
                        saved: false,
                        inverse: Some(json!({"delta": RowDelta::default()})),
                        after: Some(rows_of(open, &[])),
                    });
                }
                let n = m.delta.add_placements.len();
                (m.delta, text, n)
            }
        };
        let before = open.map.clone();
        let inverse = delta.apply(&mut open.map)?;
        let Some(project) = ctx.project.as_ref() else {
            open.map = before;
            return Err(Refusal::new("no project is open."));
        };
        if let Err(e) = open.save(project) {
            open.map = before;
            return Err(Refusal::new(format!(
                "the file refused the moved rows, so nothing was written and nothing moved: {e}"
            )));
        }
        let ids: Vec<String> = delta.add_placements.iter().map(|a| a.row.id.clone()).collect();
        Ok(Outcome {
            summary: text,
            affected: moved,
            saved: true,
            inverse: Some(json!({"delta": inverse})),
            after: Some(rows_of(open, &ids)),
        })
    }
}
