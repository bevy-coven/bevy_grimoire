//! **Command+K clears the terminal, as in VS Code on a mac; Ctrl+K stays the shell's** (#326).
//!
//! The chord is on the Command key (Super off a mac), never the primary one, so Ctrl+K reaches the shell
//! as its kill-line everywhere. Clearing keeps the prompt's line as the first line and drops everything
//! above it, as xterm.js's `clear()` does for VS Code.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::TerminalFocus;

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Lines of output, so there is history to clear.
const PRINTED: usize = 300;

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

/// A modifier held round K, as a hand makes the chord.
fn chord_k(app: &mut App, modifier: KeyCode) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, modifier, unidentified(), state);
        send(app, KeyCode::KeyK, Key::Character("k".into()), state);
        app.update();
    }
    settle(app, 3);
}

fn history(app: &App) -> usize {
    app.world().resource::<Terminal>().tab().expect("a shell is open").screen().scrolled_lines()
}

fn with_history(tag: &str) -> App {
    let mut app = editor_with_terminal(tag);
    app.world_mut().resource_mut::<TerminalFocus>().take();
    let text: String = (1..=PRINTED).map(|n| format!("{n}\r\n")).collect();
    feed(&mut app, text.as_bytes());
    settle(&mut app, 3);
    assert!(history(&app) > 0, "the output left no history, so this test proves nothing");
    app
}

#[test]
fn command_k_clears_the_scrollback() {
    let mut app = with_history("command_k_clears");
    chord_k(&mut app, KeyCode::SuperLeft);
    assert_eq!(history(&app), 0, "Command+K left the scrollback where it was");
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    let screen = terminal.tab_mut().expect("a shell is open").screen_mut();
    screen.lines();
    assert_eq!(screen.cursor().row, 0, "the prompt's line is the first line after a clear, as VS Code leaves it");
}

#[test]
fn ctrl_k_is_the_shells_and_clears_nothing() {
    let mut app = with_history("ctrl_k_is_the_shells");
    let before = history(&app);
    chord_k(&mut app, KeyCode::ControlLeft);
    assert_eq!(history(&app), before, "Ctrl+K cleared the terminal: it is the shell's kill-line, not the window's");
}
