//! **A right-click on a piece can also pick it up to move it, or leave a note on it** (UI-36, #230).
//!
//! Move and Note are the two verbs that are tools, not one-shot operations: choosing Move arms the move
//! tool with this piece already in hand (the next click puts it down, one `placement_move`), and Note
//! arms the Note tool with the note box open on this piece, so the person types and Enter saves a
//! `note_add` about that piece. Both are the keys' own tools, entered on the piece the menu was for.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState, InputSystems};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use bevy_gooey::{ContextTarget, MenuItem, MenuPanel};
use bevy_grimoire_core::notes;
use bevy_cauldron::editor::{EditorState, Held, MoveDrag, NoteBox, Placement, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

/// One frame's worth of person: keys held, the pointer, and a press or release of one mouse button.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<(MouseButton, bool)>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

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
        Some((button, true)) => mouse.press(button),
        Some((button, false)) => mouse.release(button),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

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
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// A right press and release over one spot: one right-click.
fn right_click(app: &mut App, at: Vec2) {
    for down in [true, false] {
        step(
            app,
            Frame {
                pointer: Some(at),
                mouse: Some((MouseButton::Right, down)),
                ..default()
            },
        );
    }
    app.update();
}

/// The piece the open context menu is for, if a menu is open.
fn menu_for(app: &mut App) -> Option<String> {
    let mut open = app
        .world_mut()
        .query_filtered::<&ContextTarget, With<MenuPanel>>();
    let target = open.iter(app.world()).next()?.0;
    app.world().get::<Placement>(target).map(|p| p.0.clone())
}

/// The menu item whose words are `label`: the entity an `Activate` goes to.
fn item(app: &mut App, label: &str) -> Entity {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text, &ChildOf)>();
    let mut items = world.query_filtered::<(), With<MenuItem>>();
    let mut found = Vec::new();
    for (_, text, parent) in texts.iter(world) {
        if text.0 == label && items.get(world, parent.parent()).is_ok() {
            found.push(parent.parent());
        }
    }
    assert_eq!(found.len(), 1, "one menu item says `{label}`: {found:?}");
    found[0]
}


#[test]
fn moving_from_the_menu_picks_that_piece_up_and_the_next_click_puts_it_down() {
    let mut app = editor("piece_menu_move");
    let over_second = pixel_over(&mut app, (3.0, 0.0));
    right_click(&mut app, over_second);
    let mv = item(&mut app, "Move");
    app.world_mut().commands().trigger(Activate { entity: mv });
    app.update();
    app.update();
    assert_eq!(menu_for(&mut app), None, "choosing an item closes the menu");
    assert_eq!(app.world().resource::<EditorState>().tool, Tool::Move, "Move arms the move tool");
    assert_eq!(
        app.world().resource::<MoveDrag>().held,
        Some(Held::Piece("site/wall@1".into())),
        "the piece the menu was for is already in hand, with no first click"
    );

    let there = pixel_over(&mut app, (9.0, 0.0));
    for down in [true, false] {
        step(&mut app, Frame { pointer: Some(there), mouse: Some((MouseButton::Left, down)), ..default() });
    }
    app.update();
    app.update();
    let at: Vec<(f32, f32)> = app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.at).collect();
    assert_eq!(at[0], (0.0, 0.0), "the other piece did not move");
    assert!(at[1].0 > 6.0, "the piece came down where the click was, not where it stood: {at:?}");
    assert_eq!(app.world().resource::<MoveDrag>().held, None, "the drop empties the hand");
}

#[test]
fn noting_from_the_menu_opens_the_note_box_on_that_piece_and_enter_saves_a_note_about_it() {
    let mut app = editor("piece_menu_note");
    let over_second = pixel_over(&mut app, (3.0, 0.0));
    right_click(&mut app, over_second);
    let note = item(&mut app, "Note");
    app.world_mut().commands().trigger(Activate { entity: note });
    app.update();
    app.update();
    assert_eq!(app.world().resource::<EditorState>().tool, Tool::Note, "Note arms the Note tool");
    let draft = app.world().resource::<NoteBox>().draft.clone().expect("the note box is open");
    assert_eq!(draft.pieces, ["site/wall@1"], "the note is about the piece the menu was for");

    for (ch, code) in [('d', KeyCode::KeyD), ('o', KeyCode::KeyO)] {
        tap(&mut app, Key::Character(ch.to_string().into()), code);
    }
    tap(&mut app, Key::Enter, KeyCode::Enter);
    assert!(app.world().resource::<NoteBox>().draft.is_none(), "Enter saves and closes the box");
    let saved = notes::Notes::load(&notes::path_for_map(&app.world().resource::<OpenMap>().map_path))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(saved.notes.len(), 1, "one note: {:?}", saved.notes);
    assert_eq!(saved.notes[0].text, "do");
    assert_eq!(saved.notes[0].site.as_ref().expect("a site").pieces, ["site/wall@1"]);
    assert_eq!(log(&app).iter().filter(|e| e.op == "note_add").count(), 1, "one registry line");
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
