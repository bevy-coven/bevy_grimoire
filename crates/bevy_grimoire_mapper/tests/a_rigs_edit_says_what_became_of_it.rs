//! **An edit to `rigs.ron` says what became of it** (#200, H3: the `needs_restart` outcome).
//!
//! The animation tab reads the rigs manifest once, when it is first entered, and holds what it read
//! (the registry classes the kind `Restart`). An edit to the file on disk was silently not in use: the
//! author saw nothing happen, and a broken edit looked the same as a good one. Now each distinct edit
//! is decided once into `grimoire_state.reloads` as kind `rigs`: `needs_restart` when the file reads
//! (the tab keeps what it holds), `refused` with the file and the field when it does not.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_mapper::anim_tab::BenchState;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::tiles::Mode;
use serde_json::{Value, json};

use crate::fixtures;

const FILE: &str = "rigs.ron";
const GOOD: &str = "(version: 3, rigs: {})";
const EDITED: &str = "(version: 3, note: Some(\"edited by hand\"), rigs: {})";

/// The three ways to break it, and the field the verdict must name (`None`: the file is cut off, so
/// where reading stopped is all the position says).
const BAD: [(&str, &str, Option<&str>); 3] = [
    ("corrupt", "(version: 3, rigs: {", None),
    ("wrong type", "(version: \"three\", rigs: {})", Some("version")),
    ("missing field", "(version: 3)", Some("rigs")),
];

fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let path = root.join("assets/grimoire").join(FILE);
    std::fs::write(&path, GOOD).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    // The tab reads the manifest the first time it is entered.
    app.world_mut().insert_resource(Mode::Anim);
    for _ in 0..6 {
        app.update();
    }
    let bench = app.world().resource::<BenchState>();
    assert!(bench.loaded && bench.text.as_deref() == Some(GOOD), "the tab holds the manifest it read: {:?}", bench.text);
    (app, path)
}

fn write(path: &std::path::Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
    // Distinct modification times, so the watcher sees every edit.
    std::thread::sleep(Duration::from_millis(30));
}

fn reloads(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["reloads"].clone()
}

fn verdict(app: &mut App) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "rigs").cloned()).unwrap_or(Value::Null)
}

fn seq(app: &mut App) -> u64 {
    reloads(app)["seq"].as_u64().unwrap()
}

fn held(app: &App) -> Option<String> {
    app.world().resource::<BenchState>().text.clone()
}

fn pump(app: &mut App, secs: f64) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
}

/// Frames until the `rigs` verdict has this outcome (`""`: none at all).
fn until_outcome(app: &mut App, outcome: &str) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 6.0 {
        let v = verdict(app);
        if v["outcome"].as_str().unwrap_or("") == outcome {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("never saw outcome `{outcome}` for rigs; reloads {}", reloads(app));
}

#[test]
fn a_valid_edit_says_it_needs_a_restart_once_and_the_tab_keeps_what_it_holds() {
    let (mut app, path) = editor("rigs_edit_needs_restart");
    // Nothing edited: no verdict, and the reload state is fine.
    pump(&mut app, 1.2);
    assert_eq!(verdict(&mut app), Value::Null, "an untouched manifest has nothing to say");
    assert_eq!(reloads(&mut app)["ok"], json!(true));

    let before = seq(&mut app);
    write(&path, EDITED);
    until_outcome(&mut app, "needs_restart");
    let v = verdict(&mut app);
    assert_eq!(v["file"], json!(FILE), "{v}");
    assert_eq!(v["field"], Value::Null, "a valid file has no field it is wrong at: {v}");
    assert!(v["reason"].as_str().unwrap().contains("restart"), "the verdict says what to do: {v}");
    assert_eq!(reloads(&mut app)["ok"], json!(false), "an edit that is not in use did not land");
    assert_eq!(held(&app).as_deref(), Some(GOOD), "the tab keeps the manifest it read");
    assert_eq!(seq(&mut app), before + 1, "one edit is one decision");

    // However many frames pass, the same edit is not decided again.
    pump(&mut app, 2.2);
    assert_eq!(seq(&mut app), before + 1, "one edit is one verdict however many frames pass");
    assert_eq!(verdict(&mut app)["outcome"], json!("needs_restart"));

    // Put back as it was, the file agrees with what the tab holds and there is nothing to restart for.
    write(&path, GOOD);
    until_outcome(&mut app, "");
    assert_eq!(reloads(&mut app)["ok"], json!(true));
}

#[test]
fn a_manifest_that_is_gone_says_it_needs_a_restart() {
    let (mut app, path) = editor("rigs_edit_gone");
    std::fs::remove_file(&path).unwrap();
    until_outcome(&mut app, "needs_restart");
    let v = verdict(&mut app);
    assert!(v["reason"].as_str().unwrap().contains("gone"), "{v}");
    assert_eq!(held(&app).as_deref(), Some(GOOD), "the tab keeps the manifest it read");
}

#[test]
fn a_broken_edit_is_refused_naming_the_file_and_the_field_once() {
    let (mut app, path) = editor("rigs_edit_refused");
    for (what, text, field) in BAD {
        let before = seq(&mut app);
        let raised_before = app.world().resource::<EditorState>().status.raised();
        write(&path, text);
        until_outcome(&mut app, "refused");
        let v = verdict(&mut app);
        assert_eq!(v["file"], json!(FILE), "{what}: {v}");
        if let Some(field) = field {
            assert_eq!(v["field"], json!(field), "{what}: {v}");
        }
        assert!(!v["reason"].as_str().unwrap().is_empty(), "{what}: the verdict says why: {v}");
        assert_eq!(reloads(&mut app)["ok"], json!(false), "{what}");
        assert_eq!(held(&app).as_deref(), Some(GOOD), "{what}: the tab keeps the manifest it read");
        assert_eq!(seq(&mut app), before + 1, "{what}: one edit is one decision");

        pump(&mut app, 2.2);
        assert_eq!(seq(&mut app), before + 1, "{what}: the same bad file is decided once, not every frame");
        let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
        let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
        assert_eq!(raised, 1, "{what}: the refusal is said once: {said:?}");
        assert!(said[0].contains(FILE) && said[0].contains("refused"), "{what}: the status line names the file: {said:?}");

        // A fix is not a refusal any more: it is a good file the tab cannot take yet.
        write(&path, EDITED);
        until_outcome(&mut app, "needs_restart");
        write(&path, GOOD);
        until_outcome(&mut app, "");
    }
}
