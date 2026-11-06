//! **The caret takes the shape a program asks for, and is hollow while the shell lacks the keys** (#320).
//!
//! vim asks for a block in normal mode and a bar in insert mode (DECSCUSR, `ESC[<n> q`), which is how
//! its user knows which mode they are in; the pane drew a 2 px bar whatever was asked. VS Code honours
//! the request, draws a block by default, and outlines the caret while the terminal does not have the
//! keyboard, so a glance says where the keys go.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::terminal::{CaretShape, TerminalScreen};
use bevy_cauldron::terminal_pane::{TerminalCaret, TerminalCaretGlyph, TerminalFocus};
use bevy_cauldron::terminal_view::CellMetrics;

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed, settle};

/// Frames for a feed to be rebuilt and laid out.
const LAYOUT_FRAMES: usize = 6;

/// How many frames, 50 ms apart, a test waits for the caret's lit half of its blink.
const CARET_TRIES: usize = 60;

fn shape(screen: &mut TerminalScreen) -> CaretShape {
    screen.lines();
    screen.cursor().shape
}

#[test]
fn a_program_sets_the_carets_shape_and_a_reset_puts_the_block_back() {
    let mut screen = TerminalScreen::new(80, 24);
    assert_eq!(shape(&mut screen), CaretShape::Block, "a fresh terminal's caret is a block, as VS Code's is");
    // DECSCUSR: 1 and 2 are a block, 3 and 4 an underline, 5 and 6 a bar (blinking, then steady); 0 is
    // the terminal's default.
    for (request, want) in [
        (&b"\x1b[6 q"[..], CaretShape::Bar),
        (b"\x1b[4 q", CaretShape::Underline),
        (b"\x1b[2 q", CaretShape::Block),
        (b"\x1b[5 q", CaretShape::Bar),
        (b"\x1b[3 q", CaretShape::Underline),
        (b"\x1b[1 q", CaretShape::Block),
        (b"\x1b[6 q", CaretShape::Bar),
        (b"\x1b[0 q", CaretShape::Block),
    ] {
        screen.feed(request);
        assert_eq!(shape(&mut screen), want, "after {:?}", String::from_utf8_lossy(request));
    }
    screen.feed(b"\x1b[?25l");
    screen.lines();
    assert!(!screen.cursor().visible, "a hidden caret stays hidden, whatever its shape");
}

/// The caret's laid-out size in logical px, its border, and its ground, once it is in the lit half.
fn lit_caret(app: &mut App) -> (Vec2, f32, Color) {
    for _ in 0..CARET_TRIES {
        let world = app.world_mut();
        let (node, _, ground) = world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform, &BackgroundColor), With<TerminalCaret>>()
            .iter(world)
            .next()
            .map(|(node, transform, ground)| (*node, *transform, ground.0))
            .expect("the pane has a caret");
        if node.size().y > 0.0 {
            return (node.size() * node.inverse_scale_factor(), node.border().min_inset.x * node.inverse_scale_factor(), ground);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        settle(app, 1);
    }
    panic!("the caret was never lit in {CARET_TRIES} tries");
}

fn cell(app: &App) -> Vec2 {
    let metrics = app.world().resource::<CellMetrics>();
    Vec2::new(metrics.width(), metrics.height())
}

#[test]
fn the_caret_is_drawn_in_the_shape_asked_for() {
    let mut app = editor_with_terminal("caret_shapes");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, LAYOUT_FRAMES);
    let cell = cell(&app);

    let (block, border, ground) = lit_caret(&mut app);
    assert!(
        (block.x - cell.x).abs() < 0.5 && (block.y - cell.y).abs() < 0.5,
        "a block caret covers its cell ({cell:?}), not {block:?}"
    );
    assert_eq!(border, 0.0, "with the keyboard in the shell the block is solid, not outlined");
    assert_ne!(ground.alpha(), 0.0, "with the keyboard in the shell the block is filled");

    feed(&mut app, b"\x1b[6 q");
    settle(&mut app, LAYOUT_FRAMES);
    let (bar, _, _) = lit_caret(&mut app);
    assert!(bar.x < cell.x / 2.0, "a bar caret is a thin stroke, not {bar:?} of a {cell:?} cell");
    assert!((bar.y - cell.y).abs() < 0.5, "a bar caret is a cell tall, not {bar:?}");

    feed(&mut app, b"\x1b[4 q");
    settle(&mut app, LAYOUT_FRAMES);
    let (underline, _, _) = lit_caret(&mut app);
    assert!((underline.x - cell.x).abs() < 0.5, "an underline caret is a cell wide, not {underline:?}");
    assert!(underline.y < cell.y / 2.0, "an underline caret is a strip at the foot, not {underline:?}");
}

#[test]
fn without_the_keyboard_the_caret_is_an_outline() {
    let mut app = editor_with_terminal("caret_outline");
    app.world_mut().resource_mut::<TerminalFocus>().release();
    settle(&mut app, LAYOUT_FRAMES);
    let cell = cell(&app);
    let (size, border, ground) = lit_caret(&mut app);
    assert!(border > 0.0, "without the keyboard the caret is outlined, so a glance says the keys are elsewhere");
    assert_eq!(ground.alpha(), 0.0, "an outlined caret is not filled: it would hide the character it is on");
    assert!((size.x - cell.x).abs() < 0.5, "the outline is one cell, not {size:?}");
}

#[test]
fn a_solid_block_shows_the_character_under_it() {
    let mut app = editor_with_terminal("caret_glyph");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    // The cursor ends on the `q` of `seq`: three letters, then one step back.
    feed(&mut app, b"\r\n\x1b[2Kseq\x1b[D");
    settle(&mut app, LAYOUT_FRAMES);
    let world = app.world_mut();
    let glyph = world
        .query_filtered::<&Text, With<TerminalCaretGlyph>>()
        .iter(world)
        .next()
        .map(|text| text.0.clone())
        .expect("a block caret carries the character it covers");
    assert_eq!(glyph, "q", "the block covers the `q`, and draws it so the word stays readable");
}
