//! **Cmd+Backspace clears the line the shell is reading** (feedback-1794146958145, oracle `other:key-routing`).
//!
//! The note, from the launcher's terminal on a Mac, 0.5.0, 2560x1406 (scale 2, `ui_scale` 2.4): *"I pressed Cmd+Backspace (Mac)
//! and it did not clear the line to the beginning. We need our terminal to automatically respect terminal convientions."*
//! Reproduced on the installed 0.5.0 with a plain `sh`: `echo AAAA`, Cmd+Backspace, `echo BBBB`, Enter printed
//! `AAAAecho BBBB`; the chord was dropped, because every Command chord is the editor's and never the shell's.
//!
//! The oracle is the shell's own line discipline. Mac terminals (VS Code's, iTerm2's) send `^U` for Cmd+Backspace, and `^U` kills
//! the line a program is reading, so the program sees only what was typed after it. The program is `cat -v`, which prints each
//! line it is sent: `aa`, Cmd+Backspace, `ok`, Enter must print `ok`, where a dropped chord prints `aaok`. Command is the Super key
//! here, which is what `bevy_gooey::Modifiers::META` names on every platform.

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
    let root = fixtures::Fixture::new(&format!("capture_1794146958145_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.add_plugins((LauncherPlugin, bevy_cauldron::loading::LoadingPlugin));
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 703), ..default() }, PrimaryWindow));
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
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
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
        't' => KeyCode::KeyT,
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

/// **Command and a named key, as a Mac keyboard sends them**: Command down, the key, then both up.
fn command_key(app: &mut App, code: KeyCode, key: Key) {
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
    tab.screen().built_lines().iter().map(|line| line.cells.iter().map(|c| c.ch).collect::<String>()).collect::<Vec<_>>().join("\n")
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

fn lines_of(app: &App) -> Vec<String> {
    screen_text(app).lines().map(|l| l.trim_end().to_owned()).collect()
}

#[test]
fn cmd_backspace_clears_the_line_the_shell_is_reading() {
    let mut app = the_home();
    until(&mut app, "the home's terminal never started a shell", |app| screen_text(app).contains('$'));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 2);
    type_text(&mut app, "cat -v");
    enter(&mut app);
    until(&mut app, "`cat -v` was never echoed", |app| screen_text(app).contains("cat -v"));
    std::thread::sleep(std::time::Duration::from_millis(300));

    // `aa`, Cmd+Backspace, `ok`, Enter: `cat -v` prints the line it was sent.
    type_text(&mut app, "aa");
    command_key(&mut app, KeyCode::Backspace, Key::Backspace);
    type_text(&mut app, "ok");
    enter(&mut app);
    until(&mut app, "`cat -v` never printed the line it was sent", |app| {
        lines_of(app).iter().filter(|l| l.ends_with("ok")).count() >= 2
    });
    let lines = lines_of(&app);
    assert!(
        !lines.iter().any(|l| l == "aaok"),
        "Cmd+Backspace was dropped: `cat -v` was sent `aaok`, where a person typed aa, Cmd+Backspace, ok: {lines:?}"
    );
    assert!(
        lines.iter().filter(|l| *l == "ok").count() >= 2,
        "the line was cleared, so what was typed (echoed) and what `cat -v` printed are both `ok`: {lines:?}"
    );
}

/// **Cmd+Left and Cmd+Right go to the start and the end of the line** as `^A` and `^E`, which is what VS Code's and iTerm2's terminals
/// send. Under `cat -v` there is no readline to act on them, so the line discipline passes them as characters and `cat -v` prints
/// them spelled: `^A^E`.
#[test]
fn cmd_left_and_cmd_right_move_to_the_start_and_the_end_of_the_line() {
    let mut app = the_home();
    until(&mut app, "the home's terminal never started a shell", |app| screen_text(app).contains('$'));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 2);
    type_text(&mut app, "cat -v");
    enter(&mut app);
    until(&mut app, "`cat -v` was never echoed", |app| screen_text(app).contains("cat -v"));
    std::thread::sleep(std::time::Duration::from_millis(300));

    command_key(&mut app, KeyCode::ArrowLeft, Key::ArrowLeft);
    command_key(&mut app, KeyCode::ArrowRight, Key::ArrowRight);
    enter(&mut app);
    // Typed (echoed by the line discipline) and printed by `cat -v`: two lines.
    until(&mut app, "`cat -v` never printed the codes it was sent", |app| lines_of(app).iter().filter(|l| *l == "^A^E").count() >= 2);
}
