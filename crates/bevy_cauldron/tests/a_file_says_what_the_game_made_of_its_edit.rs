//! **Each file of the open project says what became of its latest edit** (#203, H6): `no game
//! connected`, `game applied` with when, `game refused` with why, `needs restart`, or `pending` while
//! the game has not answered an edit. The same rows are in `grimoire_state.reload_files` for an agent
//! and in the Reload panel for a person.
//!
//! The game's answer is the verdict it writes to `<project>/.live/game.ron` (what a running game does,
//! `bevy_grimoire::live`), written here by hand: a game is not a thing this repository can run headless.

use std::time::Duration;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_grimoire_core::live::{KindVerdict, Verdict};
use bevy_cauldron::live::LiveWrite;
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

const BAD_NOTES: &str = "(version: \"one\", notes: [], lease_secs: 300)";

fn editor(tag: &str, live: bool) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    if live {
        app.insert_resource(LiveWrite);
    }
    for _ in 0..4 {
        app.update();
    }
    app
}

/// The game's verdict reaches the editor four times a second and the panel looks twice a second.
fn settle(app: &mut App) {
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(300));
        app.update();
    }
}

fn state(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    app.world_mut().run_system_with(id, Some(json!({}))).unwrap_or_else(|e| panic!("{e:?}")).unwrap_or_else(|e| panic!("{e:?}"))
}

fn row(state: &Value, file: &str) -> Value {
    state["reload_files"]["files"].as_array().and_then(|f| f.iter().find(|r| r["file"] == file).cloned()).unwrap_or(Value::Null)
}

/// Rewrite a file with its own text: a new modification time and nothing else changed.
fn touch(path: &std::path::Path) {
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(path, std::fs::read_to_string(path).unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(30));
}

fn paths(app: &App) -> (std::path::PathBuf, std::path::PathBuf) {
    (app.world().resource::<Project>().project_dir.clone(), app.world().resource::<OpenMap>().map_path.clone())
}

#[test]
fn with_no_game_every_file_says_so_and_the_editors_own_refusal_rides_beside_it() {
    let mut app = editor("reload_files_no_game", true);
    let (_, map) = paths(&app);
    std::fs::write(bevy_grimoire_core::notes::path_for_map(&map), BAD_NOTES).unwrap();
    settle(&mut app);

    let s = state(&mut app);
    assert_eq!(s["reload_files"]["game"]["connected"], json!(false), "{s}");
    assert_eq!(s["reload_files"]["game"]["watching"], json!(true), "{s}");
    let files = s["reload_files"]["files"].as_array().expect("a list of files");
    for expected in ["kits.ron", "maps/m.map.ron", "maps/m.notes.ron"] {
        assert!(files.iter().any(|f| f["file"] == expected), "{expected} is a row: {files:?}");
    }
    assert!(files.len() >= 4, "the library files are rows too: {files:?}");
    for f in files {
        assert_eq!(f["state"], json!("no_game_connected"), "every file reads it with no game: {f}");
    }
    let notes = row(&s, "maps/m.notes.ron");
    assert_eq!(notes["editor"]["outcome"], json!("refused"), "{notes}");
    assert_eq!(notes["editor"]["field"], json!("version"), "{notes}");
    assert!(!notes["editor"]["reason"].as_str().unwrap().is_empty(), "{notes}");
    assert_eq!(row(&s, "kits.ron")["editor"], Value::Null, "a file the editor made nothing of has no editor verdict");
}

#[test]
fn a_game_that_answered_shows_each_file_applied_with_when_refused_with_why_or_pending() {
    let mut app = editor("reload_files_game", true);
    let (project_dir, map) = paths(&app);
    let kits = project_dir.join("kits.ron");
    touch(&kits);
    touch(&map);
    let kinds = vec![
        KindVerdict::applied("kits", "kits.ron"),
        KindVerdict::refused("map", "maps/m.map.ron", Some("placements[0].at[1]".to_owned()), "1:47: Expected float"),
    ];
    Verdict::with_kinds(3, "maps/m.map.ron: refused", kinds).write(&project_dir).unwrap();
    settle(&mut app);

    let s = state(&mut app);
    assert_eq!(s["reload_files"]["game"]["connected"], json!(true), "{s}");
    assert_eq!(s["reload_files"]["game"]["seq"], json!(3));
    let applied = row(&s, "kits.ron");
    assert_eq!(applied["state"], json!("game_applied"), "{applied}");
    let (at, edited) = (applied["at_unix_ms"].as_u64().unwrap(), applied["modified_unix_ms"].as_u64().unwrap());
    assert!(at > edited, "the game decided after the edit: decided {at}, edited {edited}");
    let refused = row(&s, "maps/m.map.ron");
    assert_eq!(refused["state"], json!("game_refused"), "{refused}");
    assert_eq!(refused["field"], json!("placements[0].at[1]"));
    assert_eq!(refused["reason"], json!("1:47: Expected float"));
    let silent = row(&s, "furniture/library.ron");
    assert_eq!(silent["state"], json!("not_read"), "a file the game's decision did not read: {silent}");
    assert_eq!(silent["at_unix_ms"], Value::Null);

    // An edit after the decision is one the game has not answered: pending, keeping the older decision.
    touch(&kits);
    settle(&mut app);
    let s = state(&mut app);
    let pending = row(&s, "kits.ron");
    assert_eq!(pending["state"], json!("pending"), "{pending}");
    assert_eq!(pending["at_unix_ms"], json!(at), "it keeps the decision the edit overtook");
    assert!(pending["modified_unix_ms"].as_u64().unwrap() > at, "{pending}");
    assert_eq!(row(&s, "maps/m.map.ron")["state"], json!("game_refused"), "a file left alone keeps its answer");
}

/// (text, top y) of every drawn text.
fn drawn(app: &mut App) -> Vec<(String, f32)> {
    settle(app);
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform)>();
    q.iter(world)
        .filter(|(_, n, t)| n.size().x > 0.0 && n.size().y > 0.0 && t.translation.y.is_finite())
        .map(|(t, n, tf)| (t.0.clone(), tf.translation.y - n.size().y / 2.0))
        .collect()
}

#[test]
fn the_reload_panel_draws_whether_a_game_is_connected_and_each_files_answer() {
    let mut app = editor("reload_files_panel", true);
    let (project_dir, map) = paths(&app);
    std::fs::write(bevy_grimoire_core::notes::path_for_map(&map), BAD_NOTES).unwrap();

    let texts = drawn(&mut app);
    let heading = texts.iter().find(|(t, _)| t == "RELOAD").expect("the panel has its heading").1;
    let header = texts.iter().find(|(t, _)| t.starts_with("no game connected (")).unwrap_or_else(|| panic!("no header; drawn: {texts:?}"));
    assert!(header.1 > heading, "the header is under the heading");
    assert!(
        texts.iter().any(|(t, _)| t.starts_with("maps/m.notes.ron · editor refused (version): ")),
        "the editor's own refusal is a line even with no game; drawn: {texts:?}"
    );
    assert!(!texts.iter().any(|(t, _)| t.contains("game applied")), "no game has applied anything");

    touch(&project_dir.join("kits.ron"));
    touch(&map);
    let kinds = vec![
        KindVerdict::applied("kits", "kits.ron"),
        KindVerdict::refused("map", "maps/m.map.ron", Some("placements[0].at[1]".to_owned()), "1:47: Expected float"),
    ];
    Verdict::with_kinds(4, "refused", kinds).write(&project_dir).unwrap();
    let texts = drawn(&mut app);
    assert!(texts.iter().any(|(t, _)| t.starts_with("game decided ") && t.ends_with("(decision 4)")), "{texts:?}");
    let applied = texts.iter().find(|(t, _)| t.starts_with("kits.ron · game applied ")).unwrap_or_else(|| panic!("{texts:?}"));
    assert!(applied.0.ends_with(" ago"), "{}", applied.0);
    let refused = texts
        .iter()
        .find(|(t, _)| t.starts_with("maps/m.map.ron · game refused (placements[0].at[1]): 1:47: Expected float · "))
        .unwrap_or_else(|| panic!("{texts:?}"));
    assert!(refused.1 > heading, "the lines are under the heading");
    assert!(!texts.iter().any(|(t, _)| t.starts_with("no game connected")), "the header changed");
}

#[test]
fn an_editor_that_does_not_read_verdicts_says_how_to_connect_one() {
    let mut app = editor("reload_files_not_live", false);
    let (project_dir, _) = paths(&app);
    // A game's verdict is on disk, and the editor was not started with --live, so it never reads it.
    Verdict::with_kinds(5, "applied", vec![KindVerdict::applied("kits", "kits.ron")]).write(&project_dir).unwrap();

    let texts = drawn(&mut app);
    assert!(
        texts.iter().any(|(t, _)| t.starts_with("no game connected: the editor reads a game's verdicts only with --live (")),
        "the header says why; drawn: {texts:?}"
    );
    let s = state(&mut app);
    assert_eq!(s["reload_files"]["game"]["connected"], json!(false), "{s}");
    assert_eq!(s["reload_files"]["game"]["watching"], json!(false), "{s}");
    assert_eq!(row(&s, "kits.ron")["state"], json!("no_game_connected"));
}

#[test]
fn a_refusal_about_how_files_fit_together_stays_a_refusal_when_a_file_is_made() {
    let mut app = editor("reload_files_project_row", true);
    let (project_dir, _) = paths(&app);
    let kinds = vec![KindVerdict::refused("project", ".", None, "vocabulary: token `wobbly` is not declared")];
    Verdict::with_kinds(2, "refused", kinds).write(&project_dir).unwrap();
    // The project directory's own time moves when a file is made in it; the entry names no file.
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(project_dir.join("scratch.txt"), "x").unwrap();
    settle(&mut app);

    let s = state(&mut app);
    let project = row(&s, ".");
    assert_eq!(project["state"], json!("game_refused"), "{project}");
    assert_eq!(project["reason"], json!("vocabulary: token `wobbly` is not declared"), "{project}");
    assert_eq!(project["modified_unix_ms"], Value::Null, "{project}");
}
