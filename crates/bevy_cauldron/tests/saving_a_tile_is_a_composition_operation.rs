//! **Saving a tile is the operation an agent calls (E-002, #129 slice 15).**
//!
//! `Cmd+S` on the Tiles tab wrote the tile in hand to `compositions.ron` with a write of its own
//! (`build::save`): no log line, no undo, and no way for an agent to author a tile with slots, nested
//! groups and an envelope of its own, because `composition_define` could only make a group out of pieces.
//! `composition_define` now takes `composition`, a whole definition as `compositions.ron` holds it, and
//! the tab's save dispatches it. The first two tests go through the registered BRP methods, as an
//! agent's request goes; the last presses the real keys, as a person does.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy_cauldron::build::Build;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("wall", "alpha", 1.0, 1.0)
        .bounded_composition("bay", (1.0, 2.4, 1.0), &[("north", "wall", (0.0, 0.0))])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Tiles).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn define(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "composition_define", "parameters": parameters, "dry_run": false}),
    )
}

fn comps(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("assets/grimoire/compositions.ron")).unwrap_or_default()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// The shipped `bay` as JSON, renamed: a whole definition an agent could have read from the file.
fn whole(app: &App, id: &str) -> serde_json::Value {
    let project = app.world().resource::<Project>();
    let bay = project.compositions.compositions.iter().find(|c| c.id == "bay").expect("the fixture's bay");
    let mut v = serde_json::to_value(bay).expect("a composition serialises");
    v["id"] = serde_json::json!(id);
    v["note"] = serde_json::json!("a tile an agent wrote whole");
    v
}

/// **An agent writes a whole tile definition, with no map open, and one undo takes it out.**
#[test]
fn an_agent_writes_a_whole_tile_and_one_undo_takes_it_back() {
    let (root, mut app) = editor("tile_agent");
    let lines = log(&app).len();
    let tile = whole(&app, "bay_two");

    let made = define(&mut app, serde_json::json!({"id": "bay_two", "composition": tile}));
    assert_eq!(made["outcome"], "applied", "{made}");
    let after = comps(&root);
    assert!(after.contains("id: \"bay_two\""), "the tile is written: {after}");
    assert!(after.contains("a tile an agent wrote whole"), "the note rides along: {after}");
    assert_eq!(log(&app).len(), lines + 1, "one tile is one log line");

    let id = made["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert!(!comps(&root).contains("bay_two"), "one undo takes the tile out: {}", comps(&root));
    assert!(comps(&root).contains("id: \"bay\""), "and leaves the others");

    // And redefining one that exists asks for `replace`, and its undo puts the old body back.
    let tile = whole(&app, "bay");
    let refused = define(&mut app, serde_json::json!({"id": "bay", "composition": tile.clone()}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let before = comps(&root);
    let replaced = define(&mut app, serde_json::json!({"id": "bay", "composition": tile, "replace": true}));
    assert_eq!(replaced["outcome"], "applied", "{replaced}");
    assert!(comps(&root).contains("a tile an agent wrote whole"));
    let id = replaced["operation_id"].as_str().expect("id").to_owned();
    call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(comps(&root), before, "undo of a redefinition puts the file back byte for byte");
}

/// **A definition that cannot be a tile is refused whole and writes nothing.**
#[test]
fn a_tile_that_does_not_validate_is_refused_and_nothing_is_written() {
    let (root, mut app) = editor("tile_refused");
    let before = comps(&root);
    let lines = log(&app).len();
    let good = whole(&app, "bay_two");
    let mut no_members = good.clone();
    no_members["members"] = serde_json::json!([]);
    let nests_nothing = {
        let mut c: bevy_grimoire_core::composition::Composition =
            serde_json::from_value(good.clone()).expect("a definition reads back");
        c.members[0].body = bevy_grimoire_core::composition::Body::Composition { id: "no_such_group".to_owned() };
        serde_json::to_value(c).expect("serialises")
    };
    for (what, params) in [
        ("a name that is not the definition's", serde_json::json!({"id": "other", "composition": good.clone()})),
        ("a definition with no members", serde_json::json!({"id": "bay_two", "composition": no_members})),
        ("a group that nests one that does not exist", serde_json::json!({"id": "bay_two", "composition": nests_nothing})),
        ("a definition and placements", serde_json::json!({"id": "bay_two", "composition": good.clone(), "from": ["a"]})),
        ("a definition and a set", serde_json::json!({"id": "bay_two", "composition": good.clone(), "set": {"pieces": []}})),
        ("a typo'd field", serde_json::json!({"id": "bay_two", "composition": {"id": "bay_two", "members": [], "envelop": 1}})),
    ] {
        let r = define(&mut app, params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(comps(&root), before, "a refusal wrote compositions.ron");
    assert_eq!(log(&app).len(), lines, "a refusal logged a line");
}

fn press_with_mod(app: &mut App, key: KeyCode) {
    let modifier = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(modifier);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(modifier);
    app.update();
}

/// **A person's Cmd+S on the Tiles tab is one `composition_define` line stamped as a person's**, writes
/// the tile in hand, and one undo takes it back; saving it again replaces it as the tab always did.
#[test]
fn a_persons_cmd_s_on_the_tiles_tab_saves_through_the_operation() {
    let (root, mut app) = editor("tile_person");
    let lines = log(&app).len();
    let mut tile: bevy_grimoire_core::composition::Composition =
        serde_json::from_value(whole(&app, "hall")).expect("a definition reads back");
    tile.note = Some("a tile a person built".to_owned());
    app.world_mut().resource_mut::<Build>().open = Some(tile);
    app.update();

    press_with_mod(&mut app, bevy_cauldron::keys::binding(bevy_cauldron::keys::Action::Save).key);
    assert!(comps(&root).contains("id: \"hall\""), "Cmd+S did not reach compositions.ron: {}", comps(&root));
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "one save is one log line");
    let last = entries.last().expect("a line");
    assert_eq!(last.op, "composition_define");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's tile must be stamped as a person's");

    // The tab goes on: the same tile saved again replaces it, as it always has.
    app.world_mut().resource_mut::<Build>().open.as_mut().expect("still open").note = Some("edited".to_owned());
    press_with_mod(&mut app, bevy_cauldron::keys::binding(bevy_cauldron::keys::Action::Save).key);
    assert!(comps(&root).contains("edited"), "the second save did not replace the tile");
    assert_eq!(log(&app).len(), lines + 2, "the second save is its own line");

    let id = log(&app).last().expect("line").operation_id.clone();
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert!(!comps(&root).contains("edited"), "one undo takes the second save back");
    assert!(comps(&root).contains("a tile a person built"), "and leaves the first");
}
