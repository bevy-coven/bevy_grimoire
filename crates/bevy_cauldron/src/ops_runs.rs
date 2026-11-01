//! **Runs as registered operations** (#115): `run_add`, `run_set`, `run_remove`.
//!
//! A run is a piece repeated along a path, held in the map as the run and not the rows it lays
//! (`bevy_grimoire_core::run`, `Map::runs`). These three operations are the only way the editor
//! changes `Map::runs`, so the run tool's drag and an agent's `apply` are one change with one log
//! line and one undo, and the map file is saved as the change commits.
//!
//! Each operation's inverse is the same operation sent a parameter only an undo may send
//! (`take_back`, `put_back`, `restore`), the way `note_add` is undone. A plan is the commit run on a
//! copy that is never written, so a preview cannot disagree with what a dispatch does.

use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::run::{Corner, Gap, Run, RunPlaced};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// **A run put back where it stood**, for the undo of a removal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub index: usize,
    pub run: RunPlaced,
}

/// **The map's runs and its name**, which is the state an undo checks.
fn state_of_open(ctx: &Ctx) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    Some(json!({"map": open.map.name, "runs": serde_json::to_value(&open.map.runs).ok()?}))
}

struct Change {
    summary: String,
    inverse: serde_json::Value,
}

fn bad_params(what: &str, fields: &str, e: serde_json::Error) -> Refusal {
    Refusal::new(format!("these are not {what}: {e}. The fields are {fields}, and nothing else."))
}

/// **Run one change on the open map's runs.** `write` false is the plan: the same code on the same
/// map, and nothing is saved. The map is validated and every run laid before anything is written, so
/// a run that does not lay refuses naming itself and the file is untouched.
fn apply(
    ctx: &mut Ctx,
    write: bool,
    change: impl FnOnce(&mut Map) -> Result<Change, Refusal>,
) -> Result<Option<Outcome>, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open."));
    };
    let Some(open) = ctx.open.as_mut() else {
        return Err(Refusal::new("no map is open, so there are no runs to change."));
    };
    let mut next = open.map.clone();
    let done = change(&mut next)?;
    next.validate().map_err(Refusal::new)?;
    for r in &next.runs {
        bevy_grimoire_core::run::lay(&r.run, &project.library)
            .map_err(|e| Refusal::new(format!("run `{}` does not lay: {e}. Nothing was changed.", r.id)))?;
    }
    if !write {
        return Ok(None);
    }
    let before = std::mem::replace(&mut open.map, next);
    if let Err(e) = open.save(project) {
        open.map = before;
        return Err(Refusal::new(format!("the file refused the change, so nothing was written: {e}")));
    }
    Ok(Some(Outcome {
        summary: done.summary,
        affected: 1,
        saved: true,
        inverse: Some(done.inverse),
        after: state_of_open(ctx),
    }))
}

fn position(map: &Map, id: &str, what: &str) -> Result<usize, Refusal> {
    map.runs
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| Refusal::new(format!("no run `{id}` on this map, so nothing was {what}.")))
}

fn take_out(map: &mut Map, id: &str, what: &str) -> Result<Change, Refusal> {
    let index = position(map, id, what)?;
    let run = map.runs.remove(index);
    Ok(Change { summary: format!("{what} run {id}"), inverse: json!({"put_back": PutBack { index, run }}) })
}

/// `by_add` says which operation is putting it back, because an inverse is dispatched under the
/// operation that logged it: `run_add` is undone with `take_back`, `run_remove` with `id`.
fn put_back(map: &mut Map, pb: &PutBack, by_add: bool) -> Result<Change, Refusal> {
    if map.runs.iter().any(|r| r.id == pb.run.id) {
        return Err(Refusal::new(format!("run {} is already on the map, so it was not put back.", pb.run.id)));
    }
    let at = pb.index.min(map.runs.len());
    let id = pb.run.id.clone();
    map.runs.insert(at, pb.run.clone());
    let inverse = if by_add { json!({"take_back": id}) } else { json!({"id": id}) };
    Ok(Change { summary: format!("put back run {id}"), inverse })
}

const RUN_FIELDS: &str = "`descriptor`, `pitch`, `from`, `to`, `corners` and `gaps`";

fn point_schema(what: &str) -> serde_json::Value {
    json!({"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": what})
}

fn corners_schema() -> serde_json::Value {
    json!({
        "type": "array",
        "description": "The turns, in the order the path meets them. Replaces the whole list.",
        "items": {
            "type": "object", "additionalProperties": false, "required": ["at", "yaw"],
            "properties": {
                "at": point_schema("Where the path turns, in map space."),
                "yaw": {"type": "number", "description": "Degrees; the direction after the turn."}
            }
        }
    })
}

fn gaps_schema() -> serde_json::Value {
    json!({
        "type": "array",
        "description": "The missing posts. Replaces the whole list.",
        "items": {
            "type": "object", "additionalProperties": false, "required": ["count", "at_post"],
            "properties": {
                "count": {"type": "integer", "minimum": 1, "description": "How many posts are missing."},
                "at_post": {"type": "integer", "minimum": 0, "description": "Index of the first missing post along the whole run."}
            }
        }
    })
}

// ----------------------------------------------------------------- run_add

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddParams {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Run>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub owned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned_because: Option<String>,
    /// Undo only: take a run this operation added back off the map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take_back: Option<String>,
    /// Undo only: put a run an undo took off back where it stood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl AddParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "run"],
            "properties": {
                "id": {"type": "string", "description": "The run's id, unique among the map's placements, stamps and runs. Its rows are `<id>/<n>`."},
                "run": {
                    "type": "object", "additionalProperties": false,
                    "required": ["descriptor", "from", "to"],
                    "properties": {
                        "descriptor": {"type": "string", "description": "The piece to repeat."},
                        "pitch": {"type": "number", "description": "Metres between posts. Absent: the piece's own footprint along the run."},
                        "from": point_schema("Where the run starts, in map space (metres)."),
                        "to": point_schema("Where the run ends, in map space (metres)."),
                        "corners": corners_schema(),
                        "gaps": gaps_schema()
                    }
                },
                "owned": {"type": "boolean", "description": "Every row the run lays is owned by the author."},
                "owned_because": {"type": "string", "description": "Why they are owned."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<AddParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| bad_params("a run", "`id`, `run`, `owned` and `owned_because`", e))
    }

    fn undo_only(&self) -> bool {
        self.take_back.is_some() || self.put_back.is_some()
    }
}

/// **Add a run to the map**: a piece repeated along a path, stored as the run.
pub struct RunAdd;

impl RunAdd {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = AddParams::parse(params)?;
        apply(ctx, write, |map| {
            if let Some(id) = &p.take_back {
                return take_out(map, id, "took back");
            }
            if let Some(pb) = &p.put_back {
                return put_back(map, pb, true);
            }
            let Some(run) = p.run.clone() else {
                return Err(Refusal::new("a run to add is named by `id` and `run`. Nothing was added."));
            };
            if p.id.is_empty() {
                return Err(Refusal::new("a run needs an `id`. Nothing was added."));
            }
            map.runs.push(RunPlaced { id: p.id.clone(), run, owned: p.owned, owned_because: p.owned_because.clone() });
            Ok(Change { summary: format!("added run {}", p.id), inverse: json!({"take_back": p.id}) })
        })
    }
}

impl Operation for RunAdd {
    fn name(&self) -> &'static str {
        "run_add"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
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
        let mut scratch = Ctx { project: ctx.project.clone(), open: ctx.open.clone(), ..Ctx::default() };
        Self::go(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::go(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// ----------------------------------------------------------------- run_set

/// `pitch: null` is "the piece's own footprint", not "leave it": serde reads an explicit `null` into
/// `Option<f32>` as absent, so the field is a double option.
fn pitch_present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<f32>>, D::Error> {
    <Option<f32> as Deserialize>::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<String>,
    #[serde(default, deserialize_with = "pitch_present", skip_serializing_if = "Option::is_none")]
    pub pitch: Option<Option<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<(f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<(f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corners: Option<Vec<Corner>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gaps: Option<Vec<Gap>>,
    /// Undo only: the run as it stood before the edit, put back whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Run>,
}

impl SetParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id"],
            "properties": {
                "id": {"type": "string", "description": "The run to change."},
                "descriptor": {"type": "string", "description": "A different piece to repeat."},
                "pitch": {"type": ["number", "null"], "description": "Metres between posts; null returns to the piece's own footprint."},
                "from": point_schema("Move the start, in map space (metres)."),
                "to": point_schema("Move the end, in map space (metres)."),
                "corners": corners_schema(),
                "gaps": gaps_schema()
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<SetParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| bad_params("a run edit", RUN_FIELDS, e))
    }

    fn changes_nothing(&self) -> bool {
        self.descriptor.is_none()
            && self.pitch.is_none()
            && self.from.is_none()
            && self.to.is_none()
            && self.corners.is_none()
            && self.gaps.is_none()
    }
}

/// **Change one run's path, pitch, corners or gaps**; its rows follow.
pub struct RunSet;

impl RunSet {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = SetParams::parse(params)?;
        apply(ctx, write, |map| {
            let i = position(map, &p.id, "changed")?;
            let before = map.runs[i].run.clone();
            if let Some(restore) = &p.restore {
                map.runs[i].run = restore.clone();
                return Ok(Change { summary: format!("took back the edit of run {}", p.id), inverse: json!({"restore": before}) });
            }
            if p.changes_nothing() {
                return Err(Refusal::new(format!("nothing to change on run `{}`: name one of {RUN_FIELDS}.", p.id)));
            }
            let run = &mut map.runs[i].run;
            if let Some(d) = &p.descriptor {
                run.descriptor = d.clone();
            }
            if let Some(pitch) = p.pitch {
                run.pitch = pitch;
            }
            if let Some(f) = p.from {
                run.from = f;
            }
            if let Some(t) = p.to {
                run.to = t;
            }
            if let Some(c) = &p.corners {
                run.corners = c.clone();
            }
            if let Some(g) = &p.gaps {
                run.gaps = g.clone();
            }
            if *run == before {
                return Err(Refusal::new(format!("run `{}` already says that. Nothing was changed.", p.id)));
            }
            Ok(Change { summary: format!("changed run {}", p.id), inverse: json!({"id": p.id, "restore": before}) })
        })
    }
}

impl Operation for RunSet {
    fn name(&self) -> &'static str {
        "run_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        SetParams::parse(params).is_ok_and(|p| p.restore.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        SetParams::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_open(ctx)
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

// -------------------------------------------------------------- run_remove

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveParams {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Undo only: put a run a removal took back where it stood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl RemoveParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id"],
            "properties": {"id": {"type": "string", "description": "The run to remove, whole, with every row it lays."}}
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<RemoveParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| bad_params("a run removal", "`id`", e))
    }
}

/// **Take a run off the map**, whole.
pub struct RunRemove;

impl RunRemove {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = RemoveParams::parse(params)?;
        apply(ctx, write, |map| {
            if let Some(pb) = &p.put_back {
                return put_back(map, pb, false);
            }
            take_out(map, &p.id, "removed")
        })
    }
}

impl Operation for RunRemove {
    fn name(&self) -> &'static str {
        "run_remove"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
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
        let mut scratch = Ctx { project: ctx.project.clone(), open: ctx.open.clone(), ..Ctx::default() };
        Self::go(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::go(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// ------------------------------------------------------- placements_to_run

/// **The map's runs and its rows**: a conversion changes both, so its undo checks both.
fn state_with_rows(ctx: &Ctx) -> Option<serde_json::Value> {
    let open = ctx.open.as_ref()?;
    Some(json!({
        "map": open.map.name,
        "runs": serde_json::to_value(&open.map.runs).ok()?,
        "placements": serde_json::to_value(&open.map.placements).ok()?,
    }))
}

/// **Rows a conversion took, put back where they stood**, for its undo: the run to remove and each
/// row with its index in `placements`, ascending.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    pub run: String,
    pub rows: Vec<(usize, bevy_grimoire_core::map::Placed)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToRunParams {
    /// The id the new run is given.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// The placement ids to replace, in path order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<String>,
    /// Undo only: take the run off and put its rows back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Restore>,
}

impl ToRunParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "rows"],
            "properties": {
                "id": {"type": "string", "description": "The id the new run is given."},
                "rows": {"type": "array", "items": {"type": "string"}, "minItems": 1,
                    "description": "Placement ids in path order: the rows the run replaces. Refused, naming the first row that does not fit, unless the run lays exactly them."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<ToRunParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| bad_params("a conversion", "`id` and `rows`", e))
    }
}

/// **Replace rows by the run that lays exactly them** (#115). One way: no operation turns a run back
/// into rows except this one's undo, so the file never grows a second way to say the same line.
pub struct PlacementsToRun;

impl PlacementsToRun {
    fn go(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = ToRunParams::parse(params)?;
        let Some(library) = ctx.project.as_ref().map(|pr| pr.library.clone()) else {
            return Err(Refusal::new("no project is open."));
        };
        let outcome = apply(ctx, write, |map| {
            if let Some(r) = &p.restore {
                position(map, &r.run, "restored")?;
                map.runs.retain(|x| x.id != r.run);
                for (at, row) in &r.rows {
                    map.placements.insert((*at).min(map.placements.len()), row.clone());
                }
                let rows: Vec<String> = r.rows.iter().map(|(_, row)| row.id.clone()).collect();
                return Ok(Change {
                    summary: format!("put back the rows of run {}", r.run),
                    inverse: json!({"id": r.run, "rows": rows}),
                });
            }
            if p.id.is_empty() || p.rows.is_empty() {
                return Err(Refusal::new("a conversion needs an `id` and the `rows` to replace. Nothing was changed."));
            }
            if map.runs.iter().any(|r| r.id == p.id) {
                return Err(Refusal::new(format!("run `{}` is already on the map. Nothing was changed.", p.id)));
            }
            let mut taken: Vec<(usize, bevy_grimoire_core::map::Placed)> = p
                .rows
                .iter()
                .filter_map(|id| map.placements.iter().position(|r| &r.id == id).map(|i| (i, map.placements[i].clone())))
                .collect();
            taken.sort_by_key(|(i, _)| *i);
            map.placements_to_run(&p.id, &p.rows, &library).map_err(Refusal::new)?;
            Ok(Change {
                summary: format!("replaced {} row(s) by run {}", taken.len(), p.id),
                inverse: json!({"restore": Restore { run: p.id.clone(), rows: taken }}),
            })
        })?;
        Ok(outcome.map(|mut o| {
            o.affected = p.rows.len().max(1);
            o.after = state_with_rows(ctx);
            o
        }))
    }
}

impl Operation for PlacementsToRun {
    fn name(&self) -> &'static str {
        "placements_to_run"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        ToRunParams::parse(params).is_ok_and(|p| p.restore.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        ToRunParams::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_with_rows(ctx)
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
