//! **The note box's text is on screen while it is typed** (#177).
//!
//! The note box has no field of its own: `type_note` writes the draft into the Inspector's receipt row
//! (LAST). With the Inspector scrolled down to NOTES, where a person watches the list a new note joins,
//! that row was above the pane's top edge, so the typed text, its caret and `[auto]` were drawn nowhere
//! a person could see. Found recording #175 on sib, at 1280 x 720.
//!
//! The real layout, headless: the surface at 1280 x 720, the Inspector scrolled to its bottom, the box
//! opened the way `drive_note` opens it when a drag is released, and three keys typed through
//! `type_note`.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy_cauldron::editor::{EditorState, NoteBox, NoteDraft, Tool};

use crate::fixtures;

const HEIGHT: u32 = 720;

fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn tap(app: &mut App, c: char) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: match c {
                'a' => KeyCode::KeyA,
                'b' => KeyCode::KeyB,
                'c' => KeyCode::KeyC,
                _ => panic!("no key code for {c:?}"),
            },
            logical_key: Key::Character(c.to_string().into()),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **The drawn text whose words match**, laid out and shown.
fn drawn(app: &mut App, matches: impl Fn(&str) -> bool) -> Option<Entity> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Text, &ComputedNode, &Node)>();
    q.iter(world)
        .find(|(_, t, n, style)| matches(&t.0) && style.display == Display::Flex && n.size().y > 0.0)
        .map(|(e, ..)| e)
}

/// Top and bottom edge in physical pixels.
fn span(app: &App, e: Entity) -> (f32, f32) {
    let node = app.world().get::<ComputedNode>(e).expect("a laid-out node");
    let tf = app.world().get::<UiGlobalTransform>(e).expect("a laid-out node");
    let half = node.size().y * 0.5;
    (tf.translation.y - half, tf.translation.y + half)
}

/// **The nearest scrolling ancestor**, which is the edge that clips the node. Every `Node` carries a
/// `ScrollPosition`, so the scroller is the one whose overflow scrolls.
fn scrolled_parent(app: &App, mut e: Entity) -> Entity {
    while let Some(parent) = app.world().get::<ChildOf>(e).map(ChildOf::parent) {
        if app.world().get::<Node>(parent).is_some_and(|n| n.overflow.y == OverflowAxis::Scroll) {
            return parent;
        }
        e = parent;
    }
    panic!("the Inspector's receipt row is not inside a scrolling pane");
}

fn inside(row: (f32, f32), pane: (f32, f32)) -> bool {
    row.0 >= pane.0 - 0.5 && row.1 <= pane.1 + 0.5 && row.0 >= 0.0 && row.1 <= HEIGHT as f32 + 0.5
}

/// **Scrolled to the bottom, the typed text is drawn inside the pane anyway.**
///
/// Failed before the fix: the text was laid out above the Inspector's top edge, clipped away.
#[test]
fn typed_text_is_inside_the_inspector_scrolled_to_its_bottom() {
    let root = fixtures::Fixture::new(&format!("note_box_on_screen_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    bevy_cauldron::harness::resize_surface(&mut app, 1280, HEIGHT).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);

    // --- Scroll the Inspector to its bottom, as a person reading NOTES has it.
    let label = drawn(&mut app, |t| t == "LAST").expect("the Inspector draws its LAST label");
    let pane = scrolled_parent(&app, label);
    app.world_mut().get_mut::<ScrollPosition>(pane).expect("a scrolling pane").0.y = 100_000.0;
    settle(&mut app);
    // The case under test: the receipt row really is off the pane, or this proves nothing.
    assert!(
        !inside(span(&app, label), span(&app, pane)),
        "LAST {:?} is still inside the Inspector {:?} scrolled to its bottom, so the pane does not \
         overflow at 1280 x {HEIGHT} and this test no longer covers a hidden receipt",
        span(&app, label),
        span(&app, pane),
    );

    // --- Open the note box the way a drag's release does, and type into it.
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Note;
    app.world_mut().resource_mut::<NoteBox>().draft =
        Some(NoteDraft { area: [0, 0, 2, 2], editing: None, text: String::new(), pieces: Vec::new(), auto: false });
    app.update();
    for c in ['a', 'b', 'c'] {
        tap(&mut app, c);
    }
    settle(&mut app);

    let typed = drawn(&mut app, |t| t.contains("note: abc")).expect("the note box's text is laid out");
    assert!(
        inside(span(&app, typed), span(&app, pane)),
        "the typed note {:?} is at {:?}, outside the Inspector's visible {:?} (window {HEIGHT} px): the \
         person types blind",
        app.world().get::<Text>(typed).map(|t| t.0.clone()),
        span(&app, typed),
        span(&app, pane),
    );
}
