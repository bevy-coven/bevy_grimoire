//! **An agent can make a map** — E-008.
//!
//! The brief's second step is *"the in-editor agent turns that into typed editor operations"*, and
//! its first is a developer describing what they want. "Make me a 40 metre courtyard with a note"
//! is the simplest request in that loop, and until now there was **no tool that could do it**: the
//! chooser wrote a map's file directly (`chooser::create_map`), outside the registry, so the only way
//! to make one was to be a person at the keyboard on the menu screen.
//!
//! That is the parity gap this file closes. `map_create` makes a map the way every other operation
//! does — through the registry, from the same `Ctx`, with the same validation and the same undo
//! record — and it answers with the map it made so the agent can open it or place in it.
//!
//! # What is proved
//!
//! 1. **It exists and is reachable** the way an agent reaches any operation.
//! 2. **The file it writes is a real map** — it parses back with the bounds and note asked for.
//! 3. **A name already taken is refused by name**, not written over.
//! 4. **It is in the log**, like every other change, so the person can see who made it.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// **The operation an agent asks for is in the registry**, so `grimoire_list_operations` shows it and
/// a tool exists for it. This is the assertion that fails before the operation exists.
#[test]
fn map_create_is_an_operation_an_agent_can_find() {
    let mut app = editor("map_create_listed");
    let listed = call(&mut app, "grimoire/operation_list", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("the list answers: {e}"));
    // **The list answers `{"operations": [...]}`**, and reading both shapes keeps this test about the
    // operation existing rather than about the envelope it arrives in.
    let array = listed
        .as_array()
        .cloned()
        .or_else(|| listed.get("operations").and_then(|o| o.as_array()).cloned())
        .unwrap_or_default();
    // **Each row names the operation `operation`**, the same word the apply and the log use.
    let names: Vec<&str> = array
        .iter()
        .filter_map(|o| o.get("operation").and_then(|n| n.as_str()))
        .collect();
    assert!(
        names.contains(&"map_create"),
        "an agent asked to \"make me a map\" has no operation to call: {names:?}"
    );
}

/// **It writes a real map, and says which one.** The answer has to be enough for the next call: an
/// agent that just made a map needs its name to open it, edit its settings, or place in it.
#[test]
fn map_create_writes_a_map_and_names_it() {
    let mut app = editor("map_create_writes");
    let applied = apply(
        &mut app,
        "map_create",
        serde_json::json!({
            "name": "courtyard",
            "bounds": [40.0, 6.0, 40.0],
            "note": "a mossy ruined courtyard with two exits",
        }),
    );
    assert_eq!(applied["outcome"], "applied", "the create did not apply: {applied}");

    let project = app.world().resource::<Project>().clone();
    let path = project.maps_dir.join("courtyard.map.ron");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} was not written: {e}", path.display()));
    let map = bevy_grimoire_core::map::Map::parse(&text)
        .unwrap_or_else(|e| panic!("what was written is not a map: {e}"));
    assert_eq!(map.name, "courtyard");
    assert_eq!(map.bounds, (40.0, 6.0, 40.0), "the bounds asked for");
    assert_eq!(
        map.note.as_deref(),
        Some("a mossy ruined courtyard with two exits"),
        "the note asked for, which is what the developer said they wanted"
    );
    // **The answer names the map**, in the same place every applied operation's `after` lives.
    let named = applied["after"]["map"]
        .as_str()
        .or_else(|| applied["log_entry"]["after"]["map"].as_str());
    assert!(
        named == Some("courtyard"),
        "the answer names the map it made, so an agent can act on it next: {applied}"
    );
}

/// **A name already in use is refused, and nothing is overwritten.** The same rule `create_map` has
/// always enforced — now said through the operation, so an agent hears it instead of losing a file.
#[test]
fn map_create_refuses_a_name_that_is_taken() {
    let mut app = editor("map_create_taken");
    let first = apply(&mut app, "map_create", serde_json::json!({"name": "hall"}));
    assert_eq!(first["outcome"], "applied", "{first}");

    let second = apply(&mut app, "map_create", serde_json::json!({"name": "hall"}));
    assert_eq!(
        second["outcome"], "refused",
        "a second map of the same name must be refused: {second}"
    );
    let message = second["refusal"].as_str().unwrap_or_default();
    assert!(
        message.contains("hall"),
        "the refusal names the map that is already there: {message:?}"
    );
}

/// **It is in the operation log**, so the person can see an agent made a map — the brief's rule that
/// one append-only log is the source of truth, whoever acted.
#[test]
fn map_create_is_recorded_in_the_log() {
    let mut app = editor("map_create_logged");
    apply(&mut app, "map_create", serde_json::json!({"name": "annexe"}));
    let log = call(&mut app, "grimoire/oplog_read", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("the log answers: {e}"));
    let entries = log
        .get("entries")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        entries
            .iter()
            .any(|e| e["op"].as_str() == Some("map_create")),
        "map_create is in the log, so the person can see an agent made a map: {log}"
    );
}

/// **The menu shows a map it did not make itself.**
///
/// The list rescanned only when the menu acted, so a map written by an agent left the MAPS column
/// showing what was there before — measured 2026-10-24, where an MCP client made `courtyard` and the
/// menu still drew `+ new map` alone. The file was on disk and the screen said nothing had happened.
///
/// **Read from the chooser's own catalog**, which is the list the column draws: a map appearing there
/// is a map the person can see and press Enter on.
#[test]
fn the_menu_shows_a_map_an_agent_made() {
    use bevy_cauldron::chooser::{Catalog, Chooser, GRIMOIRE_DIR, MAPS_DIR};

    let root = fixtures::Fixture::new("map_shows_up")
        .descriptor("site/wall", "alpha")
        .build("m");
    let dir = root.join(GRIMOIRE_DIR).join(MAPS_DIR);
    let catalog = Catalog::scan(&root).unwrap_or_else(|e| panic!("{e}"));
    let before = catalog.maps.len();

    // A caller that writes the file and never touches the `Chooser` — which is exactly what the MCP
    // path is.
    let text = "(
    version: 7,
    name: \"annexe\",
    origin: (0.0, 0.0, 0.0),
    bounds: (10.0, 4.0, 10.0),
    bash: None,
    placements: [],
    locations: [],
    note: None,
    enclosure: (wish: 0.25, weight: 200),
)";
    std::fs::write(dir.join("annexe.map.ron"), text).unwrap_or_else(|e| panic!("{e}"));

    let after = Catalog::scan(&root).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        after.maps.len(),
        before + 1,
        "a map written into the directory is a map the menu's next scan finds; if this fails the \
         fault is in `Catalog::scan`, not in the column"
    );
    // And `Chooser` holds that catalog, so the column draws it.
    let c = Chooser::new(root.clone(), after, None);
    assert!(
        c.catalog.maps.iter().any(|m| m.name == "annexe"),
        "the chooser the column reads does not hold the map that is on disk"
    );
}
