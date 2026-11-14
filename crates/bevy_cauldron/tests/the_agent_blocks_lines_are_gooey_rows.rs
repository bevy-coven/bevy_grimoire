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

/// **A wish row and its header fill the width they stand in**, now that they no longer ask for `width: 100%`: the row is as wide as the list it is in, and
/// the header as wide as the row.
#[test]
fn a_wish_row_and_its_header_fill_the_width_they_stand_in() {
    let (mut app, id) = editor_with_a_wish("width");
    let header = of_wish::<WishUnfold>(&mut app, &id);
    let row = parent(&app, header);
    let list = parent(&app, row);
    let logical = |entity: Entity| {
        let computed = app.world().get::<bevy::ui::ComputedNode>(entity).expect("laid out");
        (computed.size().x * computed.inverse_scale_factor(), computed.content_box().width() * computed.inverse_scale_factor())
    };
    let (header_w, _) = logical(header);
    let (row_w, row_room) = logical(row);
    let (_, list_room) = logical(list);
    assert!((row_w - list_room).abs() < 1.0, "the wish row is {row_w} px wide in a list with {list_room} px of room");
    assert!((header_w - row_room).abs() < 1.0, "the header is {header_w} px wide in a row with {row_room} px of room");
}

/// **A wish row's title takes the room its line leaves and keeps its ink**: gooey's `TextFlow` (grow 1, no least width) on the text, whose colour and font are the host's.
#[test]
fn a_wish_titles_flow_is_gooeys_and_keeps_its_ink() {
    let (mut app, id) = editor_with_a_wish("flow");
    let title = of_wish::<WishTitle>(&mut app, &id);
    assert_eq!(app.world().get::<Role>(title), Some(&Role::Sized), "the title carries gooey's layout role, which `TextFlow` is written on");
    let node = app.world().get::<Node>(title).expect("a title has a Node");
    assert_eq!(node.flex_grow, 1.0, "the title takes what its line leaves");
    assert_eq!(node.min_width, Val::Px(0.0), "and may narrow to nothing");
    assert_eq!(app.world().get::<TextColor>(title).map(|c| c.0), Some(bevy_cauldron::chrome::ink().text), "its ink is the host's");
}

/// **A readout's label keeps its column**: at least the 58 px readout label column wide, never shrunk, and a 5 px margin of its own so the value never starts at its last glyph.
#[test]
fn a_readouts_label_keeps_its_column_through_textflow() {
    let (mut app, _) = editor_with_a_wish("label");
    let label = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Text)>();
        q.iter(world).find(|(_, text)| text.0 == "HEARD").map(|(e, _)| e).expect("the Familiar block has its HEARD label")
    };
    let node = app.world().get::<Node>(label).expect("a label has a Node");
    assert_eq!(node.min_width, Val::Px(58.0), "at least the readout label column (58 px) wide");
    assert_eq!(node.flex_shrink, 0.0, "never shrunk");
    assert_eq!(node.margin.right, Val::Px(bevy_cauldron::chrome::GAP_ROW), "and a 5 px margin of its own");
    assert_eq!(app.world().get::<TextColor>(label).map(|c| c.0), Some(bevy_cauldron::chrome::ink().label), "in the label ink");
}
