//! **Command chords are the terminal's, not the shell's letters** (feedback-1794024642593, oracle
//! `other:key-routing`).
//!
//! Reported on mac_air, 0.4.1, the projects home at 2560x1406 (scale 2, `ui_scale` 2.4): *"I can't hit Cmd+A to
//! select all and Cmd+V. We need a better way for the terminal to capture key strokes."* Reproduced on the installed
//! 0.4.1: with text on the clipboard, Cmd+V pasted nothing (Cmd+Shift+V did), and Cmd+A selected nothing.
//!
//! The oracle is the person's: with the shell holding the keys, Command+A selects everything the terminal shows,
//! and a Command chord never reaches the shell as its letter (a Mac keyboard reports Cmd+Z as a `z`). The shell is
//! `cat -v`, which prints every byte it is sent, so a leaked letter is on screen. Command is the Super key here,
//! which is what `bevy_gooey::Modifiers::META` names on every platform. Paste itself reads the system clipboard,
//! which a headless run does not have; its chord is proved by `terminal_view::tests`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::screen::Screen;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::TerminalFocus;

use crate::fixtures;

/// How many 50 ms checks a wait makes.
const TRIES: usize = 80;
/// Frames for a screen change to tear one screen down and build the next.
const SCREEN_FRAMES: usize = 6;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The application on the projects home, the capture's screen, with its terminal's shell running.
fn the_home() -> App {
    let root = fixtures::Fixture::new(&format!("capture_1794024642593_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.add_plugins((LauncherPlugin, bevy_cauldron::loading::LoadingPlugin));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(1280, 703), ..default() },
        PrimaryWindow,
    ));
    settle(&mut app, 10);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app, SCREEN_FRAMES);
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window");
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

/// A letter as a keyboard reports it: its key and its character.
fn letter(c: char) -> (KeyCode, Key) {
    let code = match c {
        'a' => KeyCode::KeyA,
        'c' => KeyCode::KeyC,
        'k' => KeyCode::KeyK,
        'o' => KeyCode::KeyO,
        'z' => KeyCode::KeyZ,
        _ => KeyCode::Space,
    };
    (code, Key::Character(c.to_string().as_str().into()))
}

/// Text typed a key at a time.
fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        let (code, key) = letter(c);
        for state in [ButtonState::Pressed, ButtonState::Released] {
            send(app, code, key.clone(), state);
            app.update();
        }
    }
}

/// **Command and a letter, as a Mac keyboard sends them**: Command down, the letter with its character, then
/// both up.
fn command(app: &mut App, c: char) {
    let (code, key) = letter(c);
    let modifier = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::SuperLeft, modifier(), state);
        send(app, code, key.clone(), state);
        app.update();
    }
    settle(app, 2);
}

fn enter(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::Enter, Key::Enter, state);
        app.update();
    }
}

fn screen_text(app: &App) -> String {
    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        return String::new();
    };
    tab.screen()
        .built_lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        settle(app, 2);
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}. The shell shows: {:?}", screen_text(app));
}

#[test]
fn command_a_selects_the_terminal_and_no_command_chord_reaches_the_shell() {
    let mut app = the_home();
    until(&mut app, "the home's terminal never started a shell", |app| screen_text(app).contains('$'));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 2);
    type_text(&mut app, "cat -v");
    enter(&mut app);
    until(&mut app, "`cat -v` was never echoed", |app| screen_text(app).contains("cat -v"));
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Cmd+Z and Cmd+C between two plain letters: `cat -v` prints the line it was sent when Enter arrives.
    type_text(&mut app, "o");
    command(&mut app, 'z');
    command(&mut app, 'c');
    type_text(&mut app, "k");
    enter(&mut app);
    until(&mut app, "`cat -v` never printed the typed line", |app| {
        screen_text(app).lines().filter(|line| line.trim_end().ends_with('k')).count() >= 2
    });
    let printed: Vec<String> = screen_text(&app).lines().map(|l| l.trim_end().to_owned()).filter(|l| l.ends_with('k')).collect();
    assert!(
        printed.iter().all(|line| line == "ok"),
        "a Command chord reached the shell as its letter: `cat -v` was sent {printed:?}, where a person typed o, \
         Cmd+Z, Cmd+C, k"
    );

    command(&mut app, 'a');
    let selected = app.world().resource::<Terminal>().tab().and_then(|tab| tab.screen().selection_text());
    let selected = selected.unwrap_or_else(|| panic!("Cmd+A selected nothing; the terminal shows {:?}", screen_text(&app)));
    assert!(
        selected.contains("cat -v") && selected.lines().any(|line| line == "ok"),
        "Cmd+A selects everything the terminal shows, the command and its output: {selected:?}"
    );
}
