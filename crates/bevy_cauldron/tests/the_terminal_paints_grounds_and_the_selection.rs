//! **The terminal draws a cell's ground and the selection** (#309).
//!
//! The emulator swapped an inverse cell's colours and the drag set a selection, but the pane drew each
//! run as ink alone: `ESC[7m` text came out in the pane's own background colour — invisible — and a
//! selection showed nothing. This file reads the drawn spans of a real editor: a run's ground is a
//! `TextBackgroundColor` on its `TextSpan`, so a test that sees the span sees what Bevy paints.
//!
//! The bytes are fed to the open tab's screen directly, after the shell has drawn its prompt, so the
//! assertion is about the drawing and not about which shell printed them.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::text::TextBackgroundColor;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalRow};

use crate::fixtures;

/// Frames a fed screen gets to reach its drawn spans: one to rebuild, one for the commands to land.
const DRAW_FRAMES: usize = 4;

/// How many times the test waits for the shell's first prompt before feeding.
const PROMPT_TRIES: usize = 60;

/// The editor with a window, the terminal raised and its shell started.
pub(crate) fn editor_with_terminal(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    settle(&mut app, 10);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [KeyCode::ControlLeft, KeyCode::Backquote] {
            app.world_mut().write_message(KeyboardInput {
                key_code: key,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window,
            });
        }
        app.update();
    }
    for _ in 0..PROMPT_TRIES {
        settle(&mut app, 2);
        if !spans(&mut app).is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(!app.world().resource::<Terminal>().is_empty(), "Ctrl+` started no shell");
    app
}

pub(crate) fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// One drawn span: its row, its text, its ink and its ground.
#[derive(Debug, Clone)]
struct Span {
    row: usize,
    text: String,
    ink: Color,
    ground: Option<Color>,
}

/// **Every span the pane draws**, with the ground Bevy will paint behind it.
fn spans(app: &mut App) -> Vec<Span> {
    let world = app.world_mut();
    let Some(body) = world.query_filtered::<Entity, With<TerminalBody>>().iter(world).next() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut query = world.query::<(&TextSpan, &TextColor, Option<&TextBackgroundColor>, &ChildOf)>();
    for (span, ink, ground, parent) in query.iter(world) {
        let Some(row) = world.get::<TerminalRow>(parent.parent()) else {
            continue;
        };
        let under_body = world.get::<ChildOf>(parent.parent()).is_some_and(|p| p.parent() == body);
        if !under_body {
            continue;
        }
        found.push(Span {
            row: row.0,
            text: span.0.clone(),
            ink: ink.0,
            ground: ground.map(|g| g.0),
        });
    }
    found
}

fn span_with<'a>(spans: &'a [Span], text: &str) -> &'a Span {
    spans
        .iter()
        .find(|s| s.text.contains(text))
        .unwrap_or_else(|| panic!("no drawn span holds {text:?}; the pane draws {spans:#?}"))
}

pub(crate) fn feed(app: &mut App, bytes: &[u8]) {
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    terminal.tab_mut().expect("the raised pane has a tab").screen_mut().feed(bytes);
}

#[test]
fn inverse_text_is_drawn_on_its_ground_and_plain_text_on_none() {
    let mut app = editor_with_terminal("terminal_inverse_ground");
    feed(&mut app, b"\r\n\x1b[7mINVERSE\x1b[0m PLAIN\r\n");
    settle(&mut app, DRAW_FRAMES);
    let drawn = spans(&mut app);

    let plain = span_with(&drawn, "PLAIN");
    assert_eq!(plain.ground, None, "plain text on the pane's own ground carries no background: {plain:?}");
    let inverse = span_with(&drawn, "INVERSE");
    assert!(!inverse.text.contains("PLAIN"), "the inverse word is its own run: {inverse:?}");
    assert_eq!(
        inverse.ground,
        Some(plain.ink),
        "ESC[7m swaps the colours, so the inverse run's ground is the plain ink — without it the word is \
         drawn in the pane's background colour and cannot be seen: {inverse:?}"
    );
    assert_ne!(inverse.ink, plain.ink, "and its ink is the swapped one, not the plain ink: {inverse:?}");
}

#[test]
fn a_selection_paints_its_cells_and_only_them() {
    let mut app = editor_with_terminal("terminal_selection_ground");
    feed(&mut app, b"\r\nLEFT RIGHT\r\n");
    settle(&mut app, DRAW_FRAMES);
    let before = spans(&mut app);
    let row = span_with(&before, "LEFT RIGHT").row;
    assert_eq!(span_with(&before, "LEFT RIGHT").ground, None, "nothing is selected yet");

    // Select `RIGHT`: columns 5 to 9 of that row, as a drag over it would.
    app.world_mut()
        .resource_mut::<Terminal>()
        .tab_mut()
        .expect("a tab")
        .screen_mut()
        .select((row, 5), (row, 9));
    settle(&mut app, DRAW_FRAMES);
    let after = spans(&mut app);

    let right = span_with(&after, "RIGHT");
    assert_eq!(right.text, "RIGHT", "the selection is its own run, exactly the selected cells: {after:#?}");
    let ground = right.ground.expect("the selected cells are drawn on a ground");
    let left = span_with(&after, "LEFT");
    assert_eq!(left.ground, None, "an unselected cell keeps the pane's ground: {left:?}");
    assert_eq!(right.ink, left.ink, "a selection keeps the cells' own ink");
    assert_ne!(ground, right.ink, "the selection's ground is not the ink it sits under");
}
