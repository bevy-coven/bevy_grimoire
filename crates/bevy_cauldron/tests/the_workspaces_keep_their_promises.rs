//! **What #379 promises, measured the way a person reaches it** (review of `bf2c272e`):
//!
//! - every built workspace is reachable by its **command id** (`workspace.<id>`, the route the palette and
//!   an agent take), and a round trip leaves the open map and the selection alone;
//! - a divider moved in Map and one moved in Kit are each saved by `ui::save_the_layout` and each restored
//!   in its own workspace after a restart;
//! - **an old settings file still reads**: one with the single `shell_layout` of before the workspaces is
//!   moved to Map's entry rather than refused, and one that rebound `view.switch_door` keeps its keymap.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, Live};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::selection::Selection;
use bevy_cauldron::settings::{SettingsLayer, UserSettingsFile, SETTINGS_VERSION};
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{DockLayout, Modifiers, Shortcut, SplitAxis};

use crate::fixtures;

const SETTLE: usize = 12;

fn editor(tag: &str, user: Option<&Path>) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(&format!("promise_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let app = editor_in(&root, user);
    (app, root)
}

fn editor_in(root: &Path, user: Option<&Path>) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    if let Some(user) = user {
        app.insert_resource(UserSettingsFile(user.to_owned()));
    }
    for _ in 0..SETTLE {
        app.update();
    }
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **Run a command by id**, the way the palette does: its chord is queued and pressed.
fn run_command(app: &mut App, id: &str) {
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
}

fn shown(app: &mut App) -> DockLayout {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&bevy_gooey::Dock, With<bevy_cauldron::chrome::ArrangementDock>>();
    q.iter(world).next().expect("the shell is a gooey Dock").0.clone()
}

fn dock(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world
        .query_filtered::<Entity, (With<bevy_gooey::Dock>, With<bevy_cauldron::chrome::ArrangementDock>)>();
    q.iter(world).next().expect("the shell's dock")
}

fn moved(fraction: f32) -> DockLayout {
    DockLayout::split(
        SplitAxis::Horizontal,
        fraction,
        DockLayout::group(["place"]),
        DockLayout::split(SplitAxis::Horizontal, 0.5, DockLayout::group(["viewport"]), DockLayout::group(["inspector"])),
    )
}

/// **Acceptance 1, by command id**: every built workspace, in order, and back to Map.
#[test]
fn every_built_workspace_is_reachable_by_its_command_and_the_map_and_selection_stay() {
    let (mut app, _root) = editor("commands", None);
    assert!(bevy_cauldron::editor::lock_on_placement(app.world_mut(), "floor@0"));
    settle(&mut app);
    let map = app.world().resource::<OpenMap>().map.name.clone();
    let selected = app.world().resource::<Selection>().ids().to_vec();
    assert_eq!(selected, ["floor@0"], "the test starts with a piece selected");

    let built: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| w.is_built()).collect();
    assert!(built.len() >= 3);
    for workspace in built.iter().copied().filter(|w| *w != Workspace::Map).chain([Workspace::Map]) {
        run_command(&mut app, &format!("workspace.{}", workspace.id()));
        assert_eq!(app.world().resource::<Workspace>(), &workspace, "`workspace.{}` did not switch", workspace.id());
        assert_eq!(app.world().resource::<OpenMap>().map.name, map, "{} reloaded the map", workspace.title());
        assert_eq!(app.world().resource::<Selection>().ids(), selected.as_slice(), "{} dropped the selection", workspace.title());
    }
}

/// **Acceptance 2 for Map and for Kit**: each divider is saved by `save_the_layout` under its own
/// workspace's key and comes back in its own workspace after a restart.
#[test]
fn a_divider_moved_in_map_and_one_moved_in_kit_each_come_back_where_they_were_left() {
    let root = fixtures::Fixture::new(&format!("promise_dividers_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place("alpha/floor", (0.0, 0.0))
        .build("m");
    let user = root.join(format!("user_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();

    let mut app = editor_in(&root, Some(&user));
    let map_dock = dock(&mut app);
    app.world_mut().trigger(bevy_gooey::DockLayoutChanged { entity: map_dock, layout: moved(0.31) });
    app.update();

    run_command(&mut app, "workspace.kit");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Kit);
    let kit_dock = dock(&mut app);
    app.world_mut().trigger(bevy_gooey::DockLayoutChanged { entity: kit_dock, layout: moved(0.67) });
    app.update();

    let text = std::fs::read_to_string(&user).unwrap_or_else(|e| panic!("{}: {e}", user.display()));
    let layer = SettingsLayer::read("t", &text).unwrap_or_else(|r| panic!("{}", r.reason));
    assert_eq!(layer.workspace_layout.get("map"), Some(&moved(0.31)), "Map's divider is in its own entry");
    assert_eq!(layer.workspace_layout.get("kit"), Some(&moved(0.67)), "and Kit's in its own");
    drop(app);

    let mut again = editor_in(&root, Some(&user));
    assert_eq!(shown(&mut again), moved(0.31), "after a restart Map opens on the arrangement it was left in");
    run_command(&mut again, "workspace.kit");
    assert_eq!(shown(&mut again), moved(0.67), "and Kit on its own");
}

/// **A settings file from before the workspaces still reads.** `SettingsLayer` refuses unknown fields,
/// so without the migration every existing file with a `shell_layout` was refused wholesale.
#[test]
fn a_file_with_the_old_shell_layout_is_read_as_maps() {
    let layout = ron::ser::to_string(&moved(0.4)).unwrap_or_else(|e| panic!("{e}"));
    let text = format!("(version: {SETTINGS_VERSION}, shell_layout: Some({layout}))");
    let layer = SettingsLayer::read("old/settings.ron", &text).unwrap_or_else(|r| panic!("an old file was refused: {}: {}", r.field.unwrap_or_default(), r.reason));
    assert_eq!(layer.workspace_layout.get("map"), Some(&moved(0.4)), "the one arrangement of before is Map's");
    assert!(layer.shell_layout.is_none(), "and it is not kept a second time");
}

/// **A keymap that rebound `view.switch_door` still reads**, and the id still names the command.
#[test]
fn a_keymap_that_rebound_the_old_switch_id_still_reads() {
    let chord = ron::ser::to_string(&Shortcut::new(Modifiers::NONE, KeyCode::Backquote)).unwrap_or_else(|e| panic!("{e}"));
    let text = format!("(version: {SETTINGS_VERSION}, keymap: {{\"view.switch_door\": {chord}}})");
    let layer = SettingsLayer::read("old/settings.ron", &text).unwrap_or_else(|r| panic!("an old keymap was refused: {}: {}", r.field.unwrap_or_default(), r.reason));
    assert!(layer.keymap.contains_key("view.switch_door"));
    assert_eq!(commands::by_id("view.switch_door").map(|c| c.action), Some(Action::SwitchDoor));
}
