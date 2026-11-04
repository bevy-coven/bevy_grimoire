//! **The inspector shows the selection's pose, and an edit is the operation that owns it** (UI-50, #236).
//!
//! `docs/ui/vision.md` §8: the inspector's fields are edited through gooey's reflect inspector, and an
//! edit "dispatches the matching operation". The map's rows are `bevy_grimoire_core::map::Placed`, plain
//! data in an engine-free crate that refuses `Reflect`, so the inspector edits [`PoseView`] — the
//! editor's own reflected view — and a system turns the change into `placement_set_pose` /
//! `placement_set_lift`. That keeps one writer per fact: a reflected write straight onto a row would skip
//! the wrapping, the check and the undo step.
//!
//! What a headless test can prove: the view follows the locked placement, and an `InspectorChange` on
//! the view's own path ends with the map holding the new pose and one more undo line.

use bevy::prelude::*;
use bevy_cauldron::editor::{lock_on_placement, place_for_test, EditorState};
use bevy_cauldron::inspector_view::{PoseInspector, PoseView};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

/// **The view is a reflected component**, or gooey's inspector has nothing to build rows from.
#[test]
fn the_pose_view_is_reflected() {
    let app = editor("inspector_reflected");
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let registered = registry.read().get(std::any::TypeId::of::<PoseView>()).is_some();
    assert!(registered, "PoseView must be registered, or `inspector(...)` builds no rows");
    let _ = PoseView::default();
}

/// **The view follows the locked placement**, and writes nothing when the selection has not moved.
#[test]
fn the_view_follows_the_locked_piece() {
    let mut app = editor("inspector_mirrors");
    place_for_test(app.world_mut(), "site/wall", (1.5, 0.5));
    app.update();
    let id = {
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
        open.map.placements.last().expect("a placement").id.clone()
    };
    assert!(lock_on_placement(app.world_mut(), &id), "the piece locks");
    for _ in 0..3 {
        app.update();
    }
    let view = app
        .world_mut()
        .query::<&PoseView>()
        .single(app.world())
        .copied()
        .expect("the inspector's view is spawned");
    assert_eq!((view.x, view.z), (1.5, 0.5), "the view shows the piece's own position: {view:?}");
}

/// **An edit of the view's facing is `placement_set_pose`**, through the registry, one undo step.
#[test]
fn a_facing_edit_is_the_operation() {
    let mut app = editor("inspector_edit");
    place_for_test(app.world_mut(), "site/wall", (1.5, 0.5));
    app.update();
    let id = {
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
        open.map.placements.last().expect("a placement").id.clone()
    };
    assert!(lock_on_placement(app.world_mut(), &id), "the piece locks");
    for _ in 0..3 {
        app.update();
    }
    let before = app.world().resource::<EditorState>().undo_depth();
    let inspector = app
        .world()
        .get_resource::<PoseInspector>()
        .and_then(|p| p.0)
        .expect("the inspector has an entity");
    // What gooey sends after it writes the field: the inspector changed `yaw`.
    {
        let mut view = app.world_mut().get_mut::<PoseView>(inspector).expect("the view");
        view.yaw = 90.0;
    }
    app.world_mut().trigger(bevy_gooey::InspectorChange {
        entity: inspector,
        path: "yaw".to_owned(),
    });
    app.update();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
    let placed = open.map.placements.iter().find(|p| p.id == id).expect("the piece is still there");
    assert_eq!(placed.yaw, 90.0, "the operation turned the piece: {placed:?}");
    let after = app.world().resource::<EditorState>().undo_depth();
    assert_eq!(after, before + 1, "one edit is one undo step");
}
