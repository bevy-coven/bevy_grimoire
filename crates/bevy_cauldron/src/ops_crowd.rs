//! **Run the crowd preview on the open map, as one registered operation** (#107): `crowd_preview`.
//!
//! A read: it walks N people from one area to another over the open map with `bevy_orca` and
//! `bevy_stigmergy` ([`crate::crowd_preview`]) and answers with the measurement. It changes no
//! document and writes nothing, so it has no undo. The map is expanded (stamps and runs) first.

use serde::Deserialize;
use serde_json::json;

use crate::crowd_preview::{Area, Preview, Spec};
use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// Steps a run may take when the caller names no limit: 5 minutes of walking at the default 0.1 s step.
const DEFAULT_MAX_STEPS: usize = 3000;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Where the walkers start, `[min_x, min_z, max_x, max_z]` in metres.
    pub from: Area,
    /// Where they are going, in the same form.
    pub to: Area,
    pub walkers: usize,
    #[serde(default = "one")]
    pub seed: u64,
    #[serde(default)]
    pub max_steps: Option<usize>,
}

fn one() -> u64 {
    1
}

/// A run's heat overlay cells `(centre, trail, congestion)` and their size in metres.
struct Heat {
    cells: Vec<(bevy::math::Vec2, f32, f32)>,
    cell: f32,
}

/// **Walk a crowd across the open map and measure where it queues.**
pub struct CrowdPreview;

impl CrowdPreview {
    fn run(ctx: &Ctx, raw: &serde_json::Value) -> Result<(crate::crowd_preview::Report, Params, Heat), Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("this is not a crowd preview: {e}. The fields are `from`, `to`, `walkers`, `seed` and `max_steps`."))
        })?;
        let (Some(project), Some(open)) = (ctx.project.as_ref(), ctx.open.as_ref()) else {
            return Err(Refusal::new("no map is open, so there is nothing to walk a crowd over."));
        };
        let map = open.map.expanded(&project.compositions.compositions, &project.library).map_err(Refusal::new)?;
        let mut preview = Preview::new(&map, &project.library, Spec::new(p.from, p.to, p.walkers, p.seed)).map_err(Refusal::new)?;
        let report = preview.run(p.max_steps.unwrap_or(DEFAULT_MAX_STEPS));
        let heat = preview.heat();
        Ok((report, p, Heat { cells: heat, cell: preview.frame().cell_size }))
    }
}

fn report_json(r: &crate::crowd_preview::Report) -> serde_json::Value {
    json!({
        "steps": r.steps,
        "arrived": r.arrived,
        "walking": r.walking,
        "congestion_total": r.congestion_total,
        "hotspot": r.hotspot.map(|(at, value)| json!({"at": [at.x, at.y], "congestion": value})),
    })
}

impl Operation for CrowdPreview {
    fn name(&self) -> &'static str {
        "crowd_preview"
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        let area = |what: &str| json!({"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4, "description": what});
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["from", "to", "walkers"],
            "properties": {
                "from": area("Where the walkers start: [min_x, min_z, max_x, max_z] in metres."),
                "to": area("Where they are going, in the same form."),
                "walkers": {"type": "integer", "description": "How many people."},
                "seed": {"type": "integer", "description": "Seeds their start spots; the same seed repeats the run. Default 1."},
                "max_steps": {"type": "integer", "description": "Stop after this many 0.1 s steps if not everyone has arrived. Default 3000."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (report, p, _) = Self::run(ctx, params)?;
        Ok(json!({"walkers": p.walkers, "report": report_json(&report)}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (report, p, Heat { cells: heat, cell }) = Self::run(ctx, params)?;
        let hot = match report.hotspot {
            Some((at, _)) => format!(", queue at ({:.1}, {:.1})", at.x, at.y),
            None => String::new(),
        };
        Ok(Outcome {
            summary: format!("crowd of {}: {} arrived in {} steps{hot}", p.walkers, report.arrived, report.steps),
            affected: 0,
            saved: false,
            inverse: None,
            after: Some({
                let mut after = report_json(&report);
                // The heat overlay's cells: `[x, z, trail, congestion]`, each channel 0..=1 of its peak.
                after["cell"] = json!(cell);
                after["heat"] = json!(heat.iter().map(|(at, t, g)| [at.x, at.y, *t, *g]).collect::<Vec<_>>());
                after
            }),
        })
    }
}
