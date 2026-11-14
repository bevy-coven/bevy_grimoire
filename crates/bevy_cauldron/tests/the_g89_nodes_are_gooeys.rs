//! **The panels, the journal, the bottom dock and the terminal's own nodes are gooey's** (#260, UI-99, round 10: bevy_gooey G-89).
//!
//! `panel_root`, the journal, the bottom dock (spawned CLOSED through `ShownFollow::closed()`, their `display` still written by the editor's own systems), and the terminal's grip, bar, body, caret
//! and rows (placed / sized / `HostGeometry`) were raw `Node`s. This file asks a running editor what they are and what they measure.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::chrome::{self, BottomDock, Frame, JournalPanel};
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::ShownFollow;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("g89_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **The journal starts closed and keeps its place**: 8% down, 10% in, 80% wide, at most 76% tall, hovered (it must not be click-through), closed with nothing focusable.
#[test]
fn the_journal_starts_closed_and_is_placed() {
    let mut app = editor("journal");
    let world = app.world_mut();
    let journal = world.query_filtered::<Entity, With<JournalPanel>>().iter(world).next().expect("the journal panel");
    let n = world.get::<Node>(journal).expect("a Node");
    assert_eq!((n.display, n.position_type), (Display::None, PositionType::Absolute), "closed until Cmd+E, out of the flow");
    assert_eq!(world.get::<Visibility>(journal), Some(&Visibility::Hidden), "and hidden from focus");
    assert_eq!((n.top, n.left, n.width, n.max_height), (Val::Percent(8.0), Val::Percent(10.0), Val::Percent(80.0), Val::Percent(76.0)));
    assert_eq!((n.row_gap, n.padding.left, n.border.left), (Val::Px(5.0), Val::Px(12.0), Val::Px(1.0)));
    assert!(world.get::<ShownFollow>(journal).is_some());
    assert_ne!(world.get::<Pickable>(journal), Some(&Pickable::IGNORE), "the journal answers the pointer, so a click on it never reaches the map behind it");
    // Opening it is the editor's own write of `display`: the skin gives the visibility back and leaves the display alone.
    world.get_mut::<Node>(journal).expect("a Node").display = Display::Flex;
    for _ in 0..3 {
        app.update();
    }
    let world = app.world();
    assert_eq!(world.get::<Node>(journal).map(|n| n.display), Some(Display::Flex), "the skin does not close it again");
    assert_ne!(world.get::<Visibility>(journal), Some(&Visibility::Hidden), "and it is visible once open");
}

/// **The bottom dock starts closed and keeps its height, width and top edge.**
#[test]
fn the_bottom_dock_starts_closed_at_its_height() {
    let app = editor("dock");
    let frame = app.world().resource::<Frame>();
    let dock = frame.bottom;
    let world = app.world();
    assert!(world.get::<BottomDock>(dock).is_some());
    let n = world.get::<Node>(dock).expect("a Node");
    assert_eq!((n.width, n.height, n.flex_shrink, n.border.top), (Val::Percent(100.0), Val::Px(chrome::BOTTOM_DOCK_H), 0.0, Val::Px(1.0)), "the window wide, 148 px tall, never shrinking, a 1 px top edge");
    assert!(world.get::<ShownFollow>(dock).is_some(), "`give_the_bottom_dock_its_room` writes its display");
    assert_eq!(world.get::<BorderColor>(dock).map(|b| b.top), Some(chrome::ink().panel_edge), "in the editor's edge colour");
}

/// **The terminal's grip, bar, body and caret are gooey nodes with the numbers they had** (found by their markers; nothing here starts a shell).
#[test]
fn the_pane_s_grip_bar_body_and_caret_are_gooey_nodes() {
    use bevy_cauldron::terminal_pane::{TerminalBar, TerminalBody, TerminalCaret, TerminalGrip};
    let mut app = editor("pane");
    let world = app.world_mut();
    let grip = world.query_filtered::<Entity, With<TerminalGrip>>().iter(world).next().expect("the pane's grip");
    let bar = world.query_filtered::<Entity, With<TerminalBar>>().iter(world).next().expect("the bar");
    let body = world.query_filtered::<Entity, With<TerminalBody>>().iter(world).next().expect("the body");
    let caret = world.query_filtered::<Entity, With<TerminalCaret>>().iter(world).next().expect("the caret");
    let node = |e: Entity| world.get::<Node>(e).unwrap_or_else(|| panic!("{e:?} has a Node"));
    assert_eq!((node(grip).width, node(grip).flex_shrink), (Val::Percent(100.0), 0.0));
    let b = node(bar);
    assert_eq!((b.height, b.column_gap, b.padding.left, b.align_items, b.flex_shrink, b.flex_direction), (Val::Px(24.0), Val::Px(3.0), Val::Px(12.0), AlignItems::Center, 0.0, FlexDirection::Row), "a 24 px bar, 3 px between parts, 12 px in");
    let d = node(body);
    assert_eq!((d.flex_grow, d.min_height, d.flex_direction, d.overflow), (1.0, Val::ZERO, FlexDirection::Column, Overflow::clip()), "the body grows, clips, and is a column");
    assert_eq!((d.padding.left, d.padding.top), (Val::Px(bevy_cauldron::terminal_view::BODY_INSET.x), Val::Px(bevy_cauldron::terminal_view::BODY_INSET.y)));
    assert!(world.get::<bevy_gooey::HostGeometry>(caret).is_some(), "the caret's geometry is `draw_the_terminal`'s");
    assert_eq!(node(caret).position_type, PositionType::Absolute);
    assert_eq!(world.get::<Pickable>(caret), Some(&Pickable::IGNORE), "and it is never a target");
    let _ = world.get::<ComputedNode>(grip);
}
