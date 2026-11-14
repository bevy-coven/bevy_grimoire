//! **Every part NV-08 lists is found by its label in its own workspace, and by no other route** (NV-08, #385; series getting around,
//! #377). The rest of the lists is `every_panel_is_in_its_workspace.rs` (Map's, World's terrain line, Kit's three, Sound); this file is
//! the rows that file did not hold:
//!
//! - **Map: the outliner**, asked for on the Map (View ▸ Show outliner), where the issue puts it;
//! - **World: water and forest**, which exist since NV-11 (#393, #394);
//! - **Kit: the library browser** (UI-52, #238), which was a toggle of the Map's inspector until it moved here: it is asked for in the Kit
//!   workspace (View ▸ Show library, offered only there) and the Map's inspector no longer holds it;
//! - **Animation: the rig bench**, **Files: the file tree**;
//! - **Project: the project's page** (its maps, its kits and the agent guide), drawn inside the workbench. `Screen::Project` stays the page of a project with
//!   no door open; this is the same page as the Project workspace's panel.
//!
//! Each is read the way the sibling file reads it: a `Text` with exactly the heading's words, drawn (visible, with a laid-out box).

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::library_browser::LibraryItem;
use bevy_cauldron::outliner::OutlinerItem;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{Disabled, ValueChange};

use crate::fixtures;

/// **Frames the editor is given to settle** after a switch: the transition runs the frame after the key and the panels are spawned the
/// frame after that.
const SETTLE_FRAMES: usize = 12;

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("every_listed_part_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m")
}

fn editor(tag: &str) -> App {
    let mut app = harness::build_headless(&project(tag), "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **No home's plugins** (`harness::build_headless` adds none): the Project workspace draws its page all the same.
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// **Switch the way the palette does**: the workspace's command id, its chord queued and pressed.
fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the `{id}` command did not open the {} workspace", workspace.title());
}

/// **Is a heading with exactly these words on screen?** Visible and laid out with a box.
fn shows(app: &mut App, label: &str) -> bool {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
    q.iter(world).any(|(text, shown, node)| shown.get() && text.0.trim() == label && node.size().x > 0.0 && node.size().y > 0.0)
}

/// **Map: the outliner**, asked for by its View row (the on-demand route inside the Map workspace: a text box and a tree are entities the
/// screens that count every button or take "the" first empty input would find, so it is never spawned unasked).
#[test]
fn the_outliner_is_found_by_label_in_the_map_workspace() {
    let mut app = editor("outliner");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map);
    assert!(!shows(&mut app, "OUTLINER"), "the outliner is on the Map before it is asked for");
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("the View menu carries one Show outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    settle(&mut app);
    assert!(shows(&mut app, "OUTLINER"), "the Map workspace does not show the OUTLINER once it is asked for");
}

/// **World: the water and the forest the map holds** are found by label, which NV-11 (#393, #394) built.
#[test]
fn water_and_forest_are_found_by_label_in_the_world_workspace() {
    let mut app = editor("world");
    switch_to(&mut app, Workspace::World);
    for label in ["WATER", "FOREST"] {
        assert!(shows(&mut app, label), "the World workspace does not show `{label}`");
    }
}

/// **Ask for the library by its View row**, the one route to it (`library_browser::LibraryItem`).
fn ask_for_the_library(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<LibraryItem>>().single(world).expect("the View menu carries one Show library row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    settle(app);
}

/// **Kit: the library browser** is found by label in the Kit workspace once it is asked for there (on demand, for the outliner's reason: a
/// text box and a grid of cards are entities the screens that count every button would find), and goes with the workspace.
#[test]
fn the_library_browser_is_found_by_label_in_the_kit_workspace_only() {
    let mut app = editor("library");
    assert!(!shows(&mut app, "LIBRARY"), "the library browser is on the Map, which is its old route");
    switch_to(&mut app, Workspace::Kit);
    assert!(!shows(&mut app, "LIBRARY"), "the library browser is on the Kit workspace before it is asked for");
    ask_for_the_library(&mut app, true);
    assert!(shows(&mut app, "LIBRARY"), "the Kit workspace does not show the LIBRARY browser once it is asked for");
    switch_to(&mut app, Workspace::Map);
    assert!(!shows(&mut app, "LIBRARY"), "the library browser stayed on screen when the Kit workspace was left");
}

/// **No route is left by which the Map reaches the library**: on the Map the View row is disabled, and asking anyway puts nothing there.
#[test]
fn the_library_is_not_reachable_from_the_map() {
    let mut app = editor("old_route");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map);
    let refused = {
        let world = app.world_mut();
        world.query_filtered::<Has<Disabled>, With<LibraryItem>>().single(world).expect("one Show library row")
    };
    assert!(refused, "View ▸ Show library is offered on the Map, so the library is reached from two workspaces");
    ask_for_the_library(&mut app, true);
    assert!(!shows(&mut app, "LIBRARY"), "asking for the library on the Map put it in the Map's panels");
}

/// **Animation: the rig bench**, and **Files: the file tree**, each found by label in its own workspace.
#[test]
fn the_rig_bench_and_the_file_tree_are_found_by_label_in_their_workspaces() {
    let mut app = editor("bench_and_files");
    switch_to(&mut app, Workspace::Animation);
    assert!(shows(&mut app, "RIGS"), "the Animation workspace does not show the rig bench's RIGS list");
    switch_to(&mut app, Workspace::Files);
    assert!(shows(&mut app, "FILES"), "the Files workspace does not show the FILES tree");
}

/// **Project: the page's content is found by label in the Project workspace**: its panel heading, the project's name, the Maps and Kits sections
/// with their new-thing buttons, the map the fixture holds, and the agent guide section. Not on the Map first, so the page is the workspace's.
#[test]
fn the_project_page_is_found_by_label_in_the_project_workspace() {
    let mut app = editor("project_page");
    for label in ["PROJECT", "Maps", "Kits", "+ New map", "+ New kit"] {
        assert!(!shows(&mut app, label), "`{label}` is on the Map before the Project workspace is opened");
    }
    switch_to(&mut app, Workspace::Project);
    for label in ["PROJECT", "Maps", "Kits", "+ New map", "+ New kit", "m"] {
        assert!(shows(&mut app, label), "the Project workspace does not show `{label}`");
    }
    // The heading is the project's folder name (the page's title); the fixture's folders all start the same way.
    let named = {
        let world = app.world_mut();
        world.query::<&Text>().iter(world).any(|t| t.0.starts_with("grimoire-fixture-"))
    };
    assert!(named, "the page does not name the project");
    switch_to(&mut app, Workspace::Map);
    assert!(!shows(&mut app, "Maps"), "the project page stayed on screen when the Project workspace was left");
}

/// **The project's folder with a second map**, `other`, beside the fixture's `m`.
fn two_maps(tag: &str) -> PathBuf {
    let root = project(tag);
    fixtures::Fixture::write_map(&root, "other", 8.0, &[]);
    root
}

fn editor_on(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// **Click the page's card for the map `name`**, as a person does: the `Activate` on the card, which asks the page to open it.
fn click_the_card(app: &mut App, name: &str) {
    let card = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_cauldron::project_page::MapCard)>();
        q.iter(world).find(|(_, c)| c.0 == name).map(|(e, _)| e).unwrap_or_else(|| panic!("the page draws no card for the map `{name}`"))
    };
    app.world_mut().trigger(bevy_gooey::Activate { entity: card });
    settle(app);
}

fn open_map_name(app: &App) -> String {
    app.world().resource::<bevy_cauldron::project::OpenMap>().map.name.clone()
}

/// **The map already open, clicked on the page, shows the Map workspace** and leaves it open: the project stays open, nothing goes through the home.
#[test]
fn clicking_the_open_map_shows_the_map_workspace() {
    let mut app = editor("project_open");
    switch_to(&mut app, Workspace::Project);
    click_the_card(&mut app, "m");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "opening a map from the page shows the Map workspace");
    assert_eq!(open_map_name(&app), "m");
    assert_eq!(*app.world().resource::<State<bevy_cauldron::screen::Screen>>().get(), bevy_cauldron::screen::Screen::Editor, "the editor never left");
}

/// **Another map, clicked while one is open, becomes the open one**, through the editor's own tab route.
#[test]
fn a_second_map_clicked_while_another_is_open_becomes_the_open_one() {
    let mut app = editor_on(&two_maps("project_second"));
    assert_eq!(open_map_name(&app), "m");
    switch_to(&mut app, Workspace::Project);
    click_the_card(&mut app, "other");
    assert_eq!(open_map_name(&app), "other", "the clicked map did not become the open one");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "and the Map workspace shows it");
}

/// **An editor with no map open (one opened on a kit) opens the map that was clicked**, not the project's last one and not nothing.
#[test]
fn an_editor_with_no_map_open_opens_the_map_that_was_clicked() {
    use bevy_cauldron::project::{OpenMap, Project};
    let mut app = editor_on(&two_maps("project_kit_only"));
    switch_to(&mut app, Workspace::Project);
    // The kit-only editor: no `OpenMap`, while the project remembers `m` as the map it was last on.
    app.world_mut().remove_resource::<OpenMap>();
    assert_eq!(app.world().resource::<Project>().open_map_name.as_deref(), Some("m"), "the project remembers the first map");
    settle(&mut app);
    click_the_card(&mut app, "other");
    assert!(app.world().get_resource::<OpenMap>().is_some(), "clicking a map card opened nothing");
    assert_eq!(open_map_name(&app), "other", "the editor opened another map than the one clicked");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map);
}

/// **A map with unsaved edits asks first**, with the question the tab route raises, and stays open until it is answered.
#[test]
fn a_dirty_open_map_asks_before_another_is_opened() {
    use bevy_cauldron::confirm::Confirm;
    use bevy_cauldron::editor::EditorState;
    use bevy_cauldron::project::OpenMap;
    let mut app = editor_on(&two_maps("project_dirty"));
    switch_to(&mut app, Workspace::Project);
    app.world_mut().resource_mut::<OpenMap>().map.origin.0 += 1.0;
    assert!(app.world().resource::<OpenMap>().dirty(), "the test's edit did not make the map dirty");
    click_the_card(&mut app, "other");
    assert_eq!(open_map_name(&app), "m", "the dirty map was left without asking");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Project, "the question is answered where it was asked: the workspace was changed under it");
    assert_eq!(app.world().resource::<EditorState>().map_request.as_deref(), Some("other"), "the request to open `other` was not held for the answer");
    assert!(app.world().resource::<Confirm>().is_open(), "no question was raised about the unsaved edits");
}
