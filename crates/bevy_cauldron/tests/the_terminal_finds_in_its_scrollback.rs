//! **Find in the terminal's scrollback**, as VS Code's terminal does it (#321).
//!
//! A build error three hundred lines up was found by scrolling and reading. Primary+F, with the shell
//! holding the keys, opens a find box over the terminal; every match is drawn on a ground, the current
//! one is scrolled into view, Enter walks to older matches and Shift+Enter to newer ones, and the box
//! says `n of m`. The query is plain text, matched case-blind unless it has a capital (smart case).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_gooey::{TextInputOf, ValueChange};

use bevy_cauldron::terminal::{FindStatus, Terminal, TerminalScreen};
use bevy_cauldron::terminal_pane::{TerminalFind, TerminalFindBox, TerminalFocus};

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, settle};

/// A screen five rows tall, so most of what is printed is scrollback.
const ROWS: u16 = 5;
const COLS: u16 = 40;

/// Thirty numbered lines with `needle` on three of them, the newest of which is off the screen.
fn printed() -> TerminalScreen {
    let mut screen = TerminalScreen::new(COLS, ROWS);
    for n in 1..=30 {
        let line = match n {
            2 | 9 | 16 => format!("line {n:02} needle here\r\n"),
            _ => format!("line {n:02}\r\n"),
        };
        screen.feed(line.as_bytes());
    }
    screen
}

/// The rows on screen, trimmed.
fn visible(screen: &mut TerminalScreen) -> Vec<String> {
    screen
        .lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>().trim_end().to_owned())
        .collect()
}

fn on_screen(screen: &mut TerminalScreen, row: &str) -> bool {
    visible(screen).iter().any(|line| line == row)
}

fn status(screen: &TerminalScreen) -> Option<(usize, usize)> {
    screen.find_status().map(|FindStatus { current, total, .. }| (current, total))
}

#[test]
fn a_query_finds_the_newest_match_and_enter_walks_to_older_ones() {
    let mut screen = printed();
    assert!(!on_screen(&mut screen, "line 16 needle here"), "the newest match starts off the screen");

    screen.find("needle");
    assert_eq!(status(&screen), Some((3, 3)), "three matches, and the newest is the current one");
    assert!(on_screen(&mut screen, "line 16 needle here"), "the current match is scrolled into view: {:?}", visible(&mut screen));

    screen.find_next(true);
    assert_eq!(status(&screen), Some((2, 3)), "Enter goes to the next match up, in older output");
    assert!(on_screen(&mut screen, "line 09 needle here"), "{:?}", visible(&mut screen));
    screen.find_next(true);
    assert_eq!(status(&screen), Some((1, 3)));
    assert!(on_screen(&mut screen, "line 02 needle here"), "{:?}", visible(&mut screen));
    screen.find_next(true);
    assert_eq!(status(&screen), Some((3, 3)), "past the oldest match the search wraps to the newest");
    screen.find_next(false);
    assert_eq!(status(&screen), Some((1, 3)), "Shift+Enter goes down, and wraps past the newest to the oldest");
}

#[test]
fn every_match_is_drawn_on_a_ground_and_clearing_takes_them_away() {
    let mut screen = printed();
    screen.find("needle");
    let lines = screen.lines().to_vec();
    let row = lines
        .iter()
        .find(|line| line.cells.iter().map(|c| c.ch).collect::<String>().starts_with("line 16 needle"))
        .expect("the current match is on screen");
    let at = "line 16 ".len();
    assert!(row.cells[at..at + "needle".len()].iter().all(|c| c.has_ground()), "every cell of the match is drawn on a ground");
    assert!(!row.cells[at - 1].has_ground(), "the cell before the match is not");
    assert!(!row.cells[at + "needle".len()].has_ground(), "nor the cell after it");

    screen.find_clear();
    assert_eq!(screen.find_status(), None, "a cleared search says nothing");
    assert!(
        screen.lines().iter().all(|line| line.cells.iter().all(|c| !c.has_ground())),
        "clearing the search takes its grounds away"
    );
}

#[test]
fn the_query_is_plain_text_in_smart_case() {
    let mut screen = TerminalScreen::new(COLS, ROWS);
    screen.feed(b"a.b axb\r\nf(x) NEEDLE needle\r\n");
    screen.find("a.b");
    assert_eq!(status(&screen), Some((1, 1)), "a `.` is a dot, not any character");
    screen.find("f(x");
    assert_eq!(status(&screen), Some((1, 1)), "an unbalanced bracket is a bracket, not a broken pattern");
    screen.find("needle");
    assert_eq!(status(&screen).map(|s| s.1), Some(2), "a query in lower case matches either case");
    screen.find("Needle");
    assert_eq!(status(&screen).map(|s| s.1), Some(0), "a query with a capital matches only its own case");
    screen.find("");
    assert_eq!(screen.find_status(), None, "an empty query searches nothing");
}

#[test]
fn output_that_arrives_while_the_search_is_open_is_searched_too() {
    let mut screen = printed();
    screen.find("needle");
    screen.feed(b"line 31 needle again\r\n");
    screen.lines();
    assert_eq!(status(&screen).map(|s| s.1), Some(4), "a match printed after the search began is counted");
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

/// **Ctrl+F as a hand makes it**: the editor's Primary+F off a mac.
fn ctrl_f(app: &mut App) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::ControlLeft, unidentified(), state);
        send(app, KeyCode::KeyF, Key::Character("f".into()), state);
        app.update();
    }
    settle(app, 3);
}

fn shell_text(app: &App) -> String {
    app.world()
        .resource::<Terminal>()
        .tab()
        .map(|tab| {
            tab.screen()
                .built_lines()
                .iter()
                .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn find_box(app: &mut App) -> Option<Entity> {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<TerminalFindBox>>().iter(world).next()
}

#[test]
fn primary_f_in_the_shell_opens_the_find_box_with_the_keys_and_escape_gives_them_back() {
    let mut app = editor_with_terminal("terminal_find_box");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 6);
    let before = shell_text(&app);

    ctrl_f(&mut app);
    let row = find_box(&mut app).expect("Primary+F with the shell holding the keys opens the find box");
    let world = app.world_mut();
    let field = world
        .query::<(Entity, &TextInputOf)>()
        .iter(world)
        .find(|(_, of)| of.0 == row)
        .map(|(field, _)| field)
        .expect("the find box has a text field");
    assert_eq!(
        app.world().resource::<bevy::input_focus::InputFocus>().get(),
        Some(field),
        "the find box opens with the keyboard in it"
    );
    assert!(app.world().resource::<TerminalFocus>().holds(), "the terminal keeps the keys: the editor's bindings stay quiet");

    tap(&mut app, KeyCode::KeyZ, Key::Character("z".into()));
    settle(&mut app, 3);
    std::thread::sleep(std::time::Duration::from_millis(300));
    settle(&mut app, 3);
    let after = shell_text(&app);
    assert!(
        !after.contains("^F") && after.matches('z').count() == before.matches('z').count(),
        "neither the chord nor a key typed into the box reached the shell: {after:?}"
    );

    // The query searches the active shell's screen.
    app.world_mut().commands().trigger(ValueChange::<String> { source: row, value: "$".to_owned(), is_final: false });
    settle(&mut app, 3);
    assert!(
        app.world().resource::<Terminal>().tab().and_then(|tab| tab.screen().find_status()).is_some_and(|s| s.total >= 1),
        "a query typed into the box searches the shell's scrollback"
    );

    tap(&mut app, KeyCode::Escape, Key::Escape);
    settle(&mut app, 3);
    assert!(find_box(&mut app).is_none(), "Escape closes the find box");
    assert!(!app.world().resource::<TerminalFind>().is_open(), "and the search with it");
    assert!(
        app.world().resource::<Terminal>().tab().is_some_and(|tab| tab.screen().find_status().is_none()),
        "closing the box clears the search"
    );
    assert!(app.world().resource::<TerminalFocus>().holds(), "Escape gives the keys back to the shell, not to the editor");
}
