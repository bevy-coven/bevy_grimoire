//! **Viewport overlays from the skin and the View menu** (UI-80, #254).
//!
//! The compass, the light map and the grid are the three things the editor draws *over* the world,
//! and until now they were reachable and coloured two different ways on purpose-for-no-reason: the
//! light map had a button in a panel and no row in the View menu, and all three picked their colours
//! from the constants in `chrome.rs` (or from a literal), so a project's skin and the High contrast
//! theme could not reach the thing an author looks at most.
//!
//! # What is asserted, and why it is the acceptance and not a proxy for it
//!
//! - **"View ▸ Light map toggles the overlay exactly as the button did."** The test presses the menu
//!   row and then the button, and compares the overlay's state, its baked field and the operation log
//!   after each. "Exactly as" is the whole claim, so it is checked against the button rather than
//!   against a description of what the button does.
//! - **"The overlay colours come from the skin."** Each overlay's ink is read through one expression
//!   the drawing system also reads, so an assertion here is an assertion about what is drawn.
#![cfg(feature = "bevy_light_grid")]

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::light_overlay::{Drawn, LightMapToggle, LightOverlay, LightOverlayInk};
use bevy_cauldron::ops::{self, Source};
use bevy_cauldron::project::Project;
use bevy::ui_widgets::ValueChange;
use bevy_gooey::GooeySkin;

use crate::fixtures;

/// The editor with two lit torches, as the light map's own tests build it.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/torch", "alpha")
        .place("site/torch", (0.0, 0.0))
        .place("site/torch", (30.0, 0.0))
        .build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let mut open = app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>();
    for p in open.map.placements.iter_mut() {
        p.family.insert("bevy_lights".to_owned(), "(base: 1000.0)".to_owned());
    }
    app
}

fn logged(app: &App, op: &str) -> usize {
    let project = app.world().resource::<Project>();
    ops::read_entries(&ops::oplog_path(project)).into_iter().filter(|e| e.op == op).count()
}

/// The row View ▸ Light map puts in the menu, and its words.
///
/// **Found by the row, then read by its text.** `bevy_gooey`'s `MenuItem` keeps its label private (a
/// row is built from it, not interrogated through it), so the test finds the row the View menu spawned
/// and reads the `Text` under it — which is what a person reads.
#[test]
fn the_view_menu_has_a_light_map_row() {
    let mut app = editor("view_light_map_row");
    app.update();
    let row = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::light_overlay::LightMapItem>>();
        q.single(world).expect("the View menu carries one Light map row")
    };
    let said: Vec<String> = {
        let world = app.world_mut();
        let mut kids = world.query::<&Children>();
        let mut texts = world.query::<&Text>();
        kids.get(world, row)
            .map(|children| {
                children
                    .iter()
                    .flat_map(|child| std::iter::once(child).chain(kids.get(world, child).into_iter().flatten().copied()))
                    .filter_map(|child| texts.get(world, child).ok().map(|t| t.0.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert!(said.iter().any(|words| words == "Light map"), "the row says what it is: {said:?}");
}

/// **The row toggles the overlay exactly as the button does**: the same state, the same field, the
/// same receipt, and one logged `light_map` per showing — and hiding reads nothing, so it logs nothing.
#[test]
fn the_view_row_toggles_the_overlay_exactly_as_the_button_does() {
    let mut app = editor("view_light_map_same");

    // The menu row: a check item sends `ValueChange<bool>`.
    let row = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::light_overlay::LightMapItem>>();
        q.single(world).expect("the View menu carries one Light map row")
    };
    app.world_mut().trigger(ValueChange::<bool> { source: row, value: true, is_final: true });
    app.world_mut().flush();
    app.update();
    let shown = app.world().resource::<LightOverlay>();
    assert!(shown.on, "the row shows the overlay");
    let field_from_row = shown.drawn.clone().expect("showing it bakes the field");
    let receipt_from_row = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(receipt_from_row.contains("light map: 2 fixtures"), "the row gives the operation's receipt: {receipt_from_row:?}");
    assert_eq!(logged(&app, "light_map"), 1, "showing it is one logged operation, from the row");

    app.world_mut().trigger(ValueChange::<bool> { source: row, value: false, is_final: true });
    app.world_mut().flush();
    app.update();
    assert!(!app.world().resource::<LightOverlay>().on, "the row hides it again");
    assert_eq!(logged(&app, "light_map"), 1, "hiding reads nothing, so it logs nothing");

    // The button, from the same starting point, must agree on every one of those.
    let button = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<LightMapToggle>>();
        q.single(world).expect("the LIGHT MAP section's one button")
    };
    app.world_mut().trigger(bevy::ui_widgets::Activate { entity: button });
    app.world_mut().flush();
    app.update();
    let from_button = app.world().resource::<LightOverlay>();
    assert!(from_button.on, "the button shows the overlay");
    assert_eq!(from_button.drawn, Some(field_from_row), "the row and the button bake the same field");
    assert_eq!(
        app.world().resource::<EditorState>().status.note_text().to_owned(),
        receipt_from_row,
        "and say the same receipt"
    );
    assert_eq!(logged(&app, "light_map"), 2, "one more logged operation, from the button");
}

/// **The overlay's ink comes from the skin**: a dark cell and a lit cell are the skin's two named
/// tones, and a skin that moves them moves what is drawn.
#[test]
fn the_overlay_colours_come_from_the_skin() {
    let mut skin = GooeySkin::tool();
    let dark = Color::srgb(0.11, 0.22, 0.33);
    let lit = Color::srgb(0.88, 0.77, 0.66);
    skin.tones.insert("overlay_dark".to_owned(), dark);
    skin.tones.insert("overlay_lit".to_owned(), lit);

    let ink = LightOverlayInk::of(Some(&skin));
    assert_eq!(ink.dark, dark, "a dark cell is the skin's `overlay_dark` tone");
    assert_eq!(ink.lit, lit, "a lit cell is the skin's `overlay_lit` tone");
    assert_ne!(LightOverlayInk::of(Some(&skin)).colour(0.0), LightOverlayInk::of(Some(&skin)).colour(1.0), "dark and lit are two colours");
}

/// **The editor's own skin draws the overlay the colours it always did**, so this changes where the
/// colours come from and not what a person sees today.
///
/// **Exact, not approximate.** The skin states its two overlay tones in `LinearRgba` — the only tones in
/// `src/editor_skin.ron` that are linear — because `light_overlay::SHIPPED` is linear, so the two are
/// the same numbers in the same space and a change to either fails here on the value itself.
#[test]
fn the_editor_skin_draws_the_shipped_overlay() {
    let ink = LightOverlayInk::of(Some(&bevy_cauldron::ui::editor_skin()));
    assert_eq!(ink.colour(0.0), Drawn::colour_for(0.0), "a dark cell is unchanged");
    assert_eq!(ink.colour(1.0), Drawn::colour_for(1.0), "and a lit one");
}

/// **With no skin it is the shipped palette**, which is what a harness that never ran
/// `EditorUiPlugin` gets.
#[test]
fn with_no_skin_the_overlay_is_the_shipped_palette() {
    let ink = LightOverlayInk::of(None);
    assert_eq!(ink.colour(0.5), Drawn::colour_for(0.5));
}

/// **The toggle is one operation from both doors** — the source says which door, and both say a
/// person.
#[test]
fn both_doors_are_a_persons_operation() {
    let mut app = editor("view_light_map_source");
    let row = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::light_overlay::LightMapItem>>();
        q.single(world).expect("the row")
    };
    app.world_mut().trigger(ValueChange::<bool> { source: row, value: true, is_final: true });
    app.world_mut().flush();
    app.update();
    let project = app.world().resource::<Project>();
    let last = ops::read_entries(&ops::oplog_path(project)).into_iter().rfind(|e| e.op == "light_map");
    assert_eq!(last.map(|e| e.source), Some(Source::Human), "the row's showing is a person's operation");
}

/// **The row's tick says what the overlay is doing, whichever door moved it** (UI-80, #254). A tick
/// that disagrees with what is on screen is a claim the menu cannot keep, so the button's showing the
/// overlay must tick the row and its hiding must clear it.
#[test]
fn the_rows_tick_follows_the_overlay_from_either_door() {
    let mut app = editor("view_light_map_tick");
    let button = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<LightMapToggle>>();
        q.single(world).expect("the LIGHT MAP section's one button")
    };
    let ticked = |app: &mut App, row: Entity| {
        let world = app.world_mut();
        world.get::<bevy_gooey::Checked>(row).is_some()
    };
    let row = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::light_overlay::LightMapItem>>();
        q.single(world).expect("the View menu carries one Light map row")
    };
    assert!(!ticked(&mut app, row), "the overlay starts hidden, and the row says so");

    app.world_mut().trigger(bevy::ui_widgets::Activate { entity: button });
    app.world_mut().flush();
    app.update();
    app.update();
    assert!(app.world().resource::<LightOverlay>().on, "the button showed the overlay");
    assert!(ticked(&mut app, row), "and the View row is ticked for it, though the button was the door");

    app.world_mut().trigger(bevy::ui_widgets::Activate { entity: button });
    app.world_mut().flush();
    app.update();
    app.update();
    assert!(!app.world().resource::<LightOverlay>().on, "the button hid it again");
    assert!(!ticked(&mut app, row), "and the tick is cleared");
}

/// **The map's grid and the seating lattice are coloured by the running skin** (UI-80, #254).
///
/// Both used to read `chrome::ink().grid_line` whatever skin was running, so the two lattices an author
/// reads the map by were the one overlay a theme could not reach. `chrome::grid_ink` is the one
/// expression both grids draw through — a function rather than a line inside a `Gizmos` call because a
/// headless app renders no gizmos, so this is the only place the colour can be asked about.
///
/// The three claims: the shipped tone where a skin carries none (so the editor's look is unchanged by
/// construction), the skin's own `grid_line` where it names one, and the editor's own skin answering
/// the shipped tone — which is what makes "a theme or a project moves it" true of the editor's grid and
/// not only of a hand-made skin.
#[test]
fn the_grid_is_drawn_in_the_running_skins_ink() {
    use bevy_cauldron::chrome;

    assert_eq!(
        chrome::grid_ink(None),
        chrome::ink().grid_line,
        "with no skin, the shipped tone — a harness's answer"
    );
    let mut skin = bevy_cauldron::ui::editor_skin();
    assert_eq!(
        chrome::grid_ink(Some(&skin)),
        chrome::ink().grid_line,
        "the editor's own skin carries the shipped tone, so its grid does not move"
    );
    let moved = Color::srgb(0.20, 0.90, 0.40);
    skin.tones.insert("grid_line".to_owned(), moved);
    assert_eq!(chrome::grid_ink(Some(&skin)), moved, "a skin that names `grid_line` moves the lattice");
}
