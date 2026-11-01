//! **The INSPECTOR's status slot is one labelled row, and a hint does not move the panel.**
//!
//! B-012. The receipt sits in UNDER's value column with no label of its own, so "map is 300 x 44 x
//! 1120 m, and the file was written" reads as the value of UNDER, the row above it.
//!
//! B-013. The hint row (`Field::Hint`) was zero tall while it was empty, so a hint appearing (a
//! surface or mounted piece armed over floor it cannot stand on) pushed EDGES, MAP SIZE and the
//! lists below it down. The fix puts the hint in the receipt's slot: while a hint speaks, the slot
//! shows the hint (labelled `HINT`) and the receipt waits; when the hint clears, the receipt returns.
//!
//! **The real geometry, from the real layout**: `ComputedNode` and `UiGlobalTransform` in a headless
//! app, the receipt produced by the size field itself, and the hint set on the editor's state.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::chrome::BODY_LINE_H;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .mounted_descriptor("market_stall_awning_with_lanterns", "alpha", "worktop")
        .surface_descriptor("worktop_table", "alpha", "worktop")
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

/// **The hint the ghost writes over bare floor for `id`.** The core's refusal for a surface piece
/// with no surface under it, put through the editor's own `ghost_hint`, so a test that sets the hint
/// sets the one the ghost would write.
fn ghost_text(app: &App, id: &str) -> String {
    let project = app.world().resource::<Project>();
    let map = &app.world().resource::<OpenMap>().map;
    let d = project
        .library
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("no descriptor {id}"));
    let ys = bevy_grimoire_core::stack::resolve_y(map, &project.library).expect("heights resolve");
    let refusal = match bevy_grimoire_core::stack::placement_at(map, &project.library, &ys, d, (0.0, 0.0)) {
        Err(e) => e,
        Ok(_) => panic!("{id} found a surface at the origin; the fixture should have none"),
    };
    bevy_cauldron::editor::ghost_hint(d, &refusal)
}

/// **A hint leaves MAP SIZE where it was.**
///
/// Failed before the fix: with the hint set, the hint row grew from zero and MAP SIZE moved down by
/// the hint's height.
#[test]
fn a_hint_does_not_push_map_size_down() {
    let mut app = editor("hint-holds-still");
    let before = drawn_texts(&mut app);
    let heading = top_of(&before, "MAP SIZE  (m)");

    let hint = ghost_text(&app, "market_stall_awning_with_lanterns");
    app.world_mut().resource_mut::<EditorState>().hint = hint.to_owned();
    let after = drawn_texts(&mut app);

    assert!(
        after.iter().any(|(t, _, _)| *t == hint),
        "the hint is not drawn, so this test is not about a hint that is there; drawn: {after:?}"
    );
    let moved = top_of(&after, "MAP SIZE  (m)") - heading;
    assert!(
        moved.abs() < 0.5,
        "the hint pushed MAP SIZE down by {moved:.1} px; a hint that comes and goes with the cursor \
         must not move the panel under the author's pointer"
    );
}

/// **The receipt has a label, so it does not read as the value of the row above it.**
///
/// Failed before the fix: the receipt's row had an empty label, and no text read `LAST` at its top.
#[test]
fn the_receipt_row_is_labelled_so_it_is_not_taken_for_under() {
    let mut app = editor("receipt-is-labelled");
    resize(&mut app, 'y', "44");
    let after = drawn_texts(&mut app);

    let receipt = after
        .iter()
        .find(|(t, _, _)| t.contains("and the file was written"))
        .unwrap_or_else(|| panic!("the resize's receipt is not drawn; drawn: {after:?}"));
    let label = top_of(&after, "LAST");
    assert!(
        (label - receipt.1).abs() < 0.5,
        "the LAST label sits at {label:.1} px but the receipt at {:.1} px; the receipt has no \
         label on its own row",
        receipt.1,
    );
}

/// **A receipt longer than two lines is drawn whole.** The LAST slot reserves two lines (B-009), but
/// a longer receipt grows the row to fit it; it is never cut.
///
/// Failed before the fix: `cbf3423` put `Overflow::clip_y()` on the value column at two lines, so a
/// third line of the receipt was painted into nothing. The text node itself still measured three
/// lines, so only the clipping ancestor shows it — which is what this test reads.
#[test]
fn a_long_receipt_is_drawn_whole() {
    let mut app = editor("receipt-whole");
    let receipt = "ran 8 piece(s) along the line, and this receipt is long enough that it wraps onto a \
                   second line and then keeps going onto a third one";
    app.world_mut().resource_mut::<EditorState>().status.note(receipt.to_owned());
    let drawn = drawn_texts(&mut app);
    assert!(
        drawn.iter().any(|(t, _, _)| t == receipt),
        "the receipt is not drawn, so the test drives nothing; drawn: {drawn:?}"
    );

    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Text)>();
    let entity = q
        .iter(world)
        .find(|(_, t)| t.0 == receipt)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no entity holds the receipt"));

    let bounds = |world: &World, e: Entity| -> (f32, f32) {
        let node = world.get::<ComputedNode>(e).expect("a drawn text has a ComputedNode");
        let tf = world.get::<UiGlobalTransform>(e).expect("a drawn text has a transform");
        let h = node.size().y;
        (tf.translation.y - h / 2.0, tf.translation.y + h / 2.0)
    };
    let (top, bottom) = bounds(world, entity);
    assert!(
        bottom - top > 2.0 * 14.0,
        "the receipt is not three lines tall ({:.1} px), so the test does not reach the clip",
        bottom - top
    );

    let mut at = entity;
    while let Some(parent) = world.get::<ChildOf>(at).map(|c| c.parent()) {
        at = parent;
        let Some(node) = world.get::<bevy::ui::Node>(at) else { continue };
        if node.overflow.y == bevy::ui::OverflowAxis::Visible {
            continue;
        }
        let (ptop, pbottom) = bounds(world, at);
        assert!(
            bottom <= pbottom + 0.5 && top >= ptop - 0.5,
            "the receipt spans {top:.1}..{bottom:.1} px but a clipping ancestor shows only \
             {ptop:.1}..{pbottom:.1} px; the receipt is cut, not drawn whole"
        );
    }
}

/// **A hint and a receipt share the slot, and the hint speaks while it is set.** While the hint is
/// set, the receipt is not drawn; when the hint clears, the receipt comes back.
///
/// Failed before the fix: the hint and the receipt were drawn in two rows, so the receipt was still
/// in the list while the hint was set.
#[test]
fn the_receipt_returns_when_the_hint_clears() {
    let mut app = editor("receipt-returns");
    let receipt = "map size to 300 x 44 x 1120 m, and the file was written";
    app.world_mut().resource_mut::<EditorState>().status.note(receipt.to_owned());
    let receipted = drawn_texts(&mut app);
    assert!(
        receipted.iter().any(|(t, _, _)| t == receipt),
        "the receipt is not drawn before the hint, so the test drives nothing; drawn: {receipted:?}"
    );

    let hint = ghost_text(&app, "market_stall_awning_with_lanterns");
    app.world_mut().resource_mut::<EditorState>().hint = hint.to_owned();
    let hinted = drawn_texts(&mut app);
    assert!(
        hinted.iter().any(|(t, _, _)| *t == hint),
        "the hint is not drawn; drawn: {hinted:?}"
    );
    assert!(
        !hinted.iter().any(|(t, _, _)| t == receipt),
        "the receipt is still drawn while the hint speaks, so they share no slot; drawn: {hinted:?}"
    );

    app.world_mut().resource_mut::<EditorState>().hint.clear();
    let cleared = drawn_texts(&mut app);
    assert!(
        cleared.iter().any(|(t, _, _)| t == receipt),
        "the receipt did not come back when the hint cleared; drawn: {cleared:?}"
    );
    assert!(
        !cleared.iter().any(|(t, _, _)| *t == hint),
        "the hint is still drawn after it cleared; drawn: {cleared:?}"
    );
}

/// **A surface piece's hint fits the two body lines the LAST slot holds.** The core's refusal names
/// the piece and its class in a sentence that ran to three lines, and the slot is two (B-009), so the
/// hint the ghost writes is the short sentence and this holds it to two lines.
///
/// Failed before the fix: `ghost_hint` passed the refusal through, which the layout drew three body
/// lines tall.
#[test]
fn a_surface_hint_fits_the_two_lines_the_slot_holds() {
    let mut app = editor("hintlines");
    let hint = ghost_text(&app, "market_stall_awning_with_lanterns");
    app.world_mut().resource_mut::<EditorState>().hint = hint.clone();
    let drawn = drawn_texts(&mut app);
    let (_, _, height) = drawn
        .iter()
        .find(|(t, _, _)| *t == hint)
        .unwrap_or_else(|| panic!("the hint is not drawn; drawn: {drawn:?}"));
    let lines = height / BODY_LINE_H;
    assert!(
        lines <= 2.01,
        "the hint is {lines:.2} body lines tall ({height} px); the LAST slot holds two; hint: {hint:?}"
    );
}
