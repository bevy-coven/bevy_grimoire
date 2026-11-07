//! **A line as wide as the terminal says it is fits inside the terminal** (feedback-1794011769640, oracle
//! `containment`).
//!
//! Reported on mac_air, 0.3.8, 2560x1406 at scale 2 (`ui_scale` 2.4): *"I went into OhMyPi here and the screen
//! started stretching to the right without stopping."* omp redraws a status rule exactly as many columns wide
//! as the terminal reports. The terminal's cell had been measured short (from the ink of the prompt's first two
//! letters, `l` and `a`), so it reported more columns than fit: the rule overflowed the body, the body widened
//! to hold it, the wider body reported more columns, and omp drew a wider rule. Reproduced on the installed
//! 0.3.8: the body grew from 2560 to 3774 px inside a 2560 px window in 17 s.
//!
//! The shell here prints the macOS prompt and nothing else, so the first row the pane measures starts with the
//! same two letters as the capture's; then the test plays omp, one redraw at a time.

use std::os::unix::fs::PermissionsExt;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalPane, TerminalRow};
use bevy_cauldron::terminal_view::CellMetrics;

use crate::fixtures;

/// The capture's prompt: zsh's default on that Mac.
const PROMPT: &str = "ladvien@mac_air ~ % ";
/// The capture's window, logical, and its display's scale.
const WINDOW_LOGICAL: (u32, u32) = (1280, 703);
const SCALE: f32 = 2.0;
/// How many times omp redraws its rule here: the capture's growth was visible within two.
const REDRAWS: usize = 8;
/// Frames for a redraw to be drawn, laid out, and answered with a new column count.
const FRAMES_PER_REDRAW: usize = 4;
/// How many 50 ms waits the stand-in shell has to print its prompt.
const PROMPT_TRIES: usize = 100;
const CHECK_GAP: std::time::Duration = std::time::Duration::from_millis(50);

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn feed(app: &mut App, bytes: &[u8]) {
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    terminal.tab_mut().expect("the raised pane has a tab").screen_mut().feed(bytes);
}

/// A laid-out node's left and right edges, in the surface's pixels.
fn span_x(node: &ComputedNode, transform: &UiGlobalTransform) -> (f32, f32) {
    let half = node.size().x / 2.0;
    (transform.translation.x - half, transform.translation.x + half)
}

fn body_span(app: &mut App) -> (f32, f32) {
    let world = app.world_mut();
    world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<TerminalBody>>()
        .iter(world)
        .next()
        .map(|(node, transform)| span_x(node, transform))
        .expect("the pane has a body")
}

fn row_right(app: &mut App, index: usize) -> f32 {
    let world = app.world_mut();
    world
        .query::<(&TerminalRow, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .find(|(row, _, _)| row.0 == index)
        .map(|(_, node, transform)| span_x(node, transform).1)
        .unwrap_or_else(|| panic!("no drawn row {index}"))
}

/// The editor at the capture's window, its terminal open on a shell that prints the macOS prompt.
fn the_capture_editor() -> App {
    let root = fixtures::Fixture::new(&format!("capture_1794011769640_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let shell = root.join("prompt-only-shell");
    std::fs::write(&shell, format!("#!/bin/sh\nprintf '%s' '{PROMPT}'\nexec sleep 600\n")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    // **`SHELL` is process-wide**: nextest runs each test in its own process, which is the gate's runner.
    unsafe { std::env::set_var("SHELL", &shell) };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let mut resolution = WindowResolution::new(WINDOW_LOGICAL.0, WINDOW_LOGICAL.1);
    resolution.set_physical_resolution(
        (WINDOW_LOGICAL.0 as f32 * SCALE) as u32,
        (WINDOW_LOGICAL.1 as f32 * SCALE) as u32,
    );
    resolution.set_scale_factor_override(Some(SCALE));
    let window = app.world_mut().spawn((Window { resolution, ..default() }, PrimaryWindow)).id();
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
        if app.world().resource::<CellMetrics>().is_measured() {
            return app;
        }
        std::thread::sleep(CHECK_GAP);
    }
    panic!("the shell's prompt was never drawn, so the cell was never measured");
}

#[test]
fn a_rule_as_wide_as_the_terminal_says_stays_inside_it() {
    let mut app = the_capture_editor();
    let window_right = WINDOW_LOGICAL.0 as f32 * SCALE;
    let mut seen = Vec::new();
    for _ in 0..REDRAWS {
        let cols = app.world().resource::<TerminalPane>().cells().0 as usize;
        let rule = "-".repeat(cols);
        feed(&mut app, format!("\x1b[2J\x1b[H{PROMPT}\r\n{rule}").as_bytes());
        settle(&mut app, FRAMES_PER_REDRAW);
        let (_, body_right) = body_span(&mut app);
        seen.push((cols, body_right, row_right(&mut app, 1)));
    }
    for &(cols, body_right, rule_right) in &seen {
        assert!(
            body_right <= window_right + 0.5,
            "the terminal's body ends at x = {body_right} px, past the window's edge at {window_right}: the pane \
             was widened by a {cols}-column rule (cols, body right, rule right per redraw: {seen:?})"
        );
        assert!(
            rule_right <= body_right + 0.5,
            "a rule of the {cols} columns the terminal reported ends at x = {rule_right} px, outside its body \
             (ending at {body_right}): it reported more columns than it holds ({seen:?})"
        );
    }
    let (first, last) = (seen[0].0, seen[seen.len() - 1].0);
    assert_eq!(first, last, "the column count climbed as omp redrew: {seen:?}");
}

/// The caret's laid-out left edge, in the surface's pixels, once it is in the lit half of its blink: it blinks
/// by `Display`, so in its dark half it is a zero-size box.
fn lit_caret_left(app: &mut App) -> f32 {
    for _ in 0..PROMPT_TRIES {
        let world = app.world_mut();
        let (node, transform) = world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<bevy_cauldron::terminal_pane::TerminalCaret>>()
            .iter(world)
            .next()
            .map(|(node, transform)| (*node, *transform))
            .expect("the pane has a caret");
        if node.size().y > 0.0 {
            return span_x(&node, &transform).0;
        }
        std::thread::sleep(CHECK_GAP);
        settle(app, 1);
    }
    panic!("the caret was never lit in {PROMPT_TRIES} tries");
}

/// **The caret is drawn on the cell after the prompt**, where the next typed character goes. Reported with the
/// same build on the same Mac: "why is the terminal cursor not lined up with what's typed?" Its box stood on the
/// apostrophe of `Let's` while the shell's cursor was after the `s`.
#[test]
fn the_caret_is_on_the_cell_its_column_is_drawn_at() {
    let mut app = the_capture_editor();
    let cell_px = app.world().resource::<CellMetrics>().width() * app.world().resource::<UiScale>().0;
    let world = app.world_mut();
    let first_row_left = world
        .query::<(&TerminalRow, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .find(|(row, _, _)| row.0 == 0)
        .map(|(_, node, transform)| span_x(node, transform).0)
        .expect("the prompt's row is drawn");
    let column = PROMPT.chars().count();
    let want = first_row_left + column as f32 * cell_px;
    let caret = lit_caret_left(&mut app);
    assert!(
        (caret - want).abs() < 0.5,
        "the cursor is after the prompt, at column {column}, which the row draws at x = {want} px; the caret is at \
         x = {caret} px, {:.1} cells off",
        (caret - want) / cell_px
    );
}
