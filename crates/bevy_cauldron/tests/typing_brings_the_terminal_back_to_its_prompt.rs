//! **A key typed into a scrolled-back terminal brings the view back to the prompt** (#319).
//!
//! Scrolled up into the scrollback to read a build, a person who types sees nothing happen: the key
//! reaches the shell, but the view stays on old output, so they type blind. VS Code returns to the
//! prompt on input (`terminal.integrated.scrollOnUserInput`), as xterm, Alacritty and iTerm do. The
//! keys the pane keeps for itself — Page Up and Page Down page the scrollback — are not input to the
//! shell and leave the view where they put it. Driven through the real key path, on a real shell.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::TerminalFocus;

use crate::fixtures;

/// How many 50 ms checks a test waits for the shell.
const TRIES: usize = 80;

/// How many lines the shell prints, so the scrollback is several panes deep.
const PRINTED: usize = 200;

/// How far the view is scrolled back: far enough that neither the prompt nor the last printed line
/// is on screen, whatever the pane's row count.
const SCROLL_BACK_LINES: i32 = 60;

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

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        let s = c.to_string();
        tap(app, KeyCode::Space, Key::Character(s.as_str().into()));
    }
}

/// The rows on screen, as the pane draws them.
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

fn shows(app: &App, row: &str) -> bool {
    screen_lines(app).iter().any(|line| line == row)
}

/// **A row that ends the way the prompt does.** `/bin/sh` is bash on some machines, whose prompt is
/// `sh-5.3$`, and dash on others, whose prompt is `$`; both end in `$` and then what was typed.
fn shows_prompt_ending(app: &App, ending: &str) -> bool {
    screen_lines(app).iter().any(|line| line.ends_with(ending))
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
fn a_key_typed_into_the_scrollback_brings_the_view_back_to_the_prompt() {
    let mut app = editor("typing_scrolls_home");
    until(&mut app, "no shell", |app| screen_lines(app).iter().any(|line| line.contains('$')));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    type_text(&mut app, &format!("seq 1 {PRINTED}"));
    tap(&mut app, KeyCode::Enter, Key::Enter);
    let last = PRINTED.to_string();
    until(&mut app, "the shell never printed its last line", |app| shows(app, &last));
    until(&mut app, "the shell never came back to its prompt", |app| shows_prompt_ending(app, "$"));

    // **Scrolled back to read**, as the wheel or Page Up would leave it.
    app.world_mut()
        .resource_mut::<Terminal>()
        .tab_mut()
        .expect("a shell is open")
        .screen_mut()
        .scroll(SCROLL_BACK_LINES);
    for _ in 0..2 {
        app.update();
    }
    assert!(
        !shows(&app, &last) && !shows_prompt_ending(&app, "$"),
        "the view did not scroll back, so this test proves nothing: {:?}",
        screen_lines(&app)
    );

    type_text(&mut app, "x");
    until(
        &mut app,
        "a key typed while the view was in the scrollback left it there: the person typed blind",
        |app| shows_prompt_ending(app, "$ x"),
    );

    // **Page Up is the pane's, not the shell's**: it pages the scrollback and the view stays there.
    tap(&mut app, KeyCode::PageUp, Key::PageUp);
    for _ in 0..2 {
        app.update();
    }
    assert!(
        !shows_prompt_ending(&app, "$ x"),
        "Page Up brought the view straight back to the prompt: the pane's own paging key was taken \
         for input to the shell"
    );
}
