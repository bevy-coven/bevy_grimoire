//! **Borders in px, off-scale paddings, hidden status items and per-frame geometry are gooey's** (#260, UI-99, round 7: bevy_gooey G-86).
//!
//! The snip rectangle's and the report outline's 3 px edge, the cost readout's and the chips' 8 / 6 px insets, the jobs and wish status items and the compass
//! used to be `Node` literals. They are gooey's `BorderPx`, `Space::Px`, `status_item_quiet` + `Shown` and `placed` + `HostGeometry` now: this file asks a running editor
//! what they are and what they drew.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::{Role, Shown, Space};

use crate::fixtures;

fn editor(tag: &str, pieces: usize) -> App {
    let mut fixture = fixtures::Fixture::new(&format!("borders_{tag}_{}", std::process::id()));
    for i in 0..pieces {
        fixture = fixture.descriptor(&format!("piece_{i}"), "alpha");
    }
    let root = fixture.place("piece_0", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The jobs item is a quiet status item** that stays off the `Activate` bus, is hidden through `Shown` while nothing runs, and shows when a job does.
#[test]
fn the_jobs_item_is_a_quiet_status_item_hidden_through_shown() {
    use bevy_cauldron::jobs::JobsItem;
    let mut app = editor("jobs", 3);
    let item = app.world_mut().query_filtered::<Entity, With<JobsItem>>().single(app.world()).expect("the band carries the job item");
    assert_eq!(app.world().get::<Role>(item), Some(&Role::StatusItem), "the item is gooey's status item, not a raw `Node`");
    assert!(app.world().get::<bevy::ui_widgets::Button>(item).is_none(), "and it is not on the `Activate` bus");
    // The bake is running (a headless editor cannot finish a portrait), so the item shows.
    assert_eq!(app.world().get::<Shown>(item).map(|s| s.0), Some(true), "a running bake shows the item");
    assert_eq!(app.world().get::<Node>(item).expect("a Node").display, Display::Flex, "and takes its room");
    // Its words are a control's, never a button's.
    let words = app.world().get::<Children>(item).into_iter().flat_map(|c| c.iter()).find(|c| app.world().get::<Text>(*c).is_some()).expect("the item holds its words");
    assert_eq!(app.world().get::<Role>(words), Some(&Role::ControlText), "the words are `ControlText`: a job row is not a button");
}

/// **The wish chip is hidden until a row waits**, through `Shown`: no display flip, and none of it in the tab order.
#[test]
fn the_wish_chip_is_hidden_through_shown_until_a_row_waits() {
    use bevy_cauldron::wish_view::WishChip;
    let mut app = editor("chip", 1);
    let chip = app.world_mut().query_filtered::<Entity, With<WishChip>>().single(app.world()).expect("the band carries the wish chip");
    assert_eq!(app.world().get::<Shown>(chip).map(|s| s.0), Some(false), "nothing waits, so the chip is not shown");
    assert_eq!(app.world().get::<Node>(chip).expect("a Node").display, Display::None, "it takes no room");
    assert_eq!(*app.world().get::<Visibility>(chip).expect("a Visibility"), Visibility::Hidden, "and nothing in it is visible or focusable");
}

/// **The triangle readout keeps its reserved column and its 8 px inset**, now `Sizing` + `Spacing { Space::Px }`.
#[test]
fn the_cost_readout_keeps_its_column_and_its_inset() {
    use bevy_cauldron::editor::{COST_COL, COST_PAD_X, TriangleTotal};
    let mut app = editor("cost", 1);
    let words = app.world_mut().query_filtered::<Entity, With<TriangleTotal>>().single(app.world()).expect("the bar carries the triangle readout");
    let readout = app.world().get::<ChildOf>(words).map(ChildOf::parent).expect("the words stand in a readout box");
    let node = app.world().get::<Node>(readout).expect("the readout has a Node");
    assert_eq!(node.min_width, Val::Px(COST_COL), "it reserves its column, so the number grows leftwards into it");
    assert_eq!(node.padding.left, Val::Px(COST_PAD_X), "8 px in from the left");
    assert_eq!(node.padding.right, Val::Px(COST_PAD_X), "and from the right");
    assert_eq!(node.justify_content, JustifyContent::FlexEnd, "the digits flush right");
    assert_eq!(node.flex_direction, FlexDirection::Row, "in a line");
    assert_eq!(app.world().get::<bevy_gooey::Spacing>(readout).map(|s| s.padding.left), Some(Some(Space::Px(COST_PAD_X))), "the inset is gooey's `Space::Px`");
}

/// **The compass is a placed gizmo whose arms and dots a system moves**: the box is `SIZE` square and round, and the moving parts carry `HostGeometry`, which the skin leaves alone.
#[test]
fn the_compass_is_a_placed_gizmo_with_host_geometry_parts() {
    let mut app = editor("compass", 1);
    let world = app.world_mut();
    let parts: Vec<(Entity, bool)> = world.query::<(Entity, Has<bevy_gooey::HostGeometry>, &Role)>().iter(world).filter(|(_, _, role)| **role == Role::Placed).map(|(e, host, _)| (e, host)).collect();
    let moving = parts.iter().filter(|(_, host)| *host).count();
    assert!(moving >= 6, "the three arms and the three dots are `HostGeometry`: found {moving}");
    // The gizmo is the biggest round placed square (its dots are smaller ones).
    let gizmo = {
        let world = app.world_mut();
        let mut round: Vec<(Entity, f32)> = world
            .query::<(Entity, &bevy_gooey::Shape, &bevy_gooey::Sizing)>()
            .iter(world)
            .filter(|(_, shape, sizing)| shape.radius == Some(bevy_gooey::Radius::Full) && sizing.width == sizing.height)
            .filter_map(|(e, _, sizing)| match sizing.width {
                Some(bevy_gooey::Extent::Px(px)) => Some((e, px)),
                _ => None,
            })
            .collect();
        round.sort_by(|a, b| b.1.total_cmp(&a.1));
        round.first().map(|(e, _)| *e).expect("the gizmo's box is a round placed square")
    };
    let computed = app.world().get::<ComputedNode>(gizmo).expect("the gizmo is laid out");
    assert!(computed.size().x > 0.0 && (computed.size().x - computed.size().y).abs() < 2.0, "the gizmo is a square: {:?}", computed.size());
}

/// **The frame's root is window-sized through `Extent::Vh`**: 100% wide, 100 viewport heights tall.
#[test]
fn the_frames_root_is_a_sized_box_a_window_tall() {
    let app = editor("root", 1);
    let root = app.world().resource::<bevy_cauldron::chrome::Frame>().root;
    assert_eq!(app.world().get::<Role>(root), Some(&Role::Sized), "the frame's root is a gooey sized box");
    let node = app.world().get::<Node>(root).expect("a Node");
    assert_eq!((node.width, node.height), (Val::Percent(100.0), Val::Vh(100.0)), "100% wide and a window tall");
}
