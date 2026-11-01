//! **A person removes a piece with the mouse, and Cmd+Z puts it back, through the one undo stack.**
//!
//! The removal click is the person's path: `X` arms removal mode, a press and a release over a piece
//! remove it. Before the registry carried the removal, that click deleted the piece without a line
//! in the operation log, so the log could not say what the person had done and Cmd+Z had no
//! recorded operation to take back. This drives the real `keys` and `drive_removal` systems.
//!
//! The pointer and the mouse button are injected in `PreUpdate`, after `InputSystems`. The window's
//! cursor is set there too, so `view::sense_pointer` reads the same pointer in `Update` whatever
//! order the unset `drive_removal` runs in. Keys go through `ButtonInput<KeyCode>`, which the key
//! census reads.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

/// One frame's worth of person: the keys held down, the pointer, and a mouse press or release.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

/// The next frame to play. `held` is what the previous frame pressed, released at the start of the
/// next so no key is left down by accident.
#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

/// Runs in `PreUpdate` after `InputSystems`, so the presses survive into `Update` and the window's
/// cursor is already where the person put it when `sense_pointer` reads it.
fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

/// Where on the screen a person would point to hit the ground at a map-space spot.
fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .single(app.world())
        .expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the piece is not on screen: {e:?}"))
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn ids(map: &bevy_grimoire_core::map::Map) -> Vec<String> {
    map.placements.iter().map(|p| p.id.clone()).collect()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

#[test]
fn a_person_removes_a_piece_with_the_mouse_and_cmd_z_puts_it_back() {
    let root = fixtures::Fixture::new("person_remove")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    // A window with a cursor, so `sense_pointer` reads the injected position. Headless, nothing
    // moves the OS pointer: this entity is only read.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1"],
        "the fixture starts with two"
    );

    let over_first = pixel_over(&mut app, (0.0, 0.0));

    // **X arms removal mode.** The keyboard does nothing else here.
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyX],
            ..default()
        },
    );
    assert!(
        matches!(app.world().resource::<EditorState>().tool, Tool::Remove),
        "X must arm removal mode"
    );

    // **A click on the first piece removes it.** Press, then release, over the same spot.
    step(
        &mut app,
        Frame {
            pointer: Some(over_first),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(over_first),
            mouse: Some(false),
            ..default()
        },
    );
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "the click took the piece it was over"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@1"],
        "the file agrees with the editor"
    );

    // **The click went through the operation log, as the person's own removal.**
    let removal = log(&app)
        .into_iter()
        .find(|e| e.op == "placement_remove")
        .expect("the person's click is recorded as a placement_remove line");
    assert!(
        matches!(removal.source, ops::Source::Human),
        "a click is the person's, not an agent's: {removal:?}"
    );
    assert_eq!(removal.outcome, "applied", "{removal:?}");
    assert_eq!(removal.params["id"], "site/wall@0", "{removal:?}");

    // **Cmd+Z takes the removal back through the unified stack.**
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1"],
        "Cmd+Z puts the piece back in the editor, at the row it held"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@0", "site/wall@1"],
        "Cmd+Z puts the piece back in the file, at the row it held"
    );
    assert!(
        log(&app).iter().any(
            |e| e.undoes.as_deref() == Some(removal.operation_id.as_str())
                && e.outcome == "applied"
        ),
        "the undo is a logged line that names the removal it took back"
    );

    // **Cmd+Shift+Z puts the removal back.** The redo is the same stack, so the piece leaves again.
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::ShiftLeft, KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "Cmd+Shift+Z takes the piece out of the editor again"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@1"],
        "Cmd+Shift+Z takes the piece out of the file again"
    );
}

/// Through the registered method, as an agent's request goes.
fn call(app: &mut App, method: &str, parameters: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(parameters))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// A press and a release over the same spot: one click.
fn click(app: &mut App, at: Vec2) {
    step(
        app,
        Frame {
            pointer: Some(at),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(at),
            mouse: Some(false),
            ..default()
        },
    );
}

/// **An agent's undo of its own removal waits while the person has moved a piece since.**
///
/// The person's move is recorded by row number (`Undo::Moved` names each piece's index), and an
/// undo of a removal puts a row back, so every later row moves along with it. The move is not in
/// the operation log, so the log alone cannot see it: the undo is refused on the editor's own
/// stack. Once the person takes the move back, the same undo is allowed again.
#[test]
fn an_agents_undo_of_a_removal_waits_while_the_person_has_moved_a_piece_since() {
    let root = fixtures::Fixture::new("agent_undo_after_person_move")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .place("site/wall", (6.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }

    // **The agent removes the first piece.** Rows are now [@1, @2].
    let removed = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": {"id": "site/wall@0"}, "dry_run": false}),
    );
    assert_eq!(removed["outcome"], "applied", "{removed}");
    assert_eq!(ids(&held(&app)), ["site/wall@1", "site/wall@2"]);
    let removal = log(&app)
        .into_iter()
        .find(|e| e.op == "placement_remove")
        .expect("the agent's removal is in the log");

    // **The person picks up the last piece and sets it down clear of the others.** B arms move mode.
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyB],
            ..default()
        },
    );
    let on_last = pixel_over(&mut app, (6.0, 0.0));
    let clear = pixel_over(&mut app, (9.0, 0.0));
    click(&mut app, on_last);
    click(&mut app, clear);
    assert!(
        held(&app)
            .placements
            .iter()
            .any(|p| p.id == "site/wall@2" && p.at.0 > 7.0),
        "the person moved @2 to the right"
    );

    // **The agent's undo would put @0 back at row 0 and push the person's moved piece along.** Refused.
    let refused = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": null, "dry_run": false}),
    );
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains(&removal.operation_id),
        "the refusal names the removal it would undo: {refused}"
    );
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1", "site/wall@2"],
        "a refused undo changes nothing in the editor"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@1", "site/wall@2"],
        "nor the file"
    );

    // **Once the person takes the move back, the newest thing on their stack is the removal again.**
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    let allowed = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": null, "dry_run": false}),
    );
    assert_eq!(allowed["outcome"], "applied", "{allowed}");
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "with nothing row-recorded above it, the removal's undo goes through"
    );
}
