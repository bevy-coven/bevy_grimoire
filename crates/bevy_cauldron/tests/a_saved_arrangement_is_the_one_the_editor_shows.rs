//! **A person's arrangement of the shell is theirs, across a restart** (UI-33, #227).
//!
//! `docs/ui/vision.md` §4 asks that each door's arrangement be data a person edits. `bevy_gooey`'s
//! `Dock` already rebuilds itself from a `DockLayout` and reports every change a person makes as
//! `DockLayoutChanged`; what the editor lacked was a place to keep it, a way to read it back, and a way
//! to return to the default.
//!
//! The oracle is the person's own claim, in three parts:
//! - **a restart keeps it**: a layout saved in the user's `settings.yaml` is the one a fresh editor opens
//!   on, so an inspector moved to the other side is still there;
//! - **Reset restores it**: Window ▸ Reset layout puts the shell back on `ui::shell_layout` and takes the
//!   key out of the file, so a restart after a reset is the default again;
//! - **a drag is saved**: `DockLayoutChanged` writes the layout gooey reports into that file.
//!
//! Every check reads `Dock.0` — the arrangement the editor is actually showing — or the file the next
//! run would read, never an intermediate.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use bevy_cauldron::app_verbs::{self, AppVerb};
use bevy_cauldron::settings::{EditorSettings, UserSettingsFile};
use bevy_gooey::{DockLayout, SplitAxis};

use crate::fixtures;

/// An arrangement no default has: the docks and the viewport swap sides.
fn moved() -> DockLayout {
    DockLayout::split(
        SplitAxis::Horizontal,
        0.78,
        DockLayout::group(["place"]),
        DockLayout::split(SplitAxis::Horizontal, 0.22, DockLayout::group(["viewport"]), DockLayout::group(["inspector"])),
    )
}

struct Rig {
    app: App,
    user: PathBuf,
}

/// The editor on a project, with the user's settings file at `user`.
fn editor_at(tag: &str, user: PathBuf) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place("alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user));
    for _ in 0..10 {
        app.update();
    }
    app
}

fn rig(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(&format!("layout_{tag}_{}", std::process::id())).descriptor("alpha/floor", "alpha").place("alpha/floor", (0.0, 0.0)).build("m");
    let user = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    let app = editor_at(tag, user.clone());
    Rig { app, user }
}

/// The layout the editor is showing.
fn shown(app: &mut App) -> DockLayout {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&bevy_gooey::Dock, With<bevy_cauldron::chrome::ArrangementDock>>();
    q.iter(world).next().expect("the shell is a gooey Dock").0.clone()
}

/// What the next start would read: the Map workspace's layout in the user's file, if it has one.
///
/// **Per workspace since NV-02 (#379)**: the key is a map of workspace id to layout, and this file
/// is about the arrangement Map opens on, so it reads Map's entry.
fn saved(path: &Path) -> Option<DockLayout> {
    let text = std::fs::read_to_string(path).ok()?;
    bevy_cauldron::settings::SettingsLayer::read(&path.to_string_lossy(), &text)
        .unwrap_or_else(|r| panic!("the editor's own write must read back: {}: {}", r.field.unwrap_or_default(), r.reason))
        .workspace_layout
        .remove("map")
}

/// **A saved arrangement is the one the editor opens on**, which is what "survives a restart" means.
#[test]
fn a_saved_arrangement_is_the_one_the_editor_shows() {
    let Rig { mut app, user } = rig("saved");
    assert_eq!(shown(&mut app), bevy_cauldron::ui::shell_layout(bevy_cauldron::workspace::Workspace::Map), "a new person starts on the default");
    assert_eq!(saved(&user), None, "and has no layout in their file");

    // A person's arrangement, as their file would hold it after a drag.
    bevy_cauldron::setting_catalogue::edit_layer(&user, |layer| {
        layer.workspace_layout.insert("map".to_owned(), moved());
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e}"));

    // **A restart**: a fresh application reading that file.
    let mut again = editor_at("saved_again", user.clone());
    assert_eq!(shown(&mut again), moved(), "the editor opens on the arrangement in the file");
}

/// **Window ▸ Reset layout** puts the default back, on screen and in the file.
#[test]
fn reset_puts_the_default_back_and_the_file_agrees() {
    let Rig { mut app, user } = rig("reset");
    bevy_cauldron::setting_catalogue::edit_layer(&user, |layer| {
        layer.workspace_layout.insert("map".to_owned(), moved());
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    assert_eq!(shown(&mut app), moved(), "the editor is on the person's arrangement before the reset");

    app.world_mut().run_system_cached_with(run_the_verb, AppVerb::ResetLayout).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(shown(&mut app), bevy_cauldron::ui::shell_layout(bevy_cauldron::workspace::Workspace::Map), "Reset is the default, on screen");
    assert_eq!(saved(&user), None, "and the key is gone, so the next start is the default too");
    assert_eq!(
        app.world().resource::<EditorSettings>().workspace_layout.get("map").cloned(),
        None,
        "and the running settings agree with the file"
    );
}

fn run_the_verb(In(verb): In<AppVerb>, mut commands: Commands) {
    commands.queue(move |world: &mut World| app_verbs::run(world, verb));
}

/// **A drag is saved**: the layout gooey reports is what the file holds afterwards.
#[test]
fn a_layout_change_gooey_reports_reaches_the_file() {
    let Rig { mut app, user } = rig("drag");
    let dock = {
        let world = app.world_mut();
        let mut q = world
            .query_filtered::<Entity, (With<bevy_gooey::Dock>, With<bevy_cauldron::chrome::ArrangementDock>)>();
        q.iter(world).next().expect("the shell's dock")
    };
    app.world_mut().trigger(bevy_gooey::DockLayoutChanged { entity: dock, layout: moved() });
    app.update();
    assert_eq!(saved(&user), Some(moved()), "the change a person made is in their file");
}
