//! **A map's water is reached in one press** (NV-11, #393; series getting around, #377).
//!
//! `docs/ui/navigation.md` measured the World workspace as one chord away and drawing no water: water is the `water_place` operation,
//! written through the terrain line, and nothing showed what a map's plan held. The World workspace now has a WATER section that reads
//! `<grimoire>/water/<map>.ron`, the file the operation writes. It lists; it does not place (the terrain line does, and says how under
//! the heading), because a second way to place water would be a second path to one operation.
//!
//! The editor is driven as a person drives it: the workspace's command, then a read of what is on screen.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to settle after a switch and between reads.
const SETTLE_FRAMES: usize = 60;

/// A plan with one spring and one lake, in the engine-free core's own RON shape (`version` is optional and reads as the first).
const PLAN: &str = "(springs: [(x: 4.0, z: 6.0, rate_m3_s: 2.5, radius_m: 3.0)], lakes: [(x: 10.0, z: 12.0, radius_m: 5.0, level_m: 1.5)])";

fn editor(tag: &str, plan: Option<&str>) -> App {
    let root = fixtures::Fixture::new(&format!("water_list_{tag}_{}", std::process::id())).descriptor("stool", "alpha").place("stool", (0.0, 0.0)).build("m");
    if let Some(text) = plan {
        let dir = bevy_cauldron::project::Project::open(&root, None).unwrap_or_else(|e| panic!("{e}")).grimoire_dir.join("water");
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        std::fs::write(dir.join("m.ron"), text).unwrap_or_else(|e| panic!("water plan: {e}"));
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
fn one_press_of_the_world_chord_shows_the_maps_water() {
    let mut app = editor("plan", Some(PLAN));
    press_the_world_chord(&mut app);
    assert_eq!(drawn(&mut app, "WATER"), 1, "the World workspace does not show the WATER heading");
    for row in ["spring at (4, 6): 2.5 m3/s, radius 3 m", "lake at (10, 12): radius 5 m, level 1.5 m"] {
        assert_eq!(drawn(&mut app, row), 1, "the map's water plan holds `{row}` and the World workspace does not list it");
    }
}

#[test]
fn a_map_with_no_water_says_so() {
    let mut app = editor("none", None);
    press_the_world_chord(&mut app);
    assert_eq!(drawn(&mut app, "no water placed"), 1, "a map with no water says nothing under WATER");
}

#[test]
fn a_water_plan_that_does_not_read_is_said_not_shown_as_empty() {
    let mut app = editor("broken", Some("(springs: [(x: "));
    press_the_world_chord(&mut app);
    let world = app.world_mut();
    let said: Vec<String> = world.query::<&Text>().iter(world).map(|t| t.0.clone()).filter(|t| t.contains("water/m.ron does not read")).collect();
    assert_eq!(said.len(), 1, "a water plan that does not parse is not named under WATER");
}
