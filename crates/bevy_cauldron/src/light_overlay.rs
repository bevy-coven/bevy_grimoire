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

    /// **A cell's colour as the shipped palette draws it**: dark is a cool blue (where a light-shy
    /// creature hides), lit runs to amber at intensity 1, the reference `Flicker::base`.
    ///
    /// This is the no-skin answer and the editor's own; [`LightOverlayInk::colour`] is what the
    /// viewport draws, so a project's skin and a theme reach the overlay (UI-80, #254).
    pub fn colour_for(lux: f32) -> Color {
        SHIPPED.colour(lux)
    }

    /// The drawn value of the cell a map-space point falls in, from `ink`.
    pub fn colour_with(lux: f32, ink: &LightOverlayInk) -> Color {
        ink.colour(lux)
    }
}

/// **The overlay's two inks, from the running skin** (UI-80, #254): a cell that is dark, and one that
/// is lit, mixed between by the cell's own illuminance.
///
/// The two colours are the skin's own named tones (`bevy_gooey`'s `GooeySkin::tones`), because no
/// field of a skin means "this map is dark" — the same reason `SUGGEST` and `GRID_LINE` are tones. A
/// skin that carries neither falls back to the shipped pair, so the editor's own look is unchanged by
/// construction: `tests/the_view_menu_toggles_the_overlays.rs` pins that the editor's skin draws
/// exactly what the constants drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightOverlayInk {
    /// A cell with no light on it: cool, faint, and the colour a light-shy creature's cover reads as.
    pub dark: Color,
    /// A cell lit to intensity 1 by a placed fixture's `Flicker`.
    pub lit: Color,
}

impl LightOverlayInk {
    /// From the running skin when there is one, the shipped pair when there is not.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> LightOverlayInk {
        let Some(skin) = skin else { return SHIPPED };
        LightOverlayInk {
            dark: skin.tone(DARK_TONE).unwrap_or(SHIPPED.dark),
            lit: skin.tone(LIT_TONE).unwrap_or(SHIPPED.lit),
        }
    }

    /// A cell's colour: `dark` at no light, `lit` at intensity 1, linear between.
    pub fn colour(&self, lux: f32) -> Color {
        Color::from(LinearRgba::from(self.dark).mix(&LinearRgba::from(self.lit), lux.clamp(0.0, 1.0)))
    }
}

/// The skin's tone for a cell with no light on it.
const DARK_TONE: &str = "overlay_dark";
/// The skin's tone for a cell lit to intensity 1.
const LIT_TONE: &str = "overlay_lit";

/// The pair the editor has always drawn with, and the no-skin fallback.
const SHIPPED: LightOverlayInk = LightOverlayInk {
    dark: Color::linear_rgba(0.05, 0.12, 0.45, 0.35),
    lit: Color::linear_rgba(1.0, 0.62, 0.12, 0.85),
};

/// The LIGHT MAP section's one button.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LightMapToggle;

/// The LIGHT MAP section of the controls panel.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::{Severity, button, section};
    section(s, "LIGHT MAP");
    button(s, LightMapToggle, "Show / hide", Severity::Plain);
}

/// **Show or hide the overlay, for `want`, and say what happened** — the one path both doors take
/// (UI-80, #254): the LIGHT MAP button and the View ▸ Light map row.
///
/// **Showing dispatches `light_map`**, so a person's click, the menu row and an agent's `apply` are
/// one operation with one log line, and the status says the operation's receipt. Hiding reads
/// nothing, so it dispatches nothing — there is no value to take back.
///
/// The overlay's state is set to `want` rather than flipped, because a menu row is a tick that says
/// what the overlay should be while a button is an action that reverses it: one function that obeys
/// `want` is the only way the two cannot disagree after a sequence of either.
fn set_the_light_map(
    want: bool,
    overlay: &mut LightOverlay,
    state: &mut EditorState,
    project: Option<&Project>,
    open: Option<&OpenMap>,
) {
    overlay.on = want;
    if !overlay.on {
        state.status.note("light map hidden".to_owned());
        return;
    }
    let mut ctx = Ctx {
        project: project.cloned(),
        open: open.cloned(),
        source: Source::Human,
        session: session_id(),
        ..Ctx::default()
    };
    match dispatch(&mut ctx, "light_map", &serde_json::json!({})) {
        Ok(Applied::Done { outcome, .. }) => state.status.note(format!("{}; blue is dark, amber is lit by a placed fixture's Flicker", outcome.summary)),
        Ok(Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **The button** shows or hides the overlay, through [`set_the_light_map`]. `Option`, because this is a
/// GLOBAL observer: see `editor::on_size_field_click`.
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
    let want = !overlay.on;
    set_the_light_map(want, &mut overlay, &mut state, project.as_deref(), open.as_deref());
}

/// **The View ▸ Light map row** (UI-80, #254), through the same [`set_the_light_map`] the button uses.
///
/// A check item reports what it should be, so the row's tick is the state and choosing it again is a
/// no-op rather than a reversal. Both doors therefore end in one call, and
/// `tests/the_view_menu_toggles_the_overlays.rs` compares the two: the same field, the same receipt,
/// one logged operation per showing.
pub fn on_light_map_item(
    change: On<bevy::ui_widgets::ValueChange<bool>>,
    items: Query<(), With<LightMapItem>>,
    overlay: Option<ResMut<LightOverlay>>,
    state: Option<ResMut<EditorState>>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
) {
    let (Some(mut overlay), Some(mut state)) = (overlay, state) else {
        return;
    };
    if items.get(change.source).is_err() {
        return;
    }
    set_the_light_map(change.value, &mut overlay, &mut state, project.as_deref(), open.as_deref());
}

/// The View ▸ Light map row, so its observer knows the change came from it.
#[derive(Component)]
pub struct LightMapItem;

/// **The row's tick is the overlay's state** (UI-80, #254), whichever door changed it — the rule
/// `menu_bar::follow_the_statistics_setting` keeps for its setting, and the reason a check item is
/// honest: a tick that disagrees with what is on screen is worse than no tick, because it is a claim.
///
/// Compared before writing, so a settled editor costs one comparison.
pub fn follow_the_light_map_tick(
    overlay: Option<Res<LightOverlay>>,
    items: Query<(Entity, Has<bevy_gooey::Checked>), With<LightMapItem>>,
    mut commands: Commands,
) {
    let Some(overlay) = overlay else { return };
    for (item, ticked) in &items {
        match (overlay.on, ticked) {
            (true, false) => {
                commands.entity(item).insert(bevy_gooey::Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<bevy_gooey::Checked>();
            }
            _ => {}
        }
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
pub fn draw_light_overlay(
    overlay: Res<LightOverlay>,
    open: Res<OpenMap>,
    project: Res<Project>,
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut gizmos: Gizmos,
) {
    let Some(d) = overlay.drawn.as_ref() else {
        return;
    };
    let ink = LightOverlayInk::of(skin.as_deref());
    let y = open.map.origin.1 - crate::editor::ground_drop(&project) + 0.03;
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for (i, lux) in d.cells.iter().enumerate() {
        let (cx, cz) = ((i % d.width) as f32 + 0.5, (i / d.width) as f32 + 0.5);
        let at = Vec3::new(open.map.origin.0 + d.min.0 + cx * d.cell_m, y, open.map.origin.2 + d.min.1 + cz * d.cell_m);
        for k in [0.9, 0.6, 0.3] {
            gizmos.rect(Isometry3d::new(at, flat), Vec2::splat(d.cell_m * k), ink.colour(*lux));
        }
    }
}
