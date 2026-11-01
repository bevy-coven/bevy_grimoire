//! **A receipt does not move the panel under the author's pointer.**
//!
//! B-009. The INSPECTOR's receipt row (`Field::Last`) was empty until a note arrived, and then grew
//! to the note's height: "map is 300 x 44 x 1120 m, and the file was written" wraps to two lines,
//! so MAP SIZE, the three size fields and the composition list all moved down two lines, and moved
//! back when the next receipt was shorter. Measured in the 2026-10-21 B-008 sequences (f002 → f003):
//! the field the author had just typed into jumped away from the pointer.
//!
//! **The real geometry, from the real layout**: `ComputedNode` and `UiGlobalTransform` in a headless
//! app, and the receipt produced by the size field itself, through its own `Activate` and keys.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **Resize one axis through the size field**, as a person does: open it, type, Enter.
fn resize(app: &mut App, axis: char, metres: &str) {
    bevy_cauldron::editor::size_field_for_test(app.world_mut(), axis);
    app.update();
    for c in metres.chars() {
        let code = match c {
            '0' => KeyCode::Digit0,
            '1' => KeyCode::Digit1,
            '2' => KeyCode::Digit2,
            '3' => KeyCode::Digit3,
            '4' => KeyCode::Digit4,
            '5' => KeyCode::Digit5,
            '6' => KeyCode::Digit6,
            '7' => KeyCode::Digit7,
            '8' => KeyCode::Digit8,
            '9' => KeyCode::Digit9,
            _ => panic!("not a digit: {c}"),
        };
        tap(app, Key::Character(c.to_string().into()), code);
    }
    tap(app, Key::Enter, KeyCode::Enter);
}

/// **Every drawn text and its box**, in physical pixels: (text, top y, height).
fn drawn_texts(app: &mut App) -> Vec<(String, f32, f32)> {
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform, &bevy::ui::Node)>();
    q.iter(world)
        .filter(|(_, node, tf, style)| {
            style.display == bevy::ui::Display::Flex
                && node.size().x > 0.0
                && node.size().y > 0.0
                && tf.translation.y.is_finite()
        })
        .map(|(t, node, tf, _)| {
            (
                t.0.clone(),
                tf.translation.y - node.size().y / 2.0,
                node.size().y,
            )
        })
        .collect()
}

fn top_of(texts: &[(String, f32, f32)], want: &str) -> f32 {
    texts
        .iter()
        .find(|(t, _, _)| t == want)
        .map(|(_, top, _)| *top)
        .unwrap_or_else(|| panic!("no drawn text reads {want:?}; drawn: {texts:?}"))
}

/// **A two-line receipt leaves MAP SIZE where it was.**
///
/// Failed before the fix: the heading moved down by the receipt's two lines.
#[test]
fn the_size_receipt_does_not_push_map_size_down() {
    let mut app = editor("receipt-holds-still");
    let before = drawn_texts(&mut app);
    let heading = top_of(&before, "MAP SIZE  (m)");

    resize(&mut app, 'y', "44");
    let after = drawn_texts(&mut app);

    // --- The receipt is really on screen, so the test is about a receipt that is there.
    let receipt = after
        .iter()
        .find(|(t, _, _)| t.contains("and the file was written"))
        .unwrap_or_else(|| panic!("the resize's receipt is not drawn; drawn: {after:?}"));
    // --- And it is the case under test: more than one line, the height the reserve has to hold.
    // Measured against a one-line text of the same role in the same layout (the X size field reads
    // the fixture's 16 m), so the comparison is in the layout's own pixels.
    let one_line = after
        .iter()
        .find(|(t, _, _)| t == "16")
        .map(|(_, _, h)| *h)
        .unwrap_or_else(|| panic!("no size field reads 16; drawn: {after:?}"));
    assert!(
        receipt.2 > 1.5 * one_line,
        "the receipt {:?} is {:.1} px tall against a {one_line:.1} px line, so it did not wrap and \
         this test no longer covers a two-line receipt",
        receipt.0,
        receipt.2,
    );
    let moved = top_of(&after, "MAP SIZE  (m)") - heading;
    assert!(
        moved.abs() < 0.5,
        "the receipt {:?} ({:.1} px tall) pushed MAP SIZE down by {moved:.1} px; the field the author \
         just typed into moved away from the pointer",
        receipt.0,
        receipt.2,
    );
}
