//! **A note's body is edited on several lines in the Notes panel, and Ctrl+Enter saves it as one `note_edit`** (UI-55, #241).
//!
//! Each note row has an Edit button. It puts a gooey text area under the list (not in it, so the list's twice-a-second rebuild cannot
//! take the field out from under a typist) holding the note's text; Enter adds a line, Ctrl+Enter ends the edit, and the final text
//! goes through the same `note_edit` operation the notes end-to-end test drives. The ground note box stays as it was (plain Enter saves).

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;
use bevy_cauldron::editor::NoteEditButton;
use bevy_cauldron::ops;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_gooey::testing;
use bevy_gooey::Role;
use bevy_grimoire_core::notes::{self, Notes, Target};

use crate::fixtures;

fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
}

fn text_areas(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<&Role>().iter(world).filter(|r| **r == Role::TextArea).count()
}

#[test]
fn ctrl_enter_in_the_panels_text_area_saves_the_note_on_two_lines_as_one_note_edit() {
    let root = fixtures::Fixture::new(&format!("note_body_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..4 {
        app.update();
    }
    let path = notes::path_for_map(&app.world().resource::<OpenMap>().map_path);
    let mut n = Notes::default();
    n.add(vec![Target::Cell(0, 0)], "first draft").unwrap();
    n.save(&path).unwrap();
    settle(&mut app);

    let button = {
        let world = app.world_mut();
        world.query::<(Entity, &NoteEditButton)>().iter(world).find(|(_, b)| b.id == "n1").map(|(e, _)| e).expect("the note has an Edit button")
    };
    app.world_mut().commands().trigger(Activate { entity: button });
    settle(&mut app);
    assert_eq!(text_areas(&mut app), 1, "Edit opens one text area");

    // The area holds the note's text; replace it.
    for _ in 0.."first draft".len() {
        testing::press(&mut app, KeyCode::Backspace);
    }
    testing::type_text(&mut app, "first line");
    testing::press(&mut app, KeyCode::Enter);
    testing::type_text(&mut app, "second line");

    // Ctrl+Enter ends the edit.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).unwrap();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput { key_code: KeyCode::Enter, logical_key: Key::Enter, state, text: None, repeat: false, window });
        app.update();
    }
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ControlLeft);
    settle(&mut app);

    let saved = Notes::load(&path).unwrap();
    assert_eq!(saved.get("n1").map(|n| n.text.clone()).as_deref(), Some("first line\nsecond line"));
    let project = app.world().resource::<Project>().clone();
    let edits = ops::read_entries(&ops::oplog_path(&project)).into_iter().filter(|e| e.op == "note_edit").count();
    assert_eq!(edits, 1, "one note_edit line in the log");
    assert_eq!(text_areas(&mut app), 0, "the area is gone once it has saved");
}
