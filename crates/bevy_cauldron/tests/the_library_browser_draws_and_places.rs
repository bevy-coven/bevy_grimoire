//! **The LIBRARY browser draws the project's pieces as a grid of portraits, narrows them by a search and
//! by tag, and a card is a drag source onto the map** (UI-52, #238).
//!
//! The issue's What is "a grid of piece thumbnails with search and tag filters. Dragging a piece onto the
//! viewport places it." The palette is the list-shaped view of the same library; this is the picture-shaped
//! one, drawn with gooey's `card_grid`.
//!
//! The acceptance lines this file holds:
//!
//! - **A card dragged from the browser onto the map is placed there**, through the same payload the
//!   palette's rows carry (`editor::PieceDrag`), so it is the same placing code as a click: the pointer's
//!   cell, snapped, one undo step.
//! - **The search and the tags narrow the grid**, on the pieces' own vocabulary (`kind`), and the two fold
//!   together.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, PieceDrag};
use bevy_cauldron::library_browser::{LibraryBrowser, LibraryItem, LibrarySearch};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::MainCamera;
use bevy_gooey::{CardGrid, CardRef, Dock, DragPayload, DropTarget, Dropped, ValueChange};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// A project with three pieces, two of them lights.
fn rig(tag: &str) -> App {
    let root: PathBuf = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .kind_tokens(&["light-source"])
        .descriptor("lamp_tall", "alpha")
        .descriptor("lamp_short", "alpha")
        .descriptor("chair", "alpha")
        .build("m");
    // The fixture gives every descriptor the kind `prop`; the lamps are lights.
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.ron");
    let mut text = std::fs::read_to_string(&library).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    for lamp in ["lamp_tall", "lamp_short"] {
        let at = text.find(&format!("id: \"{lamp}\"")).unwrap_or_else(|| panic!("no `{lamp}` in the library"));
        let kind = text[at..].find("kind: [\"prop\"]").expect("the lamp has a kind") + at;
        text.replace_range(kind..kind + "kind: [\"prop\"]".len(), "kind: [\"light-source\"]");
    }
    std::fs::write(&library, text).unwrap_or_else(|e| panic!("{library:?}: {e}"));

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // A window, so the camera has a surface to project the pointer onto.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    updates(&mut app, 8);
    show_the_library(&mut app, true);
    app
}

fn show_the_library(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<LibraryItem>>()
            .single(world)
            .expect("the View menu carries one Show library row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

/// The cards' grid: the one thing the section spawns for them (gooey's `CardGrid`).
fn grid(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CardGrid>>().single(world).expect("one library grid")
}

fn browser(app: &mut App) -> &LibraryBrowser {
    let world = app.world_mut();
    world.query::<&LibraryBrowser>().single(world).expect("the browser is open")
}

fn search_for(app: &mut App, words: &str) {
    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<LibrarySearch>>().single(world).expect("one search box")
    };
    app.world_mut().commands().trigger(ValueChange { source: search, value: words.to_owned(), is_final: false });
    updates(app, 4);
}

/// Where on the screen a person would point to hit the ground at a map-space spot.
fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app.world_mut().query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    camera.world_to_viewport(transform, world).unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

/// Drop the card at `index` on the map at `pointer`, as gooey does when a drag of a card ends there: the
/// payload the card carries, delivered to the target that covers the viewport.
fn drop_a_card(app: &mut App, index: usize, pointer: Vec2) {
    let id = {
        let world = app.world_mut();
        let mut cards = world.query::<(&CardRef, &DragPayload<PieceDrag>)>();
        cards
            .iter(world)
            .find(|(c, _)| c.index == index)
            .map(|(_, p)| p.0.0.clone())
            .unwrap_or_else(|| panic!("the grid draws a card at {index} that carries its piece"))
    };
    let dock = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<Dock>>().single(world).expect("one dock covers the viewport")
    };
    assert!(app.world().entity(dock).contains::<DropTarget<PieceDrag>>(), "the dock takes a library piece");
    app.world_mut().trigger(Dropped { target: dock, source: Entity::PLACEHOLDER, payload: PieceDrag(id), pointer });
    updates(app, 4);
}

/// **Dropping a card on the map places its piece at the pointer cell**, through the place operation, one
/// undo step — the acceptance line.
#[test]
fn a_card_dragged_onto_the_map_is_placed_at_the_pointer_cell_and_one_undo_takes_it_back() {
    let mut app = rig("library_drop");
    assert!(app.world().resource::<OpenMap>().map.placements.is_empty(), "the map starts empty");
    let over = pixel_over(&mut app, (2.0, 2.0));
    drop_a_card(&mut app, 0, over);

    let map = app.world().resource::<OpenMap>().map.clone();
    let said = app.world().resource::<EditorState>().status.line().to_owned();
    assert_eq!(map.placements.len(), 1, "one piece was placed (the editor said `{said}`)");
    let placed = &map.placements[0];
    assert!((placed.at.0 - 2.0).abs() <= 0.5 && (placed.at.1 - 2.0).abs() <= 0.5, "at the cell under the pointer, snapped: {:?}", placed.at);
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 1, "the drop is one undo step, as a click is");

    bevy_cauldron::editor::undo_for_test(app.world_mut());
    updates(&mut app, 3);
    assert!(app.world().resource::<OpenMap>().map.placements.is_empty(), "one undo takes the dropped piece back");
}

/// **The browser draws a card per piece**, and each card carries its descriptor as the payload a drop reads.
#[test]
fn the_grid_draws_a_card_per_piece_and_each_card_carries_its_piece() {
    let mut app = rig("library_cards");
    assert_eq!(browser(&mut app).len(), 3, "three pieces, three cards");
    let grid = grid(&mut app);
    let carried: Vec<String> = {
        let world = app.world_mut();
        let mut q = world.query::<(&CardRef, &DragPayload<PieceDrag>)>();
        q.iter(world).filter(|(c, _)| c.grid == grid).map(|(_, p)| p.0.0.clone()).collect()
    };
    assert_eq!(carried.len(), 3, "every card carries a piece: {carried:?}");
    assert!(carried.iter().any(|id| id == "chair"), "and the chair is one of them: {carried:?}");
}

/// **The search narrows the grid**, on the piece's id, and clearing it draws every card again.
#[test]
fn the_search_narrows_the_grid_and_clearing_it_brings_every_card_back() {
    let mut app = rig("library_search");
    search_for(&mut app, "lamp");
    assert_eq!(browser(&mut app).len(), 2, "the two lamps");
    search_for(&mut app, "zzz");
    assert_eq!(browser(&mut app).len(), 0, "nothing matches");
    search_for(&mut app, "");
    assert_eq!(browser(&mut app).len(), 3, "a cleared search is the whole library again");
}

/// **A tag narrows the grid too**, and the two fold together: a piece passes only when it passes both.
#[test]
fn a_tag_narrows_the_grid_and_folds_with_the_search() {
    let mut app = rig("library_tags");
    bevy_cauldron::library_browser::keep_tag_for_test(app.world_mut(), "light-source", true);
    updates(&mut app, 4);
    assert_eq!(browser(&mut app).len(), 2, "only the lights");

    search_for(&mut app, "tall");
    assert_eq!(browser(&mut app).len(), 1, "the tall lamp: a light, and its id says tall");

    bevy_cauldron::library_browser::keep_tag_for_test(app.world_mut(), "light-source", false);
    updates(&mut app, 4);
    assert_eq!(browser(&mut app).len(), 1, "with no tag kept, the search alone decides");
}

/// **Choosing a card arms its piece**, through the operation a palette row's click dispatches, so a card is
/// usable with the mouse alone. The click is Bevy's `Activate` on the card — what gooey turns into the grid's
/// `ValueChange<usize>` — so this is the path a person's press takes.
#[test]
fn choosing_a_card_arms_its_piece() {
    let mut app = rig("library_arm");
    let (card, index) = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &CardRef)>();
        let (entity, at) = q.iter(world).find(|(_, c)| c.index == 0).expect("the grid draws a first card");
        (entity, at.index)
    };
    let first = browser(&mut app).descriptor(index).expect("the first card names a piece").to_owned();
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: card });
    updates(&mut app, 4);

    // **The piece, not the index**: the id is what a person chose.
    let armed = {
        let state = app.world().resource::<EditorState>().brush;
        let project = app.world().resource::<bevy_cauldron::project::Project>();
        state.and_then(|ix| project.library.descriptors.get(ix).map(|d| d.id.clone()))
    };
    assert_eq!(armed.as_deref(), Some(first.as_str()), "the first card armed its own piece");
    let said = app.world().resource::<EditorState>().status.line().to_owned();
    assert!(said.contains(&first), "and the editor said so: `{said}`");
}

/// **The browser is not there until the View menu asks**, and goes when it is asked again — the trap that made
/// the outliner a section created on demand (a text box and a grid of cards are entities, and the screens that
/// count every button or look for "the" empty text input find them).
#[test]
fn the_browser_is_not_there_until_the_view_menu_asks_and_goes_when_it_is_asked_again() {
    let root = fixtures::Fixture::new(&format!("library_menu_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    let count = |app: &mut App| {
        let world = app.world_mut();
        (
            world.query_filtered::<(), With<CardGrid>>().iter(world).count(),
            world.query_filtered::<(), With<LibrarySearch>>().iter(world).count(),
        )
    };
    let ticked = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<Has<bevy_gooey::Checked>, With<LibraryItem>>().single(world).expect("one row")
    };
    assert_eq!(count(&mut app), (0, 0), "nothing is there until it is asked for");
    assert!(!ticked(&mut app));
    show_the_library(&mut app, true);
    assert_eq!(count(&mut app), (1, 1), "asked for, one grid and one search box");
    assert!(ticked(&mut app), "and the row is ticked");
    show_the_library(&mut app, true);
    assert_eq!(count(&mut app), (1, 1), "asking twice does not make two");
    show_the_library(&mut app, false);
    assert_eq!(count(&mut app), (0, 0), "and asked away it goes");
}
