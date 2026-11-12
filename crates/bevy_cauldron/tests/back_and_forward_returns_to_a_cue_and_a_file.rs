//! **Back and Forward reach the cue and the file stops** (NV-06, #383; series getting around, #377).
//!
//! `back_and_forward_across_workspaces.rs` walked the route with the panels that existed. The Sound and Files workspaces exist now, and a place
//! there is more than a workspace: the cue the list marks, the file that is open and the line the caret is on. Each is measured by what a person
//! finds after the key:
//!
//! 1. Map → a cue → a file, then **Back twice**: first the cue is marked again, then the map is on screen **with the camera it had**; **Forward**
//!    walks to the cue and to the file, at the line the person left it on.
//! 2. **A place whose file is gone is skipped, with a notice**, and the walk stays where it is.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::crosslinks;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::files;
use bevy_cauldron::harness;
use bevy_cauldron::ops::Shown;
use bevy_cauldron::reveal::Revealing;
use bevy_cauldron::session::Session;
use bevy_cauldron::view::Rig;
use bevy_cauldron::workspace::Workspace;
use bevy_cauldron::world_lists::SelectedCue;
use bevy_gooey::CodeEditor;

use crate::fixtures;

/// Frames a workspace switch and its panels need.
const SETTLE: usize = 40;

/// The project's sound file.
const CUES: &str = "(cues: (cues: [(name: \"door.creak\"), (name: \"wind.loop\")]))";

/// A data file three lines long: the person leaves it on its third.
const DOOR: &str = "name: door\nopen: true\ncue: door.creak\n";

/// The file's own path, relative to the project.
const DOOR_FILE: &str = "data/door.yaml";

/// The line (0-based) the person is left on in that file.
const LEFT_ON_LINE: usize = 2;

fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("nav_stops_{tag}_{}", std::process::id()))
        .sized_descriptor("bed", "alpha", 2.0, 1.0)
        .place_as("bed@0", "bed", (2.0, 2.0))
        .side(64.0)
        .build("m");
    for (rel, text) in [("family/bevy_siren.ron", CUES), (DOOR_FILE, DOOR)] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("{path:?}"))).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    (app, root)
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn chord(app: &mut App, key: KeyCode) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).unwrap_or(Entity::PLACEHOLDER);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in [primary(), key] {
            app.world_mut().write_message(KeyboardInput {
                key_code: code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window,
            });
        }
        app.update();
    }
    settle(app);
}

fn camera(app: &App) -> (Vec3, f32) {
    let rig = app.world().resource::<Rig>();
    (rig.focus, rig.height)
}

fn on(app: &App) -> Workspace {
    *app.world().resource::<Workspace>()
}

fn cue(app: &App) -> Option<String> {
    app.world().resource::<SelectedCue>().0.clone()
}

fn active(app: &App) -> Option<String> {
    files::active_file(app.world().resource::<Session>())
}

fn caret_line(app: &mut App) -> Option<usize> {
    let world = app.world_mut();
    world.query::<&CodeEditor>().iter(world).next().map(|e| e.buffer().caret().line)
}

/// **Walk Map → a cue → a file**, leaving the person on the third line of the file.
fn walk_the_route(app: &mut App) {
    app.world_mut().resource_mut::<Revealing>().request_by_person("test", &Shown::Cue { name: "door.creak".to_owned() });
    settle(app);
    assert_eq!(on(app), Workspace::Sound, "the route's first stop is the cue");
    assert_eq!(cue(app).as_deref(), Some("door.creak"));
    crosslinks::open_in_files(app.world_mut(), DOOR_FILE, Some(LEFT_ON_LINE));
    settle(app);
    assert_eq!(on(app), Workspace::Files, "the route's second stop is the file");
    assert_eq!(active(app).as_deref(), Some(DOOR_FILE));
    assert_eq!(caret_line(app), Some(LEFT_ON_LINE), "and the caret is where the open put it");
}

/// **Back twice from the file** is the cue, marked again, and then the Map with its camera.
#[test]
fn map_to_a_cue_to_a_file_and_back_twice_restores_the_cue_and_then_the_maps_camera() {
    let (mut app, _root) = editor("route");
    {
        let mut rig = app.world_mut().resource_mut::<Rig>();
        rig.focus = Vec3::new(6.0, 0.0, 6.0);
        rig.height = 7.5;
    }
    settle(&mut app);
    let home = camera(&app);

    walk_the_route(&mut app);
    // **The mark is forgotten while the person is elsewhere**, so a Back that finds it marked again has restored it rather than found it
    // standing.
    app.world_mut().resource_mut::<SelectedCue>().0 = None;

    chord(&mut app, KeyCode::BracketLeft);
    assert_eq!(on(&app), Workspace::Sound, "one Back from the file is the cue's workspace");
    assert_eq!(cue(&app).as_deref(), Some("door.creak"), "with the cue marked again");

    chord(&mut app, KeyCode::BracketLeft);
    assert_eq!(on(&app), Workspace::Map, "the second Back reaches the Map");
    let now = camera(&app);
    assert!(
        now.0.abs_diff_eq(home.0, 1e-3) && (now.1 - home.1).abs() <= 1e-3,
        "Back twice must restore the camera the person left: was {home:?}, now {now:?}"
    );
}

/// **Forward walks the route again**, to the file and the line the person left it on.
#[test]
fn forward_returns_to_the_cue_and_then_to_the_file_at_its_line() {
    let (mut app, _root) = editor("forward");
    walk_the_route(&mut app);
    chord(&mut app, KeyCode::BracketLeft);
    chord(&mut app, KeyCode::BracketLeft);
    assert_eq!(on(&app), Workspace::Map);

    chord(&mut app, KeyCode::BracketRight);
    assert_eq!(on(&app), Workspace::Sound, "Forward from the Map is the cue");
    assert_eq!(cue(&app).as_deref(), Some("door.creak"));

    chord(&mut app, KeyCode::BracketRight);
    assert_eq!(on(&app), Workspace::Files, "and Forward again is the file");
    assert_eq!(active(&app).as_deref(), Some(DOOR_FILE));
    assert_eq!(caret_line(&mut app), Some(LEFT_ON_LINE), "at the line the person left it on, not the top");
}

/// **A file that is gone is skipped, with a notice**: the walk does not land on a tab that cannot open.
#[test]
fn a_place_whose_file_is_gone_is_skipped_with_a_notice() {
    let (mut app, root) = editor("gone");
    crosslinks::open_in_files(app.world_mut(), DOOR_FILE, Some(LEFT_ON_LINE));
    settle(&mut app);
    assert_eq!(on(&app), Workspace::Files);
    chord(&mut app, KeyCode::BracketLeft);
    assert_eq!(on(&app), Workspace::Map, "Back from the file is the map the person came from");

    std::fs::remove_file(root.join(DOOR_FILE)).unwrap_or_else(|e| panic!("{e}"));
    chord(&mut app, KeyCode::BracketRight);
    assert_eq!(on(&app), Workspace::Map, "Forward does not land on a file that is no longer there");
    let said = app.world().resource::<EditorState>().status.problem_text().to_owned();
    assert!(said.contains(DOOR_FILE) && said.contains("no longer"), "the skipped place is said: {said:?}");
}
