//! **Every part NV-08 lists is found by its label in its own workspace, and by no other route** (NV-08, #385; series getting around,
//! #377). The rest of the lists is `every_panel_is_in_its_workspace.rs` (Map's, World's terrain line, Kit's three, Sound); this file is
//! the rows that file did not hold:
//!
//! - **Map: the outliner**, asked for on the Map (View ▸ Show outliner), where the issue puts it;
//! - **World: water and forest**, which exist since NV-11 (#393, #394);
//! - **Kit: the library browser** (UI-52, #238), which was a toggle of the Map's inspector until it moved here: it is asked for in the Kit
//!   workspace (View ▸ Show library, offered only there) and the Map's inspector no longer holds it;
//! - **Animation: the rig bench**, **Files: the file tree**;
//! - **Project: the project's page** (its maps, its kits and the agent guide), drawn inside the workbench. `Screen::Project` stays the home a project-less
//!   start needs; this is the same page as the Project workspace's panel.
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
    // **The home's plugins, as `main.rs` adds them**: the project page is the home's, and the Project workspace draws it.
    app.add_plugins(bevy_cauldron::launcher::LauncherPlugin);
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

/// **A map opened from the Project workspace becomes the live one and the Map workspace is shown**, through the editor's own tabs: the project
/// stays open, nothing goes through the home.
#[test]
fn opening_a_map_from_the_project_workspace_shows_it_in_the_map_workspace() {
    use bevy_cauldron::project::OpenMap;
    let mut app = editor("project_open");
    switch_to(&mut app, Workspace::Project);
    let card = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_cauldron::project_page::MapCard)>();
        q.iter(world).find(|(_, c)| c.0 == "m").map(|(e, _)| e).expect("the page draws the map `m` as a card")
    };
    app.world_mut().trigger(bevy_gooey::Activate { entity: card });
    settle(&mut app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "opening a map from the page shows the Map workspace");
    assert_eq!(app.world().resource::<OpenMap>().map.name, "m", "and the map is the open one");
    assert_eq!(*app.world().resource::<State<bevy_cauldron::screen::Screen>>().get(), bevy_cauldron::screen::Screen::Editor, "and the editor never left");
}
