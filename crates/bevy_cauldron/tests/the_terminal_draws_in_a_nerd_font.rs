//! **The terminal draws in a Nerd Font, so a prompt's icons are glyphs and not boxes** (#323).
//!
//! starship, powerlevel10k, `eza --icons` and `lazygit` draw icons from the Private Use Area that Nerd
//! Fonts fill. The editor's face is plain FiraMono, which has none, and the terminal drew in it, so every
//! icon was a box. The terminal's rows are now drawn in FiraMono Nerd Font Mono.

use bevy::prelude::*;
use bevy::text::FontSource;

use bevy_cauldron::terminal_pane::TerminalRow;
use bevy_cauldron::terminal_view::TERMINAL_FONT;

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Frames for a feed to be rebuilt, spawned and laid out.
const DRAW_FRAMES: usize = 6;

/// Three icons prompts draw: the powerline arrow, Font Awesome's open folder, and Devicons' Rust.
const ICONS: [char; 3] = ['\u{e0b0}', '\u{f115}', '\u{e7a8}'];

/// Does the face in `bytes` have a glyph for `c`, read from its own character map?
fn covers(bytes: &[u8], c: char) -> bool {
    ttf_parser::Face::parse(bytes, 0)
        .unwrap_or_else(|e| panic!("the face does not parse: {e}"))
        .glyph_index(c)
        .is_some()
}

#[test]
fn the_terminal_draws_in_a_face_that_has_the_icons_a_prompt_uses() {
    let mut app = editor_with_terminal("nerd_font");
    feed(&mut app, "\r\n\u{e0b0} \u{f115} src \u{e7a8} main\r\n".as_bytes());
    settle(&mut app, DRAW_FRAMES);

    let world = app.world_mut();
    let faces: Vec<FontSource> = world
        .query_filtered::<&TextFont, With<TerminalRow>>()
        .iter(world)
        .map(|font| font.font.clone())
        .collect();
    assert!(!faces.is_empty(), "the terminal drew no rows, so this test proves nothing");
    assert!(
        faces.iter().all(|face| *face == FontSource::from(TERMINAL_FONT)),
        "every terminal row is drawn in the terminal's face, not {faces:?}"
    );

    let fonts = app.world().resource::<Assets<Font>>();
    let terminal = fonts.get(&TERMINAL_FONT).expect("the terminal's face is installed before a row is drawn");
    for icon in ICONS {
        assert!(covers(terminal.data.as_ref(), icon), "the terminal's face has no glyph for U+{:04X}", icon as u32);
    }
    let editor = fonts.get(AssetId::<Font>::default()).expect("the editor's face is installed");
    assert!(
        !covers(editor.data.as_ref(), ICONS[0]),
        "the editor's own face draws the powerline arrow, so the terminal would not have needed a face of its own"
    );
}
