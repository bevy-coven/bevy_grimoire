//! **Every run of a terminal row is drawn in the terminal's Nerd Font, not only the row** (#323).
//!
//! A row is one `Text` root with a `TextSpan` per colour run, and in Bevy every span is a node with its
//! own `TextFont`. The root named the Nerd Font and the spans named nothing, so they took the editor's
//! face: live, `printf '\ue0b0 \uf115 src'` drew `▯ ▯ src`. The text a person reads is the spans', so
//! they are what this asks about.

use bevy::prelude::*;
use bevy::text::FontSource;

use bevy_cauldron::terminal_pane::TerminalRow;
use bevy_cauldron::terminal_view::TERMINAL_FONT;

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Frames for a feed to be rebuilt, spawned and laid out.
const DRAW_FRAMES: usize = 6;

#[test]
fn every_run_a_row_draws_names_the_terminal_face() {
    let mut app = editor_with_terminal("nerd_font_spans");
    // Three runs on one row: plain, red, then plain again, with a prompt icon in the red one.
    feed(&mut app, "\r\n\x1b[2Kmain \x1b[31m\u{e0b0} src\x1b[0m done\r\n".as_bytes());
    settle(&mut app, DRAW_FRAMES);

    let world = app.world_mut();
    let mut spans = world.query_filtered::<(&TextSpan, &TextFont, &ChildOf), With<TextSpan>>();
    let drawn: Vec<(String, FontSource)> = spans
        .iter(world)
        .filter(|(_, _, parent)| world.get::<TerminalRow>(parent.parent()).is_some())
        .map(|(span, font, _)| (span.0.clone(), font.font.clone()))
        .collect();
    assert!(
        drawn.iter().any(|(text, _)| text.contains('\u{e0b0}')),
        "the icon's run was never drawn, so this test proves nothing: {drawn:?}"
    );
    let wrong: Vec<&String> = drawn
        .iter()
        .filter(|(_, font)| *font != FontSource::from(TERMINAL_FONT))
        .map(|(text, _)| text)
        .collect();
    assert!(
        wrong.is_empty(),
        "these runs are drawn in another face than the terminal's, so their icons draw as boxes: {wrong:?}"
    );
}
