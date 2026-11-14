//! **The Tiles tab's page keys are operations, so `right`, `left`, `Enter`, `C`, `N` and an agent's call are one** (E-002, #129 slice 36).
//!
//! `PageEnter`, `PageLeave`, `TileOpen`, `CloseTile` and `BuildNew` leave the allowlist of unmigrated actions. The keys dispatch
//! `tile_page_enter`, `tile_page_leave`, `tile_open`, `tile_close` and `tile_new`; an agent calls the same operations through
//! `grimoire/operation_apply`, naming the tile where a key has a cursor and a prompt. Either way the tab's page, the tile in hand
//! and the name prompt are what move, and one line is logged per call.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::build::Build;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Tiles tab over a kit with a wall, a post, a slot token and one bounded tile (`bay`) holding a single post.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("tile_page_{tag}_{}", std::process::id()))
        .sized_descriptor("wall", "alpha", 1.0, 1.0)
        .sized_descriptor("post", "alpha", 0.2, 0.2)
        .slot_token("opening")
        .bounded_composition("bay", (1.0, 2.4, 1.0), &[("pillar", "post", (0.0, 0.0))])
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Tiles).unwrap_or_else(|e| panic!("{e}"));
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

fn tap(app: &mut App, key: KeyCode) {
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
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

fn build(app: &App) -> &Build {
    app.world().resource::<Build>()
}

fn open_id(app: &App) -> Option<String> {
    build(app).open.as_ref().map(|c| c.id.clone())
}

fn bay(app: &App) -> String {
    app.world().resource::<Project>().compositions.compositions[0].id.clone()
}

fn note(app: &App) -> String {
    app.world().resource::<ImportState>().status.note_text().to_owned()
}

/// **`right`, `Enter` and `C` and an agent open and close a tile through one operation each**: the tile in hand, the page the tab
/// shows and the sentence are the operation's own, and one line is logged per call.
#[test]
fn keys_and_an_agent_open_and_close_a_tile_through_operations() {
    let mut app = editor("open_close");
    let bay = bay(&app);
    assert_eq!(build(&app).browsing, Some(1), "the tab opens on the Tiles page, the cursor on the kit's first tile");
    assert!(open_id(&app).is_none());

    // `Enter` opens the tile under the cursor and lands on the Meshes page.
    tap(&mut app, KeyCode::Enter);
    assert_eq!(open_id(&app), Some(bay.clone()), "Enter opens the tile under the cursor");
    assert_eq!(build(&app).browsing, None, "and lands on the Meshes page");
    assert!(build(&app).placing, "a reopened tile is held: its arrows steer it");
    assert!(note(&app).contains("opened"), "the status line is the operation's own sentence: {}", note(&app));
    let lines = logged(&app, "tile_open");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    // `C` puts it down and goes back to the page, the cursor where it was.
    tap(&mut app, KeyCode::KeyC);
    assert_eq!(open_id(&app), None, "C puts the tile down");
    // The cursor stays where it was: the Enter that opened the tile left `browsing` unset, so it lands on `+ New Tile` (row 0), as it did.
    assert_eq!(build(&app).browsing, Some(0), "back on the Tiles page");
    let lines = logged(&app, "tile_close");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    // Nothing to close is guidance on the key: a note, no problem.
    tap(&mut app, KeyCode::KeyC);
    let state = app.world().resource::<ImportState>();
    assert!(!state.status.has_problem(), "nothing open to close is guidance: {:?}", state.status.problem_text());
    assert!(state.status.note_text().starts_with("nothing open to close"), "{}", state.status.note_text());

    // On `+ New Tile`, `right` only drills; walked down to the kit's tile, it opens it.
    tap(&mut app, KeyCode::ArrowRight);
    assert_eq!(open_id(&app), None, "right on + New Tile opens nothing");
    assert_eq!(build(&app).browsing, None, "and drills to the Meshes page");
    apply(&mut app, "tile_page_leave", json!({}));
    apply(&mut app, "tile_page_walk", json!({"by": "next"}));
    assert_eq!(build(&app).browsing, Some(1));
    // `right` opens the tile under the cursor too, and drills to the Meshes page.
    tap(&mut app, KeyCode::ArrowRight);
    assert_eq!(open_id(&app), Some(bay.clone()), "right opens the tile under the cursor");
    assert_eq!(build(&app).browsing, None);
    let lines = logged(&app, "tile_page_enter");
    assert_eq!(lines.len(), 2, "the drill on + New Tile and the open on the tile's row: {lines:?}");
    assert_eq!(lines[1].0, "human", "{:?}", lines[1]);
    assert_eq!(lines[1].1["after"]["opens"], true, "{:?}", lines[1]);

    // An agent closes it, names the tile to open it, and is refused a second close.
    let said = apply(&mut app, "tile_close", json!({}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(open_id(&app), None, "an agent puts the editor's own tile down");
    assert_eq!(build(&app).browsing, Some(0), "closed from the Meshes page, so back on the page at `+ New Tile`, as the key does");
    let refused = apply(&mut app, "tile_close", json!({}));
    assert_eq!(refused["outcome"], "refused", "no tile is open: {refused}");
    let said = apply(&mut app, "tile_open", json!({"id": bay}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(open_id(&app), Some(bay.clone()), "an agent opens the tile it names");
    assert_eq!(build(&app).browsing, None);
    let lines = logged(&app, "tile_open");
    assert_eq!(lines.len(), 2, "the agent's call is logged too: {lines:?}");
    assert_eq!(lines[1].0, "agent", "{:?}", lines[1]);
    let refused = apply(&mut app, "tile_open", json!({"id": "no_such_tile"}));
    assert_eq!(refused["outcome"], "refused", "a tile nobody has is refused: {refused}");

    // The page's own cursor, for an agent: close, then enter from the row it stands on (`+ New Tile` only drills).
    apply(&mut app, "tile_close", json!({}));
    apply(&mut app, "tile_page_walk", json!({"by": "next"}));
    let said = apply(&mut app, "tile_page_enter", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(open_id(&app), Some(bay), "an agent drills from the row the cursor is on");
    let refused = apply(&mut app, "tile_page_enter", json!({}));
    assert_eq!(refused["outcome"], "refused", "the Tiles page is not up: {refused}");
}

/// **`N` and an agent name a new tile through one operation**, and `left` and an agent go back to the page through another: the
/// prompt, the blank tile it opens and a name the kit already has.
#[test]
fn n_and_an_agent_name_a_new_tile_and_left_goes_back_through_operations() {
    let mut app = editor("new_leave");

    tap(&mut app, KeyCode::KeyN);
    assert!(build(&app).naming.is_some(), "N raises the name prompt");
    let lines = logged(&app, "tile_new");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert!(note(&app).starts_with("name the tile"), "{}", note(&app));

    // An agent names the tile outright, which also takes the prompt down.
    let said = apply(&mut app, "tile_new", json!({"name": "fresh_one"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    let id = open_id(&app).expect("the new tile is open");
    assert!(id.ends_with("/fresh_one"), "{id}");
    assert!(build(&app).open.as_ref().is_some_and(|c| c.members.is_empty()), "a blank tile");
    assert!(build(&app).naming.is_none(), "the prompt is gone");
    assert!(!build(&app).placing, "nothing is picked up in a blank tile");
    assert_eq!(build(&app).browsing, None, "it lands on the Meshes page");
    assert_eq!(logged(&app, "tile_new").len(), 2);

    // An empty name is refused.
    let refused = apply(&mut app, "tile_new", json!({"name": ""}));
    assert_eq!(refused["outcome"], "refused", "a tile needs a name: {refused}");

    // `left` goes back to the Tiles page: with the blank tile in hand it is a draft row, so the cursor lands on the first tile.
    tap(&mut app, KeyCode::ArrowLeft);
    assert_eq!(build(&app).browsing, Some(1), "left goes back to the Tiles page");
    let lines = logged(&app, "tile_page_leave");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    let refused = apply(&mut app, "tile_page_leave", json!({}));
    assert_eq!(refused["outcome"], "refused", "the Tiles page is already up: {refused}");

    // On `+ New Tile`, `Enter` raises the prompt and an agent that names no tile does too.
    let said = apply(&mut app, "tile_page_walk", json!({"by": "prev"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(build(&app).browsing, Some(0));
    tap(&mut app, KeyCode::Enter);
    assert!(build(&app).naming.is_some(), "Enter on + New Tile raises the name prompt");
    let lines = logged(&app, "tile_open");
    assert_eq!(lines.len(), 1, "{lines:?}");
}

/// **The actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_page_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::PageEnter.operation(), Some("tile_page_enter"));
    assert_eq!(Action::PageLeave.operation(), Some("tile_page_leave"));
    assert_eq!(Action::TileOpen.operation(), Some("tile_open"));
    assert_eq!(Action::CloseTile.operation(), Some("tile_close"));
    assert_eq!(Action::BuildNew.operation(), Some("tile_new"));
}

/// **A key as the name prompt reads it**: by its logical key, which is what `build::naming_keys` matches on.
fn press_logical(app: &mut App, logical_key: Key) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
        logical_key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press_logical(app, Key::Character(c.to_string().into()));
    }
}

/// **A name the kit already has is refused, for an agent and for the name prompt alike** (E-002, #129 slice 36 review): `tile_new`
/// says `` `{id}` is already in the kit `` rather than silently replacing somebody's tile, and the prompt stays up for the person to
/// pick another name. The sentence used to live in the key's own code and was deleted with it, so this pins it where it is now.
#[test]
fn a_name_the_kit_already_has_is_refused_and_the_prompt_stays_up() {
    let mut app = editor("collision");
    let id = format!("{}/taken", app.world().resource::<Project>().namespace);
    // A tile saved under that name: `bay`'s definition, renamed, through the operation `Cmd+S` dispatches.
    let tile = {
        let project = app.world().resource::<Project>();
        let bay = project.compositions.compositions.iter().find(|c| c.id == "bay").expect("the fixture's bay");
        let mut v = serde_json::to_value(bay).expect("a composition serialises");
        v["id"] = json!(id);
        v
    };
    let made = apply(&mut app, "composition_define", json!({"id": id, "composition": tile}));
    assert_eq!(made["outcome"], "applied", "the kit gains a tile named `{id}`: {made}");
    assert!(app.world().resource::<Project>().compositions.compositions.iter().any(|c| c.id == id), "and holds it");
    let before = logged(&app, "tile_new").len();

    // An agent that names it is refused, and nothing opens.
    let refused = apply(&mut app, "tile_new", json!({"name": "taken"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert!(refused.to_string().contains("is already in the kit"), "the refusal says why: {refused}");
    assert_eq!(open_id(&app), None, "nothing opened");
    assert_eq!(logged(&app, "tile_new").len(), before, "a refusal is not a logged change");

    // The person types the same name into the prompt: a problem, the prompt still up, nothing opened.
    tap(&mut app, KeyCode::KeyN);
    assert!(build(&app).naming.is_some(), "N raises the name prompt");
    type_text(&mut app, "taken");
    press_logical(&mut app, Key::Enter);
    assert!(build(&app).naming.is_some(), "the prompt stays up so the name can be fixed");
    assert_eq!(open_id(&app), None, "nothing opened");
    let state = app.world().resource::<ImportState>();
    assert!(state.status.has_problem(), "a refusal is a problem where the author looks");
    assert!(state.status.problem_text().contains("is already in the kit"), "{}", state.status.problem_text());
    assert_eq!(logged(&app, "tile_new").len(), before + 1, "only the press of N that raised the prompt was logged");

    // Another name goes through, and takes the prompt down.
    for _ in 0..5 {
        press_logical(&mut app, Key::Backspace);
    }
    type_text(&mut app, "fresh");
    press_logical(&mut app, Key::Enter);
    assert!(build(&app).naming.is_none(), "a name the kit lacks is taken");
    assert!(open_id(&app).is_some_and(|id| id.ends_with("/fresh")), "{:?}", open_id(&app));
}
