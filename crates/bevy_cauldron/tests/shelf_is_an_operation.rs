//! **The shelf's arrows and its two lists are operations, so a key, a chip and an agent's call are one** (E-002, #129 slice 37).
//!
//! `PrevCandidate`, `NextCandidate`, `TileListPrev`, `TileListNext`, `FocusCandidates` and `FocusLibrary` leave the allowlist of
//! unmigrated actions. Up and down walk the shelf's list (the Meshes tab's library or candidates, the Tiles tab's library) and left,
//! right and the two chips choose which list has the arrows. They dispatch `shelf_walk` and `shelf_focus`; an agent calls the same two
//! through `grimoire/operation_apply`, and the editor's own `ImportState` holds the answer either way. The Meshes shelf's selection
//! still leaves its `piece_select` line, under whoever made the walk.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::events::{Pointer, PointerClick};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode, Shelf, ShelfChip};
use serde_json::{Value, json};

use crate::fixtures;

/// An editor over a library of two pieces (`oldest`, then `newest`, which the shelf draws first) and two packs of three candidates.
fn editor(tag: &str, mode: Mode, library: bool) -> App {
    let fixture = fixtures::Fixture::new(&format!("shelf_{tag}_{}", std::process::id()));
    let fixture = if library { fixture.descriptor("oldest", "alpha").descriptor("newest", "alpha") } else { fixture };
    let root = fixture.pack("aaa", &["a1", "a2", "a3"]).pack("bbb", &["b1", "b2", "b3"]).build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn apply(app: &mut App, operation: &str, parameters: Value) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered");
    };
    app.world_mut()
        .run_system_with(id, Some(json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

/// **A key as a person presses it**, with `held` down around it.
fn tap(app: &mut App, held: &[KeyCode], key: KeyCode) {
    for h in held {
        send(app, *h, ButtonState::Pressed);
    }
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
    for h in held {
        send(app, *h, ButtonState::Released);
    }
    app.update();
    app.update();
}

fn logged(app: &App, operation: &str) -> Vec<(String, Value)> {
    let root = app.world().resource::<Project>().root.clone();
    let text = std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(&root)).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == operation)
        .map(|v| (v["source"].as_str().unwrap_or_default().to_owned(), v))
        .collect()
}

fn state(app: &App) -> &ImportState {
    app.world().resource::<ImportState>()
}

fn selected(app: &App) -> Option<String> {
    state(app).selected_library_id.clone()
}

/// **Where the candidate cursor stands**: the heading it is on, whether it is on the `EXCLUDED` band, and the mesh row.
fn cursor(app: &App) -> (Option<String>, bool, usize) {
    let s = state(app);
    (s.focused_pack.clone(), s.excluded_focused, s.selected)
}

fn down() -> KeyCode {
    binding(Action::NextCandidate).key
}

fn up() -> KeyCode {
    binding(Action::PrevCandidate).key
}

/// **`right` and the arrows walk the library shelf through the operations, and an agent walks it the same way**: the landing is the
/// operation's, the selection leaves its `piece_select` line, and each press leaves a line of its own under the name of whoever made it.
#[test]
fn right_and_the_arrows_and_an_agent_walk_the_library_shelf_through_operations() {
    let mut app = editor("library", Mode::Meshes, true);
    assert_eq!(selected(&app), None);

    tap(&mut app, &[], binding(Action::FocusLibrary).key);
    assert_eq!(selected(&app).as_deref(), Some("newest"), "right seeds the shelf on its first row, the newest piece");
    let focus = logged(&app, "shelf_focus");
    assert_eq!(focus.len(), 1, "one key, one logged operation: {focus:?}");
    assert_eq!(focus[0].0, "human", "{:?}", focus[0]);
    assert_eq!(focus[0].1["params"]["to"], "library", "{:?}", focus[0]);
    assert_eq!(logged(&app, "piece_select").len(), 1, "the seed is a selection and leaves its own line");

    tap(&mut app, &[], down());
    assert_eq!(selected(&app).as_deref(), Some("oldest"), "down steps along the shelf");
    let walk = logged(&app, "shelf_walk");
    assert_eq!(walk.len(), 1, "{walk:?}");
    assert_eq!(walk[0].0, "human", "{:?}", walk[0]);
    assert_eq!(walk[0].1["params"], json!({"by": "next", "rows": 1}), "{:?}", walk[0]);
    assert!(walk[0].1["result"].as_str().is_some_and(|r| r.starts_with("`oldest` selected")), "the line carries the operation's sentence: {:?}", walk[0]);
    let picks = logged(&app, "piece_select");
    assert_eq!(picks.len(), 2, "the walk's selection is a line of its own: {picks:?}");
    assert_eq!(picks[1].1["params"]["id"], "oldest", "{:?}", picks[1]);

    // An agent walks the same shelf: the editor's own selection moves, and both lines are the agent's.
    let said = apply(&mut app, "shelf_walk", json!({"by": "prev"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(selected(&app).as_deref(), Some("newest"), "an agent's step is the editor's own");
    let walk = logged(&app, "shelf_walk");
    assert_eq!(walk.len(), 2, "{walk:?}");
    assert_eq!(walk[1].0, "agent", "{:?}", walk[1]);
    let picks = logged(&app, "piece_select");
    assert_eq!(picks.len(), 3, "{picks:?}");
    assert_eq!(picks[2].0, "agent", "the selection a walk makes is under the walker's name: {:?}", picks[2]);

    // The shelf ends where it ends, and a stride longer than it clamps.
    apply(&mut app, "shelf_walk", json!({"by": "prev"}));
    assert_eq!(selected(&app).as_deref(), Some("newest"), "a step stops at the top");
    apply(&mut app, "shelf_walk", json!({"by": "next", "rows": 5}));
    assert_eq!(selected(&app).as_deref(), Some("oldest"), "a stride stops at the bottom");

    // What an agent is refused: a direction that is not one, a stride that is not one, a list that is not one.
    for (what, operation, parameters) in [
        ("a direction that is not one", "shelf_walk", json!({"by": "sideways"})),
        ("no stride at all", "shelf_walk", json!({"by": "next", "rows": 0})),
        ("a typo'd field", "shelf_walk", json!({"by": "next", "row": 2})),
        ("a list that is not one", "shelf_focus", json!({"to": "tiles"})),
        ("no list", "shelf_focus", json!({})),
    ] {
        let refused = apply(&mut app, operation, parameters);
        assert_eq!(refused["outcome"], "refused", "{what} was accepted: {refused}");
    }
    assert_eq!(selected(&app).as_deref(), Some("oldest"), "a refusal moves nothing");
}

/// **`left` hands the arrows to the candidates, and Shift makes the stride five rows**, for the key and for an agent: the cursor
/// moves along the drawn rows (headings included), and a held key's repeats are the same change again and write no second line.
#[test]
fn left_and_the_arrows_and_an_agent_walk_the_candidate_list_through_operations() {
    let mut app = editor("candidates", Mode::Meshes, true);
    tap(&mut app, &[], binding(Action::FocusLibrary).key);
    assert!(selected(&app).is_some());

    tap(&mut app, &[], binding(Action::FocusCandidates).key);
    assert_eq!(selected(&app), None, "left hands the arrows to the candidates");
    let focus = logged(&app, "shelf_focus");
    assert_eq!(focus.len(), 2, "{focus:?}");
    assert_eq!(focus[1].1["params"]["to"], "candidates", "{:?}", focus[1]);
    assert_eq!(focus[1].1["result"], "candidates", "{:?}", focus[1]);

    let c0 = cursor(&app);
    tap(&mut app, &[], down());
    let c1 = cursor(&app);
    assert_ne!(c1, c0, "down moves the candidate cursor");
    let walk = logged(&app, "shelf_walk");
    assert_eq!(walk.len(), 1, "one key, one logged operation: {walk:?}");
    assert_eq!(walk[0].0, "human", "{:?}", walk[0]);
    assert_eq!(logged(&app, "piece_select").len(), 1, "a candidate is not a library piece: only the seed was selected");

    let said = apply(&mut app, "shelf_walk", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    let c2 = cursor(&app);
    assert_ne!(c2, c1, "an agent's step moves the same cursor");
    apply(&mut app, "shelf_walk", json!({"by": "prev"}));
    assert_eq!(cursor(&app), c1, "and back");
    tap(&mut app, &[], up());
    let base = cursor(&app);

    // Shift is the long stride, five rows for the key as for an agent who asks for them.
    tap(&mut app, &[KeyCode::ShiftLeft], down());
    let long = cursor(&app);
    apply(&mut app, "shelf_walk", json!({"by": "prev", "rows": 5}));
    assert_ne!(cursor(&app), long, "an agent's five rows back moves the cursor the key's long stride moved");
    assert_ne!(long, base, "the long stride went further than one row");
    let last = logged(&app, "shelf_walk").into_iter().rev().find(|(who, _)| who == "human").expect("a human line");
    assert_eq!(last.1["params"], json!({"by": "next", "rows": 5}), "{:?}", last);
}

/// **A held key walks every repeat and writes one line**: the press is the change and the repeats are it continuing.
#[test]
fn a_held_arrow_walks_the_candidates_and_logs_one_line() {
    let mut app = editor("held", Mode::Meshes, true);
    let start = cursor(&app);
    send(&mut app, down(), ButtonState::Pressed);
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(30));
        app.update();
    }
    send(&mut app, down(), ButtonState::Released);
    app.update();
    app.update();
    assert_ne!(cursor(&app), start, "a held arrow walks");
    let walk = logged(&app, "shelf_walk");
    assert_eq!(walk.len(), 1, "one hold, one logged line however many rows it walked: {walk:?}");
    assert_eq!(walk[0].0, "human", "{:?}", walk[0]);
}

/// **The two chips are the same press as the keys**: a click on `MESHES` leaves the `shelf_focus` line the key leaves, and a click on
/// `CANDIDATES` hands the arrows back.
#[test]
fn the_shelf_chips_focus_through_the_operation() {
    let mut app = editor("chips", Mode::Meshes, true);
    let click = |app: &mut App, shelf: Shelf| {
        let chip = {
            let world = app.world_mut();
            let mut chips = world.query::<(Entity, &ShelfChip)>();
            chips.iter(world).find(|(_, c)| c.0 == shelf).map(|(entity, _)| entity).expect("the shelf strip draws the chip")
        };
        use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerButton, PointerId};
        app.world_mut().trigger(PointerClick {
            entity: chip,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: Default::default(), scale_factor: 1.0 }),
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            hit: HitData { camera: Entity::PLACEHOLDER, depth: 0.0, position: None, normal: None, extra: None },
            duration: std::time::Duration::ZERO,
            count: 1,
        });
        app.update();
        app.update();
    };
    click(&mut app, Shelf::Library);
    assert_eq!(selected(&app).as_deref(), Some("newest"), "the chip lands where the key lands");
    click(&mut app, Shelf::Candidates);
    assert_eq!(selected(&app), None, "the candidates chip hands the arrows back");
    let focus = logged(&app, "shelf_focus");
    assert_eq!(focus.len(), 2, "a click is the key's press: {focus:?}");
    assert_eq!(focus[0].1["params"]["to"], "library", "{:?}", focus[0]);
    assert_eq!(focus[1].1["params"]["to"], "candidates", "{:?}", focus[1]);
    assert_eq!(focus[0].0, "human", "{:?}", focus[0]);
}

/// **The Tiles tab walks its one list**: the first step seeds the first row and stops there, a tile's source is a direct write that no
/// `piece_select` line is made for, and an empty library is refused with the sentence the author reads. A tab with no shelf refuses.
#[test]
fn the_tiles_tab_walks_its_one_list_through_the_operation() {
    let mut app = editor("tiles", Mode::Tiles, true);
    let said = apply(&mut app, "shelf_walk", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(selected(&app).as_deref(), Some("newest"), "the step that seeds the selection lands on the first row and stops there");
    apply(&mut app, "shelf_walk", json!({"by": "next"}));
    assert_eq!(selected(&app).as_deref(), Some("oldest"), "the next step moves");
    apply(&mut app, "shelf_walk", json!({"by": "prev"}));
    assert_eq!(selected(&app).as_deref(), Some("newest"));
    let walk = logged(&app, "shelf_walk");
    assert_eq!(walk.len(), 3, "{walk:?}");
    assert_eq!(walk[0].0, "agent", "{:?}", walk[0]);
    assert!(logged(&app, "piece_select").is_empty(), "a tile's source is not a Meshes selection, so no `piece_select` line is made for it");

    let mut empty = editor("tiles_empty", Mode::Tiles, false);
    let refused = apply(&mut empty, "shelf_walk", json!({"by": "next"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert!(refused.to_string().contains("the library is empty"), "{refused}");
    assert!(logged(&empty, "shelf_walk").is_empty(), "a refused walk is not a logged change");

    let mut map = editor("map_tab", Mode::Map, true);
    let refused = apply(&mut map, "shelf_walk", json!({"by": "next"}));
    assert_eq!(refused["outcome"], "refused", "no shelf is up on the Map tab: {refused}");
    assert!(refused.to_string().contains("no shelf is up"), "{refused}");
    let refused = apply(&mut map, "shelf_focus", json!({"to": "library"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
}

/// **A library with nothing to land on says so**, for `right` on the Meshes tab: a note, and nothing selected.
#[test]
fn right_on_an_empty_library_says_so_and_selects_nothing() {
    let mut app = editor("empty", Mode::Meshes, false);
    tap(&mut app, &[], binding(Action::FocusLibrary).key);
    assert_eq!(selected(&app), None);
    let focus = logged(&app, "shelf_focus");
    assert_eq!(focus.len(), 1, "{focus:?}");
    assert_eq!(focus[0].1["result"], bevy_cauldron::ops::shelf_cursors::shelf::LIBRARY_EMPTY_NOTE, "{:?}", focus[0]);
    assert!(!state(&app).status.has_problem(), "guidance, not a problem");
    assert!(logged(&app, "piece_select").is_empty());
}

/// **The actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_shelf_actions_name_the_operation_they_dispatch() {
    for action in [Action::PrevCandidate, Action::NextCandidate, Action::TileListPrev, Action::TileListNext] {
        assert_eq!(action.operation(), Some("shelf_walk"), "{action:?}");
    }
    assert_eq!(Action::FocusCandidates.operation(), Some("shelf_focus"));
    assert_eq!(Action::FocusLibrary.operation(), Some("shelf_focus"));
}
