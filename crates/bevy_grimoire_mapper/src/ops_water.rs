//! **Placing water, as one registered operation** (#100): `water_place`.
//!
//! The plan is `<grimoire_dir>/water/<map>.ron`, the engine-free `bevy_grimoire_core::water::WaterPlan`
//! as RON, saved beside the map and the terrain. One call places one spring, sink or lake, or clears
//! the plan; either is one undo. Placing never runs the simulation.

use bevy_grimoire_core::water::{Placed, WaterPlan};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// An undo's put-back: the file as it was (`None`: there was none).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub map: String,
    /// One thing to place: `{"Spring": {x, z, rate_m3_s, radius_m}}`, `{"Sink": {...}}` or `{"Lake": {x, z, radius_m, level_m}}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Placed>,
    /// Remove every spring, sink and lake of the map.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    /// An undo's put-back. Not a thing a caller sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The map whose water is edited."},
                "place": {"type": "object", "description": "One thing to place: {\"Spring\": {x, z, rate_m3_s, radius_m}}, {\"Sink\": {x, z, rate_m3_s, radius_m}} or {\"Lake\": {x, z, radius_m, level_m}}."},
                "clear": {"type": "boolean", "description": "Remove every spring, sink and lake of the map."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a water change: {e}. The fields are `map`, `place` and `clear`.")))?;
        if p.place.is_none() && !p.clear && p.put_back.is_none() {
            return Err(Refusal::new("a water change needs `place` or `clear`. Nothing was changed."));
        }
        Ok(p)
    }
}

fn path_of(ctx: &Ctx, map: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open, so there is no water to place."));
    };
    if map.is_empty() || map.contains(['/', '\\', '.']) {
        return Err(Refusal::new(format!("`{map}` is not a map name. Nothing was written.")));
    }
    Ok(project.grimoire_dir.join("water").join(format!("{map}.ron")))
}

fn read(ctx: &Ctx, map: &str) -> Option<String> {
    std::fs::read_to_string(path_of(ctx, map).ok()?).ok()
}

/// **The saved water plan of the open map**, or `None` when it has none.
pub fn plan_of(project: &crate::project::Project, open: &crate::project::OpenMap) -> Option<WaterPlan> {
    let name = open.map_path.file_name()?.to_str()?.strip_suffix(".map.ron")?;
    ron::from_str(&std::fs::read_to_string(project.grimoire_dir.join("water").join(format!("{name}.ron"))).ok()?).ok()
}

/// **Place a spring, a sink or a lake, or clear the map's water.**
pub struct WaterPlace;

impl WaterPlace {
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let path = path_of(ctx, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});
        let (text, affected) = if let Some(back) = &p.put_back {
            (back.text.clone(), 0)
        } else if p.clear {
            (None, 0)
        } else {
            let mut plan: WaterPlan = match &before {
                Some(t) => ron::from_str(t).map_err(|e| Refusal::new(format!("{} is not a water plan: {e}", path.display())))?,
                None => WaterPlan::default(),
            };
            plan.place(p.place.expect("parse needs a place, a clear or a put_back")).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
            let t = ron::ser::to_string_pretty(&plan, ron::ser::PrettyConfig::default())
                .map_err(|e| Refusal::new(format!("the water could not be written: {e}")))?;
            (Some(t), 1)
        };
        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    std::fs::write(&path, t)
                        .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(Outcome {
            summary: format!("water {}: {}", p.map, if affected == 1 { "placed one" } else if p.clear { "cleared" } else { "put back" }),
            affected,
            saved: write,
            inverse: Some(inverse),
            after: Some(json!({"map": p.map, "text": text})),
        })
    }
}

impl Operation for WaterPlace {
    fn name(&self) -> &'static str {
        "water_place"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["water_plan"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).map(|p| p.put_back.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        Some(json!({"map": map, "text": read(ctx, map)}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        Ok(json!({"map": Params::parse(params)?.map, "placed": o.affected}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}

/// `water_run`'s parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunParams {
    pub map: String,
    /// Seconds the springs pour before the water runs on by itself. Default 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pour_s: Option<f32>,
    /// Step cap. Default 20000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<u32>,
    /// An undo's put-back. Not a thing a caller sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

fn field_path(ctx: &Ctx, map: &str) -> Result<std::path::PathBuf, Refusal> {
    path_of(ctx, map).map(|p| p.with_file_name(format!("{map}.field.ron")))
}

/// **The kept water of the open map**, the field `water_run` saved, or `None` when it has none.
pub fn field_of(project: &crate::project::Project, open: &crate::project::OpenMap) -> Option<bevy_grimoire_core::water::WaterField> {
    let name = open.map_path.file_name()?.to_str()?.strip_suffix(".map.ron")?;
    ron::from_str(&std::fs::read_to_string(project.grimoire_dir.join("water").join(format!("{name}.field.ron"))).ok()?).ok()
}

/// **Run the map's springs, sinks and lakes over its terrain until the water settles, and keep it.**
/// Previewing (`plan`) runs the same simulation and writes nothing; applying keeps the result as the
/// map's starting water, in one undo. It runs only when asked, never while sculpting.
pub struct WaterRun;

impl WaterRun {
    fn parse(raw: &serde_json::Value) -> Result<RunParams, Refusal> {
        serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a water run: {e}. The fields are `map`, `pour_s` and `max_steps`.")))
    }

    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        use bevy_grimoire_core::water::{simulate, RunSettings};
        let p = Self::parse(params)?;
        let path = field_path(ctx, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});
        let (text, summary, affected) = if let Some(back) = &p.put_back {
            (back.text.clone(), format!("water {}: put back", p.map), 0)
        } else {
            let plan_text = read(ctx, &p.map).ok_or_else(|| Refusal::new(format!("map `{}` has no springs, sinks or lakes to run. Place some with `water_place`.", p.map)))?;
            let plan: WaterPlan = ron::from_str(&plan_text).map_err(|e| Refusal::new(format!("the water plan is not readable: {e}")))?;
            if plan.is_empty() {
                return Err(Refusal::new(format!("map `{}` has no springs, sinks or lakes to run.", p.map)));
            }
            let ground_path = ctx.project.as_ref().expect("path_of needs a project").grimoire_dir.join("terrain").join(format!("{}.ron", p.map));
            let ground = bevy_grimoire_core::heightfield::Heightfield::parse(
                &std::fs::read_to_string(&ground_path)
                    .map_err(|_| Refusal::new(format!("map `{}` has no terrain for the water to run on. Sculpt one with `terrain_stroke`.", p.map)))?,
            )
            .map_err(|e| Refusal::new(format!("{} does not read, so no water was run: {e}", ground_path.display())))?;
            let mut settings = RunSettings::default();
            if let Some(s) = p.pour_s {
                if !s.is_finite() || s < 0.0 {
                    return Err(Refusal::new("`pour_s` must be a number of seconds, zero or more."));
                }
                settings.pour_s = s;
            }
            if let Some(m) = p.max_steps {
                settings.max_steps = m.max(1);
            }
            let field = simulate(&ground, &plan, settings);
            let summary = format!(
                "water {}: {} wet cells, {:.1} m3 after {} steps{}",
                p.map,
                field.wet_cells(0.01),
                field.volume_m3(),
                field.steps,
                if field.settled { ", settled" } else { ", not settled (step cap)" }
            );
            let t = ron::ser::to_string_pretty(&field, ron::ser::PrettyConfig::default())
                .map_err(|e| Refusal::new(format!("the water could not be written: {e}")))?;
            (Some(t), summary, field.wet_cells(0.01))
        };
        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    std::fs::write(&path, t)
                        .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(Outcome { summary, affected, saved: write, inverse: Some(inverse), after: Some(json!({"map": p.map, "text": text})) })
    }
}

impl Operation for WaterRun {
    fn name(&self) -> &'static str {
        "water_run"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["water_field"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Self::parse(params).map(|p| p.put_back.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The map whose water is run."},
                "pour_s": {"type": "number", "description": "Seconds the springs pour before the water runs on by itself until it settles. Default 60."},
                "max_steps": {"type": "integer", "description": "Step cap, so a run always ends. Default 20000."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        Some(json!({"map": map, "text": std::fs::read_to_string(field_path(ctx, map).ok()?).ok()}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        Ok(json!({"map": Self::parse(params)?.map, "summary": o.summary, "wet_cells": o.affected}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}
