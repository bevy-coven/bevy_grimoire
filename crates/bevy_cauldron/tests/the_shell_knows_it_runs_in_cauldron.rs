//! **The shell is told it runs in Cauldron, in a locale that carries UTF-8** (#324).
//!
//! VS Code's integrated terminal sets `TERM_PROGRAM=vscode`, and prompts and tools branch on it. The
//! editor's shell had only `TERM` and `COLORTERM`, so a prompt could not tell where it ran; and a shell
//! started from a desktop launcher with no locale drew every `—` and box line as `?`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::{TERM_PROGRAM, Terminal, session_env};
use bevy_cauldron::terminal_pane::TerminalFocus;

use crate::the_terminal_paints_grounds_and_the_selection::editor_with_terminal;

/// How many 50 ms checks the test waits for the shell.
const TRIES: usize = 80;

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

fn screen_lines(app: &App) -> Vec<String> {
    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        return Vec::new();
    };
    tab.screen()
        .built_lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>().trim_end().to_owned())
        .collect()
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
    panic!("{why}. The pane shows: {:?}", screen_lines(app));
}

#[test]
fn the_shell_reads_cauldron_and_its_version_in_term_program() {
    let mut app = editor_with_terminal("term_program");
    until(&mut app, "no shell", |app| screen_lines(app).iter().any(|line| line.contains('$')));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    for c in "echo \"[$TERM_PROGRAM $TERM_PROGRAM_VERSION]\"".chars() {
        let s = c.to_string();
        tap(&mut app, KeyCode::Space, Key::Character(s.as_str().into()));
    }
    tap(&mut app, KeyCode::Enter, Key::Enter);
    let want = format!("[{TERM_PROGRAM} {}]", env!("CARGO_PKG_VERSION"));
    until(&mut app, &format!("the shell never printed {want:?}: it was not told where it runs"), |app| {
        screen_lines(app).iter().any(|line| line == &want)
    });
}

fn lang_of(env: &[(String, String)]) -> Option<&str> {
    env.iter().find(|(name, _)| name == "LANG").map(|(_, value)| value.as_str())
}

#[test]
fn a_shell_with_no_utf8_locale_is_given_one_and_a_persons_own_is_kept() {
    let given = |lc_all: Option<&str>, lang: Option<&str>| {
        session_env(lc_all.map(str::to_owned), lang.map(str::to_owned))
    };
    for plain in [None, Some(""), Some("C"), Some("POSIX")] {
        let env = given(None, plain);
        assert!(
            lang_of(&env).is_some_and(|lang| lang.ends_with("UTF-8")),
            "a shell whose LANG is {plain:?} is given a UTF-8 locale, not {:?}",
            lang_of(&env)
        );
    }
    assert_eq!(lang_of(&given(None, Some("de_DE.UTF-8"))), None, "a person's own LANG is inherited, never overridden");
    assert_eq!(lang_of(&given(Some("fr_FR.UTF-8"), None)), None, "an LC_ALL wins over any LANG, so none is set beside it");
    let env = given(Some("fr_FR.UTF-8"), None);
    for name in ["TERM", "COLORTERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION"] {
        assert!(env.iter().any(|(n, _)| n == name), "{name} is always set, whatever the locale");
    }
}
