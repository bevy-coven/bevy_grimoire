//! **A right-click on a palette row opens that library piece's menu, and each item is the verb the
//! editor already has** (UI-36, #230).
//!
//! The menu is `bevy_gooey`'s own `context_menu`, named on every palette row. The rows are real UI
//! nodes, so the right-click is the secondary `PointerClick` gooey's guide says a host sends when it
//! does its own picking; choosing an item is the `Activate` the item gets. The assertions are on what a
//! person sees change: the brush, the library, the operation log and the status row.

use bevy::camera::NormalizedRenderTarget;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, WindowRef};
use bevy_gooey::{ContextTarget, MenuItem, MenuPanel};
use bevy_grimoire_core::descriptor::Mount;
use bevy_cauldron::editor::{EditorState, PaletteRow};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn row_of(app: &mut App, descriptor: &str) -> Entity {
    let ix = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == descriptor)
        .unwrap_or_else(|| panic!("no `{descriptor}` in the library"));
    let mut rows = app.world_mut().query::<(Entity, &PaletteRow)>();
    rows.iter(app.world())
        .find(|(_, r)| r.0 == ix)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("`{descriptor}` has no palette row"))
}

/// A right-click on `row`: the secondary click gooey opens a context menu on.
fn right_click(app: &mut App, row: Entity) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window");
    let target = WindowRef::Primary.normalize(Some(window)).expect("a window target");
    app.world_mut().commands().trigger(PointerClick {
        entity: row,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(target),
                position: Vec2::new(120.0, 200.0),
            },
        ),
        button: PointerButton::Secondary,
        hit: HitData::new(window, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    app.update();
    app.update();
}

/// The library piece the open context menu is for, if one is open.
fn menu_for(app: &mut App) -> Option<String> {
    let mut open = app
        .world_mut()
        .query_filtered::<&ContextTarget, With<MenuPanel>>();
    let target = open.iter(app.world()).next()?.0;
    let ix = app.world().get::<PaletteRow>(target)?.0;
    app.world()
        .resource::<Project>()
        .library
        .descriptors
        .get(ix)
        .map(|d| d.id.clone())
}

fn choose(app: &mut App, label: &str) {
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &ChildOf)>();
    let mut items = world.query_filtered::<(), With<MenuItem>>();
    let found: Vec<Entity> = texts
        .iter(world)
        .filter(|(t, p)| t.0 == label && items.get(world, p.parent()).is_ok())
        .map(|(_, p)| p.parent())
        .collect();
    assert_eq!(found.len(), 1, "one menu item says `{label}`: {found:?}");
    app.world_mut().commands().trigger(Activate { entity: found[0] });
    app.update();
    app.update();
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Entry>(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

fn library_ids(app: &App) -> Vec<String> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .map(|d| d.id.clone())
        .collect()
}

#[test]
fn a_right_click_on_a_row_opens_the_menu_of_that_piece() {
    let mut app = editor("library_menu_opens");
    assert_eq!(menu_for(&mut app), None, "no click, no menu");
    let gate = row_of(&mut app, "site/gate");
    right_click(&mut app, gate);
    assert_eq!(menu_for(&mut app).as_deref(), Some("site/gate"));
    let wall = row_of(&mut app, "site/wall");
    right_click(&mut app, wall);
    assert_eq!(menu_for(&mut app).as_deref(), Some("site/wall"), "the menu moves to the next row");
}

#[test]
fn place_arms_the_piece_the_menu_is_for_as_a_click_does() {
    let mut app = editor("library_menu_place");
    let gate = row_of(&mut app, "site/gate");
    right_click(&mut app, gate);
    choose(&mut app, "Place");
    let ix = app.world().resource::<EditorState>().brush.expect("a brush is armed");
    assert_eq!(app.world().resource::<Project>().library.descriptors[ix].id, "site/gate");
    let arms: Vec<Entry> = log(&app).into_iter().filter(|e| e.op == "brush_arm").collect();
    assert!(!arms.is_empty() && arms.iter().all(|e| matches!(e.source, ops::Source::Human)), "{arms:?}");
}

#[test]
fn mounting_from_the_menu_is_the_registrys_one_logged_mount_of_that_piece() {
    let mut app = editor("library_menu_mount");
    let gate = row_of(&mut app, "site/gate");
    right_click(&mut app, gate);
    choose(&mut app, "Mount on ceiling");
    let project = app.world().resource::<Project>();
    let mount = |id: &str| {
        project
            .measured
            .descriptors
            .iter()
            .find(|d| d.id == id)
            .and_then(|d| d.mount.clone())
    };
    assert_eq!(mount("site/gate"), Some(Mount::OnCeiling), "the piece the menu was for is mounted");
    assert_ne!(mount("site/wall"), Some(Mount::OnCeiling), "and only that one");
    let lines: Vec<Entry> = log(&app).into_iter().filter(|e| e.op == "piece_set_mount").collect();
    assert_eq!(lines.len(), 1, "one mount, one log line: {lines:?}");
    assert!(matches!(lines[0].source, ops::Source::Human), "{:?}", lines[0]);
    assert_eq!(lines[0].params["id"], "site/gate");
}

#[test]
fn removing_a_piece_no_map_places_takes_it_out_of_the_kit_in_one_logged_removal() {
    let mut app = editor("library_menu_remove");
    let gate = row_of(&mut app, "site/gate");
    right_click(&mut app, gate);
    choose(&mut app, "Remove from the kit");
    assert_eq!(library_ids(&app), ["site/wall"], "the gate left the library");
    let lines: Vec<Entry> = log(&app).into_iter().filter(|e| e.op == "piece_remove").collect();
    assert_eq!(lines.len(), 1, "one removal, one log line: {lines:?}");
    assert!(matches!(lines[0].source, ops::Source::Human), "{:?}", lines[0]);
    assert_eq!(lines[0].params["id"], "site/gate");
}

#[test]
fn removing_a_piece_a_map_places_is_refused_and_says_which_map() {
    let mut app = editor("library_menu_remove_placed");
    let wall = row_of(&mut app, "site/wall");
    right_click(&mut app, wall);
    choose(&mut app, "Remove from the kit");
    assert_eq!(library_ids(&app), ["site/wall", "site/gate"], "the library is as it was");
    assert_eq!(log(&app).iter().filter(|e| e.op == "piece_remove").count(), 0, "no line is written");
    let said = app.world().resource::<EditorState>().status.problem_text().to_owned();
    assert!(said.contains("placed by") && said.contains("`m`"), "the status row names the map: {said:?}");
}
