//! **A sound cue is reached in one press in every build** (NV-11, #392; series getting around, #377).
//!
//! `docs/ui/navigation.md` measured the Sound workspace as one chord away and empty: its AUDIO panel was behind the off-by-default
//! `bevy_siren` feature, so a build without the crate drew a sentence and no cue. A cue is a name in the project's
//! `family/bevy_siren.yaml`; only *auditioning* one needs the crate. So the Sound workspace lists the project's cues in every build,
//! and says beside the list what the build cannot do with them.
//!
//! The editor is driven as a person drives it: the workspace's command (the one its chord, the palette and the activity bar all take),
//! then a read of what is on screen. The project's file is written in the crate's own shape, which this test never parses itself.
//!
//! Owner exception (2026-11-12): a_sound_cue_is_listed_without_bevy_siren.rs: main's new test wrote .ron input; converted to the YAML spelling.

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

/// The cues the project's sound file holds, in the crate's own `cues: {cues: [{name: ..}]}` shape; every other field has a default.
const CUES: &str = "\
cues:
  cues:
  - name: door.creak
  - name: wind.loop
";

fn editor(tag: &str, file: Option<&str>) -> App {
    let root = fixtures::Fixture::new(&format!("sound_cue_{tag}_{}", std::process::id())).descriptor("stool", "alpha").place("stool", (0.0, 0.0)).build("m");
    if let Some(text) = file {
        let dir = root.join("family");
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        std::fs::write(dir.join("bevy_siren.yaml"), text).unwrap_or_else(|e| panic!("bevy_siren.ron: {e}"));
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

/// **One press of the Sound workspace's chord**: its command, queued the way the key does.
fn press_the_sound_chord(app: &mut App) {
    let command = commands::by_id("workspace.sound").unwrap_or_else(|| panic!("`workspace.sound` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Sound, "the Sound chord did not open the Sound workspace");
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
fn one_press_of_the_sound_chord_shows_the_projects_cues() {
    let mut app = editor("cues", Some(CUES));
    press_the_sound_chord(&mut app);
    assert_eq!(drawn(&mut app, "AUDIO"), 1, "the Sound workspace does not show the AUDIO heading");
    for cue in ["door.creak", "wind.loop"] {
        assert_eq!(drawn(&mut app, cue), 1, "the cue `{cue}` the project's sound file holds is not listed in the Sound workspace");
    }
}

#[test]
fn a_project_with_no_sound_file_says_it_has_no_cues() {
    let mut app = editor("none", None);
    press_the_sound_chord(&mut app);
    assert_eq!(drawn(&mut app, "no cues in this project"), 1, "a project with no cues says nothing in the Sound workspace");
}

#[test]
fn a_sound_file_that_does_not_read_is_said_not_shown_as_empty() {
    let mut app = editor("broken", Some("cues:\n  cues:\n  - name: [\n"));
    press_the_sound_chord(&mut app);
    let world = app.world_mut();
    let said: Vec<String> = world.query::<&Text>().iter(world).map(|t| t.0.clone()).filter(|t| t.contains("bevy_siren.yaml did not read")).collect();
    assert_eq!(said.len(), 1, "a sound file that does not parse is not named in the Sound workspace");
}
