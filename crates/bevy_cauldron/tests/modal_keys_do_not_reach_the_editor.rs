//! **A key that answers a modal is that modal's alone** (#286).
//!
//! Measured at the keyboard, 2026-11-04: `Esc` closing Settings also fired the editor's back-out
//! ("Leave this map?"), and answering `N` (Stay) to that question also fired *Rename the map* (`N`),
//! leaving the name field open on `_`. Both were one press read twice in one frame: once by the thing
//! that closed, once by the editor's verbs behind it.
//!
//! Each test presses the real key through `ButtonInput`, one frame down and one up, and asserts on
//! what a person would see: the question or the dialog is gone, and nothing else moved.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy_cauldron::confirm::Confirm;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::keymap_screen::{KeymapDialog, OpenKeymap};
use bevy_cauldron::settings_screen::SettingsDialog;
use bevy_gooey::MenuOpen;
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

/// The keys to hold down in the next frame, and the ones held in this one.
#[derive(Resource, Default)]
struct Script {
    next: Vec<KeyCode>,
    held: Vec<KeyCode>,
}

/// Presses what the script says, after Bevy's own input system has cleared the last frame's, so a key
/// is `just_pressed` for exactly one frame, as a hand makes it.
fn drive(mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let pressed = std::mem::take(&mut script.next);
    for key in &pressed {
        keys.press(*key);
    }
    script.held = pressed;
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("barrel", "alpha")
        .place_as("barrel@0", "barrel", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **A chord as a hand makes it**: down together for a frame, up for a frame, then a few to settle.
fn press(app: &mut App, keys: &[KeyCode]) {
    app.world_mut().resource_mut::<Script>().next = keys.to_vec();
    app.update();
    app.update();
    for _ in 0..3 {
        app.update();
    }
}

/// The platform's primary modifier, which the settings chord is bound to.
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn dirty_the_map(app: &mut App) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let mut extra = open.map.placements[0].clone();
    extra.id = "barrel@1".to_owned();
    extra.at = (2.0, 2.0);
    open.map.placements.push(extra);
    assert!(open.dirty(), "the map holds an edit the file does not");
}

/// **(a) `Esc` raises the leave question; `N` answers it, and only it.** Before the fix `N` also ran
/// the map-name field (`Action::RenameMap`), because the editor's verbs read the same key in the same
/// frame the question took it.
#[test]
fn n_that_stays_does_not_also_rename_the_map() {
    let mut app = editor("modal_keys_n");
    dirty_the_map(&mut app);
    app.world_mut().resource_mut::<EditorState>().brush = None;

    press(&mut app, &[KeyCode::Escape]);
    assert!(
        app.world().resource::<Confirm>().is_open(),
        "Esc on a dirty map with nothing armed asks whether to leave"
    );

    press(&mut app, &[KeyCode::KeyN]);
    assert!(!app.world().resource::<Confirm>().is_open(), "N answers the question");
    assert_ne!(
        app.world().resource::<keys::Live>().0,
        keys::Context::Typing,
        "the N that said Stay is not also Rename the map, whose field takes the keyboard"
    );
    assert!(app.world().resource::<OpenMap>().dirty(), "Stay keeps the edit");
}

/// **(b) `Esc` that closes Settings is not also the editor's back-out.**
#[test]
fn escape_that_closes_settings_does_not_ask_to_leave_the_map() {
    let mut app = editor("modal_keys_settings");
    app.world_mut().resource_mut::<EditorState>().brush = None;

    press(&mut app, &[primary(), KeyCode::Comma]);
    let open = app.world_mut().query::<&SettingsDialog>().iter(app.world()).count();
    assert_eq!(open, 1, "Ctrl+Comma opens Settings");

    press(&mut app, &[KeyCode::Escape]);
    let open = app.world_mut().query::<&SettingsDialog>().iter(app.world()).count();
    assert_eq!(open, 0, "Esc closes Settings");
    assert!(
        !app.world().resource::<Confirm>().is_open(),
        "the Esc that closed Settings must not also raise \"Leave this map?\""
    );
}

/// **The keymap screen closes on `Esc` the same way**: it is the other dialog that does.
#[test]
fn escape_that_closes_the_keymap_does_not_ask_to_leave_the_map() {
    let mut app = editor("modal_keys_keymap");
    app.world_mut().resource_mut::<EditorState>().brush = None;

    app.world_mut().write_message(OpenKeymap);
    for _ in 0..4 {
        app.update();
    }
    let open = app.world_mut().query::<&KeymapDialog>().iter(app.world()).count();
    assert_eq!(open, 1, "the keymap is open");

    press(&mut app, &[KeyCode::Escape]);
    let open = app.world_mut().query::<&KeymapDialog>().iter(app.world()).count();
    assert_eq!(open, 0, "Esc closes the keymap");
    assert!(
        !app.world().resource::<Confirm>().is_open(),
        "the Esc that closed the keymap must not also raise \"Leave this map?\""
    );
}

/// **`Esc` that closes a menu is not also "put the brush down"** (#287). `Esc` is the editor's
/// back-out, which peels the armed piece; a menu that closes on it took the press, and the piece in
/// hand stays in hand.
#[test]
fn escape_that_closes_a_menu_keeps_the_armed_brush() {
    let mut app = editor("modal_keys_menu");
    app.world_mut().resource_mut::<EditorState>().brush = Some(0);

    let file = find_by_label(&mut app, "File").expect("the menu bar has a File menu");
    click(&mut app, file);
    let open = |app: &mut App| app.world_mut().query::<&MenuOpen>().iter(app.world()).count();
    assert_eq!(open(&mut app), 1, "a click on File opens its menu");

    press(&mut app, &[KeyCode::Escape]);
    assert_eq!(open(&mut app), 0, "Esc closes the menu");
    assert_eq!(
        app.world().resource::<EditorState>().brush,
        Some(0),
        "the Esc that closed the menu must not also put the armed piece down"
    );
}
