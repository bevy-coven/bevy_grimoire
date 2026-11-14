//! **The terminal takes the person's settings, and takes an edit of them while it runs** (#324).
//!
//! VS Code's terminal has a font size, a cursor style and a cursor blink in its settings. Cauldron's
//! settings are files (`settings.yaml` for the person, `<project>/assets/grimoire/editor.yaml` for the
//! project) read while the editor runs, and the terminal's three keys are `terminal_text_scale`,
//! `terminal_cursor` and `terminal_cursor_blink`.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::{CaretShape, Terminal};
use bevy_cauldron::terminal_pane::TerminalCaret;
use bevy_cauldron::terminal_view::{CellMetrics, TERMINAL_CELL_H};

use crate::fixtures;

/// How many frames, 50 ms apart, a test watches or waits.
const TRIES: usize = 40;

/// The terminal text size the project asks for.
const SCALE: f32 = 1.5;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project whose `editor.yaml` says `settings`.
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

fn caret_shape(app: &mut App) -> CaretShape {
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    let screen = terminal.tab_mut().expect("a shell is open").screen_mut();
    screen.lines();
    screen.cursor().shape
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}");
}

#[test]
fn the_projects_terminal_settings_give_the_caret_its_shape_its_blink_and_the_text_its_size() {
    let (mut app, _) = editor_with_settings(
        "terminal_settings",
        &format!("{{version: 1, terminal_cursor: Bar, terminal_cursor_blink: false, terminal_text_scale: {SCALE}}}"),
    );
    until(&mut app, "the shell's caret never took the `terminal_cursor: Bar` setting", |app| {
        caret_shape(app) == CaretShape::Bar
    });
    until(&mut app, "the terminal never measured its cell at the set text size", |app| {
        let metrics = app.world().resource::<CellMetrics>();
        metrics.is_measured() && metrics.height() > TERMINAL_CELL_H * (1.0 + SCALE) / 2.0
    });
    let height = app.world().resource::<CellMetrics>().height();
    assert!(
        (height / TERMINAL_CELL_H - SCALE).abs() < 0.15,
        "a {SCALE}x terminal text size draws {SCALE}x cells, not {height} px against {TERMINAL_CELL_H}"
    );

    // **A caret that does not blink is lit on every frame**, across more than one blink period.
    for _ in 0..TRIES {
        app.update();
        let world = app.world_mut();
        let display = world
            .query_filtered::<&Node, With<TerminalCaret>>()
            .iter(world)
            .next()
            .map(|node| node.display)
            .expect("the pane has a caret");
        assert_eq!(display, Display::Flex, "`terminal_cursor_blink: false`, and the caret went dark");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn an_edit_of_the_settings_file_reaches_a_running_terminal() {
    let (mut app, file) = editor_with_settings("terminal_settings_live", "{version: 1}");
    until(&mut app, "a terminal with no setting has a block caret", |app| caret_shape(app) == CaretShape::Block);
    std::fs::write(&file, "{version: 1, terminal_cursor: Underline}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the running terminal never took the edited `terminal_cursor: Underline`", |app| {
        caret_shape(app) == CaretShape::Underline
    });
    // A program that asks for a shape still gets it over the setting.
    app.world_mut()
        .resource_mut::<Terminal>()
        .tab_mut()
        .expect("a shell is open")
        .screen_mut()
        .feed(b"\x1b[6 q");
    assert_eq!(caret_shape(&mut app), CaretShape::Bar, "a program's DECSCUSR wins over the setting");
}
