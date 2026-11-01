//! **A second Shift-click adds a piece to the open note** (#173).
//!
//! The Note tool's first Shift-click on a piece opens the note box on that piece, and the status says
//! "Shift-click more". The box counts as a text field (`Fields::typing`), and `drive_note` ran only
//! while nothing was typing, so every later Shift-click was dropped and the note was saved on the
//! first piece alone. Measured walking #95 on a scratch copy of Sanctuary's town.
//!
//! The person route, headless: Shift+N, a Shift-click on each of two pieces, a word, Enter. The
//! pointer and keys are injected the way `a_person_removes_a_box_and_undoes_it.rs` does it.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_grimoire_core::notes::{self, Notes};
use bevy_cauldron::editor::{EditorState, NoteBox, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
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
    let mut cameras = app.world_mut().query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

/// A Shift-click: Shift held with the press, and still held with the release.
fn shift_click(app: &mut App, at: Vec2) {
    for mouse in [true, false] {
        step(app, Frame { keys: vec![KeyCode::ShiftLeft], pointer: Some(at), mouse: Some(mouse) });
    }
}

/// A key typed into the note box: its press and release, as the window sends them.
fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

fn picked(app: &App) -> Vec<String> {
    app.world().resource::<NoteBox>().draft.as_ref().map(|d| d.pieces.clone()).unwrap_or_default()
}

#[test]
fn two_shift_clicks_leave_one_note_on_both_pieces_and_not_the_third() {
    let root = fixtures::Fixture::new("note_two_shift_clicks")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 5.0))
        .place("site/wall", (6.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    let first = pixel_over(&mut app, (0.0, 0.0));
    let second = pixel_over(&mut app, (6.0, 0.0));

    // **Shift+N arms the Note tool.**
    step(&mut app, Frame { keys: vec![KeyCode::ShiftLeft, KeyCode::KeyN], ..default() });
    step(&mut app, Frame::default());
    assert!(matches!(app.world().resource::<EditorState>().tool, Tool::Note), "Shift+N must arm the Note tool");

    shift_click(&mut app, first);
    assert_eq!(picked(&app), ["site/wall@0"], "the first Shift-click opens the box on that piece");

    shift_click(&mut app, second);
    assert_eq!(
        picked(&app),
        ["site/wall@0", "site/wall@2"],
        "a second Shift-click with the box open adds its piece, as the status line promises"
    );

    for (ch, code) in [('s', KeyCode::KeyS), ('w', KeyCode::KeyW), ('a', KeyCode::KeyA), ('p', KeyCode::KeyP)] {
        tap(&mut app, Key::Character(ch.to_string().into()), code);
    }
    tap(&mut app, Key::Enter, KeyCode::Enter);
    assert!(app.world().resource::<NoteBox>().draft.is_none(), "Enter saves and closes the box");

    let path = notes::path_for_map(&app.world().resource::<OpenMap>().map_path);
    let saved = Notes::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(saved.notes.len(), 1, "one note: {:?}", saved.notes);
    assert_eq!(saved.notes[0].text, "swap");
    let mut pieces = saved.notes[0].site.as_ref().expect("a note on pieces has a site").pieces.clone();
    pieces.sort();
    assert_eq!(pieces, ["site/wall@0", "site/wall@2"], "the saved note holds both picked pieces and not the third");
}
