//! **The reload panel is gooey's, in the bottom dock** (UI-61, #247).
//!
//! #203 put what became of each file's latest edit in a block the editor built by hand inside the
//! INSPECTOR: a `section` heading, a column, and one `chrome::typed` line per file. That was the
//! right data in the wrong furniture — `docs/ui/vision.md`'s shell has a **bottom dock: history ·
//! problems · notes · log**, and the reload state is the first of those to move there.
//!
//! So the panel is `bevy_gooey`'s now: `ui::register_the_shell_panels` registers `reload` with
//! gooey's `PanelRegistry`, `chrome::spawn_frame` spawns a `bevy_gooey::dock` across the foot of the
//! workbench, and the dock builds the page, the tab and the body. `editor::refresh_reload_list` only
//! fills the column the page spawns, from the same `reload_files` rows `grimoire_state.reload_files`
//! reports.
//!
//! Three claims, and each can fail:
//!
//! 1. the dock is gooey's, laid out, under the body and above the status band — not a hand-built row;
//! 2. the panel it shows is a `DockPanel("reload")`, and the lines are inside it;
//! 3. the dock is not the arrangement's: a drag in it saves nothing over the workspace's layout.

use std::time::Duration;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::chrome::{ArrangementDock, BottomDock, Frame};
use bevy_cauldron::editor::{ReloadList, ReloadPanel};
use bevy_cauldron::harness;
use bevy_gooey::{Dock, DockLayout, DockPanel};
use bevy_grimoire_core::live::{KindVerdict, Verdict};
use bevy_cauldron::project::Project;

use crate::fixtures;

/// The editor on a project with one piece, with the game's verdict read (`--live`).
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("reload_dock_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_cauldron::live::LiveWrite);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The panel looks at the project twice a second**, so a test that writes a verdict and reads the
/// lines has to give it that long: three frames 300 ms apart, as `a_file_says_what_the_game_made_of_its_edit`
/// does, and for the same reason.
fn settle(app: &mut App) {
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(300));
        app.update();
    }
}

/// A laid-out box, logical px.
fn laid(app: &mut App, entity: Entity) -> Rect {
    let world = app.world_mut();
    let (node, at) = world
        .query::<(&ComputedNode, &UiGlobalTransform)>()
        .get(world, entity)
        .expect("laid out");
    let scale = node.inverse_scale_factor();
    Rect::from_center_size(at.translation * scale, node.size() * scale)
}

#[test]
fn the_bottom_dock_is_gooeys_and_stands_under_the_body() {
    let mut app = editor("laid_out");
    let (bottom, body, status, arrangement) = {
        let world = app.world_mut();
        let bottom = world.query_filtered::<Entity, With<BottomDock>>().single(world).expect("the frame has a bottom dock");
        let arrangement = world.query_filtered::<Entity, With<ArrangementDock>>().single(world).expect("and an arrangement dock");
        let frame = world.resource::<Frame>();
        (bottom, frame.body, frame.status, arrangement)
    };
    assert!(app.world().get::<Dock>(bottom).is_some(), "the bottom dock is a `bevy_gooey::Dock`");
    assert!(app.world().get::<Dock>(arrangement).is_some(), "and the arrangement is still one too");
    assert_ne!(bottom, arrangement, "the workspace's layout and the bottom dock are two docks");

    let dock = laid(&mut app, bottom);
    let body_box = laid(&mut app, body);
    let band = laid(&mut app, status);
    assert!(dock.height() > 40.0, "the bottom dock is a real row: {dock:?}");
    assert!(dock.min.y >= body_box.max.y - 2.0, "it stands under the workbench: dock {dock:?}, body {body_box:?}");
    assert!(dock.max.y <= band.min.y + 2.0, "and above the status band: dock {dock:?}, band {band:?}");
}

#[test]
fn the_reload_panel_is_the_docks_and_its_lines_are_in_it() {
    let mut app = editor("panel");
    // Gooey builds the page from the registry, a frame after the dock is first seen.
    for _ in 0..4 {
        app.update();
    }
    let page = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<ReloadPanel>>().single(world).expect("the dock built the reload panel").to_owned()
    };
    let id = app.world().get::<DockPanel>(page).map(|p| p.0.clone());
    assert_eq!(id.as_deref(), Some("reload"), "the page is the registered panel, not an anonymous column");

    // **A game's refusal is one of the lines**, so the list is filled rather than empty: the same
    // `reload_files` rows `grimoire_state.reload_files` reports reach the dock's column.
    let project_dir = app.world().resource::<Project>().project_dir.clone();
    Verdict::with_kinds(4, "refused", vec![KindVerdict::applied("kits", "kits.ron")]).write(&project_dir).unwrap();
    settle(&mut app);
    let lines: Vec<String> = {
        let world = app.world_mut();
        let list = world.query_filtered::<Entity, With<ReloadList>>().single(world).expect("the panel spawns its list").to_owned();
        let mut texts = world.query::<(&Text, &ChildOf)>();
        texts
            .iter(world)
            .filter(|(_, parent)| parent.parent() == list)
            .map(|(text, _)| text.0.clone())
            .collect()
    };
    assert!(!lines.is_empty(), "the dock's reload panel draws the state of every file with news");
    assert!(
        lines.iter().any(|line| line.starts_with("game decided ")),
        "the header that says a game has answered is one of them: {lines:?}"
    );
}

#[test]
fn a_dock_layout_change_in_the_bottom_dock_is_not_the_workspaces_arrangement() {
    let mut app = editor("not_the_arrangement");
    let user = app.world().resource::<Project>().root.join(format!("user_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(user.clone()));
    for _ in 0..4 {
        app.update();
    }
    let bottom = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<BottomDock>>().single(world).unwrap().to_owned()
    };
    // What a person dragging a divider inside the bottom dock reports.
    app.world_mut().trigger(bevy_gooey::DockLayoutChanged {
        entity: bottom,
        layout: bevy_gooey::DockLayout::group(["reload"]),
    });
    app.update();
    let text = std::fs::read_to_string(&user).unwrap_or_default();
    assert!(
        !text.contains("reload"),
        "a drag in the bottom dock wrote this into the workspace's arrangement: {text}"
    );
}

/// **Window ▸ Reset layout leaves the bottom dock alone** (review of #247, MUST-FIX 1).
///
/// `app_verbs::reset_the_layout` writes the current workspace's default arrangement to the dock that
/// arranges the shell. It used to write it to *every* `bevy_gooey::Dock`, so Reset replaced the bottom
/// dock's `group(["reload"])` with the workspace's split: the reload panel was despawned and three
/// placeholder pages were built in the 148-px strip.
#[test]
fn resetting_the_layout_leaves_the_bottom_dock_holding_the_reload_panel() {
    let mut app = editor("reset_keeps_reload");
    let bottom = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<BottomDock>>().single(world).expect("the frame has a bottom dock").to_owned()
    };
    app.world_mut().run_system_cached_with(run_the_verb, bevy_cauldron::app_verbs::AppVerb::ResetLayout)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let layout = app.world().get::<Dock>(bottom).expect("the bottom dock is still a gooey Dock").0.clone();
    assert_eq!(
        layout,
        DockLayout::group(["reload"]),
        "Reset layout wrote the workspace's arrangement into the bottom dock, which despawns the reload panel"
    );
    assert!(
        app.world_mut().query_filtered::<Entity, With<ReloadPanel>>().iter(app.world()).count() == 1,
        "and the reload panel is still the dock's one page"
    );
}

fn run_the_verb(In(verb): In<bevy_cauldron::app_verbs::AppVerb>, mut commands: Commands) {
    commands.queue(move |world: &mut World| bevy_cauldron::app_verbs::run(world, verb));
}
