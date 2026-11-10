//! **A placed torch flickers in the viewport** (#103).
//!
//! A placement that carries a `bevy_lights` `Flicker` (`placement_set_family`, the value is the crate's
//! own RON type) is drawn with a real [`PointLight`] hung over it, and the crate's own `FlickerPlugin`
//! drives that light's intensity every frame, the way a game running the plugin does. One that carries a
//! `Fire` holds the crate's `Fire` component at its foot instead, and a window's editor
//! ([`FirePreviewPlugin`]) lets the crate make its flame and its pooled light. So the author
//! sees the intensity the game draws, not the number in the file. Nothing is saved: the light is a
//! drawing of the placement's value, rebuilt when the map or the value changes.
//!
//! A value that does not read leaves the light as it was (the operation refuses such a value; a hand
//! edit of the map file that writes one is reported by the light map, which names the placement).

use bevy::prelude::*;
use bevy_lights::{Fire, Flicker, FlickerPlugin, FlamePlugin, FirePlugin, LightPoolPlugin};

use crate::editor::Placement;
use crate::fixture::Fixture;
use crate::project::OpenMap;

/// The family key a placement's fixture is saved under.
pub use crate::fixture::KEY;

/// How far above a placement's origin its light hangs, metres.
const LIFT: f32 = 1.2;

/// The preview light of one placement; its parent is the [`Placement`].
#[derive(Component, Clone, Copy, Debug)]
pub struct PlacedLight;

/// The preview of one placement that carries a `Fire`; its parent is the [`Placement`]. The component beside it is the crate's
/// own [`Fire`]: [`FirePreviewPlugin`] (a window's editor) turns it into a flame and a pooled light.
#[derive(Component, Clone, Copy, Debug)]
pub struct PlacedFire;

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

/// Hang, retune or take down each placement's light or fire so it matches the open map. Runs when the map
/// changed or a placement was spawned, so a settled map costs nothing.
pub fn sync_placed_lights(
    mut commands: Commands,
    open: Option<Res<OpenMap>>,
    placed: Query<(Entity, &Placement, Option<&Children>)>,
    spawned: Query<(), Added<Placement>>,
    mut lights: Query<(&mut Flicker, &mut PointLight), With<PlacedLight>>,
    mut fires: Query<&mut Fire, With<PlacedFire>>,
) {
    let Some(open) = open else { return };
    if !open.is_changed() && spawned.is_empty() {
        return;
    }
    for (entity, Placement(id), children) in &placed {
        let wanted = open.map.placements.iter().find(|p| &p.id == id).and_then(|p| p.family.get(crate::chrome::ink().key)).map(|text| Fixture::read(text));
        let existing = children.and_then(|c| c.iter().find(|e| lights.contains(*e) || fires.contains(*e)));
        let existing_is_fire = existing.is_some_and(|e| fires.contains(e));
        match (wanted, existing) {
            (Some(Ok(Fixture::Flicker(flicker))), Some(light)) if !existing_is_fire => {
                if let Ok((mut now, mut point)) = lights.get_mut(light) {
                    if *now != flicker {
                        *now = flicker;
                    }
                    point.intensity = flicker.base;
                }
            }
            (Some(Ok(Fixture::Fire(fire))), Some(held)) if existing_is_fire => {
                if let Ok(mut now) = fires.get_mut(held)
                    && *now != fire
                {
                    *now = fire;
                }
            }
            // The value changed from one kind to the other: take the old drawing down, hang the new one.
            (Some(Ok(fixture)), Some(held)) => {
                commands.entity(held).despawn();
                hang(&mut commands, entity, fixture);
            }
            (Some(Ok(fixture)), None) => hang(&mut commands, entity, fixture),
            // A value that does not read keeps what it had.
            (Some(Err(_)), _) => {}
            (None, Some(held)) => commands.entity(held).despawn(),
            (None, None) => {}
        }
    }
}

fn hang(commands: &mut Commands, placement: Entity, fixture: Fixture) {
    match fixture {
        Fixture::Flicker(flicker) => {
            commands.entity(placement).with_child((
                PlacedLight,
                flicker,
                PointLight { intensity: flicker.base, shadow_maps_enabled: false, ..default() },
                Transform::from_xyz(0.0, LIFT, 0.0),
            ));
        }
        // The fire stands at its foot, on the placement's origin; its flame and light are the crate's.
        Fixture::Fire(fire) => {
            commands.entity(placement).with_child((PlacedFire, fire, Transform::default()));
        }
    }
}

/// **The flame and the pooled light of a placed `Fire`, in a running editor** (#103). `bevy_lights`' `FlamePlugin` needs a
/// render app, so only the binary adds this (the headless harness holds the `Fire` and nothing more). The pool measures from the
/// map camera, the way a game's pool measures from its player.
pub struct FirePreviewPlugin;

impl Plugin for FirePreviewPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FlickerPlugin>() {
            app.add_plugins(FlickerPlugin);
        }
        app.add_plugins((FlamePlugin, LightPoolPlugin::<crate::view::MainCamera>::default(), FirePlugin));
    }
}
