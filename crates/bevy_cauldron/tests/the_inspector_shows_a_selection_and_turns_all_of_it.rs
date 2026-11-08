//! **A selection of several pieces reads mixed where they differ, and typing a facing turns all of them** (UI-50, #236).
//!
//! The inspector is gooey's reflect inspector over `PoseView`. Two pieces with different turns show "—" in the
//! facing (`InspectMixed` names `yaw`, gooey marks the row `MixedValue`), and a typed 90 reaches both
//! through `placement_set_pose` (one call per distinct old facing and tip, since a batch is undone by one
//! inverse), in one undo step, each piece's own tip kept.

use bevy::prelude::*;
use bevy_cauldron::editor::{lock_on_placement, place_for_test, EditorState, Placement};
use bevy_cauldron::inspector_view::{PoseBlock, PoseInspector, PoseView, PoseWidget};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::selection::Selection;
use bevy_gooey::{InspectMixed, InspectRow, MixedValue};

use crate::fixtures;

/// The editor with two pieces on the map: `a` facing `yaw_a` and tipped `tip_a`, `b` likewise. `a` is locked and both are selected.
fn two_pieces(name: &str, (yaw_a, tip_a): (f32, (u8, u8)), (yaw_b, tip_b): (f32, (u8, u8))) -> (App, String, String) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    place_for_test(app.world_mut(), "site/wall", (1.5, 0.5));
    place_for_test(app.world_mut(), "site/wall", (3.5, 0.5));
    app.update();
    let (a, b) = {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        let n = open.map.placements.len();
        assert!(n >= 2, "two pieces were placed: {n}");
        let (a, b) = (open.map.placements[n - 2].id.clone(), open.map.placements[n - 1].id.clone());
        for p in open.map.placements.iter_mut() {
            if p.id == a {
                p.yaw = yaw_a;
                p.tip = tip_a;
            } else if p.id == b {
                p.yaw = yaw_b;
                p.tip = tip_b;
            }
        }
        (a, b)
    };
    assert!(lock_on_placement(app.world_mut(), &a), "the first piece locks");
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().resource_mut::<Selection>().set(vec![a.clone(), b.clone()]);
    for _ in 0..4 {
        app.update();
    }
    (app, a, b)
}

fn mixed_paths(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.query_filtered::<&InspectMixed, With<PoseWidget>>().iter(world).next().map(|m| m.0.clone()).expect("the pose inspector carries its mixed list")
}

/// Does the row editing `path` read mixed (gooey's marker on it or anything under it)?
fn row_reads_mixed(app: &mut App, path: &str) -> bool {
    let world = app.world_mut();
    let rows: Vec<Entity> = world.query::<(Entity, &InspectRow)>().iter(world).filter(|(_, row)| row.path == path).map(|(e, _)| e).collect();
    assert_eq!(rows.len(), 1, "one row edits `{path}`: {rows:?}");
    let mut stack = vec![rows[0]];
    while let Some(at) = stack.pop() {
        if world.get::<MixedValue>(at).is_some() {
            return true;
        }
        if let Some(kids) = world.get::<Children>(at) {
            stack.extend(kids.iter());
        }
    }
    false
}

fn yaw_of(app: &App, id: &str) -> f32 {
    app.world().resource::<OpenMap>().map.placements.iter().find(|p| p.id == id).map(|p| p.yaw).expect("the piece is on the map")
}

/// The turn of the piece **as drawn**: the entity the viewport shows, not the row of the map.
fn drawn_turn_of(app: &mut App, id: &str) -> Quat {
    let world = app.world_mut();
    let mut q = world.query::<(&Placement, &Transform)>();
    let found: Vec<Quat> = q.iter(world).filter(|(p, _)| p.0 == id).map(|(_, t)| t.rotation).collect();
    assert_eq!(found.len(), 1, "one entity draws `{id}`: {}", found.len());
    found[0]
}

fn tip_of(app: &App, id: &str) -> (u8, u8) {
    app.world().resource::<OpenMap>().map.placements.iter().find(|p| p.id == id).map(|p| p.tip).expect("the piece is on the map")
}

/// What gooey sends after it writes the facing the player typed into the view.
fn type_a_facing(app: &mut App, degrees: f32) {
    let view_entity = app.world().resource::<PoseInspector>().0.expect("the view exists");
    let widget = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PoseWidget>>().single(world).expect("one pose inspector in the INSPECTOR panel")
    };
    app.world_mut().get_mut::<PoseView>(view_entity).expect("the view").yaw = degrees;
    app.world_mut().trigger(bevy_gooey::InspectorChange { entity: widget, path: "yaw".to_owned() });
    for _ in 0..4 {
        app.update();
    }
}

#[test]
fn two_pieces_with_different_turns_read_mixed_and_typing_90_turns_both_in_one_undo_step() {
    let (mut app, a, b) = two_pieces("inspector_mixed", (0.0, (0, 0)), (30.0, (0, 0)));
    let mixed = mixed_paths(&mut app);
    assert!(mixed.iter().any(|p| p == "yaw"), "the turns differ, so the facing is mixed: {mixed:?}");
    assert!(!mixed.iter().any(|p| p == "lift"), "the lifts are equal, so the lift is not: {mixed:?}");
    assert!(row_reads_mixed(&mut app, "yaw"), "gooey draws the facing row as mixed (the em dash)");
    assert!(!row_reads_mixed(&mut app, "lift"), "and not the lift row");

    let before = app.world().resource::<EditorState>().undo_depth();
    let drawn_before = drawn_turn_of(&mut app, &a);
    type_a_facing(&mut app, 90.0);
    assert_eq!((yaw_of(&app, &a), yaw_of(&app, &b)), (90.0, 90.0), "typing 90 turned both pieces");
    assert_ne!(drawn_turn_of(&mut app, &a), drawn_before, "and the piece on screen turned, not only the row of the map");
    assert_eq!(drawn_turn_of(&mut app, &a), drawn_turn_of(&mut app, &b), "both are drawn facing 90");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), before + 1, "and it is one undo step");
    assert!(!mixed_paths(&mut app).iter().any(|p| p == "yaw"), "the pieces now agree, so the facing is no longer mixed");
    assert!(!row_reads_mixed(&mut app, "yaw"), "and the row reads 90 again");

    // One undo is the whole edit: each piece goes back to the turn it had, not both to one.
    bevy_cauldron::editor::undo_for_test(app.world_mut());
    for _ in 0..3 {
        app.update();
    }
    assert_eq!((yaw_of(&app, &a), yaw_of(&app, &b)), (0.0, 30.0), "one undo put both pieces back to the turns they had");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), before, "and took only the edit");
}

#[test]
fn pieces_tipped_differently_keep_their_own_tips_and_the_edit_is_still_one_undo_step() {
    let (mut app, a, b) = two_pieces("inspector_tips", (0.0, (0, 0)), (30.0, (1, 0)));
    let before = app.world().resource::<EditorState>().undo_depth();
    type_a_facing(&mut app, 90.0);
    assert_eq!((yaw_of(&app, &a), yaw_of(&app, &b)), (90.0, 90.0), "both are turned");
    assert_eq!((tip_of(&app, &a), tip_of(&app, &b)), ((0, 0), (1, 0)), "a facing never changes how a piece is tipped");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), before + 1, "two calls, one undo step");

    bevy_cauldron::editor::undo_for_test(app.world_mut());
    for _ in 0..3 {
        app.update();
    }
    assert_eq!((yaw_of(&app, &a), yaw_of(&app, &b)), (0.0, 30.0), "one undo put both pieces back to the turns they had");
    assert_eq!((tip_of(&app, &a), tip_of(&app, &b)), ((0, 0), (1, 0)), "and the tips as they were");
}

#[test]
fn one_piece_is_never_mixed_and_the_block_is_drawn_only_while_something_is_selected() {
    let root = fixtures::Fixture::new("inspector_block").descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let shown = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<&Node, With<PoseBlock>>().iter(world).map(|n| n.display != Display::None).collect::<Vec<bool>>()
    };
    assert_eq!(shown(&mut app), vec![false], "nothing is selected, so the block is not drawn");

    place_for_test(app.world_mut(), "site/wall", (1.5, 0.5));
    app.update();
    let id = app.world().resource::<OpenMap>().map.placements.last().expect("a piece").id.clone();
    assert!(lock_on_placement(app.world_mut(), &id));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(shown(&mut app), vec![true], "one piece is selected, so the block is drawn");
    assert!(mixed_paths(&mut app).is_empty(), "one piece differs from nothing");
}

#[test]
fn a_position_typed_with_several_pieces_selected_moves_none_of_them() {
    let (mut app, a, b) = two_pieces("inspector_position", (10.0, (0, 0)), (20.0, (0, 0)));
    let (at_a, at_b) = {
        let open = app.world().resource::<OpenMap>();
        let at = |id: &str| open.map.placements.iter().find(|p| p.id == id).map(|p| p.at).expect("a piece");
        (at(&a), at(&b))
    };
    let before = app.world().resource::<EditorState>().undo_depth();
    let view_entity = app.world().resource::<PoseInspector>().0.expect("the view exists");
    let widget = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PoseWidget>>().single(world).expect("the inspector")
    };
    app.world_mut().get_mut::<PoseView>(view_entity).expect("the view").x = 9.0;
    app.world_mut().trigger(bevy_gooey::InspectorChange { entity: widget, path: "x".to_owned() });
    for _ in 0..4 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>();
    let at = |id: &str| open.map.placements.iter().find(|p| p.id == id).map(|p| p.at).expect("a piece");
    assert_eq!((at(&a), at(&b)), (at_a, at_b), "no operation moves a piece by typing");
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), before, "and nothing was recorded");
}
