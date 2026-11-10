//! **Every panel can be staged for a capture** (UI-92, #258).
//!
//! A clip of a panel has to start on that panel. [`stage_panels::STAGES`] is the catalog and [`stage_panels::arrange`] the
//! one function that puts the shell on one: a screen is a `Screen` transition, the settings screen is the dialog over the
//! workbench, and a dock panel is its group's tab selected — the arrangement a person's click makes, so a clip shows what
//! a person would be looking at rather than a layout built for the camera.
//!
//! These tests read the registry and the dock the editor actually has, so a panel added to the shell without a stage, or a
//! stage with nothing behind it, fails here rather than in a blank clip.

use bevy::prelude::*;
use bevy_cauldron::screen::Screen;
use bevy_cauldron::stage_panels::{self, STAGES};
use bevy_gooey::{Dock, DockLayout, PanelRegistry};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    app
}

/// The tab each group of `layout` shows, one entry per group, in tree order.
fn shown_tabs(layout: &DockLayout) -> Vec<(Vec<String>, String)> {
    fn walk(node: &DockLayout, out: &mut Vec<(Vec<String>, String)>) {
        match node {
            DockLayout::Split { first, second, .. } => {
                walk(first, out);
                walk(second, out);
            }
            DockLayout::Group { panels, selected } => {
                out.push((panels.clone(), panels.get(*selected).cloned().unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(layout, &mut out);
    out
}

/// The dock's arrangement as the editor holds it, or the default one when no dock has been built yet.
fn dock_here(app: &mut App) -> DockLayout {
    app.world_mut()
        .query_filtered::<&Dock, ()>()
        .iter(app.world())
        .next()
        .map_or_else(bevy_cauldron::ui::shell_layout, |dock| dock.0.clone())
}

/// **The catalog names every panel the shell has, and each name is a screen or a registered panel.** A dock panel in the
/// catalog and not in the registry is a name `record.py` would send and nothing would show.
#[test]
fn the_catalog_is_the_shells_own_panels() {
    let mut app = editor(&format!("stage_catalog_{}", std::process::id()));
    let registry = app.world_mut().resource::<PanelRegistry>().clone();
    let missing: Vec<&str> = stage_panels::dock_panels().into_iter().filter(|id| !registry.contains(id)).collect();
    assert!(missing.is_empty(), "a staged panel the shell does not register would be a blank clip: {missing:?}");
    let staged = stage_panels::dock_panels().len();
    assert!(staged >= 3, "the shell has more than three dock panels: {staged}");
    // **And nothing registered is left without an answer**: either it stages, or the catalog says why it does not.
    for id in ["inspector", "viewport", "place", "status"] {
        assert!(registry.contains(id), "`{id}` is a panel the shell registers");
        let covered = stage_panels::dock_panels().contains(&id) || stage_panels::NOT_STAGED.iter().any(|(name, _)| *name == id);
        assert!(covered, "`{id}` is registered and no capture can stage it, with no reason on the record");
    }
    for screen in ["launcher", "loader", "settings"] {
        assert!(stage_panels::is_screen(screen), "{screen} is a screen, not a dock panel");
        assert!(stage_panels::stage(screen).is_some(), "the catalog names {screen}");
    }
    assert!(stage_panels::stage("no.such.panel").is_none());
    assert!(stage_panels::dock_panels().iter().all(|id| !stage_panels::is_screen(id)));
}

/// **Every panel in the catalog stages**, and the answer names the screen it left the editor on. The run walks
/// [`stage_panels::capture_order`], which is the order `record.py` stages them in, so the test fails if that order is one
/// the editor cannot follow.
#[test]
fn every_panel_in_the_catalog_stages() {
    let mut app = editor(&format!("stage_every_{}", std::process::id()));
    let order = stage_panels::capture_order();
    assert_eq!(order.len(), STAGES.len(), "the capture order names every panel once: {order:?}");
    let mut staged = Vec::new();
    for id in &order {
        let stage = stage_panels::stage(id).unwrap_or_else(|| panic!("{id} is not in the catalog"));
        let answer = stage_panels::arrange(app.world_mut(), stage.id)
            .unwrap_or_else(|e| panic!("`{}` is in the catalog and will not stage: {e}", stage.id));
        assert_eq!(answer.id, stage.id);
        let want = match stage.id {
            "launcher" => Screen::Launcher,
            "loader" => Screen::Loading,
            _ => Screen::Editor,
        };
        assert_eq!(answer.screen, want, "`{}` says which screen it staged", stage.id);
        staged.push(stage.id);
    }
    assert_eq!(staged.len(), STAGES.len(), "one answer per panel, and the test visited every one");
    // **The last stage's move is queued**, which is what a capture started in the same frame reads: a state change is
    // applied by the next state-transition pass, so the queue is the editor's answer this frame.
    assert!(
        matches!(app.world().resource::<NextState<Screen>>(), NextState::Pending(Screen::Launcher)),
        "the last stage in the order is the launcher: {:?}",
        app.world().resource::<NextState<Screen>>()
    );
}

/// **A dock panel is staged by selecting its tab**, in every group that holds it, and the layout's other groups are left as
/// they were.
#[test]
fn a_dock_panel_is_staged_by_selecting_its_tab() {
    let mut app = editor(&format!("stage_dock_{}", std::process::id()));
    let groups = shown_tabs(&dock_here(&mut app));
    assert!(groups.len() >= 2, "the shell has a left dock and a right one: {groups:?}");
    for (panels, was_showing) in groups {
        // The tab that is not in front, where the group has more than one.
        let panel = panels.iter().find(|p| **p != was_showing).cloned().unwrap_or(was_showing.clone());
        stage_panels::arrange(app.world_mut(), &panel).unwrap_or_else(|e| panic!("{panel}: {e}"));
        let now = shown_tabs(&dock_here(&mut app));
        assert!(
            now.iter().any(|(group, shown)| group.contains(&panel) && *shown == panel),
            "`{panel}` is the tab its group shows after being staged: {now:?}"
        );
    }
}

/// **A name the shell has not got is refused in words that say what it has.**
#[test]
fn an_unknown_panel_is_refused_by_name() {
    let mut app = editor(&format!("stage_refused_{}", std::process::id()));
    let refused = stage_panels::arrange(app.world_mut(), "not a panel").expect_err("an unknown name stages nothing");
    assert!(refused.contains("not a panel"), "{refused}");
    assert!(refused.contains("inspector"), "the refusal names what would have worked: {refused}");
}

/// **A workbench panel cannot be staged from another screen**, and the refusal says so rather than leaving a stale layout
/// behind for the next capture.
#[test]
fn a_workbench_panel_is_refused_off_the_workbench() {
    let mut app = editor(&format!("stage_closed_{}", std::process::id()));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 6);
    let refused = stage_panels::arrange(app.world_mut(), "place").expect_err("the workbench is closed");
    assert!(refused.contains("place") && refused.contains("Launcher"), "{refused}");
}

/// **Staging a panel of the workbench leaves the project open**, so a capture of the next panel starts where the last one
/// ended rather than on an empty editor.
#[test]
fn staging_the_workbench_leaves_the_project_open() {
    let mut app = editor(&format!("stage_keeps_{}", std::process::id()));
    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    stage_panels::arrange(app.world_mut(), "viewport").expect("the viewport stages");
    updates(&mut app, 3);
    assert_eq!(app.world().resource::<bevy_cauldron::project::Project>().root, root);
    assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Editor);
}

/// **The answer a caller gets is the arrangement, not a promise.** `grimoire/stage_panel` answers with what the shell is now
/// on; a queued arrangement could only report a refusal in a log line, and the live editor measured on 2026-11-09 answered
/// `staged: true` for an `inspector` the loading screen had no workbench to show — the warning sat in the editor's own log
/// while the caller read success. So the arrangement runs in the request, and a refusal is what the caller gets.
#[test]
fn the_staging_command_answers_with_the_arrangement_or_the_refusal() {
    let mut app = editor(&format!("stage_op_{}", std::process::id()));
    // The workbench's own panel: arranged, and the answer names the screen it is on.
    let staged = stage_panels::arrange(app.world_mut(), "viewport").expect("the viewport stages");
    assert_eq!(staged.id, "viewport");
    assert_eq!(staged.screen, Screen::Editor);
    // A name nobody has is a refusal in words, which is what the command returns as its error.
    let refused = stage_panels::arrange(app.world_mut(), "nowhere").expect_err("a name the shell has not got");
    assert!(refused.contains("nowhere") && refused.contains("inspector"), "{refused}");
    // And a workbench panel with the workbench closed refuses rather than arranging a tab nobody can see.
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 6);
    let refused = stage_panels::arrange(app.world_mut(), "place").expect_err("the workbench is closed");
    assert!(refused.contains("place") && refused.contains("Launcher"), "{refused}");
}
