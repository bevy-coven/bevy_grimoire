//! **A map's lights are reached in one press in every build** (NV-11, #395; series getting around, #377).
//!
//! `docs/ui/navigation.md` measured the World workspace as one chord away and drawing no lights in a build without the off-by-default
//! `bevy_lights` feature: the LIGHTS block was the crate's pool button, and nothing else drew a light. A light is a placement's
//! `bevy_lights` family value, saved in the map; only the pool's *budget* needs the crate. So the World workspace's LIGHTS section lists
//! the map's fixtures in every build, and a build with the crate adds its pool button under the same heading.
//!
//! The editor is driven as a person drives it: the workspace's command, then a read of what is on screen. The light is put on a
//! placement the way an operation would leave it, as RON text under the family key.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_grimoire_core::map::Map;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to settle after a switch and between reads.
const SETTLE_FRAMES: usize = 60;

/// The family key and the value a light is saved under: the crate's own `Flicker`, as text the editor does not interpret.
const KEY: &str = "bevy_lights";
const FLICKER: &str = "(base: 800.0)";

/// A project whose torch carries a light, written into the map file the way an operation leaves it, and the editor opened on it. (Putting
/// the light into the open map in memory would leave the map with unsaved edits, and the workspace switch would ask about them.)
fn editor(tag: &str, light: bool) -> App {
    let root = fixtures::Fixture::new(&format!("lights_list_{tag}_{}", std::process::id()))
        .descriptor("torch", "alpha")
        .descriptor("stool", "alpha")
        .place_as("torch@0", "torch", (2.0, 2.0))
        .place_as("stool@0", "stool", (4.0, 2.0))
        .build("m");
    if light {
        let file = root.join("assets/grimoire/maps/m.map.ron");
        let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file:?}: {e}"));
        let mut map = Map::parse(&text).unwrap_or_else(|e| panic!("{file:?}: {e}"));
        let torch = map.placements.iter_mut().find(|p| p.id == "torch@0").unwrap_or_else(|| panic!("the fixture has no torch@0"));
        torch.family.insert(KEY.to_owned(), FLICKER.to_owned());
        std::fs::write(&file, map.to_ron().unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{file:?}: {e}"));
    }
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();
    }
}

/// **One press of the World workspace's chord**: its command, queued the way the key does.
fn press_the_world_chord(app: &mut App) {
    let command = commands::by_id("workspace.world").unwrap_or_else(|| panic!("`workspace.world` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::World, "the World chord did not open the World workspace");
}

fn box_of(entity: Entity, world: &World) -> Option<(Vec2, Vec2)> {
    let node = *world.get::<ComputedNode>(entity)?;
    let at = *world.get::<UiGlobalTransform>(entity)?;
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    (size.x > 0.0 && size.y > 0.0).then_some((at.translation * scale, size * 0.5))
}

/// Whether `mine` is inside the box of every scroll area above `entity`.
fn inside_every_scroll_area(mine: (Vec2, Vec2), entity: Entity, world: &World) -> bool {
    let mut here = entity;
    while let Some(parent) = world.get::<ChildOf>(here).map(ChildOf::parent) {
        here = parent;
        if world.get::<ScrollPosition>(here).is_none() {
            continue;
        }
        let Some(area) = box_of(here, world) else { continue };
        if (mine.0.x - area.0.x).abs() >= mine.1.x + area.1.x || (mine.0.y - area.0.y).abs() >= mine.1.y + area.1.y {
            return false;
        }
    }
    true
}

/// How many nodes a person can see whose words are exactly `label`.
fn drawn(app: &mut App, label: &str) -> usize {
    let world = app.world_mut();
    let found: Vec<Entity> = world
        .query::<(Entity, &Text, &InheritedVisibility)>()
        .iter(world)
        .filter(|(_, text, shown)| shown.get() && text.0.trim() == label)
        .map(|(entity, _, _)| entity)
        .collect();
    found.into_iter().filter(|e| box_of(*e, world).is_some_and(|mine| inside_every_scroll_area(mine, *e, world))).count()
}

#[test]
fn one_press_of_the_world_chord_shows_the_maps_lights() {
    let mut app = editor("fixtures", true);
    press_the_world_chord(&mut app);
    assert_eq!(drawn(&mut app, "LIGHTS"), 1, "the World workspace does not show the LIGHTS heading");
    assert_eq!(drawn(&mut app, "torch@0: (base: 800.0)"), 1, "the map's torch carries a light and the World workspace does not list it");
    assert_eq!(drawn(&mut app, "stool@0: (base: 800.0)"), 0, "a placement with no light is listed as one");
}

#[test]
fn a_map_with_no_lights_says_so() {
    let mut app = editor("none", false);
    press_the_world_chord(&mut app);
    assert_eq!(drawn(&mut app, "no light fixtures on this map"), 1, "a map with no lights says nothing under LIGHTS");
}
