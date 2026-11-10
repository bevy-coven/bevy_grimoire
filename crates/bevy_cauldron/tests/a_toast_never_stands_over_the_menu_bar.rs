//! **No toast is ever drawn over the menu bar** (#305; oracle `other:below-the-bar`).
//!
//! Seen on `tools/ux/first_run_walk.py` at 1088x640 and 1024x640: the "New here? Take the 2-minute
//! tour" toast stands at the window's top centre and covers the Compose/Rigs/Tools/Help titles. The
//! editor owns its own toast host inside the viewport, which starts below the bar — but a toast
//! raised on a screen that has no host (the tour offer, which the editor raises as a project opens,
//! and any refusal raised before the frame is built) makes `bevy_gooey` spawn its own stack at the
//! window's root, and nothing set the inset it honours. Measured on the editor with a tour offer up:
//! bar 0..28.33 px, toast 9.17..53.33 px — the toast's head sits over the bar's titles.
//!
//! The oracle is where a person expects it: **every toast's top edge is at or below the menu bar's
//! bottom edge**, and still on screen. Layout is logical px (`ComputedNode::size` and
//! `UiGlobalTransform` are physical, times `inverse_scale_factor`), so both are read in one space.

use bevy::prelude::*;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::menu_bar::EditorMenuBar;
use bevy_cauldron::recent::RecentProjects;

use crate::fixtures;

/// The editor on a project with one piece, its menu bar and toast host up.
fn open(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place("alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **A new person**: the editor with a recent-projects file that has never offered the tour, which is
/// what raises the offer toast this issue is about. The file is per-test, so it races with nothing.
fn a_new_person(name: &str) -> App {
    let file = std::env::temp_dir().join(format!("grimoire-toast-{name}-{}.yaml", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place("alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(file));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// `entity`'s laid-out rectangle in logical px.
fn rect_of(app: &App, entity: Entity) -> Rect {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("placed");
    Rect::from_center_size(at.translation * node.inverse_scale_factor, node.size() * node.inverse_scale_factor)
}

fn menu_bar(app: &mut App) -> Entity {
    let mut q = app.world_mut().query_filtered::<Entity, With<EditorMenuBar>>();
    q.iter(app.world()).next().expect("the menu bar is up on the editor screen")
}

/// Every toast on screen, in the order gooey holds them.
fn toasts(app: &mut App) -> Vec<(Entity, String)> {
    let mut q = app.world_mut().query::<(Entity, &bevy_gooey::Toast)>();
    q.iter(app.world()).map(|(e, t)| (e, t.text().to_owned())).collect()
}

/// The window's height in the same logical px `rect_of` reports.
fn surface_height(app: &App) -> f32 {
    rect_of(app, app.world().resource::<bevy_cauldron::chrome::Frame>().root).max.y
}

/// **The claim, for every toast on screen**: none covers the bar, and none is pushed off the bottom.
fn every_toast_stands_below_the_bar(app: &mut App, which: &str) {
    let bar_entity = menu_bar(app);
    let bar = rect_of(app, bar_entity);
    assert!(bar.height() > 0.0, "{which}: the bar was laid out: {bar:?}");
    let bottom = surface_height(app);
    let up = toasts(app);
    assert!(!up.is_empty(), "{which}: nothing was raised, so this proves nothing");
    for (entity, text) in up {
        let toast = rect_of(app, entity);
        assert!(
            toast.min.y >= bar.max.y,
            "{which}: the toast {text:?} covers the menu bar: the bar is {bar:?} and the toast is {toast:?}"
        );
        assert!(toast.height() > 0.0, "{which}: the toast {text:?} is drawn, not collapsed: {toast:?}");
        assert!(toast.max.y <= bottom, "{which}: the toast {text:?} is on screen, not past the bottom edge ({bottom}): {toast:?}");
    }
}

/// **The toast the walk photographed.** The tour's offer is raised as a project with pieces opens,
/// before the editor's own host exists, so gooey built the stack at the window's root.
#[test]
fn the_tours_offer_does_not_cover_the_menu_bar() {
    let mut app = a_new_person("tour_offer");
    assert!(
        toasts(&mut app).iter().any(|(_, text)| text.contains("tour")),
        "a new person is offered the tour: {:?}",
        toasts(&mut app)
    );
    every_toast_stands_below_the_bar(&mut app, "the tour offer");
}

/// **A refusal's toast**, which the editor hosts itself — the other path, so a fix for one cannot
/// leave the other over the bar.
#[test]
fn a_refusals_toast_does_not_cover_the_menu_bar() {
    let mut app = open("refusal_toast");
    app.world_mut().resource_mut::<EditorState>().status.problem("cannot do that: it is not allowed".to_owned());
    for _ in 0..6 {
        app.update();
    }
    every_toast_stands_below_the_bar(&mut app, "a refusal");
}
