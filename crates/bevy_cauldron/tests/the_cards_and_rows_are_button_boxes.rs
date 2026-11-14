//! **A card, a news entry and a kit row are gooey's button box** (#260, UI-99, round 7).
//!
//! They were a raw `Node` plus a bevy `Button`, `Role::Button`, `Hovered` and the navigation marker. They are `button_box()` now, and a button writes no gap, so the
//! lines of a card stand apart by a margin each: this file measures that a card is as tall as its lines, its padding and the gaps (the layout the raw column made).

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_gooey::Role;

use crate::fixtures;

#[test]
fn a_project_card_is_a_button_box_whose_lines_stand_a_tight_step_apart() {
    use bevy_cauldron::launcher::{LauncherPlugin, ProjectCard};
    use bevy_cauldron::screen::Screen;
    let root = fixtures::Fixture::new(&format!("button_box_card_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    let file = root.join("recent.yaml");
    bevy_cauldron::recent::record(&file, &root).expect("the project is recorded");
    app.insert_resource(bevy_cauldron::recent::RecentProjects(file));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    let card = app.world_mut().query_filtered::<Entity, With<ProjectCard>>().single(app.world()).expect("the project has a card");
    assert_eq!(app.world().get::<Role>(card), Some(&Role::Button), "a card is a gooey button");
    assert_eq!(app.world().get::<bevy::input_focus::tab_navigation::TabIndex>(card), Some(&bevy::input_focus::tab_navigation::TabIndex(0)), "reached by Tab");
    assert!(app.world().get::<bevy::picking::hover::Hovered>(card).is_some(), "and hovered");
    let node = app.world().get::<Node>(card).expect("a Node");
    assert_eq!(node.width, Val::Px(184.0), "a card is 184 px wide");
    assert_eq!(node.flex_direction, FlexDirection::Column, "its lines stack");
    // Measured before the swap (a raw column, at 2f83f0c3): the skin's padding 9 x 3.6 px, 3 px between lines, 184.17 x 160.83 drawn.
    let height = |e: Entity| app.world().get::<ComputedNode>(e).map(|c| c.size().y * c.inverse_scale_factor()).expect("laid out");
    let lines: Vec<Entity> = app.world().get::<Children>(card).expect("the card has lines").iter().collect();
    assert!(lines.len() >= 3, "a card has a name, a folder and a time: {}", lines.len());
    let drawn = height(card);
    assert!((drawn - 160.83).abs() < 1.0, "a card is as tall as it was, 160.83: drawn {drawn}");
    let (top, bottom) = (node.padding.top, node.padding.bottom);
    let (Val::Px(top), Val::Px(bottom)) = (top, bottom) else { panic!("a card's padding is in px") };
    let stacked = top + bottom + lines.iter().map(|l| height(*l)).sum::<f32>() + 3.0 * (lines.len() as f32 - 1.0);
    assert!((height(card) - stacked).abs() < 2.5, "a card is its lines, its padding and 3 px between each two ({stacked}), drawn {drawn}");
}
