//! **One chord shows the problems list** (NV-11, #397; series getting around, #377).
//!
//! `docs/ui/navigation.md` measured the PROBLEMS list at six wheel notches down the Map's inspector, against a target of one press, and
//! a route that needs the wheel has no keyboard form at all. `Cmd+Shift+X` (`Ctrl+Shift+X` off a Mac) now asks the reveal for the
//! inspector's `problems` section: the Map workspace opens if the person is elsewhere, and the heading is scrolled on screen. "On screen"
//! is the navigation measurement's own: a heading counts only when its box overlaps the box of every scroll area above it, which is what
//! scrolling changes.
//!
//! The editor is driven as a person drives it: real key events, no wheel, and a read of what is on screen.
//!
//! Owner exception (2026-11-12): a_chord_shows_the_problems_list.rs: main's new test wrote .ron input; converted to the YAML spelling.

use std::path::PathBuf;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to finish opening: a door's `OnEnter` spawns the panels.
const WARM_UP_FRAMES: usize = 10;

/// Frames after a press, so the reveal has crossed a workspace, laid the panel out and scrolled it.
const SETTLE_FRAMES: usize = 40;

/// The heading the list is under, and the row the fixture's broken relation makes.
const HEADING: &str = "PROBLEMS";
const ROW: &str = "chair faces table";

/// The platform's primary modifier, which the chord names.
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// A project with a chair that faces away from its table, and a relations file that says it must face it, so the list has a row to show.
fn project(tag: &str) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("problems_chord_{tag}_{}", std::process::id()))
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .build("m");
    let file = root.join("assets/grimoire/maps/m.relations.yaml");
    std::fs::write(&file, "version: 1\nrelations:\n- of: chair\n  kind: !Faces table\n").unwrap_or_else(|e| panic!("{file:?}: {e}"));
    root
}

/// The editor on the Map workspace, opened the way the navigation measurement opens it.
fn editor(tag: &str) -> App {
    let mut app = harness::build_headless_at(&project(tag), "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins((
        bevy_cauldron::launcher::LauncherPlugin,
        bevy_cauldron::loading::LoadingPlugin,
        bevy_cauldron::window_frame::WindowFramePlugin,
    ));
    for _ in 0..WARM_UP_FRAMES {
        std::thread::sleep(std::time::Duration::from_millis(60));
        app.update();
    }
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// One key event. No `PrimaryWindow` is spawned, on purpose (the navigation driver's own reason): a key's `window` is read by nothing on
/// the path a `ButtonInput` takes.
fn key(app: &mut App, code: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

/// **One press of the show-problems chord**: the modifiers and the letter down, then up.
fn press_the_chord(app: &mut App) {
    let keys = [primary(), KeyCode::ShiftLeft, KeyCode::KeyX];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in keys {
            key(app, code, state);
        }
        app.update();
    }
    settle(app);
}

/// A node's laid-out box in the surface's own pixels: `(centre, half extent)`.
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
fn one_press_of_the_chord_brings_the_problems_list_on_screen() {
    let mut app = editor("one_press");
    press_the_chord(&mut app);
    assert_eq!(drawn(&mut app, HEADING), 1, "{HEADING} is not on screen after the chord, so the list still needs the wheel");
    assert_eq!(drawn(&mut app, ROW), 1, "the relation the map breaks is not a row on screen after the chord");
}

#[test]
fn the_chord_works_again_after_the_person_scrolls_away() {
    let mut app = editor("again");
    press_the_chord(&mut app);
    assert_eq!(drawn(&mut app, HEADING), 1, "{HEADING} is not on screen after the first press");
    // The person scrolls the inspector back to the top, as the wheel would.
    {
        let world = app.world_mut();
        let mut panes = world.query_filtered::<&mut ScrollPosition, With<bevy_cauldron::editor::MapControlsPane>>();
        for mut scroll in panes.iter_mut(world) {
            scroll.0.y = 0.0;
        }
    }
    settle(&mut app);
    assert_eq!(drawn(&mut app, HEADING), 0, "the test could not scroll the inspector away from {HEADING}, so it proves nothing");
    press_the_chord(&mut app);
    assert_eq!(drawn(&mut app, HEADING), 1, "a second press did not bring {HEADING} back");
}

#[test]
fn the_chord_from_another_workspace_ends_on_the_map_with_the_list_on_screen() {
    let mut app = editor("elsewhere");
    let command = commands::by_id("workspace.world").unwrap_or_else(|| panic!("`workspace.world` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::World, "the World chord did not open the World workspace");
    press_the_chord(&mut app);
    settle(&mut app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "the chord left the editor on the World workspace");
    assert_eq!(drawn(&mut app, HEADING), 1, "{HEADING} is not on screen after the chord from another workspace");
}
