//! **A terminal keeps as much history as its scrollback setting says, and no more** (#324).
//!
//! VS Code's `terminal.integrated.scrollback`. The editor's history was a constant, five thousand
//! lines; the `terminal_scrollback` setting now sets it, and an edit of it reaches a running terminal.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::{SCROLLBACK, Terminal};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits.
const TRIES: usize = 40;

/// More lines than either length the test sets, so the history is full.
const PRINTED: usize = 6_000;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor_with_settings(tag: &str, settings: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
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
    (app, file)
}

/// Prints `PRINTED` numbered lines straight into the active screen, as a long build would.
fn print_lines(app: &mut App) {
    let text: String = (1..=PRINTED).map(|n| format!("{n}\r\n")).collect();
    app.world_mut().resource_mut::<Terminal>().tab_mut().expect("a shell is open").screen_mut().feed(text.as_bytes());
    for _ in 0..2 {
        app.update();
    }
}

fn held(app: &App) -> usize {
    app.world().resource::<Terminal>().tab().expect("a shell is open").screen().scrolled_lines()
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
    panic!("{why}: the terminal holds {} lines", held(app));
}

#[test]
fn a_terminal_holds_the_scrollback_its_setting_names() {
    let (mut app, file) = editor_with_settings("scrollback_setting", "{version: 1, terminal_scrollback: 1000}");
    until(&mut app, "the shell never took the setting", |app| {
        app.world().resource::<Terminal>().tab().is_some_and(|tab| tab.screen().scrollback() == 1000)
    });
    print_lines(&mut app);
    assert_eq!(held(&app), 1000, "`terminal_scrollback: 1000` keeps a thousand lines of history, and no more");

    // An edit reaches the running terminal: more history from here on.
    std::fs::write(&file, "{version: 1}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the running terminal never went back to the default scrollback", |app| {
        app.world().resource::<Terminal>().tab().is_some_and(|tab| tab.screen().scrollback() == SCROLLBACK)
    });
    print_lines(&mut app);
    assert_eq!(held(&app), SCROLLBACK, "with the key taken out, the terminal keeps the default {SCROLLBACK} lines");
}

#[test]
fn a_scrollback_no_terminal_should_keep_is_refused() {
    let layer = bevy_cauldron::settings::SettingsLayer::read("editor.yaml", "{version: 1, terminal_scrollback: 5}");
    let refusal = layer.expect_err("five lines of history is refused");
    assert_eq!(refusal.field.as_deref(), Some("terminal_scrollback"), "the refusal names the key: {}", refusal.reason);
}
