//! **The light map in the viewport** (#103): `bevy_light_grid`'s illuminance field drawn over the
//! open map, so an author sees where a light-shy creature will hide.
//!
//! Camera-style state, like the Environment panel's clock: the LIGHT MAP button shows or hides it,
//! which writes no file and leaves no undo step. While it is shown, every change to the map re-bakes
//! it through [`crate::ops_light::bake`], the function the `light_map` operation reads, so the cell an
//! author looks at and the cell an agent reads through `apply` are one number.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::editor::EditorState;
use crate::ops::{Applied, Ctx, Source, dispatch, session_id};
use crate::ops_light::{Params, bake};
use crate::project::{OpenMap, Project};

/// Whether the light map is shown, and the field it draws.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct LightOverlay {
    pub on: bool,
    /// The last bake while shown; `None` while hidden or when the map's fixtures do not read.
    pub drawn: Option<Drawn>,
}

/// One baked light map, as the viewport draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub cell_m: f32,
    /// Map-space metres of cell (0, 0)'s corner.
    pub min: (f32, f32),
    pub width: usize,
    pub height: usize,
    pub fixtures: usize,
    /// The grid's own values, row-major (`bevy_light_grid::row_major`).
    pub cells: Vec<f32>,
}

impl Drawn {
    /// The drawn value of the cell a map-space point falls in, or `None` off the map.
    pub fn lux_at(&self, at: (f32, f32)) -> Option<f32> {
        let c = IVec2::new(((at.0 - self.min.0) / self.cell_m).floor() as i32, ((at.1 - self.min.1) / self.cell_m).floor() as i32);
        bevy_light_grid::in_grid(c, self.width, self.height).then(|| self.cells[bevy_light_grid::row_major(c, self.width)])
    }

    /// A cell's colour: dark is a cool blue (where a light-shy creature hides), lit runs to amber at
    /// intensity 1, the reference `Flicker::base`.
    pub fn colour(lux: f32) -> Color {
        let t = lux.clamp(0.0, 1.0);
        let dark = LinearRgba::new(0.05, 0.12, 0.45, 0.35);
        let lit = LinearRgba::new(1.0, 0.62, 0.12, 0.85);
        Color::from(dark.mix(&lit, t))
    }
}

/// The LIGHT MAP section's one button.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LightMapToggle;

/// The LIGHT MAP section of the controls panel.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::{Severity, button, section};
    section(s, "LIGHT MAP");
    button(s, LightMapToggle, "Show / hide", Severity::Plain);
}

/// **The button** shows or hides the overlay. Showing it dispatches `light_map`, so a person's click and an
/// agent's `apply` are one operation with one log line, and the status says the operation's receipt.
/// Hiding reads nothing, so it dispatches nothing. `Option`, because this is a GLOBAL observer: see
/// `editor::on_size_field_click`.
pub fn on_light_map_click(
    activate: On<Activate>,
    buttons: Query<(), With<LightMapToggle>>,
    overlay: Option<ResMut<LightOverlay>>,
    state: Option<ResMut<EditorState>>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
) {
    let (Some(mut overlay), Some(mut state)) = (overlay, state) else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    overlay.on = !overlay.on;
    if !overlay.on {
        state.status.note("light map hidden".to_owned());
        return;
    }
    let mut ctx = Ctx {
        project: project.as_deref().cloned(),
        open: open.as_deref().cloned(),
        source: Source::Human,
        session: session_id(),
        ..Ctx::default()
    };
    match dispatch(&mut ctx, "light_map", &serde_json::json!({})) {
        Ok(Applied::Done { outcome, .. }) => state.status.note(format!("{}; blue is dark, amber is lit by a placed fixture's Flicker", outcome.summary)),
        Ok(Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Re-bake while shown**, when the button or the map changed. Writes `drawn` only when it differs,
/// so a bake does not mark the overlay changed and bake again next frame.
pub fn refresh_light_overlay(
    mut overlay: ResMut<LightOverlay>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    state: Option<ResMut<EditorState>>,
) {
    let moved = overlay.is_changed() || project.as_ref().is_some_and(|p| p.is_changed()) || open.as_ref().is_some_and(|o| o.is_changed());
    if !moved {
        return;
    }
    let want = match (overlay.on, project.as_deref(), open.as_deref()) {
        (true, Some(project), Some(open)) => match bake(&open.map, &project.library, &Params::default()) {
            Ok(b) => {
                let (width, height) = b.grid.dimensions();
                Some(Drawn { cell_m: b.cell_m, min: b.min, width, height, fixtures: b.fixtures, cells: b.grid.cells().to_vec() })
            }
            Err(e) => {
                if let Some(mut state) = state {
                    state.status.problem(e.0);
                }
                None
            }
        },
        _ => None,
    };
    if overlay.drawn != want {
        overlay.drawn = want;
    }
}

/// **Draw it over the floor**: one flat square per cell, nested outlines because gizmos have no fill,
/// just above the floor fill and the crowd heat so neither hides it.
pub fn draw_light_overlay(overlay: Res<LightOverlay>, open: Res<OpenMap>, project: Res<Project>, mut gizmos: Gizmos) {
    let Some(d) = overlay.drawn.as_ref() else {
        return;
    };
    let y = open.map.origin.1 - crate::editor::ground_drop(&project) + 0.03;
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for (i, lux) in d.cells.iter().enumerate() {
        let (cx, cz) = ((i % d.width) as f32 + 0.5, (i / d.width) as f32 + 0.5);
        let at = Vec3::new(open.map.origin.0 + d.min.0 + cx * d.cell_m, y, open.map.origin.2 + d.min.1 + cz * d.cell_m);
        for k in [0.9, 0.6, 0.3] {
            gizmos.rect(Isometry3d::new(at, flat), Vec2::splat(d.cell_m * k), Drawn::colour(*lux));
        }
    }
}
