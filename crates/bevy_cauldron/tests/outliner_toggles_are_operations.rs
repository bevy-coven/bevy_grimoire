//! **The outliner's visibility and lock toggles are editor operations, so a person and an agent change one thing**
//! (UI-51, #237; E-002, #129).
//!
//! The issue's *What* ends "includes search and visibility/lock toggles". The row menu already hid a piece for the
//! session; this file holds what makes the toggles real:
//!
//! - **Three operations, one per verb**: `placement_set_hidden`, `layer_set_visible` and `placement_set_locked`. The
//!   outliner's menus dispatch them and an agent calls them through `grimoire/operation_apply`, and what the editor
//!   holds afterwards is the same.
//! - **A hidden piece stays on the map.** Hiding changes what the viewport draws and what a click can pick, never the
//!   map, the file or the dirty flag; a whole layer hides the same way.
//! - **A locked piece is not moved or removed by a click**, and an operation still can.
//! - **The tree says what is hidden or locked**, on the piece's row and on the layer's row.
//!
//! Every click here is the real thing: the pointer is injected in `PreUpdate`, `X` and `B` arm the tools, and the menus
//! are the row's own `ContextMenu` with the panel's `ContextTarget` and the item's `Activate`.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, MoveDrag, Placement};
use bevy_cauldron::outliner::{Eyes, LAYER_ITEM, Locks, Outliner, OutlinerItem, OutlinerTree, group_key, placement_key};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;
use bevy_gooey::{ContextMenu, ContextTarget, MenuItem, Table, TableRow, Tree, ValueChange};
use serde_json::{Value, json};

use crate::fixtures;

/// One frame's worth of person: the keys held down, the pointer, and a mouse press or release.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else { return };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **Two floor pieces and one ceiling piece**, with the outliner open: two layers, so the tree has a row for each.
fn rig(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("outliner_toggles_{tag}_{}", std::process::id()))
        .descriptor("on_floor", "alpha")
        .descriptor("on_ceiling", "alpha")
        .place("on_floor", (0.0, 0.0))
        .place("on_floor", (3.0, 0.0))
        .place("on_ceiling", (6.0, 0.0))
        .build("m");
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.yaml");
    let text = std::fs::read_to_string(&library).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    let at = text.find("id: on_ceiling\n").expect("the ceiling piece is in the library");
    let was = "mount: OnFloor";
    let found = text[at..].find(was).expect("the fixture's floor mount") + at;
    let mut text = text;
    text.replace_range(found..found + was.len(), "mount: OnCeiling");
    std::fs::write(&library, text).unwrap_or_else(|e| panic!("{library:?}: {e}"));

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    updates(&mut app, 8);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("the View menu carries one Show outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    app
}

/// **Through the registered method**, as an agent's request goes.
fn apply(app: &mut App, operation: &str, parameters: Value) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered");
    };
    let said = app
        .world_mut()
        .run_system_with(id, Some(json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    updates(app, 3);
    said
}

/// The log lines of `operation`, as `(source, line)`.
fn logged(app: &App, operation: &str) -> Vec<(String, Value)> {
    let root = app.world().resource::<Project>().root.clone();
    let text = std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(&root)).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == operation)
        .map(|v| (v["source"].as_str().unwrap_or_default().to_owned(), v))
        .collect()
}

fn ids(app: &App) -> Vec<String> {
    app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).collect()
}

fn row_entity(app: &mut App, key: u64) -> Entity {
    let world = app.world_mut();
    let mut rows = world.query::<(Entity, &TableRow)>();
    rows.iter(world).find(|(_, row)| row.key == key).map(|(entity, _)| entity).expect("the row is drawn")
}

/// **Choose `label` from the menu a right-click on the row `key` opens**, through the real path.
fn choose(app: &mut App, key: u64, label: &str) {
    let row = row_entity(app, key);
    let panel = app.world().entity(row).get::<ContextMenu>().expect("the row names a menu").0;
    app.world_mut().entity_mut(panel).insert(ContextTarget(row));
    let item = {
        let world = app.world_mut();
        let mut texts = world.query::<(&Text, &ChildOf)>();
        let mut items = world.query_filtered::<(), With<MenuItem>>();
        let found: Vec<Entity> =
            texts.iter(world).filter(|(t, p)| t.0 == label && items.get(world, p.parent()).is_ok()).map(|(_, p)| p.parent()).collect();
        assert_eq!(found.len(), 1, "one menu item says `{label}`: {found:?}");
        found[0]
    };
    app.world_mut().commands().trigger(Activate { entity: item });
    updates(app, 4);
}

fn label(app: &mut App, key: u64) -> String {
    let entity = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerTree>>().single(world).expect("one outliner tree")
    };
    let table = app.world().get::<Table>(entity).expect("the tree is a table");
    table.source::<Tree>().and_then(|t| t.label(key)).unwrap_or_default().to_owned()
}

fn visible(app: &mut App, id: &str) -> bool {
    let world = app.world_mut();
    let mut q = world.query::<(&Placement, &Visibility)>();
    let (_, vis) = q.iter(world).find(|(p, _)| p.0 == id).unwrap_or_else(|| panic!("the map drew {id}"));
    *vis != Visibility::Hidden
}

fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app.world_mut().query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    camera.world_to_viewport(transform, world).unwrap_or_else(|e| panic!("the piece is not on screen: {e:?}"))
}

/// A press and a release over `at`: one click.
fn click(app: &mut App, at: (f32, f32)) {
    let over = pixel_over(app, at);
    step(app, Frame { pointer: Some(over), mouse: Some(true), ..default() });
    step(app, Frame { pointer: Some(over), mouse: Some(false), ..default() });
}

fn say(app: &App) -> String {
    app.world().resource::<EditorState>().status.line().to_owned()
}

/// **An agent hides a layer: the viewport stops drawing it, the tree says so, and the map is untouched.**
#[test]
fn an_agent_hides_a_layer_and_the_map_is_untouched() {
    let mut app = rig("layer_agent");
    let (before, dirty) = (ids(&app), app.world().resource::<OpenMap>().dirty());
    let said = apply(&mut app, "layer_set_visible", json!({"layer": "ceiling", "visible": false}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(app.world().resource::<Eyes>().layer_hidden("ceiling"), "the editor's eye holds the layer: {said}");
    assert!(!visible(&mut app, "on_ceiling@2"), "the ceiling piece is not drawn");
    assert!(visible(&mut app, "on_floor@0"), "the floor is");
    assert_eq!(ids(&app), before, "every piece is still on the map");
    assert_eq!(app.world().resource::<OpenMap>().dirty(), dirty, "and nothing is written");
    assert_eq!(label(&mut app, group_key("ceiling")), "ceiling (hidden)", "the layer's row says so");
    assert!(label(&mut app, placement_key("on_ceiling@2")).ends_with(" (hidden)"), "and so does the piece's");

    let said = apply(&mut app, "layer_set_visible", json!({"layer": "ceiling", "visible": true}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(visible(&mut app, "on_ceiling@2"), "showing the layer draws it again");
    assert_eq!(label(&mut app, group_key("ceiling")), "ceiling");

    let refused = apply(&mut app, "layer_set_visible", json!({"layer": "basement", "visible": false}));
    assert_eq!(refused["outcome"], "refused", "a layer that is not one is refused: {refused}");
}

/// **The layer row's menu hides the layer through the operation an agent calls**, and the same menu item shows it again.
#[test]
fn a_layer_rows_menu_dispatches_layer_set_visible() {
    let mut app = rig("layer_menu");
    choose(&mut app, group_key("ceiling"), LAYER_ITEM);
    assert!(app.world().resource::<Eyes>().layer_hidden("ceiling"), "the menu hid the layer");
    assert!(!visible(&mut app, "on_ceiling@2"));
    let lines = logged(&app, "layer_set_visible");
    assert_eq!(lines.len(), 1, "one operation, one log line: {lines:?}");
    assert_eq!(lines[0].0, "human", "the menu is the person's: {:?}", lines[0]);
    assert_eq!(lines[0].1["params"], json!({"layer": "ceiling", "visible": false}), "{:?}", lines[0]);
    choose(&mut app, group_key("ceiling"), LAYER_ITEM);
    assert!(!app.world().resource::<Eyes>().layer_hidden("ceiling"), "and the same item shows it again");
    assert!(visible(&mut app, "on_ceiling@2"));
    let layer = {
        let world = app.world_mut();
        world.query::<&Outliner>().single(world).expect("the outliner is open").layer_of_row(group_key("floor"))
    };
    assert_eq!(layer, Some("floor"), "the floor's row is a layer row too");
}

/// **A piece row's Hide and Lock are `placement_set_hidden` and `placement_set_locked`**, the person's lines in the log.
#[test]
fn a_rows_hide_and_lock_dispatch_the_operations_an_agent_calls() {
    let mut app = rig("row_menu");
    let id = "on_floor@0";
    choose(&mut app, placement_key(id), "Hide");
    choose(&mut app, placement_key(id), "Lock");
    assert_eq!(app.world().resource::<Eyes>().hidden(), vec![id.to_owned()]);
    assert_eq!(app.world().resource::<Locks>().locked(), vec![id.to_owned()]);
    let hide = logged(&app, "placement_set_hidden");
    let lock = logged(&app, "placement_set_locked");
    assert_eq!((hide.len(), lock.len()), (1, 1), "one line each: {hide:?} {lock:?}");
    assert_eq!((hide[0].0.as_str(), lock[0].0.as_str()), ("human", "human"));
    let said = label(&mut app, placement_key(id));
    assert!(said.starts_with(id) && said.ends_with(" (hidden) (locked)"), "the row says what was done to the piece: {said}");

    // **The agent undoes both through the same operations**, and the editor holds what it said.
    assert_eq!(apply(&mut app, "placement_set_hidden", json!({"id": id, "hidden": false}))["outcome"], "applied");
    assert_eq!(apply(&mut app, "placement_set_locked", json!({"id": id, "locked": false}))["outcome"], "applied");
    assert!(app.world().resource::<Eyes>().is_empty() && app.world().resource::<Locks>().is_empty());
    let refused = apply(&mut app, "placement_set_locked", json!({"id": "no_such_piece", "locked": true}));
    assert_eq!(refused["outcome"], "refused", "an id that is not on the map is refused: {refused}");
}

/// **A locked piece is not removed by a click, and an operation removes it all the same.**
#[test]
fn a_locked_piece_is_not_removed_by_a_click_but_an_operation_removes_it() {
    let mut app = rig("lock_remove");
    apply(&mut app, "placement_set_locked", json!({"id": "on_floor@0", "locked": true}));
    step(&mut app, Frame { keys: vec![KeyCode::KeyX], ..default() });
    click(&mut app, (0.0, 0.0));
    assert!(ids(&app).contains(&"on_floor@0".to_owned()), "the click did not take the locked piece: {:?}", ids(&app));
    assert!(say(&app).contains("locked"), "and the status line says why: {}", say(&app));
    click(&mut app, (3.0, 0.0));
    assert!(!ids(&app).contains(&"on_floor@1".to_owned()), "an unlocked piece is still taken by the same tool");
    let said = apply(&mut app, "placement_remove", json!({"id": "on_floor@0"}));
    assert_eq!(said["outcome"], "applied", "an operation still removes a locked piece: {said}");
    assert!(!ids(&app).contains(&"on_floor@0".to_owned()));
}

/// **A locked piece is not picked up by the move click.**
#[test]
fn a_locked_piece_is_not_picked_up_by_a_click() {
    let mut app = rig("lock_move");
    apply(&mut app, "placement_set_locked", json!({"id": "on_floor@0", "locked": true}));
    step(&mut app, Frame { keys: vec![KeyCode::KeyB], ..default() });
    click(&mut app, (0.0, 0.0));
    assert!(app.world().resource::<MoveDrag>().held.is_none(), "nothing is in hand");
    assert!(say(&app).contains("locked"), "{}", say(&app));
    click(&mut app, (3.0, 0.0));
    assert!(app.world().resource::<MoveDrag>().held.is_some(), "the unlocked piece is picked up by the same tool");
}

/// **A hidden piece is not under the pointer**: the removal tool's click leaves it, and a locked or hidden mark does
/// not outlive the map.
#[test]
fn a_hidden_piece_is_not_picked_and_the_marks_do_not_outlive_the_map() {
    let mut app = rig("hidden_pick");
    apply(&mut app, "placement_set_hidden", json!({"id": "on_floor@1", "hidden": true}));
    step(&mut app, Frame { keys: vec![KeyCode::KeyX], ..default() });
    click(&mut app, (3.0, 0.0));
    assert!(ids(&app).contains(&"on_floor@1".to_owned()), "a click on what is not drawn takes nothing: {:?}", ids(&app));
    apply(&mut app, "placement_set_locked", json!({"id": "on_floor@0", "locked": true}));
    bevy_cauldron::screen::reset_door_state_for_test(app.world_mut());
    assert!(app.world().resource::<Eyes>().is_empty(), "a door shows everything");
    assert!(app.world().resource::<Locks>().is_empty(), "and unlocks everything");
}

/// **What a click would replace is asked of the lock and the eye**: a locked blocker and a hidden one are each refused,
/// by name, and an unmarked one is not. This is the question `place_one`'s callers (a click, a run-mode click, a drop) and
/// the box fill ask before they take anything out of the way.
#[test]
fn a_click_may_not_replace_a_locked_or_hidden_piece() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = rig("refuse_replacing");
    apply(&mut app, "placement_set_locked", json!({"id": "on_floor@0", "locked": true}));
    apply(&mut app, "placement_set_hidden", json!({"id": "on_floor@1", "hidden": true}));
    let ask = |app: &mut App, blockers: Vec<usize>| {
        app.world_mut()
            .run_system_once(move |sight: bevy_cauldron::outliner::Sight, project: Res<Project>, open: Res<OpenMap>| {
                sight.refuse_replacing(&project.library, &open.map, &blockers)
            })
            .expect("the system ran")
    };
    let locked = ask(&mut app, vec![0]).expect_err("a locked blocker is refused");
    assert!(locked.contains("on_floor@0") && locked.contains("locked"), "{locked}");
    let hidden = ask(&mut app, vec![1]).expect_err("a hidden blocker is refused");
    assert!(hidden.contains("on_floor@1") && hidden.contains("hidden"), "{hidden}");
    assert!(ask(&mut app, vec![2]).is_ok(), "an unmarked piece may be replaced");
    apply(&mut app, "layer_set_visible", json!({"layer": "ceiling", "visible": false}));
    assert!(ask(&mut app, vec![2]).is_err(), "so is one standing in a hidden layer");
}

/// **A key aimed at the cursor does not see a hidden piece**: the piece-verbs carry the target lock, which mirrors what
/// the eye has hidden, so `under_cursor_target` and the note and Edit Tile picks skip it. A hidden layer counts, and the
/// mirror empties again when the piece is shown.
#[test]
fn the_piece_verbs_do_not_see_what_the_eye_has_hidden() {
    let mut app = rig("verbs_unseen");
    let unseen = |app: &App, id: &str| app.world().resource::<bevy_cauldron::editor::TargetLock>().is_unseen(id);
    assert!(!unseen(&app, "on_floor@0"));
    apply(&mut app, "placement_set_hidden", json!({"id": "on_floor@0", "hidden": true}));
    assert!(unseen(&app, "on_floor@0"), "a hidden piece is not what a key means");
    apply(&mut app, "layer_set_visible", json!({"layer": "ceiling", "visible": false}));
    assert!(unseen(&app, "on_ceiling@2"), "nor is a piece in a hidden layer");
    apply(&mut app, "placement_set_hidden", json!({"id": "on_floor@0", "hidden": false}));
    assert!(!unseen(&app, "on_floor@0"), "and showing it again brings it back");
}

/// **A mark does not outlive its piece**: removing a hidden and locked piece drops its ids from the eye and the locks.
#[test]
fn a_removed_pieces_marks_are_pruned() {
    let mut app = rig("prune");
    apply(&mut app, "placement_set_hidden", json!({"id": "on_floor@1", "hidden": true}));
    apply(&mut app, "placement_set_locked", json!({"id": "on_floor@1", "locked": true}));
    apply(&mut app, "placement_remove", json!({"id": "on_floor@1"}));
    updates(&mut app, 3);
    assert!(app.world().resource::<Eyes>().is_empty(), "the eye let go of the removed piece");
    assert!(app.world().resource::<Locks>().is_empty(), "and so did the lock");
}
