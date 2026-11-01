//! **The enclosure wish belongs to the map, not to the editor's source** — E-005.
//!
//! `ENCLOSURE_WISH = 0.25` and `ENCLOSURE_WEIGHT = 200` were constants in `editor.rs`, so a
//! developer who wanted more enclosed rooms — a keep, a cellar, a maze — edited Rust and rebuilt.
//! The brief's rule is direct about this: *"If a value that someone would want to tweak is
//! hard-coded in Rust, that is a usability bug. Move it into data."*
//!
//! # What is being proved
//!
//! 1. **The operation can set it**, and the change reaches the map's own data rather than a
//!    resource beside it — so a generator reads it from the same place a file edit writes it.
//! 2. **It survives the save**, so it is not a value that exists only in memory.
//! 3. **A map written before the field existed still means the shipped pair**, because a default
//!    that changed the output of every existing map would be a silent re-generation.
//! 4. **A wish outside `0..=1` is refused by name**, so an agent that typed a percentage gets told.

use bevy::prelude::*;
use bevy_grimoire_core::map::{Enclosure, MAP_VERSION};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

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

fn open_map(app: &App) -> &OpenMap {
    app.world().resource::<OpenMap>()
}

/// **An agent sets the wish, and the map carries it.**
#[test]
fn the_enclosure_wish_is_set_through_the_operation_and_lands_on_the_map() {
    let mut app = editor("enclosure_set");
    assert_eq!(
        open_map(&app).map.enclosure,
        Enclosure::default(),
        "a fresh map starts at the shipped pair"
    );

    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"enclosure_wish": 0.75, "enclosure_weight": 500},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(applied["outcome"], "applied", "the apply did not apply: {applied}");

    assert_eq!(
        open_map(&app).map.enclosure,
        Enclosure { wish: 0.75, weight: 500 },
        "the wish an author asked for is the wish the map carries — a generator reads it from here"
    );
}

/// **And it is in the file**, so the two paths to the same state converge: what an agent sets live
/// is what a file edit would have written, and a reload reproduces it.
#[test]
fn the_wish_is_written_to_the_maps_file_and_reads_back() {
    let mut app = editor("enclosure_saved");
    call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"enclosure_wish": 0.6},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));

    let path = open_map(&app).map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    assert!(
        text.contains("enclosure"),
        "the map's file does not carry the wish, so it exists only in memory: {path:?}"
    );
    let reread: bevy_grimoire_core::map::Map =
        ron::from_str(&text).unwrap_or_else(|e| panic!("the file does not read back: {e}"));
    assert_eq!(
        reread.enclosure,
        Enclosure { wish: 0.6, weight: 200 },
        "the saved wish and its weight are what the author set, and the untouched field keeps its value"
    );
}

/// **A map written before the field existed still means the shipped pair.**
///
/// The schema's rule is that an older file is *read and re-saved here*. If the default were
/// anything but `0.25 / 200`, opening an old map would silently generate a different world.
#[test]
fn a_map_from_before_the_field_keeps_the_shipped_pair() {
    let old = r#"(
        version: 6,
        name: "old",
        origin: (0.0, 0.0, 0.0),
        bounds: (32.0, 8.0, 32.0),
        bash: None,
        placements: [],
        note: None,
    )"#;
    let map: bevy_grimoire_core::map::Map =
        ron::from_str(old).unwrap_or_else(|e| panic!("a v6 map must still read: {e}"));
    assert_eq!(
        map.enclosure,
        Enclosure { wish: 0.25, weight: 200 },
        "a map written before `enclosure` existed must generate exactly as it did (E-005)"
    );
    assert_eq!(MAP_VERSION, 9, "the field arrived with a version, so an older build refuses it");
}

/// **A percentage typed as a number is refused by name**, rather than quietly clamped.
#[test]
fn a_wish_that_is_not_a_fraction_is_refused_by_name() {
    let mut app = editor("enclosure_refused");
    // **A refusal is an answer, not an error**: the call succeeds and says `refused`, which is the
    // shape every operation has, so an agent reads one field rather than guessing from a transport
    // error whether its parameters were wrong or the editor was.
    let answered = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"enclosure_wish": 25.0},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers even when it refuses: {e}"));
    assert_eq!(
        answered["outcome"], "refused",
        "a wish of 25 is not a fraction and must be refused, got {answered}"
    );
    let message = answered["refusal"].as_str().unwrap_or_default();
    assert!(
        message.contains("0 to 1"),
        "the refusal says what the range is, so the caller can fix it: {message:?}"
    );
    assert_eq!(
        open_map(&app).map.enclosure,
        Enclosure::default(),
        "a refused change changes nothing"
    );
}

/// **The generator asks for what the map says, not for a constant.**
///
/// This is the behavioural half of E-005: setting the field is worthless if `generate` still solves
/// against `ENCLOSURE_WISH`. The check reads the same function `generate_from` calls, so the two
/// numbers a solve is given are the two numbers the map carries — and it fails on the old code,
/// where there was no map field to read and the wish was `0.25` whatever the author chose.
#[test]
fn the_generator_asks_for_what_the_map_says() {
    use bevy_cauldron::editor::enclosure_wishes;

    let mut map = bevy_grimoire_core::map::Map::default();
    let shipped = enclosure_wishes(&map);
    assert_eq!(shipped.enclosure, 0.25, "a fresh map asks for the shipped fraction");
    assert_eq!(shipped.enclosure_weight, Some(200), "and the shipped weight");

    map.enclosure = Enclosure { wish: 0.8, weight: 900 };
    let tuned = enclosure_wishes(&map);
    assert_eq!(
        tuned.enclosure, 0.8,
        "the generator's wish is the map's wish; a constant here is the bug E-005 describes"
    );
    assert_eq!(tuned.enclosure_weight, Some(900), "and its weight, from the same field");
}

/// **An agent can read back what it set**, so it checks the editor's answer rather than trusting its
/// own parameters. Without this the wish was a write-only field: `grimoire/state` did not report it,
/// and an agent had no way to tell a change that landed from one that was quietly dropped.
#[test]
fn the_state_dump_reports_the_wish_an_agent_set() {
    let mut app = editor("enclosure_readback");
    call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"enclosure_wish": 0.4, "enclosure_weight": 320},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));

    let state = call(&mut app, "grimoire/state", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("the state answers: {e}"));
    // **A fraction in `f32` is not exact in JSON**, so the comparison is to the precision the field
    // has rather than to the decimal the caller typed — `0.4f32` reads back as `0.4000000059604645`.
    let reported = state["map"]["enclosure_wish"]
        .as_f64()
        .unwrap_or_else(|| panic!("the wish is a number, got {:?}", state["map"]["enclosure_wish"]));
    assert!(
        (reported - 0.4).abs() < 1e-6,
        "the state dump reports the wish, so an agent reads the editor rather than its own request; \
         got {reported}"
    );
    assert_eq!(state["map"]["enclosure_weight"], 320, "and its weight");
}
