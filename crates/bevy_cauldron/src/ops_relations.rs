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

use crate::ops::{Ctx, Indexed, Operation, Outcome, Refusal, RowDelta, Shown, Undo, enclose, point_box, row_box, shown_file};
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
    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        // **The rows this call changes, computed the way its commit computes them** — pure, on a copy of the
        // map, so a preview and its apply answer the same question with the same code.
        //
        // **A move must show where the piece left as well as where it landed**, or a person watching the
        // Familiar sees a table hop with nothing to connect the two frames. The delta's rows are the new
        // positions; the rows they replace are the old ones; their union is one box round both.
        let Ok(p) = Params::parse(params) else {
            return Shown::MapWhole;
        };
        let Some(open) = ctx.open.as_ref() else {
            return Shown::MapWhole;
        };
        let delta = match p.delta {
            Some(delta) => delta,
            None => match mend(open) {
                Ok(m) => m.delta,
                Err(_) => return Shown::MapWhole,
            },
        };
        let mut boxes: Vec<[f32; 4]> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        for added in &delta.add_placements {
            boxes.push(match ctx.project.as_ref() {
                Some(project) => row_box(&project.library, &added.row),
                None => point_box(added.row.at),
            });
            ids.push(added.row.id.clone());
        }
        for id in &delta.remove_placements {
            if let Some(before) = open.map.placements.iter().find(|q| &q.id == id) {
                boxes.push(match ctx.project.as_ref() {
                    Some(project) => row_box(&project.library, before),
                    None => point_box(before.at),
                });
            }
        }
        match enclose(boxes) {
            Some(rect) => Shown::Map { rect, placements: ids },
            None => Shown::MapWhole,
        }
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

/// **Add one relation to the map's `.relations.ron`, or take one out again** (#137, the constraint
/// layer's missing authoring door).
///
/// The relation vocabulary has been complete for weeks — `Faces`, `Near`, `Apart`, `InFrontOf`,
/// `Behind`, `LeftOf`, `RightOf`, `ClearInFront`, `VisibleFrom`, `Along`, `ReachableFrom` — and
/// `relations_solve` mends the map against them. **What was missing is the step before**: a person
/// could only *add* one by hand-writing RON, so the layer was readable by a person editing a text
/// file and by nobody else, and no log line named an authored relation.
///
/// **The relation is data, so the operation takes the file's own shape.** `of` and `kind` are the
/// `Relation` fields verbatim, serialised through the crate's own serde types, so a rename of a `Kind`
/// variant is a compile error here rather than a second vocabulary that drifts. `hard` is the same
/// flag `relations_solve` already honours.
///
/// **A refusal changes nothing.** A relation naming a placement the map does not hold is refused
/// (with the near misses), a file that does not read is refused rather than overwritten, and the write
/// is atomic — the same three rules `data_set` and `map_save` keep.
///
/// **The undo is a removal by value**, not by index: an undo that removed "the third relation" would
/// take out the wrong one the moment somebody added another in between.
pub mod relations_add {
    use super::*;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Params {
        /// Which placement the relation is about.
        pub of: String,
        /// The relation itself, in `relations::Kind`'s own shape, as JSON.
        pub kind: serde_json::Value,
        /// Must hold, rather than "prefer this". See `relations::Relation::hard`.
        #[serde(default)]
        pub hard: bool,
        /// **The undo's own form**: the relation to take out again. Only an undo sets this.
        #[serde(default)]
        pub remove: Option<serde_json::Value>,
    }

    impl Params {
        pub fn schema() -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "of": {"type": "string", "description": "The placement id the relation is about."},
                    "kind": {"type": "object", "description": "The relation, in relations::Kind's shape: {\"Faces\": \"id\"}, {\"Near\": {\"to\": \"id\", \"within\": 2.0}}, {\"ReachableFrom\": {\"from\": \"id\", \"clearance\": 0.6}}, ..."},
                    "hard": {"type": "boolean", "default": false, "description": "Must hold, not merely preferred."},
                    "remove": {"type": "object", "description": "The undo's own form: the relation to take out again."}
                },
                "required": ["of", "kind"]
            })
        }

        /// **Read the parameters**, with the crate's own error style: a caller with the wrong shape is
        /// told what the shape is rather than getting a serde message it cannot act on.
        pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
            serde_json::from_value(raw.clone()).map_err(|e| {
                Refusal::new(format!(
                    "these are not the parameters for adding a relation: {e}. Pass `of` and `kind`, and \
                     `hard` if it must hold."
                ))
            })
        }

        /// **The relation as the crate's own type**, so the shape cannot drift from the file's.
        pub fn relation(&self) -> Result<relations::Relation, Refusal> {
            let kind: relations::Kind = serde_json::from_value(self.kind.clone()).map_err(|e| {
                Refusal::new(format!(
                    "`kind` is not a relation this build understands: {e}. Every kind takes an object or \
                     a string in its own shape — see `grimoire_list_operations` for the list."
                ))
            })?;
            Ok(relations::Relation { of: self.of.clone(), kind, hard: self.hard })
        }
    }

    /// The file's text with `relation` appended, or with the first equal relation removed.
    fn changed(text: Option<&str>, relation: &relations::Relation, remove: bool) -> Result<(String, bool), Refusal> {
        let mut doc = match text {
            Some(t) => relations::Relations::parse(t)
                .map_err(|e| Refusal::new(format!("the relations file does not read, so nothing was written: {e}")))?,
            // **A map with no relations file yet is not an error**: adding the first relation is
            // exactly how one comes to exist. `Relations::default()` carries the current version.
            None => relations::Relations::default(),
        };
        let found = doc.relations.iter().position(|r| r == relation);
        if remove {
            let Some(at) = found else {
                return Err(Refusal::new(format!(
                    "`{}` no longer holds that relation, so there is nothing to take back — the file was \
                     changed by hand since.",
                    relation.of
                )));
            };
            doc.relations.remove(at);
        } else {
            if found.is_some() {
                return Err(Refusal::new(format!(
                    "`{}` already holds that relation; adding it twice would mend the map against the same \
                     rule and count it twice.",
                    relation.of
                )));
            }
            doc.relations.push(relation.clone());
        }
        let text = doc
            .to_ron()
            .map_err(|e| Refusal::new(format!("the relations could not be written: {e}")))?;
        Ok((text, found.is_some()))
    }

    /// **Write the file and say what the map now keeps.** A relation that still reads unmet is said
    /// so, because a person who adds one wants to know whether the map already honours it.
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let Some(open) = ctx.open.as_ref() else {
            return Err(Refusal::new("no map is open, so there are no relations to add."));
        };
        // **A relation about a piece the map does not hold is refused here**, before the file is
        // touched: the solver would skip it silently and the author would believe it held.
        let remove = p.remove.is_some();
        let relation = match &p.remove {
            Some(raw) => serde_json::from_value::<relations::Relation>(raw.clone())
                .map_err(|e| Refusal::new(format!("`remove` is not a relation this build understands: {e}")))?,
            None => {
                let relation = p.relation()?;
                if !open.map.placements.iter().any(|pl| pl.id == relation.of) {
                    let near: Vec<&str> = open
                        .map
                        .placements
                        .iter()
                        .map(|pl| pl.id.as_str())
                        .filter(|id| id.contains(&relation.of) || relation.of.contains(*id))
                        .take(3)
                        .collect();
                    let hint = if near.is_empty() {
                        String::new()
                    } else {
                        format!(" Did you mean {}?", near.join(", "))
                    };
                    return Err(Refusal::new(format!(
                        "`{}` is not a placement on `{}`, so the relation would hold of nothing.{hint}",
                        relation.of, open.map.name
                    )));
                }
                relation
            }
        };
        let path = relations::path_for_map(&open.map_path);
        let text = std::fs::read_to_string(&path).ok();
        let (new, existed) = changed(text.as_deref(), &relation, remove)?;
        if write {
            bevy_grimoire_core::atomic_file::write(&path, &new).map_err(|e| {
                Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display()))
            })?;
        }
        Ok(Outcome {
            summary: if remove {
                format!("took back the relation on `{}`", relation.of)
            } else if existed {
                format!("`{}` keeps that relation", relation.of)
            } else {
                format!("`{}` now keeps it", relation.of)
            },
            affected: 1,
            saved: write,
            // **By value**, so an undo cannot take out somebody else's relation.
            inverse: Some(serde_json::to_value(&relation).unwrap_or(serde_json::Value::Null)),
            after: Some(json!({"file": path.display().to_string(), "text": new})),
        })
    }

    /// **Add one relation to the open map's relations file.**
    pub struct RelationsAdd;

    impl Operation for RelationsAdd {
        fn name(&self) -> &'static str {
            "relations_add"
        }
        fn shows(&self, ctx: &Ctx, _params: &serde_json::Value) -> Shown {
            // **The relations are a file beside the map**, which nothing in the editor draws yet.
            match ctx.open.as_ref() {
                Some(open) => shown_file(ctx, &bevy_grimoire_core::relations::path_for_map(&open.map_path), None),
                None => Shown::MapWhole,
            }
        }
        fn writes(&self) -> &'static [&'static str] {
            // The relations file is a data file beside the map, not the map itself.
            &["relations"]
        }

        fn undo(&self) -> Undo {
            Undo::Inverse
        }

        // **`remove` is not "undo only".** Taking a relation out is a change a person or an agent may
        // ask for by hand — the same as deleting a piece — so the operation answers it directly and
        // the executor's undo is one more caller. `relations_solve`'s `delta` is the other shape: a row
        // delta is only ever produced by a solve, so that one is undo-only.

        fn schema(&self) -> serde_json::Value {
            Params::schema()
        }

        fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
            let outcome = apply(ctx, params, false)?;
            Ok(json!({
                "would": outcome.summary,
                "saved": false,
                "text": outcome.after.as_ref().and_then(|a| a.get("text")).cloned(),
            }))
        }

        fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
            apply(ctx, params, true)
        }
    }
}
