//! **While the shell has the keyboard, a Ctrl chord is the shell's** (#318).
//!
//! Ctrl+D is end-of-file to a shell — how a person ends a `cat` or leaves the shell — and off a mac it
//! is also the editor's report chord (`report::developers_chord`, primary+D), which opened a report over
//! the shell as the shell read the EOF. Driven through the real key path: `cat` runs in the pane's
//! shell, Ctrl+D is pressed, and the test asks both sides what they did with it.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::report::ReportEdit;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::TerminalFocus;

use crate::fixtures;

/// How many 50 ms checks a test waits for the shell.
const TRIES: usize = 80;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() },
        PrimaryWindow,
    ));
    for _ in 0..10 {
        app.update();
    }
    app
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

fn tap(app: &mut App, key_code: KeyCode, logical_key: Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, key_code, logical_key.clone(), state);
        app.update();
    }
}

fn type_line(app: &mut App, line: &str) {
    for c in line.chars() {
        let s = c.to_string();
        tap(app, KeyCode::Space, Key::Character(s.as_str().into()));
    }
    tap(app, KeyCode::Enter, Key::Enter);
}

/// **Ctrl+D as a hand makes it**: the letter carries its character, as a keyboard reports it.
fn ctrl_d(app: &mut App) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::ControlLeft, unidentified(), state);
        send(app, KeyCode::KeyD, Key::Character("d".into()), state);
        app.update();
    }
    for _ in 0..2 {
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

fn until(app: &mut App, why: &str, test: impl Fn(&App) -> bool) {
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}. The shell shows: {:?}", screen_text(app));
}

#[test]
fn ctrl_d_ends_a_cat_in_the_shell_and_opens_no_report() {
    let mut app = editor("shell_owns_ctrl_d");
    until(&mut app, "no shell", |app| screen_text(app).contains('$'));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    type_line(&mut app, "cat");
    until(&mut app, "`cat` was never echoed", |app| screen_text(app).contains("$ cat"));
    std::thread::sleep(std::time::Duration::from_millis(300));

    ctrl_d(&mut app);
    assert!(
        !app.world().resource::<ReportEdit>().claims_pointer(),
        "Ctrl+D typed into the shell opened the developer report over it: the chord is the shell's \
         end-of-file while the shell has the keyboard"
    );
    // The shell heard it: `cat` ended, so the next line runs as a command rather than being echoed.
    type_line(&mut app, "echo DONE");
    until(&mut app, "`cat` never ended, so Ctrl+D did not reach the shell", |app| {
        screen_text(app).lines().any(|line| line.trim_end() == "DONE")
    });
}

#[test]
fn the_report_chord_still_opens_a_report_when_the_editor_has_the_keys() {
    let mut app = editor("report_chord_outside_the_shell");
    assert!(!app.world().resource::<TerminalFocus>().holds(), "the editor opens with its own keyboard");
    let mods = bevy_cauldron::keys::MOD_KEYS[0];
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(&mut app, mods, unidentified(), state);
        send(&mut app, KeyCode::KeyD, Key::Character("d".into()), state);
        app.update();
    }
    for _ in 0..2 {
        app.update();
    }
    assert!(
        app.world().resource::<ReportEdit>().claims_pointer(),
        "with the editor holding the keyboard, the report chord opens a report as before"
    );
}
