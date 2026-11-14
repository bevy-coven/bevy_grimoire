//! **Shift+Cmd+J toggles the terminal between the whole window and its default height** (#426; the owner: *"Hitting Shift+Cmd+J should open
//! the terminal to max, covering the whole screen. Hitting it again toggles it back to the default size."*).
//!
//! - the pane's height equals the window's below the menu bar, and the next press gives back the height it had;
//! - with the pane collapsed it raises the pane straight to full;
//! - Cmd+J while full lowers the pane, and the next raise returns it to its normal height;
//! - the terminal program is told the new size, as on any resize;
//! - it is the command `terminal.maximise`: a View menu row, a keymap row, and a chord no command can be rebound onto.

use std::collections::BTreeMap;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::app_verbs::AppVerb;
use bevy_cauldron::keymap_screen::{self, OpenKeymap};
use bevy_cauldron::menu_bar::EditorMenuBar;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalPane, TerminalPaneRoot, maximise_chord};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    settle(&mut app, 12);
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn window(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window")
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code,
        text: match (&logical_key, state) {
            (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
            _ => None,
        },
        logical_key,
        state,
        repeat: false,
        window,
    });
}

/// **The platform's primary modifier, optionally Shift, and J**, as a hand makes it.
fn chord_j(app: &mut App, shift: bool) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    let primary = bevy_cauldron::keys::MOD_KEYS[0];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        if shift {
            send(app, KeyCode::ShiftLeft, Key::Shift, state);
        }
        send(app, primary, unidentified(), state);
        send(app, KeyCode::KeyJ, Key::Character("j".into()), state);
        app.update();
    }
    settle(app, 4);
}

/// The pane's laid-out height and the room below the menu bar, both in logical px, and whether the pane is drawn.
fn heights(app: &mut App) -> (f32, f32, bool) {
    let world = app.world_mut();
    let (pane, node, parent) = world
        .query_filtered::<(&ComputedNode, &Node, &ChildOf), With<TerminalPaneRoot>>()
        .iter(world)
        .next()
        .map(|(computed, node, parent)| (computed.size().y * computed.inverse_scale_factor(), node.display != Display::None, parent.parent()))
        .expect("the terminal's pane");
    let root = world.get::<ComputedNode>(parent).map_or(0.0, |root| root.size().y * root.inverse_scale_factor());
    let menu = world.query_filtered::<&ComputedNode, With<EditorMenuBar>>().iter(world).next().map_or(0.0, |menu| menu.size().y * menu.inverse_scale_factor());
    (pane, root - menu, node)
}

fn rows(app: &App) -> u16 {
    app.world().resource::<TerminalPane>().cells().1
}

/// **Pressing Shift+Cmd+J makes the pane's height the window's below the menu bar, and pressing it again restores the previous height**;
/// from collapsed it raises straight to full; Cmd+J while full lowers it and the next raise is the normal height; the program is told the new size.
#[test]
fn shift_primary_j_covers_the_window_below_the_menu_bar_and_again_gives_the_height_back() {
    let mut app = editor("maximise");
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    settle(&mut app, 8);
    let (normal, room, drawn) = heights(&mut app);
    assert!(drawn && normal > 0.0 && normal < room, "the pane is raised at its normal height {normal} inside {room}");
    let normal_rows = rows(&app);

    chord_j(&mut app, true);
    let (full, room, _) = heights(&mut app);
    assert!((full - room).abs() <= 1.0, "maximised, the pane's height {full} is the window's below the menu bar {room}");
    assert!(app.world().resource::<Terminal>().is_maximised());
    assert!(rows(&app) > normal_rows, "the terminal program is told the new size: {} rows, was {normal_rows}", rows(&app));

    chord_j(&mut app, true);
    let (back, _, _) = heights(&mut app);
    assert!((back - normal).abs() <= 1.0, "pressed again, the pane is {back} tall, the height it had ({normal})");
    assert_eq!(rows(&app), normal_rows, "and the program is told that size too");

    // Maximised, Cmd+J lowers the pane; the next raise returns the normal height, not full.
    chord_j(&mut app, true);
    chord_j(&mut app, false);
    assert!(!heights(&mut app).2, "Cmd+J while full lowers the pane");
    chord_j(&mut app, false);
    let (raised, _, drawn) = heights(&mut app);
    assert!(drawn && (raised - normal).abs() <= 1.0, "the next raise is the normal height {normal}, not full: {raised}");
}

/// **With the pane collapsed it raises the pane straight to full.**
#[test]
fn from_collapsed_shift_primary_j_raises_the_pane_straight_to_full() {
    let mut app = editor("maximise_from_collapsed");
    assert!(app.world().resource::<Terminal>().is_collapsed());
    chord_j(&mut app, true);
    let (full, room, drawn) = heights(&mut app);
    assert!(drawn, "the pane is raised");
    assert!((full - room).abs() <= 1.0, "straight to full: {full} of {room}");
}

/// **It is a command: a View menu row with its chord, a keymap row, and a chord no command can be rebound onto.**
#[test]
fn terminal_maximise_is_a_view_row_a_keymap_row_and_a_reserved_chord() {
    let mut app = editor("maximise_command");
    let verbs: Vec<(String, Option<bevy_gooey::Shortcut>)> = {
        let world = app.world_mut();
        world.query::<&AppVerb>().iter(world).filter(|verb| **verb == AppVerb::MaximiseTerminal).map(|verb| (verb.title(), verb.shortcut())).collect()
    };
    assert_eq!(verbs, [("Maximise terminal".to_owned(), Some(maximise_chord()))], "the View menu has its row, with the chord");

    app.world_mut().write_message(OpenKeymap);
    settle(&mut app, 6);
    let listed = {
        let world = app.world_mut();
        world.query::<&Text>().iter(world).any(|text| text.0.contains("terminal.maximise") && text.0.contains(&maximise_chord().label()))
    };
    assert!(listed, "the keymap lists terminal.maximise with its chord");

    let file = std::env::temp_dir().join(format!("terminal_maximise_keymap_{}/settings.yaml", std::process::id()));
    let refused = keymap_screen::rebind(&file, "file.save", Some(maximise_chord()), &BTreeMap::new(), None);
    let why = refused.expect_err("a command cannot be rebound onto the maximise chord");
    assert!(why.contains("terminal.maximise"), "the refusal names it: {why}");
    assert!(!file.exists(), "and nothing was saved");
}
