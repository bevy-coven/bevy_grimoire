//! **A tab's gap is blank, not a box** (feedback-1794062374181, oracle `other:no-control-glyph`).
//!
//! Reported on mac_air, 0.4.1, the projects home at 2560x1406 (scale 2, `ui_scale` 2.4): *"We still have the "?"
//! character in the terminal."* The picture is `ls` in a home folder, with a missing-glyph box after every name, at
//! each tab stop the columns jumped. Reproduced on the installed 0.4.1: `printf 'a\tb\tc|'` drew a box after `a` and
//! after `b`. The emulator marks the cell a tab starts from with the tab character itself, so that a copy keeps the
//! tab, and the pane drew that character, for which the terminal's face has no glyph.
//!
//! The oracle is what the person expected to see. The gap a tab leaves is blank: the drawn row reads `a`, blanks to
//! column 8, `b`, blanks to column 16, then `c|`. And no drawn run holds a control character, none of which the face
//! can draw. The note's second half (Cmd+A, Cmd+C) is feedback-1794024642593's, fixed by b7d5e79 after 0.4.1 was
//! released and proved by that capture's test.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::screen::Screen;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalRow};

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
    let root = fixtures::Fixture::new(&format!("capture_1794062374181_{}", std::process::id()))
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

/// **The rows as the pane draws them**, top to bottom: each row's runs, in their order, joined. This is the
/// text a person reads, so it is read from the drawn spans and not from the emulator's grid.
fn drawn_rows(app: &mut App) -> Vec<String> {
    let mut bodies = app.world_mut().query_filtered::<Entity, With<TerminalBody>>();
    let mut rows = app.world_mut().query::<(&TerminalRow, &ChildOf, Option<&Children>)>();
    let world = app.world();
    let Some(body) = bodies.iter(world).next() else {
        return Vec::new();
    };
    let mut drawn: Vec<(usize, String)> = rows
        .iter(world)
        .filter(|(_, parent, _)| parent.parent() == body)
        .map(|(row, _, children)| {
            let text = children
                .into_iter()
                .flatten()
                .filter_map(|run| world.get::<TextSpan>(*run))
                .map(|span| span.0.as_str())
                .collect();
            (row.0, text)
        })
        .collect();
    drawn.sort_by_key(|(row, _)| *row);
    drawn.into_iter().map(|(_, text)| text).collect()
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        settle(app, 2);
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}. The pane draws: {:#?}", drawn_rows(app));
}

/// The line `printf` wrote: it ends with the `c|` the command line itself does not.
fn printed(rows: &[String]) -> Option<&String> {
    rows.iter().find(|row| row.ends_with("c|"))
}

#[test]
fn a_tab_leaves_blank_columns_and_no_drawn_run_holds_a_control_character() {
    let mut app = the_home();
    until(&mut app, "the home's terminal never drew a prompt", |app| drawn_rows(app).iter().any(|row| row.contains('$')));
    // The line a person types, sent to the shell as the keys would send it.
    app.world_mut().resource_mut::<Terminal>().write(b"printf 'a\\tb\\tc|\\n'\r");
    until(&mut app, "`printf` never printed its line", |app| printed(&drawn_rows(app)).is_some());

    let rows = drawn_rows(&mut app);
    let line = printed(&rows).expect("the wait above found it");
    assert_eq!(
        line, "a       b       c|",
        "a tab's gap is blank: `b` at the tab stop in column 8 and `c` at 16, with nothing drawn between"
    );
    let control: Vec<&String> = rows.iter().filter(|row| row.chars().any(char::is_control)).collect();
    assert!(
        control.is_empty(),
        "these drawn rows hold a control character, which the terminal's face draws as a missing-glyph box: {control:#?}"
    );
}
