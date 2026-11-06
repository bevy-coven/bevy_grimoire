//! **Every row the shell was told about is drawn inside the pane, and the caret on the cursor** (#310).
//!
//! The editor runs at `UiScale(chrome::EDITOR_UI_SCALE)` (1.2), and the headless harness does too. The
//! shell's row count comes from the body's logical height over the cell's logical height; the rows
//! were then given `Val::Px` of the cell's *physical* height, which Bevy scales a second time, so each
//! row was drawn 20 % taller than its cell. Live, after a screenful of output, the prompt row fell
//! below the body and the caret sat rows above it. These tests put the cursor on the shell's last row
//! — where a prompt is after any long command — and read the laid-out rectangles.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalCaret, TerminalPane, TerminalRow};

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Frames for a feed to be rebuilt, spawned and laid out.
const LAYOUT_FRAMES: usize = 6;

/// A laid-out node's top and bottom, in the surface's pixels.
fn span_y(node: &ComputedNode, transform: &UiGlobalTransform) -> (f32, f32) {
    let centre = transform.translation.y;
    let half = node.size().y / 2.0;
    (centre - half, centre + half)
}

/// The app with the shell's cursor moved to its last row, as a screenful of output leaves it.
fn cursor_on_the_last_row(tag: &str) -> (App, usize) {
    let mut app = editor_with_terminal(tag);
    let rows = app.world().resource::<TerminalPane>().cells().1 as usize;
    assert!(rows >= 4, "the pane was sized to a usable terminal, not {rows} rows");
    feed(&mut app, "\r\n".repeat(rows).as_bytes());
    settle(&mut app, LAYOUT_FRAMES);
    let cursor = app
        .world()
        .resource::<Terminal>()
        .tab()
        .expect("a tab")
        .screen()
        .cursor()
        .row;
    assert_eq!(cursor, rows - 1, "a screenful of newlines leaves the cursor on the last row");
    (app, rows)
}

/// The rectangle of the drawn row `index`.
fn row_span(app: &mut App, index: usize) -> (f32, f32) {
    let world = app.world_mut();
    let mut query = world.query::<(&TerminalRow, &ComputedNode, &UiGlobalTransform)>();
    query
        .iter(world)
        .find(|(row, _, _)| row.0 == index)
        .map(|(_, node, transform)| span_y(node, transform))
        .unwrap_or_else(|| panic!("no drawn row {index}"))
}

#[test]
fn every_row_the_shell_was_told_about_lies_inside_the_body() {
    let (mut app, rows) = cursor_on_the_last_row("terminal_rows_fit");
    let world = app.world_mut();
    let (body_top, body_bottom) = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<TerminalBody>>()
        .iter(world)
        .next()
        .map(|(node, transform)| span_y(node, transform))
        .expect("the pane has a body");
    let (_, last_bottom) = row_span(&mut app, rows - 1);
    assert!(
        last_bottom <= body_bottom + 0.5,
        "the shell has {rows} rows but its last row ends at y = {last_bottom} px, below the body's \
         bottom at {body_bottom} px (body from {body_top}): the prompt there is clipped out of sight"
    );
}

#[test]
fn the_caret_sits_on_the_cursors_row() {
    let (mut app, rows) = cursor_on_the_last_row("terminal_caret_row");
    let (top, bottom) = row_span(&mut app, rows - 1);
    let world = app.world_mut();
    let (caret_top, caret_bottom) = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<TerminalCaret>>()
        .iter(world)
        .next()
        .map(|(node, transform)| span_y(node, transform))
        .expect("the pane has a caret");
    let middle = (caret_top + caret_bottom) / 2.0;
    assert!(
        (top..=bottom).contains(&middle),
        "the cursor is on row {} (y {top}..{bottom} px) but the caret is drawn at y {caret_top}..{caret_bottom} px",
        rows - 1
    );
}
