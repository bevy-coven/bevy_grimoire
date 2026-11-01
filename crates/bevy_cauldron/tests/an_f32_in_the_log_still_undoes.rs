//! **An `f32` an operation logs reads back from the log's text as the same number, so its undo is
//! not refused as "changed outside the operation log"** (#196).
//!
//! The log holds an operation's `after` as JSON text, and undo compares it, read back, with the map
//! as it is now. An `f32` widened to `f64` prints with up to 17 digits, and `serde_json`'s default
//! parser can read such a number back one `f64` step off, so the comparison saw an edit nobody made.
//! About one 0.1 step in seven of a yaw is such a number (`3.9`, `9.1`, `9.4`, `10.4`), and so are
//! `0.03` and `0.11`. `placement_set_lift` had a private workaround for its field alone; the cause is
//! the parser, so these go through two other operations and two other fields, all by the registered
//! BRP methods an agent's request takes.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
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

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, applied: &serde_json::Value) -> serde_json::Value {
    let id = applied["operation_id"].as_str().expect("an apply returns its id").to_owned();
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn yaw_of(app: &App) -> f32 {
    app.world().resource::<OpenMap>().map.placements[0].yaw
}

/// **A turn to a yaw the text cannot carry is still one undo.** Each of these is an `f32` whose
/// widened `f64` does not survive `serde_json`'s default text round trip (found by scanning 0.1
/// steps), so a pass here is the parser's doing and not the value's.
#[test]
fn a_yaw_an_f32_spells_in_many_digits_still_undoes() {
    let mut app = editor("yaw-digits");
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    for yaw in [3.9_f32, 9.1, 9.4, 10.4, 11.6] {
        let turned = apply(&mut app, "placement_set_yaw", serde_json::json!({"id": id, "yaw": yaw}));
        assert_eq!(turned["outcome"], "applied", "a turn to {yaw} was not applied: {turned}");
        assert_eq!(yaw_of(&app), yaw, "the map does not hold {yaw}");

        let back = undo(&mut app, &turned);
        assert_eq!(back["outcome"], "applied", "undo of a turn to {yaw} was refused: {back}");
        assert_eq!(yaw_of(&app), 0.0, "undo of {yaw} left the piece turned");
    }
}

/// **The same for the map's own settings**, whose `after` holds the origin's three `f32`s and the
/// enclosure wish: another operation, three more fields, and none of them is a yaw.
#[test]
fn a_settings_change_an_f32_spells_in_many_digits_still_undoes() {
    let mut app = editor("settings-digits");
    let held = |app: &App| {
        let m = &app.world().resource::<OpenMap>().map;
        (m.origin, m.enclosure.wish)
    };
    let before = held(&app);

    let changed = apply(
        &mut app,
        "map_set_settings",
        serde_json::json!({"origin": [0.03_f32, 0.11_f32, 0.46_f32], "enclosure_wish": 0.23_f32}),
    );
    assert_eq!(changed["outcome"], "applied", "the settings were not applied: {changed}");
    assert_eq!(
        held(&app),
        ((0.03_f32, 0.11_f32, 0.46_f32), 0.23_f32),
        "the map does not hold the settings that were applied"
    );

    let back = undo(&mut app, &changed);
    assert_eq!(back["outcome"], "applied", "undo of the settings was refused: {back}");
    assert_eq!(held(&app), before, "undo did not put the old settings back");
}
