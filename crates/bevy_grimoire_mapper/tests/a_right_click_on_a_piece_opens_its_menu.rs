//! **A right-click on a placed piece opens that piece's menu, and its Remove is the person's removal**
//! (UI-36, #230).
//!
//! The menu is `bevy_gooey`'s own `context_menu`, spawned once and named on the pieces. The editor's
//! pieces are scenes with no colliders, so the click is found the way the removal tool finds one
//! (`pick_at` over the cursor's ground point) and handed to gooey as the secondary `PointerClick` its
//! guide says a host sends. Choosing an item dispatches the registry operation the keys dispatch: a
//! removal is one `placement_remove` line in the operation log and one Cmd+Z puts the piece back.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use bevy_gooey::{ContextTarget, MenuItem, MenuPanel, Role};
use bevy_grimoire_mapper::editor::Placement;
use bevy_grimoire_mapper::harness;
use bevy_grimoire_mapper::keys::MOD_KEYS;
use bevy_grimoire_mapper::ops::{self, Entry};
use bevy_grimoire_mapper::project::{OpenMap, Project};
use bevy_grimoire_mapper::view::MainCamera;

use crate::fixtures;

/// One frame's worth of person: keys held, the pointer, and a press or release of one mouse button.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<(MouseButton, bool)>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some((button, true)) => mouse.press(button),
        Some((button, false)) => mouse.release(button),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .single(app.world())
        .expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

fn ids(app: &App) -> Vec<String> {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.id.clone())
        .collect()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// A right press and release over one spot: one right-click.
fn right_click(app: &mut App, at: Vec2) {
    for down in [true, false] {
        step(
            app,
            Frame {
                pointer: Some(at),
                mouse: Some((MouseButton::Right, down)),
                ..default()
            },
        );
    }
    app.update();
}

/// The piece the open context menu is for, if a menu is open.
fn menu_for(app: &mut App) -> Option<String> {
    let mut open = app
        .world_mut()
        .query_filtered::<&ContextTarget, With<MenuPanel>>();
    let target = open.iter(app.world()).next()?.0;
    app.world().get::<Placement>(target).map(|p| p.0.clone())
}

/// The menu item whose words are `label`: the entity an `Activate` goes to.
fn item(app: &mut App, label: &str) -> Entity {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text, &ChildOf)>();
    let mut items = world.query_filtered::<(), With<MenuItem>>();
    let mut found = Vec::new();
    for (_, text, parent) in texts.iter(world) {
        if text.0 == label && items.get(world, parent.parent()).is_ok() {
            found.push(parent.parent());
        }
    }
    assert_eq!(found.len(), 1, "one menu item says `{label}`: {found:?}");
    found[0]
}

#[test]
fn a_right_click_on_a_piece_opens_the_menu_of_that_piece_and_only_that_one() {
    let mut app = editor("piece_menu_opens");
    assert_eq!(menu_for(&mut app), None, "no click, no menu");

    let over_second = pixel_over(&mut app, (3.0, 0.0));
    right_click(&mut app, over_second);
    assert_eq!(
        menu_for(&mut app).as_deref(),
        Some("site/wall@1"),
        "the menu is for the piece under the pointer"
    );

    let over_first = pixel_over(&mut app, (0.0, 0.0));
    right_click(&mut app, over_first);
    assert_eq!(
        menu_for(&mut app).as_deref(),
        Some("site/wall@0"),
        "a second right-click moves the menu to the piece it was on"
    );
}

#[test]
fn a_right_click_where_nothing_stands_opens_no_menu() {
    let mut app = editor("piece_menu_empty");
    // Far from both walls and from any piece's reach.
    let nowhere = pixel_over(&mut app, (-40.0, 40.0));
    right_click(&mut app, nowhere);
    assert_eq!(menu_for(&mut app), None);
    let mut panels = app.world_mut().query_filtered::<&ContextTarget, With<MenuPanel>>();
    assert_eq!(panels.iter(app.world()).count(), 0, "no menu is open");
}

#[test]
fn removing_from_the_menu_is_one_logged_removal_and_cmd_z_puts_the_piece_back() {
    let mut app = editor("piece_menu_remove");
    let over_first = pixel_over(&mut app, (0.0, 0.0));
    right_click(&mut app, over_first);
    assert_eq!(menu_for(&mut app).as_deref(), Some("site/wall@0"));

    let remove = item(&mut app, "Remove");
    app.world_mut().commands().trigger(Activate { entity: remove });
    app.update();
    app.update();
    assert_eq!(ids(&app), ["site/wall@1"], "Remove took the piece the menu was for");
    assert_eq!(menu_for(&mut app), None, "choosing an item closes the menu");

    let removals: Vec<Entry> = log(&app).into_iter().filter(|e| e.op == "placement_remove").collect();
    assert_eq!(removals.len(), 1, "one removal, one log line: {removals:?}");
    assert!(matches!(removals[0].source, ops::Source::Human), "{:?}", removals[0]);
    assert_eq!(removals[0].params["id"], "site/wall@0");

    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(ids(&app), ["site/wall@0", "site/wall@1"], "one Cmd+Z puts the piece back");
}

#[test]
fn turning_from_the_menu_is_the_registrys_turn_of_that_piece() {
    let mut app = editor("piece_menu_turn");
    let over_second = pixel_over(&mut app, (3.0, 0.0));
    right_click(&mut app, over_second);
    let turn = item(&mut app, "Turn right");
    app.world_mut().commands().trigger(Activate { entity: turn });
    app.update();
    app.update();
    let yaws: Vec<f32> = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.yaw)
        .collect();
    assert_eq!(yaws, [0.0, 15.0], "only the piece the menu was for turned, one step");
    assert_eq!(
        log(&app).iter().filter(|e| e.op == "placement_set_yaw").count(),
        1,
        "the turn is one registry line"
    );
}

#[test]
fn the_menus_items_are_the_pieces_verbs_in_the_editors_words() {
    let mut app = editor("piece_menu_words");
    let over_first = pixel_over(&mut app, (0.0, 0.0));
    right_click(&mut app, over_first);
    let mut labels = app.world_mut().query::<(&Role, &Text)>();
    let words: Vec<String> = labels
        .iter(app.world())
        .filter(|(r, _)| **r == Role::MenuText)
        .map(|(_, t)| t.0.clone())
        .collect();
    for want in ["Turn left", "Turn right", "Tip about x", "Tip about z", "Lift", "Lower", "Remove"] {
        assert!(words.iter().any(|w| w == want), "the menu says `{want}`: {words:?}");
    }
}
