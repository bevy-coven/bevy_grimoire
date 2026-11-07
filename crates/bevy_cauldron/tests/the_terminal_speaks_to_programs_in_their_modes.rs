//! **A key, a wheel notch and a click reach a program in the form it asked for** (#311, #312, #313).
//!
//! Each test runs `cat -v` in the pane's real shell, puts the emulator in the mode a full-screen
//! program would set (fed to the screen, exactly the escape the program writes), then acts through the
//! editor's own key and pointer paths. `cat -v` prints what arrived with Escape spelled `^[`, so the
//! drawn text is the bytes the program received.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerPress, PointerRelease, PointerScroll};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal_pane::{TerminalBody, TerminalRow};
use bevy_cauldron::terminal_view::CellMetrics;

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// How many 50 ms checks a test waits for `cat -v` to print a line.
const ANSWER_TRIES: usize = 60;

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window")
}

/// One key, down and up, with the text a keyboard reports for it.
fn key(app: &mut App, key_code: KeyCode, logical_key: Key, text: Option<&str>) {
    let window = window(app);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: logical_key.clone(),
            state,
            text: text.filter(|_| state == ButtonState::Pressed).map(Into::into),
            repeat: false,
            window,
        });
        app.update();
    }
}

fn type_line(app: &mut App, line: &str) {
    for c in line.chars() {
        let s = c.to_string();
        key(app, KeyCode::Space, Key::Character(s.as_str().into()), Some(&s));
    }
    key(app, KeyCode::Enter, Key::Enter, None);
}

/// Every drawn row's text, in row order.
fn drawn_text(app: &mut App) -> String {
    let world = app.world_mut();
    let mut rows: Vec<(usize, String)> = Vec::new();
    let mut query = world.query::<(&TextSpan, &ChildOf)>();
    for (span, parent) in query.iter(world) {
        if let Some(row) = world.get::<TerminalRow>(parent.parent()) {
            rows.push((row.0, span.0.clone()));
        }
    }
    rows.sort_by_key(|(row, _)| *row);
    rows.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join("\n")
}

/// Wait until the pane shows `wanted`, and say what it showed if it never does.
fn shows(app: &mut App, wanted: &str) -> Result<(), String> {
    let mut seen = String::new();
    for _ in 0..ANSWER_TRIES {
        settle(app, 2);
        seen = drawn_text(app);
        if seen.contains(wanted) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(seen)
}

/// The pane with `cat -v` running in its shell, reading the keyboard.
fn cat_v(tag: &str) -> App {
    let mut app = editor_with_terminal(tag);
    type_line(&mut app, "cat -v");
    shows(&mut app, "cat -v").unwrap_or_else(|seen| panic!("the command was never echoed: {seen:?}"));
    // `cat` is reading once its line has been taken; a moment for the shell to start it.
    std::thread::sleep(std::time::Duration::from_millis(300));
    settle(&mut app, 2);
    app
}

fn body(app: &mut App) -> (Entity, Vec2) {
    let world = app.world_mut();
    world
        .query_filtered::<(Entity, &ComputedNode), With<TerminalBody>>()
        .iter(world)
        .next()
        .map(|(entity, node)| (entity, node.size() * node.inverse_scale_factor()))
        .expect("the pane has a body")
}

/// A hit at the centre of `cell` (row, column), normalized to the body as bevy_ui's backend reports it: the
/// cell's centre as the rows draw it, past the body's resolved padding (feedback-1794011769640: this counted
/// from the body's corner, as the pane did, which is two cells left of where the rows begin).
fn hit_at(app: &mut App, cell: (usize, usize)) -> (Entity, HitData) {
    let (entity, size) = body(app);
    let inset = {
        let node = app.world().get::<ComputedNode>(entity).expect("the body is laid out");
        node.padding.min_inset * node.inverse_scale_factor()
    };
    let metrics = *app.world().resource::<CellMetrics>();
    let x = (inset.x + (cell.1 as f32 + 0.5) * metrics.width()) / size.x - 0.5;
    let y = (inset.y + (cell.0 as f32 + 0.5) * metrics.height()) / size.y - 0.5;
    (entity, HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::new(x, y, 0.0)), None))
}

fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: bevy::camera::NormalizedRenderTarget::None { width: 1, height: 1 },
            position: Vec2::ZERO,
        },
    )
}

fn wheel(app: &mut App, notches: f32) {
    let (entity, hit) = hit_at(app, (1, 1));
    app.world_mut().trigger(PointerScroll {
        entity,
        pointer: pointer(),
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: notches,
        hit,
        phase: TouchPhase::Moved,
    });
    settle(app, 2);
}

fn click(app: &mut App, cell: (usize, usize)) {
    let (entity, hit) = hit_at(app, cell);
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: hit.clone(),
        count: 1,
    });
    settle(app, 1);
    app.world_mut().trigger(PointerRelease { entity, pointer: pointer(), button: PointerButton::Primary, hit });
    settle(app, 2);
}

#[test]
fn an_arrow_is_ss3_in_application_cursor_mode() {
    let mut app = cat_v("terminal_app_cursor");
    feed(&mut app, b"\x1b[?1h");
    key(&mut app, KeyCode::ArrowUp, Key::ArrowUp, None);
    key(&mut app, KeyCode::Enter, Key::Enter, None);
    shows(&mut app, "^[OA").unwrap_or_else(|seen| {
        panic!("in application cursor mode Up must reach the program as ESC O A; `cat -v` printed: {seen:?}")
    });
}

#[test]
fn the_wheel_sends_arrows_on_the_alternate_screen_and_nothing_on_the_normal_one() {
    let mut app = cat_v("terminal_alternate_scroll");
    wheel(&mut app, 1.0);
    type_line(&mut app, "MARK");
    shows(&mut app, "MARK").unwrap_or_else(|seen| panic!("the marker line never came back: {seen:?}"));
    assert!(
        !drawn_text(&mut app).contains("^[["),
        "on the normal screen the wheel is the pane's scrollback and the program hears nothing: {:?}",
        drawn_text(&mut app)
    );

    feed(&mut app, b"\x1b[?1049h");
    wheel(&mut app, 1.0);
    key(&mut app, KeyCode::Enter, Key::Enter, None);
    shows(&mut app, "^[[A^[[A^[[A").unwrap_or_else(|seen| {
        panic!("on the alternate screen a notch up must reach the program as three Ups; it printed: {seen:?}")
    });
}

#[test]
fn a_click_reaches_a_program_that_asked_for_the_mouse() {
    let mut app = cat_v("terminal_mouse_report");
    feed(&mut app, b"\x1b[?1000h\x1b[?1006h");
    click(&mut app, (2, 5));
    key(&mut app, KeyCode::Enter, Key::Enter, None);
    shows(&mut app, "^[[<0;6;3M^[[<0;6;3m").unwrap_or_else(|seen| {
        panic!("a left click at row 2, column 5 must reach the program as SGR press then release; it printed: {seen:?}")
    });
}

#[test]
fn shift_keeps_the_mouse_for_the_person() {
    let mut app = cat_v("terminal_mouse_shift");
    feed(&mut app, b"\x1b[?1000h\x1b[?1006h");
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    click(&mut app, (2, 5));
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
    type_line(&mut app, "MARK");
    shows(&mut app, "MARK").unwrap_or_else(|seen| panic!("the marker line never came back: {seen:?}"));
    assert!(
        !drawn_text(&mut app).contains("^[[<"),
        "with Shift held the click is the person's, and the program hears nothing: {:?}",
        drawn_text(&mut app)
    );
}
