//! **A shell opened in the editor is told which project the editor holds** (#324): `CAULDRON_PROJECT`.
//!
//! A script or an agent run in the terminal reads it to find the project's `assets/grimoire/` without
//! guessing from its working directory (which a `cd` has moved).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::{PROJECT_ENV, Terminal};
use bevy_cauldron::terminal_pane::TerminalFocus;

use crate::the_terminal_paints_grounds_and_the_selection::editor_with_terminal;

/// How many 50 ms checks the test waits for the shell.
const TRIES: usize = 80;

fn tap(app: &mut App, key_code: KeyCode, logical_key: Key) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            text: match (&logical_key, state) {
                (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
                _ => None,
            },
            logical_key: logical_key.clone(),
            state,
            repeat: false,
            window,
        });
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

#[test]
fn the_shell_is_told_the_project_the_editor_holds() {
    let mut app = editor_with_terminal("project_env");
    let want = {
        let project = app.world().resource::<Terminal>().project_root().expect("a project is open, so the terminal knows its root").to_path_buf();
        format!("[{}]", project.display())
    };
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if screen_lines(&app).iter().any(|line| line.contains('$')) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    app.world_mut().resource_mut::<TerminalFocus>().take();
    for c in format!("echo \"[${PROJECT_ENV}]\"").chars() {
        let s = c.to_string();
        tap(&mut app, KeyCode::Space, Key::Character(s.as_str().into()));
    }
    tap(&mut app, KeyCode::Enter, Key::Enter);
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if screen_lines(&app).iter().any(|line| line == &want) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the shell never printed {want:?}; the pane shows {:?}", screen_lines(&app));
}
