//! **An agent can enter the map it made** — E-078.
//!
//! `map_create` then `map_open` both apply from the menu, and the window stays on the menu: the
//! operation changes the world, the screen is not its business, and a person who asked an agent to
//! make a map and work in it had to relaunch the editor on top of the work the agent had already
//! done. `tools/loop/accept.py` did exactly that relaunch, and recorded it as a workaround — so the
//! loop's headline metric was measuring the script.
//!
//! # What is proved
//!
//! 1. **`map_open` on the menu enters the editor on that map**, through the same `Chosen` ->
//!    `open_the_door` path the menu's Enter key takes — so it is not a second way into the editor.
//! 2. **The map it entered is the one it named**, in the world *and* in the project's memory of it.
//! 3. **A refused `map_open` leaves the person on the menu**, with nothing half-installed: no
//!    `Chosen` waiting to be consumed by a crossing that will never happen, and no `Project` for a
//!    screen that has no door.
//!
//! # Why the screen is the thing asserted
//!
//! The gap was never "the operation does not know the map" — it did, and `grimoire/state` reported a
//! map on the menu afterwards. The gap was that **the screen did not move**, so `State<Screen>` is
//! the assertion, and the resources are checked beside it to prove the editor arrived with a door.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::screen::Screen;
use bevy_cauldron::tiles::Door;

use crate::fixtures;

/// **The menu, on a project that has a map to make.** The harness opens straight into a door, so the
/// screen is put back to the menu itself — the same construction `tests/headless.rs` uses for the
/// menu's own tests.
fn on_the_menu(tag: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.add_plugins(bevy_cauldron::chooser::ChooserPlugin {
        root: root.clone(),
        preselect: None,
    });
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Menu);
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Menu,
        "the menu is up"
    );
    (root, app)
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

/// **Making a map on the menu then opening it is entering it.**
///
/// The failing-before symptom is precise: before the fix the map is open (`grimoire/state` says so)
/// and the screen is still `Menu`, so the window shows the MAPS column while every resource behind
/// it describes a courtyard.
#[test]
fn map_open_on_the_menu_enters_the_editor_on_that_map() {
    let (_root, mut app) = on_the_menu("agent-enters");

    let made = apply(
        &mut app,
        "map_create",
        serde_json::json!({"name": "courtyard", "bounds": [12, 4, 12], "note": "a mossy courtyard"}),
    );
    assert_eq!(made["outcome"], "applied", "the map is made: {made}");

    let opened = apply(&mut app, "map_open", serde_json::json!({"name": "courtyard"}));
    assert_eq!(opened["outcome"], "applied", "the map is opened: {opened}");

    // **The crossing is the screen's, and it happens on the next state transition.** `Chosen` is
    // consumed by `open_the_door` on `OnExit(Menu)`, so a single update is not enough.
    for _ in 0..5 {
        app.update();
    }

    let world = app.world();
    assert_eq!(
        *world.resource::<State<Screen>>().get(),
        Screen::Editor,
        "opening a map from the menu enters the editor"
    );
    assert_eq!(
        world.resource::<bevy_cauldron::project::OpenMap>().map.name,
        "courtyard",
        "and the map it entered is the one it named"
    );
    assert_eq!(
        *world.resource::<Door>(),
        Door::Map,
        "it arrived through the Maps door"
    );
    assert_eq!(
        world.resource::<Project>().open_map_name.as_deref(),
        Some("courtyard"),
        "and the project remembers the map, so a crossing back to Maps reopens it"
    );
    assert!(
        world.get_resource::<bevy_cauldron::chooser::Chosen>().is_none(),
        "the crossing consumed the door it entered"
    );
}

/// **`grimoire/state` says which screen, because the map cannot.**
///
/// This is the fact the acceptance script waits on, and it is asserted here because it is the half
/// that would have hidden the bug: on the pre-fix build `map_open` applied on the menu installed
/// `Project` and `OpenMap` and left the screen at `Menu`, so `grimoire/state` reported
/// `map.name == "courtyard"` while the person was looking at the MAPS column. **A predicate on the
/// map alone passed on the defective build**, so `screen` is what the script reads, and this pins
/// that the editor's own answer distinguishes the two.
#[test]
fn grimoire_state_says_which_screen_and_the_map_alone_does_not() {
    let (_root, mut app) = on_the_menu("agent-screen");

    let before = call(&mut app, "grimoire/state", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("grimoire/state answers: {e}"));
    assert_eq!(before["screen"], "menu", "the menu is the menu: {before}");
    assert!(before["map"].is_null(), "and holds no map yet: {before}");

    apply(
        &mut app,
        "map_create",
        serde_json::json!({"name": "courtyard", "bounds": [12, 4, 12]}),
    );
    apply(&mut app, "map_open", serde_json::json!({"name": "courtyard"}));
    for _ in 0..5 {
        app.update();
    }

    let after = call(&mut app, "grimoire/state", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("grimoire/state answers: {e}"));
    assert_eq!(after["screen"], "editor", "the entry is visible here: {after}");
    assert_eq!(after["map"]["name"], "courtyard", "and so is the map: {after}");
}

/// **A `map_open` that is refused does not move the screen, and leaves nothing behind.**
///
/// The refusal is the easy half. The half worth pinning is the world: `Chosen` is what makes a
/// crossing possible, so a refused open that left one behind would enter an editor on the *next*
/// state change for any reason at all — and a `Project` on a screen with no door is what
/// `OnEnter(Editor)` panics on.
#[test]
fn a_refused_map_open_on_the_menu_stays_on_the_menu() {
    let (_root, mut app) = on_the_menu("agent-refused");

    let refused = apply(&mut app, "map_open", serde_json::json!({"name": "no_such_map"}));
    assert_eq!(refused["outcome"], "refused", "there is no such map: {refused}");

    for _ in 0..5 {
        app.update();
    }

    let world = app.world();
    assert_eq!(
        *world.resource::<State<Screen>>().get(),
        Screen::Menu,
        "a refusal is not a crossing"
    );
    assert!(
        world.get_resource::<bevy_cauldron::chooser::Chosen>().is_none(),
        "and it leaves no door waiting to be entered"
    );
    assert!(
        world.get_resource::<Project>().is_none(),
        "nor a project for a screen that has no door"
    );
}
