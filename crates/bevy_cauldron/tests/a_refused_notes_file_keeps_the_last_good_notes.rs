//! **A notes file that is refused keeps the last good notes, and the editor says which file and
//! field** (#200, H3).
//!
//! The signs over the notes and the Notes panel both draw from the map's `.notes.ron`. Each used to read
//! the file itself and take an empty list when it did not parse, so one bad edit (a hand edit, a
//! truncated write, an agent's mistake) made every sign and every row vanish until the file was fixed.
//! Now one view holds the last good notes and decides each edit once: `grimoire_state.reloads` says
//! `applied`, or `refused` with the file and the field, and a refusal leaves the notes as they were.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, Target};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::project::OpenMap;
use serde_json::{Value, json};

use crate::fixtures;

const FILE: &str = "maps/m.notes.ron";

/// The three ways to break it, and the field the verdict must name (`None`: the file is cut off, so
/// where reading stopped is all the position says).
const BAD: [(&str, &str, Option<&str>); 3] = [
    ("corrupt", "(version: 1, notes: [", None),
    ("wrong type", "(version: \"one\", notes: [], lease_secs: 300)", Some("version")),
    ("missing field", "(version: 1, lease_secs: 300)", Some("notes")),
];

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn path(app: &App) -> std::path::PathBuf {
    notes::path_for_map(&app.world().resource::<OpenMap>().map_path)
}

fn write(app: &App, text: &str) {
    std::fs::write(path(app), text).unwrap_or_else(|e| panic!("{e}"));
    // Distinct modification times, so the view sees every edit.
    std::thread::sleep(Duration::from_millis(30));
}

fn some_notes(texts: &[&str]) -> String {
    let mut n = Notes::default();
    for (i, t) in texts.iter().enumerate() {
        n.add(vec![Target::Cell(i as i32, 0)], t).unwrap();
    }
    n.to_ron().unwrap()
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
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "notes").cloned()).unwrap_or(Value::Null)
}

fn seq(app: &mut App) -> u64 {
    reloads(app)["seq"].as_u64().unwrap()
}

/// What the Notes panel lists, one line per note: it draws from the notes the editor holds.
fn rows(app: &mut App) -> Vec<String> {
    let mut lines: Vec<String> = app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).filter(|t| t.starts_with('n') && t.contains('[')).collect();
    lines.sort();
    lines
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 6.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("never saw: {what}; reloads {}", reloads(app));
}

fn pump(app: &mut App, secs: f64) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
}

/// Wait until the panel lists exactly `want`.
fn until_rows(app: &mut App, want: &[&str]) {
    until(app, &format!("the panel lists {want:?}"), |a| rows(a) == want);
}

#[test]
fn a_refused_notes_file_keeps_the_notes_and_says_which_file_and_field_once_per_edit() {
    let mut app = editor("notes_view_refused");
    write(&app, &some_notes(&["first", "second"]));
    let good = ["n1 [sent] first", "n2 [sent] second"];
    until_rows(&mut app, &good);

    for (what, text, field) in BAD {
        let before = seq(&mut app);
        let raised_before = app.world().resource::<EditorState>().status.raised();
        write(&app, text);
        // Past the panel's half-second look and the view's once-a-second re-read.
        pump(&mut app, 1.6);
        assert_eq!(rows(&mut app), good, "{what}: the notes shown before the edit stay");
        let v = verdict(&mut app);
        assert_eq!(v["outcome"], json!("refused"), "{what}: {v}");
        assert_eq!(v["file"], json!(FILE), "{what}: {v}");
        if let Some(field) = field {
            assert_eq!(v["field"], json!(field), "{what}: {v}");
        }
        assert!(!v["reason"].as_str().unwrap().is_empty(), "{what}: the verdict says why: {v}");
        assert_eq!(reloads(&mut app)["ok"], json!(false), "{what}");
        // What this edit raised, not an earlier edit's problem still in the log.
        let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
        let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
        assert_eq!(raised, 1, "{what}: the refusal is said once: {said:?}");
        assert!(said[0].contains(FILE) && said[0].contains("refused"), "{what}: the status line names the file: {said:?}");

        let refused_at = seq(&mut app);
        assert_eq!(refused_at, before + 1, "{what}: one edit, one decision");
        pump(&mut app, 1.6);
        assert_eq!(seq(&mut app), refused_at, "{what}: nothing was edited, so no new decision");
        assert_eq!(rows(&mut app), good, "{what}: still the notes shown before");

        // Back to a good file, so the next bad one is judged against the same notes.
        write(&app, &some_notes(&["first", "second"]));
        until(&mut app, "the good file applied again", |a| verdict(a)["outcome"] == json!("applied"));
        assert_eq!(rows(&mut app), good);
    }
}

#[test]
fn a_fixed_file_replaces_the_notes_and_a_removed_one_empties_them() {
    let mut app = editor("notes_view_fixed");
    write(&app, &some_notes(&["only"]));
    until_rows(&mut app, &["n1 [sent] only"]);
    write(&app, "(version: 1, notes: 7)");
    pump(&mut app, 1.6);
    assert_eq!(rows(&mut app), ["n1 [sent] only"], "a refused edit leaves the one note shown");
    assert_eq!(verdict(&mut app)["outcome"], json!("refused"));

    write(&app, &some_notes(&["only", "and another"]));
    until_rows(&mut app, &["n1 [sent] only", "n2 [sent] and another"]);
    assert_eq!(verdict(&mut app)["outcome"], json!("applied"), "the fix is applied");
    assert_eq!(reloads(&mut app)["ok"], json!(true));

    std::fs::remove_file(path(&app)).unwrap_or_else(|e| panic!("{e}"));
    until_rows(&mut app, &[]);
    assert_eq!(verdict(&mut app), Value::Null, "a removed file is no longer decided");
}
