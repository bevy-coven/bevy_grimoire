//! **The light pool's budget** (#103): which fixtures are real lights where the author stands.
//!
//! A game keeps a few real lights and hands them to the props nearest its focus (`bevy_lights`'
//! `LightPoolPlugin`). This operation answers "which of my torches are real from here?" by running
//! that plugin itself over the open map's fixtures (every placement carrying a `bevy_lights`
//! `Flicker`, its `base` as the candidate's intensity) and reading its own `PoolBudget`, so the
//! answer is the game's, not a second ranking rule. Read-only; positions are on the ground plane.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_lights::{Flicker, FlickerPlugin, LightPool, LightPoolPlugin, PoolBudget, PoolCandidate};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::editor::EditorState;
use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};
use crate::project::{OpenMap, Project};
use crate::view::Rig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Map-space `[x, z]` the viewer stands at.
    pub focus: (f32, f32),
    /// How many real lights the pool keeps. Default: the crate's own `LightPool::default().size`.
    #[serde(default)]
    pub size: Option<usize>,
    /// How far from the focus a fixture may be to get a light, metres. Default: the crate's.
    #[serde(default)]
    pub reach: Option<f32>,
}

#[derive(Component)]
struct Focus;

/// The open map's fixtures that hold a real light from `p.focus`, and the ones that stay virtual.
pub fn budget(map: &bevy_grimoire_core::map::Map, p: &Params) -> Result<serde_json::Value, Refusal> {
    let mut pool = LightPool::default();
    if let Some(size) = p.size {
        pool.size = size;
    }
    if let Some(reach) = p.reach {
        pool.reach = reach;
    }
    let problems = pool.problems();
    if !problems.is_empty() {
        return Err(Refusal::new(format!("the pool settings are unusable: {problems:?}")));
    }
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, FlickerPlugin, LightPoolPlugin::<Focus>::default())).insert_resource(pool.clone());
    app.world_mut().spawn((Focus, Transform::from_xyz(p.focus.0, 0.0, p.focus.1)));
    let mut ids = std::collections::HashMap::new();
    for pl in &map.placements {
        let Some(text) = pl.family.get("bevy_lights") else { continue };
        let f: Flicker = ron::from_str(text).map_err(|e| Refusal::new(format!("{}'s bevy_lights settings do not read: {e}", pl.id)))?;
        let e = app.world_mut().spawn((PoolCandidate { intensity: f.base, ..default() }, Transform::from_xyz(pl.at.0, 0.0, pl.at.1))).id();
        ids.insert(e, pl.id.clone());
    }
    // One frame propagates transforms, the next lets the pool read them.
    for _ in 0..3 {
        app.update();
    }
    let b = app.world().resource::<PoolBudget>().clone();
    let (mut real, mut virt): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (e, id) in &ids {
        if b.holds(*e) { real.push(id.clone()) } else { virt.push(id.clone()) }
    }
    real.sort();
    virt.sort();
    Ok(json!({
        "focus": [p.focus.0, p.focus.1], "size": pool.size, "reach": pool.reach,
        "wanting": b.wanting, "granted": b.granted(), "starved": b.starved(),
        "real": real, "virtual": virt,
    }))
}

/// **Read the pool's budget** from a viewer position.
pub struct LightPoolBudget;

impl LightPoolBudget {
    fn run(ctx: &Ctx, raw: &serde_json::Value) -> Result<(String, serde_json::Value), Refusal> {
        let p: Params = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("these are not light pool parameters: {e}. `focus` [x, z] is required; `size` and `reach` are optional.")))?;
        let Some(open) = ctx.open.as_ref() else { return Err(Refusal::new("no map is open, so there are no fixtures to budget.")) };
        let after = budget(&open.map, &p)?;
        let summary = format!("light pool: {} of {} wanting fixtures are real lights, {} starved", after["granted"], after["wanting"], after["starved"]);
        Ok((summary, after))
    }
}

impl Operation for LightPoolBudget {
    fn name(&self) -> &'static str {
        "light_pool"
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["focus"],
            "properties": {
                "focus": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "Map-space [x, z] the viewer stands at."},
                "size": {"type": "integer", "description": "How many real lights the pool keeps. Default the crate's (16)."},
                "reach": {"type": "number", "description": "How far from the focus a fixture may be to get a light, metres. Default the crate's (60)."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Self::run(ctx, params).map(|(_, after)| after)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (summary, after) = Self::run(ctx, params)?;
        Ok(Outcome { summary, affected: 0, saved: false, inverse: None, after: Some(after) })
    }
}

/// The LIGHTS section's one button: the pool's budget from where the camera looks.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PoolHere;

/// The LIGHTS section of the controls panel.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::{Severity, button, section};
    section(s, "LIGHTS");
    button(s, PoolHere, "Pool here", Severity::Plain);
}

/// **The button** dispatches `light_pool` with the camera's ground point, in map space, as the focus, and
/// says the operation's own receipt. `Option`, because this is a GLOBAL observer: see
/// `editor::on_size_field_click`.
pub fn on_pool_click(
    activate: On<Activate>,
    buttons: Query<(), With<PoolHere>>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    rig: Option<Res<Rig>>,
    state: Option<ResMut<EditorState>>,
) {
    let (Some(mut state), Some(open), Some(rig)) = (state, open, rig) else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let focus = [rig.focus.x - open.map.origin.0, rig.focus.z - open.map.origin.2];
    let mut ctx = Ctx {
        project: project.as_deref().cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "light_pool", &json!({ "focus": focus })) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => state.status.note(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}
