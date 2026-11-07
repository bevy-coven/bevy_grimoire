//! **The block caret lets its cell show through** (owner, 2026-11-06: "should be somewhat transparent, since it
//! blinks slowly").
//!
//! The block was the accent at full strength with the covered character in the pane's ground on it, so for the
//! long lit half of its blink the cell read as a solid slab. Now the fill is a share of the accent and the
//! character on it is drawn in that cell's own ink: the letter under the caret reads as it does anywhere else,
//! on a tinted cell.

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_cauldron::terminal_pane::{TerminalCaret, TerminalCaretGlyph, TerminalFocus, TerminalRow};

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Frames for a feed to be rebuilt and laid out.
const LAYOUT_FRAMES: usize = 6;
/// How many frames, 50 ms apart, the test waits for the caret's lit half of its blink.
const CARET_TRIES: usize = 60;

#[test]
fn a_block_caret_is_a_tint_with_the_letter_in_its_own_ink() {
    let mut app = editor_with_terminal("caret_tint");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    // The cursor ends on the `q` of `seq`: three letters, then one step back.
    feed(&mut app, b"\r\n\x1b[2Kseq\x1b[D");
    settle(&mut app, LAYOUT_FRAMES);

    let mut fill = None;
    for _ in 0..CARET_TRIES {
        let world = app.world_mut();
        let (size, ground) = world
            .query_filtered::<(&ComputedNode, &BackgroundColor), With<TerminalCaret>>()
            .iter(world)
            .next()
            .map(|(node, ground)| (node.size(), ground.0))
            .expect("the pane has a caret");
        if size.y > 0.0 {
            fill = Some(ground);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        settle(&mut app, 1);
    }
    let fill = fill.unwrap_or_else(|| panic!("the caret was never lit in {CARET_TRIES} tries"));
    assert!(
        fill.alpha() > 0.0 && fill.alpha() < 1.0,
        "a block caret is a tint its cell shows through, not an opaque slab: its fill's alpha is {}",
        fill.alpha()
    );

    let world = app.world_mut();
    let (letter, ink) = world
        .query_filtered::<(&Text, &TextColor), With<TerminalCaretGlyph>>()
        .iter(world)
        .next()
        .map(|(text, ink)| (text.0.clone(), ink.0))
        .expect("a block caret carries the character it covers");
    assert_eq!(letter, "q", "the block is on the `q`");
    let row_ink = world
        .query::<(&TextSpan, &TextColor, &ChildOf)>()
        .iter(world)
        .filter(|(span, _, parent)| span.0.contains("seq") && world.get::<TerminalRow>(parent.parent()).is_some())
        .map(|(_, ink, _)| ink.0)
        .next()
        .expect("the row drawing `seq` is in the pane");
    assert_eq!(
        ink, row_ink,
        "the letter on the caret is drawn in the ink its row draws it in, so it reads the same through the tint"
    );
}
