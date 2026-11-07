//! **The caret sits right after the last character drawn, wide characters included** (terminal screenshots on
//! mac_air, 2026-11-06).
//!
//! Typed into zsh, `echo 'cafe: café naïve — wide: 日本語 テスト 🎉 end'` drew `end'` and then the caret seven
//! cells further right: each of the seven wide characters takes two columns in the grid, drew as one cell,
//! and lost its second. Everything after a wide character, the caret included, then sat one cell off its
//! column per wide character before it, and a press or a selection there picked the wrong character.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalCaret, TerminalFocus, TerminalRow};

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Frames for a feed to be rebuilt and laid out.
const LAYOUT_FRAMES: usize = 6;
/// How many frames, 50 ms apart, the test waits for the caret's lit half of its blink.
const CARET_TRIES: usize = 60;

#[test]
fn the_caret_after_wide_characters_is_where_the_text_ends() {
    let mut app = editor_with_terminal("caret_wide");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    // A fresh row holding two wide characters, a space and `ok`, with the cursor after the `k`.
    feed(&mut app, "\r\n\x1b[2K日本 ok".as_bytes());
    settle(&mut app, LAYOUT_FRAMES);
    let row = app.world().resource::<Terminal>().tab().expect("a tab").screen().cursor().row;

    let world = app.world_mut();
    let (text_left, text_width) = world
        .query::<(&TerminalRow, &ComputedNode, &UiGlobalTransform, &TextLayoutInfo)>()
        .iter(world)
        .find(|(r, ..)| r.0 == row)
        .map(|(_, node, transform, layout)| (transform.translation.x - node.size().x / 2.0, layout.size.x))
        .expect("the cursor's row is drawn");

    let mut caret_left = None;
    for _ in 0..CARET_TRIES {
        let world = app.world_mut();
        let (node, transform) = world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<TerminalCaret>>()
            .iter(world)
            .next()
            .map(|(node, transform)| (*node, *transform))
            .expect("the pane has a caret");
        if node.size().y > 0.0 {
            caret_left = Some(transform.translation.x - node.size().x / 2.0);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        settle(&mut app, 1);
    }
    let caret_left = caret_left.unwrap_or_else(|| panic!("the caret was never lit in {CARET_TRIES} tries"));
    let text_right = text_left + text_width;
    assert!(
        (caret_left - text_right).abs() < 1.0,
        "the row's text ends at x = {text_right} px and the caret is at x = {caret_left} px: with the cursor after \
         the last character the two meet, wide characters or not"
    );
}
