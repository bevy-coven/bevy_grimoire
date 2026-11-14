//! **The LIBRARY browser draws the project's pieces as a grid of portraits, narrows them by a search and
//! by tag, and a card is a drag source onto the map** (UI-52, #238).
//!
//! The issue's What is "a grid of piece thumbnails with search and tag filters. Dragging a piece onto the
//! viewport places it." The palette is the list-shaped view of the same library; this is the picture-shaped
//! one, drawn with gooey's `card_grid`.
//!
//! **Owner exception (2026-11-13, NV-08 #385)**: the library browser moved from the Map's inspector to the Kit workspace, so `show_the_library` opens the
//! Kit workspace first (every check below is kept; only the route moved).
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
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::keys::Live;
use bevy_cauldron::library_browser::{LibraryBrowser, LibraryItem, LibrarySearch};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::MainCamera;
use bevy_cauldron::workspace::Workspace;
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
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.yaml");
    fixtures::edit_library(&library, |lib| {
        for lamp in ["lamp_tall", "lamp_short"] {
            let row = lib.descriptors.iter_mut().find(|d| d.id == lamp).unwrap_or_else(|| panic!("no `{lamp}` in the library"));
            row.kind = vec!["light-source".to_owned()];
        }
    });

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // A window, so the camera has a surface to project the pointer onto.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    updates(&mut app, 8);
    show_the_library(&mut app, true);
    app
}

/// **Open the Kit workspace first, the way a person does since NV-08 (#385)**: its command (the route the chord, the palette and the activity
/// bar take). The View ▸ Show library row is offered only there, so the library is asked for from the workspace that holds it.
fn in_the_kit_workspace(app: &mut App) {
    if *app.world().resource::<Workspace>() == Workspace::Kit {
        return;
    }
    let command = commands::by_id("workspace.kit").unwrap_or_else(|| panic!("no command opens the Kit workspace"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    updates(app, 12);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Kit, "the workspace command did not open Kit");
}

fn show_the_library(app: &mut App, on: bool) {
    in_the_kit_workspace(app);
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

/// **Press a tag's toggle**, the way gooey's own control does: the `ValueChange<bool>` its press sends, on the
/// toggle entity the browser drew for that `kind`.
fn toggle_a_tag(app: &mut App, kind: &str, on: bool) {
    let toggle = {
        let world = app.world_mut();
        let mut tags = world.query::<(Entity, &bevy_cauldron::library_browser::LibraryTag)>();
        tags.iter(world).find(|(_, tag)| tag.0 == kind).map(|(entity, _)| entity).unwrap_or_else(|| panic!("the browser drew no toggle for `{kind}`"))
    };
    app.world_mut().commands().trigger(ValueChange { source: toggle, value: on, is_final: true });
    updates(app, 4);
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
        world.query_filtered::<Entity, (With<Dock>, With<bevy_cauldron::chrome::ArrangementDock>)>().single(world).expect("the arrangement dock covers the viewport")
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
    toggle_a_tag(&mut app, "light-source", true);
    assert_eq!(browser(&mut app).len(), 2, "only the lights");

    search_for(&mut app, "tall");
    assert_eq!(browser(&mut app).len(), 1, "the tall lamp: a light, and its id says tall");

    toggle_a_tag(&mut app, "light-source", false);
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

/// **One grid, however many times the cards are redrawn.** A redraw replaces the grid, so a second search must not
/// leave the previous one standing: two `CardGrid`s in one section means two sets of cards and a drop that can find
/// the stale one first.
#[test]
fn a_redraw_leaves_exactly_one_grid_of_cards() {
    let mut app = rig("library_one_grid");
    let count = |app: &mut App| {
        let world = app.world_mut();
        let grids = world.query_filtered::<(), With<CardGrid>>().iter(world).count();
        let cards = world.query_filtered::<(), With<CardRef>>().iter(world).count();
        (grids, cards)
    };
    assert_eq!(count(&mut app), (1, 3), "one grid of three cards to start with");

    search_for(&mut app, "lamp");
    assert_eq!(count(&mut app), (1, 2), "a search replaces the grid rather than adding one");

    search_for(&mut app, "");
    assert_eq!(count(&mut app), (1, 3), "and clearing it replaces the grid again");

    toggle_a_tag(&mut app, "light-source", true);
    assert_eq!(count(&mut app), (1, 2), "a tag press replaces the grid too");
    assert_eq!(browser(&mut app).len(), 2, "and the browser agrees with what is drawn");
}

/// **A tag is pressed through gooey's own control**: the `ValueChange<bool>` the toggle sends, on the toggle
/// itself. The browser reads it back, so pressing a tag narrows the grid and pressing it again widens it.
#[test]
fn a_tag_toggle_narrows_the_grid_and_turning_it_off_widens_it_again() {
    let mut app = rig("library_tag_press");
    toggle_a_tag(&mut app, "light-source", true);
    assert_eq!(browser(&mut app).len(), 2, "the lights only");
    let on = browser(&mut app).kept();
    assert_eq!(on, vec!["light-source".to_owned()], "and the browser knows which tag is kept");

    toggle_a_tag(&mut app, "light-source", false);
    assert_eq!(browser(&mut app).len(), 3, "pressing it off is the whole library again");
    assert!(browser(&mut app).kept().is_empty(), "and no tag is kept");
}

/// **Another card grid's choice is not the library's.** `choose_from_a_card` is a global observer, so it hears
/// every `ValueChange<usize>` in the editor, and the editor has other card grids (the launcher's projects). A card
/// chosen in one of those must arm nothing: the guard is `grids.contains(chosen.source)`.
///
/// The choice travels the framework's own path — `Activate` on a card of a grid that is not the library's, which
/// gooey turns into that grid's `ValueChange<usize>` — so this is exactly the event the observer sees for a press.
#[test]
fn a_choice_in_another_card_grid_arms_no_piece() {
    let mut app = rig("library_other_choice");
    let brush_before = app.world().resource::<EditorState>().brush;
    let piece_of_card_one = browser(&mut app).descriptor(1).expect("the grid draws a second card").to_owned();
    let elsewhere = another_card_grid(&mut app);
    let card = {
        let world = app.world_mut();
        let mut cards = world.query::<(Entity, &CardRef)>();
        cards.iter(world).find(|(_, at)| at.grid == elsewhere).map(|(entity, _)| entity).expect("the other grid draws a card")
    };
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: card });
    updates(&mut app, 4);

    let armed = {
        let state = app.world().resource::<EditorState>();
        let project = app.world().resource::<bevy_cauldron::project::Project>();
        state.brush.and_then(|ix| project.library.descriptors.get(ix).map(|d| d.id.clone()))
    };
    let said = app.world().resource::<EditorState>().status.line().to_owned();
    assert_ne!(
        armed.as_deref(),
        Some(piece_of_card_one.as_str()),
        "a card of another grid is card one's index, and it did not arm `{piece_of_card_one}` (the editor said `{said}`)"
    );
    assert_eq!(app.world().resource::<EditorState>().brush, brush_before, "and the brush did not move (the editor said `{said}`)");
}

/// A card grid that is not the library's — the launcher's project list is one — with two cards.
fn another_card_grid(app: &mut App) -> Entity {
    app.world_mut()
        .spawn(bevy_gooey::card_grid([bevy_gooey::Card::new("One"), bevy_gooey::Card::new("Two")]))
        .id()
}


/// **A settled frame does not walk the library** (UI-93, #259): the search and the tags are compared as they stand
/// and a library edit marks `Project` changed, so the fingerprint over every descriptor is taken only when the
/// answer could differ. A drawn frame is charged for the look; a settled one is not.
#[test]
fn a_settled_frame_walks_no_descriptor_and_draws_nothing() {
    let mut app = rig("library_settled");
    let looks = |app: &mut App| browser(app).looks();
    let draws = |app: &mut App| browser(app).draws();
    let (built, drawn) = (looks(&mut app), draws(&mut app));
    updates(&mut app, 60);
    assert_eq!(looks(&mut app), built, "sixty settled frames walk the library no times");
    assert_eq!(draws(&mut app), drawn, "and draw nothing");

    search_for(&mut app, "lamp");
    assert_eq!(draws(&mut app), drawn + 1, "a search is one draw");
    let walked = looks(&mut app);
    updates(&mut app, 30);
    assert_eq!(looks(&mut app), walked, "and holding it walks nothing more");
}
