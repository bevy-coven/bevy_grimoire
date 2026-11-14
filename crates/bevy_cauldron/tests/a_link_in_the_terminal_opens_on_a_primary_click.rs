//! **An `http://` or `https://` address in the terminal's output opens on Cmd+click (Ctrl+click off a mac)** (#325).
//!
//! A build, a test runner or `git push` prints addresses, and VS Code's terminal follows one on a
//! Cmd+click. The screen finds the link under a cell (prose around it is left off, a wrapped link is one,
//! two on a row are two); the pane opens it on a Primary+click and sends nothing to the shell.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_cauldron::app_verbs::LinksOff;
use bevy_cauldron::terminal::{Terminal, TerminalScreen};
use bevy_cauldron::terminal_pane::TerminalBody;
use bevy_cauldron::terminal_view::{BODY_INSET, CellMetrics};

use crate::fixtures;

/// A screen holding `text`, 30 columns wide.
fn screen_with(text: &str) -> TerminalScreen {
    let mut screen = TerminalScreen::new(30, 6);
    screen.feed(text.as_bytes());
    screen.lines();
    screen
}

/// Where `needle` begins on row 0 of `text`.
fn col_of(text: &str, needle: &str) -> usize {
    text.find(needle).unwrap_or_else(|| panic!("{needle:?} is in {text:?}"))
}

#[test]
fn the_link_under_a_cell_starts_at_its_scheme_and_covers_it_to_the_end() {
    let text = "see https://example.com/a?x=1 now";
    let screen = screen_with(text);
    let start = col_of(text, "https");
    let url = "https://example.com/a?x=1";
    for col in [start, start + 10, start + url.len() - 1] {
        assert_eq!(screen.link_at(0, col).as_deref(), Some(url), "column {col} is in the link");
    }
    assert_eq!(screen.link_at(0, start - 1), None, "the space before the link is not in it");
    assert_eq!(screen.link_at(0, start + url.len()), None, "the space after the link is not in it");
    assert_eq!(screen_with("http://plain.example/x").link_at(0, 3).as_deref(), Some("http://plain.example/x"), "http too");
}

#[test]
fn prose_around_a_link_is_left_off_and_plain_text_has_no_link() {
    for (written, link) in [
        ("(see https://example.com/x).", "https://example.com/x"),
        ("<https://example.com/x>", "https://example.com/x"),
        ("\"https://example.com/x\",", "https://example.com/x"),
        ("at https://example.com/x.", "https://example.com/x"),
        ("[https://example.com/x]", "https://example.com/x"),
    ] {
        let screen = screen_with(written);
        let at = col_of(written, "https") + 2;
        assert_eq!(screen.link_at(0, at).as_deref(), Some(link), "in {written:?}");
        let end = col_of(written, "https") + link.len();
        assert_eq!(screen.link_at(0, end), None, "the trailing prose of {written:?} is not the link");
    }
    let plain = screen_with("no address here, just words");
    assert!((0..27).all(|col| plain.link_at(0, col).is_none()), "plain text has no link");
    assert_eq!(screen_with("https://").link_at(0, 2), None, "a scheme with nothing after it is no address");
}

#[test]
fn a_cell_past_the_end_of_its_row_or_off_the_screen_is_no_link() {
    let screen = screen_with("https://example.com");
    assert_eq!(screen.link_at(0, 25), None, "past the end of the row");
    assert_eq!(screen.link_at(5, 2), None, "an empty row");
    assert_eq!(screen.link_at(99, 2), None, "a row that is not there");
}

#[test]
fn two_links_on_a_row_are_two() {
    let text = "https://a.example/one https://b.example/two";
    let screen = screen_with(text);
    // 30 columns: the second wraps onto row 1, and each is its own link.
    assert_eq!(screen.link_at(0, 4).as_deref(), Some("https://a.example/one"));
    let second = col_of(text, "https://b") ;
    let (row, col) = (second / 30, second % 30);
    assert_eq!(screen.link_at(row, col + 3).as_deref(), Some("https://b.example/two"));
}

#[test]
fn a_link_that_wraps_onto_the_next_row_is_one_link_from_either_row() {
    let url = "https://example.com/a/very/long/path/that/wraps/the/row";
    assert!(url.len() > 30, "the address is longer than the 30 columns");
    let screen = screen_with(url);
    assert_eq!(screen.link_at(0, 5).as_deref(), Some(url), "the first row's part");
    assert_eq!(screen.link_at(1, 5).as_deref(), Some(url), "the second row's part");
}

/// **`SHELL` is process-wide**, so the tests of this file take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.init_resource::<LinksOff>();
    for _ in 0..10 {
        app.update();
    }
    app
}

fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// The physical pixel at the centre of cell (`row`, `col`) of the terminal's body.
fn pixel_of_cell(app: &mut App, row: usize, col: usize) -> Vec2 {
    let metrics = *app.world().resource::<CellMetrics>();
    let world = app.world_mut();
    let (node, at) = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<TerminalBody>>()
        .iter(world)
        .next()
        .expect("the terminal body exists");
    let to_physical = 1.0 / node.inverse_scale_factor();
    let top_left = at.translation - node.size() / 2.0;
    top_left + (BODY_INSET + Vec2::new((col as f32 + 0.5) * metrics.width(), (row as f32 + 0.5) * metrics.height())) * to_physical
}

fn click(app: &mut App, at: Vec2) {
    point(app, at, PointerAction::Move { delta: Vec2::ZERO });
    point(app, at, PointerAction::Press(PointerButton::Primary));
    point(app, at, PointerAction::Release(PointerButton::Primary));
    for _ in 0..2 {
        app.update();
    }
}

/// Clear the screen and write `text` at its top left, as a program's output would.
fn print(app: &mut App, text: &str) {
    let mut bytes = b"\x1b[2J\x1b[H".to_vec();
    bytes.extend_from_slice(text.as_bytes());
    app.world_mut().resource_mut::<Terminal>().tab_mut().expect("a shell is open").screen_mut().feed(&bytes);
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn a_primary_click_on_a_link_asks_for_it_to_be_opened_and_a_plain_click_does_not() {
    let mut app = editor("terminal_link_click");
    print(&mut app, "CI: https://ci.example/run/42, then done");
    let col = col_of("CI: https://ci.example/run/42, then done", "https") + 4;

    let at = pixel_of_cell(&mut app, 0, col);
    click(&mut app, at);
    assert!(app.world().resource::<LinksOff>().0.is_empty(), "a plain click opens nothing");

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(primary());
    click(&mut app, at);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(primary());
    assert_eq!(
        app.world().resource::<LinksOff>().0,
        vec!["https://ci.example/run/42".to_owned()],
        "Primary+click opens the address under the pointer, without the trailing comma"
    );

    app.world_mut().resource_mut::<LinksOff>().0.clear();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(primary());
    let on_text = pixel_of_cell(&mut app, 0, 1);
    click(&mut app, on_text);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(primary());
    assert!(app.world().resource::<LinksOff>().0.is_empty(), "Primary+click on plain text opens nothing");
}

#[test]
fn a_primary_click_on_a_link_wins_over_a_program_that_asked_for_the_mouse() {
    let mut app = editor("terminal_link_mouse");
    let text = "see https://ci.example/run/7 now";
    // Mouse tracking and SGR encoding on, as `htop` or `less` ask for them.
    print(&mut app, &format!("\x1b[?1000h\x1b[?1006h{text}"));
    assert!(
        app.world().resource::<Terminal>().tab().is_some_and(|tab| tab.screen().mouse_reporting().is_some()),
        "the program asked for the mouse"
    );
    let at = pixel_of_cell(&mut app, 0, col_of(text, "https") + 3);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(primary());
    click(&mut app, at);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(primary());
    assert_eq!(app.world().resource::<LinksOff>().0, vec!["https://ci.example/run/7".to_owned()], "the link opens over a mouse-tracking program");
}
