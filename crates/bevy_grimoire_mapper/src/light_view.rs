//! **A placed torch flickers in the viewport** (#103).
//!
//! A placement that carries a `bevy_lights` `Flicker` (`placement_set_family`, the value is the crate's
//! own RON type) is drawn with a real [`PointLight`] hung over it, and the crate's own `FlickerPlugin`
//! drives that light's intensity every frame, the way a game running the plugin does. So the author
//! sees the intensity the game draws, not the number in the file. Nothing is saved: the light is a
//! drawing of the placement's value, rebuilt when the map or the value changes.
//!
//! A value that does not read leaves the light as it was (the operation refuses such a value; a hand
//! edit of the map file that writes one is reported by the light map, which names the placement).

use bevy::prelude::*;
use bevy_lights::{Flicker, FlickerPlugin};

use crate::editor::Placement;
use crate::project::OpenMap;

/// The family key a placement's `Flicker` is saved under.
pub const KEY: &str = "bevy_lights";

/// How far above a placement's origin its light hangs, metres.
const LIFT: f32 = 1.2;

/// The preview light of one placement; its parent is the [`Placement`].
#[derive(Component, Clone, Copy, Debug)]
pub struct PlacedLight;

/// Draws a light over every placement that carries a `Flicker`, and lets the crate flicker it.
pub struct LightViewPlugin;

impl Plugin for LightViewPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FlickerPlugin>() {
            app.add_plugins(FlickerPlugin);
        }
        app.add_systems(Update, sync_placed_lights);
    }
}

/// Hang, retune or take down each placement's light so it matches the open map. Runs when the map
/// changed or a placement was spawned, so a settled map costs nothing.
pub fn sync_placed_lights(
    mut commands: Commands,
    open: Option<Res<OpenMap>>,
    placed: Query<(Entity, &Placement, Option<&Children>)>,
    spawned: Query<(), Added<Placement>>,
    mut lights: Query<(&mut Flicker, &mut PointLight), With<PlacedLight>>,
    marks: Query<(), With<PlacedLight>>,
) {
    let Some(open) = open else { return };
    if !open.is_changed() && spawned.is_empty() {
        return;
    }
    for (entity, Placement(id), children) in &placed {
        let wanted = open
            .map
            .placements
            .iter()
            .find(|p| &p.id == id)
            .and_then(|p| p.family.get(KEY))
            .map(|text| ron::from_str::<Flicker>(text));
        let existing = children.and_then(|c| c.iter().find(|e| marks.contains(*e)));
        match (wanted, existing) {
            (Some(Ok(flicker)), Some(light)) => {
                if let Ok((mut now, mut point)) = lights.get_mut(light) {
                    if *now != flicker {
                        *now = flicker;
                    }
                    point.intensity = flicker.base;
                }
            }
            (Some(Ok(flicker)), None) => {
                commands.entity(entity).with_child((
                    PlacedLight,
                    flicker,
                    PointLight { intensity: flicker.base, shadow_maps_enabled: false, ..default() },
                    Transform::from_xyz(0.0, LIFT, 0.0),
                ));
            }
            // A value that does not read keeps the light it had.
            (Some(Err(_)), _) => {}
            (None, Some(light)) => commands.entity(light).despawn(),
            (None, None) => {}
        }
    }
}
