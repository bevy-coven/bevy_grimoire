//! **A map's forest is reached in one press** (NV-11, #394; series getting around, #377).
//!
//! `docs/ui/navigation.md` measured the World workspace as one chord away and drawing no forest: forest is the `forest_scatter`
//! operation, written through the terrain line, and nothing showed what a map had planted. The World workspace now has a FOREST section
//! that reads `<grimoire>/forest/<map>.yaml`, the file the operation writes: how many trees, and how many of each species. It lists; it
//! does not plant (the terrain line does, and says how under the heading).
//!
//! The editor is driven as a person drives it: the workspace's command, then a read of what is on screen.
//!
//! Owner exception (2026-11-12): a_maps_forest_is_listed_in_the_world_workspace.rs: main's new test wrote .ron input; converted to the YAML spelling.

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

/// A forest of two oaks and a pine, in the file the operation writes.
///
/// **YAML, as `world_lists.rs` reads it** (GD-03, #365): this fixture was main's `(version: 1, trees: [..])`, and no reader in this
/// build parses RON any more.
///
const FOREST: &str = "\
version: 1
trees:
- id: 0
  species: oak
  at: [3.0, 4.0]
  ground_m: 0.0
  yaw: 0.0
  scale: 1.0
  variant: 0
- id: 1
  species: oak
  at: [5.0, 4.0]
  ground_m: 0.0
  yaw: 0.0
  scale: 1.0
  variant: 1
- id: 2
  species: pine
  at: [7.0, 4.0]
  ground_m: 0.0
  yaw: 0.0
  scale: 1.0
  variant: 0
";

fn editor(tag: &str, forest: Option<&str>) -> App {
    let root = fixtures::Fixture::new(&format!("forest_list_{tag}_{}", std::process::id())).descriptor("stool", "alpha").place("stool", (0.0, 0.0)).build("m");
    if let Some(text) = forest {
        let dir = bevy_cauldron::project::Project::open(&root, None).unwrap_or_else(|e| panic!("{e}")).grimoire_dir.join("forest");
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        std::fs::write(dir.join("m.yaml"), text).unwrap_or_else(|e| panic!("forest: {e}"));
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

/// Whether any node holds exactly these words, on screen or scrolled out of sight: the rows of a list are in the world whether or not the
/// panel's first screen reaches them.
fn listed(app: &mut App, label: &str) -> bool {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).any(|text| text.0.trim() == label)
}

#[test]
fn one_press_of_the_world_chord_shows_the_maps_forest() {
    let mut app = editor("trees", Some(FOREST));
    press_the_world_chord(&mut app);
    assert_eq!(drawn(&mut app, "FOREST"), 1, "the World workspace does not show the FOREST heading");
    for row in ["3 tree(s)", "oak: 2", "pine: 1"] {
        assert!(listed(&mut app, row), "the map's forest holds `{row}` and the World workspace does not list it");
    }
}

#[test]
fn a_map_with_no_forest_says_so() {
    let mut app = editor("none", None);
    press_the_world_chord(&mut app);
    assert_eq!(drawn(&mut app, "no trees planted"), 1, "a map with no forest says nothing under FOREST");
}

#[test]
fn a_forest_file_that_does_not_read_is_said_not_shown_as_empty() {
    let mut app = editor("broken", Some("version: 1\ntrees:\n- id: [\n"));
    press_the_world_chord(&mut app);
    let world = app.world_mut();
    let said: Vec<String> = world.query::<&Text>().iter(world).map(|t| t.0.clone()).filter(|t| t.contains("forest/m.yaml did not read")).collect();
    assert_eq!(said.len(), 1, "a forest file that does not parse is not named under FOREST");
}
