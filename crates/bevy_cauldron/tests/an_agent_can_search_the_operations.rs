//! **An agent finds an operation by searching for it, not by reading all of them** (#141).
//!
//! The registry is past sixty operations, and `grimoire/operation_list` answered every one with its
//! whole schema whatever the caller asked. With `query`, it answers the operations whose name or
//! parameters match, best first, and the registry's `total`; without one, every operation, as
//! before.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

fn list(app: &mut App, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get("grimoire/operation_list") {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err("grimoire/operation_list is not registered".into()),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{e:?}"))?
        .map_err(|e| format!("{e:?}"))
}

fn names(answer: &serde_json::Value) -> Vec<String> {
    answer["operations"]
        .as_array()
        .expect("the answer's operations are an array")
        .iter()
        .map(|o| o["operation"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// **A query answers only what matches, the name's match first, and says how many there are.**
#[test]
fn a_query_finds_the_operation_and_leaves_the_rest_out() {
    let mut app = editor("opsearch");
    let all = names(&list(&mut app, serde_json::json!({})).unwrap_or_else(|e| panic!("{e}")));

    // A word of the operation's name: `volume` is `volume_edit`'s, and it ranks first.
    let answer = list(&mut app, serde_json::json!({"query": "volume"})).unwrap_or_else(|e| panic!("{e}"));
    let found = names(&answer);
    assert_eq!(
        found.first().map(String::as_str),
        Some("volume_edit"),
        "searching `volume` must put volume_edit first: {found:?}"
    );
    assert!(
        found.len() < all.len() && !found.iter().any(|n| n == "map_save"),
        "a query must leave out what does not match, and map_save says nothing of volumes: {found:?}"
    );
    assert_eq!(answer["total"].as_u64(), Some(all.len() as u64), "total is the registry's size: {answer}");
    assert_eq!(answer["query"], "volume");

    // A word only the parameters say: `carve` is one of volume_edit's modes, and `carving` finds it.
    let found = names(&list(&mut app, serde_json::json!({"query": "carving"})).unwrap_or_else(|e| panic!("{e}")));
    assert!(found.iter().any(|n| n == "volume_edit"), "`carving` must find volume_edit by its Carve mode: {found:?}");
    assert!(!found.iter().any(|n| n == "map_save"), "{found:?}");

    // JSON Schema's own words are not the operation's: `object` would match every one of them.
    let found = names(&list(&mut app, serde_json::json!({"query": "additionalProperties"})).unwrap_or_else(|e| panic!("{e}")));
    assert!(found.len() < all.len(), "schema keywords must not match every operation: {}", found.len());
}

/// **No query, a blank one, or one that matches nothing**: every operation, every operation, and an
/// empty list with the total, never an error. A query that is not text is refused.
#[test]
fn no_query_lists_everything_and_a_miss_is_empty() {
    let mut app = editor("opsearchnone");
    let all = names(&list(&mut app, serde_json::json!({})).unwrap_or_else(|e| panic!("{e}")));
    assert!(all.len() > 30, "the registry is the whole list: {}", all.len());
    let blank = names(&list(&mut app, serde_json::json!({"query": "  "})).unwrap_or_else(|e| panic!("{e}")));
    assert_eq!(blank, all, "a blank query lists every operation");

    let miss = list(&mut app, serde_json::json!({"query": "zzqx"})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(names(&miss), Vec::<String>::new(), "a miss is an empty list: {miss}");
    assert_eq!(miss["total"].as_u64(), Some(all.len() as u64));

    let refused = list(&mut app, serde_json::json!({"query": 3}));
    assert!(refused.is_err(), "a query that is not text is refused: {refused:?}");
}
