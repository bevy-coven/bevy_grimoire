//! **The shell's `Dock` holds the arrangement; it does not paint over the world** (bevy-coven/bevy_gooey#85).
//!
//! UI-30 (#224) made the shell's outer layout a `bevy_gooey::Dock`, and its layout is what
//! `ui::save_the_layout` saves and `ui::follow_the_saved_layout` restores. But the *panels* are still
//! drawn by the editor itself — `spawn_panel`, `spawn_palette_panel` and the rest fill
//! `chrome::Frame`'s nodes by hand — so the dock's `PanelRegistry` entries are placeholders with no
//! contents (`ui::register_the_shell_panels`). Until the panels themselves move into the dock, the
//! dock is the arrangement's **data**, and the arrangement a person sees is the `ShellSplit` beside it.
//!
//! **gooey #85 broke exactly that assumption.** `update_docks` used to return early when a dock the
//! host had spawned had no `Role::DockBody` child, which is what the bare `Dock(layout)` in
//! `chrome::spawn_frame` was: it built nothing. Since #85 a host-placed dock grows its body, its drop
//! landing and its ghost, and builds every panel its layout names — so the shell grew three
//! `panel()` cards (INSPECTOR at 22%, VIEWPORT, PLACE) **over the map**, each filled with the skin's
//! surface: an opaque layer between the author and the thing they are authoring.
//!
//! Three claims, each of which can fail:
//!
//! 1. the dock still holds the workspace's arrangement, because that is what is saved and restored;
//! 2. no card it builds is laid out anywhere — a card with a size is a card over the world;
//! 3. the viewport is uncovered and clickable: the editor's own "is the interface under the pointer"
//!    says no at the viewport's centre, and a click there still places the armed piece.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::OverUi;
use bevy_gooey::{Dock, DockPanel};

use crate::fixtures;

/// The editor on a one-piece fixture, settled, with a window a pointer can be injected into.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("shell_dock_paints_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..12 {
        app.update();
    }
    app
}

#[derive(Default)]
struct Pointer {
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Pointer>,
}

fn drive(mut script: ResMut<Script>, mut mouse: ResMut<ButtonInput<MouseButton>>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Some(frame) = script.next.take() else {
        return;
    };
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, pointer: Option<Vec2>, mouse: Option<bool>) {
    app.world_mut().resource_mut::<Script>().next = Some(Pointer { pointer, mouse });
    app.update();
}

/// Move to `at`, press, release — the three frames `a_click_outside_the_viewport_places_nothing` uses.
fn click(app: &mut App, at: Vec2) {
    for mouse in [None, Some(true), Some(false)] {
        step(app, Some(at), mouse);
    }
}

/// The viewport's rect in logical pixels.
fn viewport(app: &mut App) -> Rect {
    let world = app.world_mut();
    let (node, at) = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<bevy_cauldron::chrome::ViewportSlot>>()
        .single(world)
        .expect("the editor has one viewport slot");
    let scale = node.inverse_scale_factor();
    Rect::from_center_size(at.translation * scale, node.size() * scale)
}

/// **The dock is still the arrangement's data**, or there is nothing for a divider drag to save.
#[test]
fn the_shells_dock_still_holds_the_workspaces_arrangement() {
    let mut app = editor("holds");
    let world = app.world_mut();
    let shown = world
        .query_filtered::<&Dock, (With<Dock>, With<bevy_cauldron::chrome::ArrangementDock>)>()
        .iter(world)
        .next()
        .map(|dock| dock.0.clone())
        .expect("the shell is a gooey Dock");
    assert_eq!(
        shown,
        bevy_cauldron::ui::shell_layout(bevy_cauldron::workspace::Workspace::Map),
        "the dock holds Map's own arrangement, which is what the editor saves and restores"
    );
}

/// **No card the dock builds is laid out**: the panels are the editor's own, so the dock's three
/// placeholder pages must stay unbuilt rather than stand over the map as empty cards.
#[test]
fn no_card_the_shells_dock_builds_is_laid_out() {
    let mut app = editor("unbuilt");
    let world = app.world_mut();
    let laid: Vec<(String, Vec2)> = world
        .query::<(&DockPanel, &ComputedNode)>()
        .iter(world)
        .map(|(panel, node)| (panel.0.clone(), node.size()))
        .collect();
    assert!(
        !laid.is_empty(),
        "the dock names `inspector`, `viewport` and `place`, so it has pages for them; finding none \
         means this test is looking at nothing"
    );
    for (id, size) in &laid {
        assert!(
            size.x <= 0.0 || size.y <= 0.0,
            "the dock laid its `{id}` page out at {size:?}: the panel the editor draws by hand is \
             under an empty card here, and the card's fill is what covers the map"
        );
    }
}

/// **The viewport is uncovered and clickable**: nothing the dock built is between the pointer and the
/// world at the viewport's centre, and a click there places the armed piece.
#[test]
fn the_viewport_is_uncovered_and_a_click_in_it_still_places_a_piece() {
    let mut app = editor("clickable");
    let rect = viewport(&mut app);
    let centre = rect.center();
    assert!(rect.width() > 200.0 && rect.height() > 200.0, "the viewport is a real area: {rect:?}");

    // The editor's own answer to "is a panel under the pointer", which is what gates every map verb.
    step(&mut app, Some(centre), None);
    assert!(
        !app.world().resource::<OverUi>().0,
        "the interface says it is under the pointer at the viewport's centre {centre:?}, so every \
         click there is taken as a click on a panel"
    );

    let index = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == "site/wall")
        .expect("the fixture defines site/wall");
    app.world_mut().resource_mut::<EditorState>().brush = Some(index);
    app.update();

    click(&mut app, centre);
    let placed: Vec<String> = app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).collect();
    assert_eq!(placed.len(), 1, "a click at the viewport's centre places the armed piece: {placed:?}");
}
