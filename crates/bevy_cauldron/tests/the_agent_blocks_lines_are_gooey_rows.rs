//! **The AGENT block's wish and readout lines are gooey rows that are never squeezed** (#260, UI-99).
//!
//! The PLAN readout, a wish row's header line and its answers line used to be raw `Node` literals. Each is now `chrome::rigid_row`,
//! gooey's `row` with the no-shrink a flex item in a short panel needs, and this file asks the opened block what it is on screen:
//! its gooey `Role`, the gap the skin resolved (the tight 3 px), that it does not shrink, and where it aligns its cells.

use bevy::prelude::*;
use bevy_cauldron::chrome::GAP_TIGHT;
use bevy_cauldron::familiar::{FamiliarItem, FamiliarPlanLine};
use bevy_cauldron::harness;
use bevy_cauldron::wish_view::{WishDiscard, WishOf, WishOpen, WishTitle, WishUnfold};
use bevy_gooey::{Role, ValueChange};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"));
    updates(app, 4);
    answer
}

/// An editor with the agent bridge, the AGENT block shown and one wish queued.
fn editor_with_a_wish(tag: &str) -> (App, String) {
    let root = fixtures::Fixture::new(&format!("agent_lines_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    updates(&mut app, 8);
    let presented = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({"title": "the courtyard", "summary": "A courtyard with its east wall placed.", "operation_ids": []}),
    );
    let after = presented["presentation"].as_str().unwrap_or_else(|| panic!("the present names its presentation: {presented}")).to_owned();
    let made = call(
        &mut app,
        "grimoire/wish",
        serde_json::json!({
            "after": {"presentation": after},
            "task": "place a wall along the courtyard's east side",
            "hardship": "three calls to grimoire_state to find the wall's piece id",
            "tool": "grimoire_find_piece: searches the library by label and answers the ids.",
            "example_call": "grimoire_find_piece({\"label\": \"wall\"})",
        }),
    );
    let id = made["id"].as_str().unwrap_or_else(|| panic!("the wish names its id: {made}")).to_owned();
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    (app, id)
}

/// The entity carrying `T` and the wish `id`.
fn of_wish<T: Component>(app: &mut App, id: &str) -> Entity {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &T, &WishOf)>();
    query.iter(world).find(|(_, _, of)| of.0 == id).map(|(entity, _, _)| entity).unwrap_or_else(|| panic!("the block shows no part for `{id}`"))
}

fn parent(app: &App, entity: Entity) -> Entity {
    app.world().get::<ChildOf>(entity).map(ChildOf::parent).unwrap_or_else(|| panic!("{entity:?} has a parent"))
}

/// A gooey row, the tight gap, never squeezed, its cells on `align`.
fn assert_rigid_row(app: &App, entity: Entity, align: AlignItems, what: &str) {
    assert_eq!(app.world().get::<Role>(entity), Some(&Role::Row), "{what} is a gooey `row`, not a raw `Node`");
    let node = app.world().get::<Node>(entity).unwrap_or_else(|| panic!("{what} has a Node"));
    assert_eq!(node.flex_shrink, 0.0, "{what} keeps its height when the panel is short");
    assert_eq!(node.align_items, align, "{what} aligns its cells");
    let Val::Px(gap) = node.column_gap else { panic!("{what}: the skin writes the gap in px, found {:?}", node.column_gap) };
    assert!((gap - GAP_TIGHT).abs() < 0.01 || gap == 0.0, "{what} stands its cells {gap} px apart");
}

/// **The PLAN readout line** is a gooey row at the top of its cells.
#[test]
fn the_plan_readout_line_is_a_gooey_row_that_is_never_squeezed() {
    let (mut app, _) = editor_with_a_wish("plan");
    let plan = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarPlanLine>>().single(world).expect("the PLAN line is in the block")
    };
    let line = parent(&app, parent(&app, plan));
    assert_rigid_row(&app, line, AlignItems::FlexStart, "the PLAN line");
}

/// **A wish row's header line and its answers line** are gooey rows that are never squeezed.
#[test]
fn a_wish_rows_header_and_answers_lines_are_gooey_rows_that_are_never_squeezed() {
    let (mut app, id) = editor_with_a_wish("wish");
    let title = of_wish::<WishTitle>(&mut app, &id);
    assert_rigid_row(&app, parent(&app, title), AlignItems::Center, "the wish header line");
    // The answers stand in the unfolded body: press the header, as a person does.
    let header = of_wish::<WishUnfold>(&mut app, &id);
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: header });
    updates(&mut app, 6);
    let open = of_wish::<WishOpen>(&mut app, &id);
    let answers = parent(&app, open);
    assert_rigid_row(&app, answers, AlignItems::Stretch, "the wish answers line");
    let discard = of_wish::<WishDiscard>(&mut app, &id);
    assert_eq!(parent(&app, discard), answers, "Open and Discard stand in the one line");
}
