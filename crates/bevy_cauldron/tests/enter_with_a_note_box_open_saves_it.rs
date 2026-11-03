//! **`Enter` with a note box open saves the note, and never also sends every draft** (#269).
//!
//! `type_note` reads `Enter` two ways: with a draft open it saves that draft; with no draft open and
//! the Note tool armed it sends every draft (`send_notes`). The two are one key in one frame, so the
//! question this file answers is whether a press can be read as both — which would save the note *and*
//! run `note_send`, and the person would see `cannot send the drafts` over the receipt for a note that
//! did save, or the reverse.
//!
//! The bug was reported from a recording (#269): two takes of the note clip turned `Enter` into
//! `cannot send the drafts: notes: there are no drafts to send` while a box was open. This file is the
//! headless statement of what that key must do, so the reason is a failing test rather than a
//! screenshot.
//!
//! **The second half is the one a capture cannot see.** A refused `note_send` raises a *problem*,
//! which sticks on the status line until Esc; a saved note is a *note*. So the assertion is on both:
//! the note reached the file, and no problem was raised.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_grimoire_core::notes::{self, Notes};
use bevy_cauldron::editor::{EditorState, NoteBox, NoteDraft, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Applied, Ctx};
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

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

/// Runs in `PreUpdate` after `InputSystems`, so the presses survive into `Update` and the window's
/// cursor is where the person put it when the pointer is read.
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

/// **A draft open on a rectangle: one `Enter` saves it, and no send is raised.**
///
/// The draft is put in place directly rather than by dragging, because the drag needs a camera ray
/// through a window and this file is about the key, not the pointer.
#[test]
fn enter_with_a_note_box_open_saves_it_and_never_sends_every_draft() {
    let root = fixtures::Fixture::new("note_enter_collision")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..6 {
        app.update();
    }

    // The Note tool armed, and a draft open on two cells that a person dragged.
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Note;
    app.world_mut().resource_mut::<NoteBox>().draft = Some(NoteDraft {
        area: [0, 0, 1, 0],
        editing: None,
        text: "a reading corner here".to_owned(),
        pieces: Vec::new(),
        auto: false,
    });
    app.update();

    tap(&mut app, Key::Enter, KeyCode::Enter);

    assert!(
        app.world().resource::<NoteBox>().draft.is_none(),
        "Enter saves the draft and closes the box"
    );
    assert!(
        app.world().resource::<EditorState>().status.problems().is_empty(),
        "saving a note is a receipt, not a refusal: the status holds {:?}",
        app.world().resource::<EditorState>().status.problems()
    );

    let open = app.world().resource::<OpenMap>().clone();
    let path = notes::path_for_map(&open.map_path);
    let saved = Notes::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(saved.notes.len(), 1, "the note reached the map's notes file");
    assert_eq!(saved.notes[0].text, "a reading corner here");
    assert!(
        saved.notes[0].is_draft(),
        "a new note is a draft until Send, so one Enter did not also run note_send"
    );
}

/// **And with no box open the same key still sends**, which is the half the fix must not remove.
///
/// `Enter` on the Note tool with nothing in hand is the panel's Send, and the code says so in
/// `type_note`. A guard that made `Enter` inert while the tool was armed would fix the collision by
/// deleting a documented behaviour, so the pair is asserted together.
#[test]
fn enter_with_no_note_box_open_still_sends_the_drafts() {
    let root = fixtures::Fixture::new("note_enter_send_kept")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..6 {
        app.update();
    }
    // One draft in the file, so sending has something to do.
    let open = app.world().resource::<OpenMap>().clone();
    let mut ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(open.clone()),
        session: "test".to_owned(),
        ..Ctx::default()
    };
    let added = ops::dispatch(
        &mut ctx,
        "note_add",
        &serde_json::json!({"cells": [0, 0, 1, 0], "text": "for the agent"}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(added, Applied::Done { .. }));
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Note;
    app.update();

    tap(&mut app, Key::Enter, KeyCode::Enter);

    let path = notes::path_for_map(&open.map_path);
    let saved = Notes::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        saved.notes.iter().all(|n| !n.is_draft()),
        "Enter with the tool armed and no box open sends: {:?}",
        saved.notes.iter().map(|n| n.state.clone()).collect::<Vec<_>>()
    );
}

/// **A Shift-click that picks no piece says so, instead of promising `Enter saves` over nothing.**
///
/// The recording failure behind #269 ran this way: the Shift-click landed on empty ground, the pick
/// found no piece, and `on_note_drag` returned **without a word** — so the box never opened, the typed
/// words went to the status line, and the next `Enter` took the documented no-box branch and sent
/// drafts. The receipt the person was following (`Enter saves`) had already gone.
///
/// The crate's rule is that a key which does nothing is the defect and guidance is a note
/// (`build.rs`: *"an arrow that answers nothing at all is the defect"*). So an empty pick is a note
/// that names the way it does work: the rectangle.
#[test]
fn a_shift_click_that_picks_no_piece_says_what_to_do_instead() {
    let root = fixtures::Fixture::new("note_pick_nothing_says")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..6 {
        app.update();
    }
    app.init_resource::<Script>();
    app.add_systems(PreUpdate, drive.after(InputSystems));
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Note;
    app.update();

    // Where the ground is empty: the fixture has one piece at (0, 0) and the map runs to 32 m.
    //
    // **Six metres out, not twelve** (UI-30, #224). The spot has to be inside the camera's viewport
    // as well as empty, and (12, 12) falls at y = 719 px against a hole that ends at 689 — it was on
    // screen only while the viewport was the whole window and nothing was docked beside it. The
    // claim this line makes is unchanged: an empty patch of ground, on screen, that no piece stands on.
    let empty = pixel_over(&mut app, (0.0, 6.0));
    for mouse in [true, false] {
        step(&mut app, Frame { keys: vec![KeyCode::ShiftLeft], pointer: Some(empty), mouse: Some(mouse) });
    }

    let box_ = app.world().resource::<NoteBox>();
    assert!(box_.draft.is_none(), "nothing was picked, so no box opened");
    let said = app.world().resource::<EditorState>().status.line().to_owned();
    assert!(
        said.contains("rectangle") || said.contains("drag"),
        "a Shift-click that picks nothing must say the way that works (the drag), not stay silent; \
         the line was {said:?}"
    );
}
