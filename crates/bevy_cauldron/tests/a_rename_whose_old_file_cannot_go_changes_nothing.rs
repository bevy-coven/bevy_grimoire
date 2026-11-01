//! **A rename that is refused changes nothing, even when the refusal comes after the write.**
//!
//! `map_rename` writes the map under its new name and then removes the old file. When the old file
//! cannot be removed, the write has already landed, and the editor used to keep the new name and the
//! new path while the registry answered `refused`. Nothing was logged, so undo could not reverse it,
//! and the list showed the map twice. A refused call must leave the editor and the disk as they were.

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

/// **Through the registered method**, as an agent's request goes.
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

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "map_rename", "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: Option<&str>) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn held_name(app: &App) -> String {
    app.world().resource::<OpenMap>().map.name.clone()
}

fn held_path(app: &App) -> std::path::PathBuf {
    app.world().resource::<OpenMap>().map_path.clone()
}

fn listing(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the maps directory reads")
        .map(|e| e.expect("an entry reads").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// **The old file cannot be removed, so the rename is refused and the editor keeps the old name.**
#[test]
fn a_rename_whose_old_file_cannot_go_is_refused_and_changes_nothing() {
    let mut app = editor("rename_old_file_stuck");
    let before = held_path(&app);
    let dir = before
        .parent()
        .expect("a map file has a directory")
        .to_path_buf();

    // A directory stands where the old file is, so the removal after the write must fail.
    std::fs::remove_file(&before).expect("the fixture map is removable at the start");
    std::fs::create_dir(&before).expect("a directory can stand where the old file was");
    let listing_before = listing(&dir);

    let answer = apply(&mut app, serde_json::json!({"name": "renamed"}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert!(
        answer.to_string().contains("m.map.ron"),
        "the refusal names the old file the author has to deal with: {answer}"
    );

    assert_eq!(held_name(&app), "m", "a refused rename keeps the name");
    assert_eq!(held_path(&app), before, "a refused rename keeps the path");
    assert!(
        !dir.join("renamed.map.ron").exists(),
        "the new file the rename wrote is taken back"
    );
    assert_eq!(
        listing(&dir),
        listing_before,
        "a refused rename leaves the maps directory as it was"
    );
    assert!(before.is_dir(), "the old path is not touched");

    let back = undo(&mut app, None);
    assert_eq!(
        back["outcome"], "refused",
        "a refused rename was never logged, so there is nothing to undo: {back}"
    );
}
