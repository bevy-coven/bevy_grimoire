//! **A STYLE proposal's row is built from gooey rows that are never squeezed** (SG-07, #406, the ratchet of UI-99).
//!
//! The style row's header line, its answers line and its `NAME  words` lines were raw `Node` literals; each is now `chrome::rigid_row`, gooey's `row` with the
//! no-shrink a flex item in a short panel needs. This file asks the opened block what each is on screen: its gooey `Role`, the tight gap, that it does not
//! shrink, where it aligns its cells, and that the row fills the width of the list it stands in.

use bevy::prelude::*;
use bevy_cauldron::chrome::GAP_TIGHT;
use bevy_cauldron::familiar::FamiliarItem;
use bevy_cauldron::harness;
use bevy_cauldron::wish_view::{StyleDiscard, StyleField, StyleKeep, StyleShow, StyleTitle, WishOf, WishUnfold};
use bevy_gooey::{Role, ValueChange};
use bevy_grimoire_core::trust::TrustStore;
use serde_json::json;

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
    let answer = app.world_mut().run_system_with(id, Some(params)).unwrap_or_else(|e| panic!("{method}: {e:?}")).unwrap_or_else(|e| panic!("{method}: {e:?}"));
    updates(app, 4);
    answer
}

/// An editor with the AGENT block shown and one style proposal queued (and unfolded).
fn editor_with_a_proposal(tag: &str) -> (App, String) {
    let root = fixtures::Fixture::new(&format!("style_rows_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let text = "# The game\n\nWarm light.\n";
    std::fs::create_dir_all(root.join("design")).expect("the folder");
    std::fs::write(root.join("design/style.md"), text).expect("the page");
    let store = root.join("trusted.yaml");
    let mut trusted = TrustStore::default();
    trusted.trust_text(&root, "design/style.md", text).expect("allowed");
    trusted.save(&store).expect("saved");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().trust.file = Some(store);
    updates(&mut app, 8);
    let made = call(&mut app, "grimoire/style_propose", json!({"path": "design/style.md", "edits": [{"find": "Warm light.", "replace": "Warm, low light."}], "why": "dusk reads cold"}));
    let id = made["id"].as_str().unwrap_or_else(|| panic!("the proposal names its id: {made}")).to_owned();
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    let header = of_style::<WishUnfold>(&mut app, &id);
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: header });
    updates(&mut app, 6);
    (app, id)
}

fn of_style<T: Component>(app: &mut App, id: &str) -> Entity {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &T, &WishOf)>();
    query.iter(world).find(|(_, _, of)| of.0 == id).map(|(entity, _, _)| entity).unwrap_or_else(|| panic!("the block shows no part for `{id}`"))
}

fn parent(app: &App, entity: Entity) -> Entity {
    app.world().get::<ChildOf>(entity).map(ChildOf::parent).unwrap_or_else(|| panic!("{entity:?} has a parent"))
}

fn assert_rigid_row(app: &App, entity: Entity, align: AlignItems, what: &str) {
    assert_eq!(app.world().get::<Role>(entity), Some(&Role::Row), "{what} is a gooey `row`, not a raw `Node`");
    let node = app.world().get::<Node>(entity).unwrap_or_else(|| panic!("{what} has a Node"));
    assert_eq!(node.flex_shrink, 0.0, "{what} keeps its height when the panel is short");
    assert_eq!(node.align_items, align, "{what} aligns its cells");
    let Val::Px(gap) = node.column_gap else { panic!("{what}: the skin writes the gap in px, found {:?}", node.column_gap) };
    assert!((gap - GAP_TIGHT).abs() < 0.01 || gap == 0.0, "{what} stands its cells {gap} px apart");
}

/// **The header line, the answers line and a field line are gooey rows that are never squeezed.**
#[test]
fn a_style_rows_lines_are_gooey_rows_that_are_never_squeezed() {
    let (mut app, id) = editor_with_a_proposal("lines");
    let title = of_style::<StyleTitle>(&mut app, &id);
    assert_rigid_row(&app, parent(&app, title), AlignItems::Center, "the style header line");
    let keep = of_style::<StyleKeep>(&mut app, &id);
    let answers = parent(&app, keep);
    assert_rigid_row(&app, answers, AlignItems::Stretch, "the style answers line");
    for part in [of_style::<StyleShow>(&mut app, &id), of_style::<StyleDiscard>(&mut app, &id)] {
        assert_eq!(parent(&app, part), answers, "Show the page, Keep and Discard stand in the one line");
    }
    let field = of_style::<StyleField>(&mut app, &id);
    let line = parent(&app, parent(&app, field));
    assert_rigid_row(&app, line, AlignItems::FlexStart, "a style field line");
}

/// **A style row and its header fill the width they stand in.**
#[test]
fn a_style_row_and_its_header_fill_the_width_they_stand_in() {
    let (mut app, id) = editor_with_a_proposal("width");
    let header = of_style::<WishUnfold>(&mut app, &id);
    let row = parent(&app, header);
    let list = parent(&app, row);
    let logical = |entity: Entity| {
        let computed = app.world().get::<bevy::ui::ComputedNode>(entity).expect("laid out");
        (computed.size().x * computed.inverse_scale_factor(), computed.content_box().width() * computed.inverse_scale_factor())
    };
    let (header_w, _) = logical(header);
    let (row_w, row_room) = logical(row);
    let (_, list_room) = logical(list);
    assert!((row_w - list_room).abs() < 1.0, "the style row is {row_w} px wide in a list with {list_room} px of room");
    assert!((header_w - row_room).abs() < 1.0, "the header is {header_w} px wide in a row with {row_room} px of room");
}
