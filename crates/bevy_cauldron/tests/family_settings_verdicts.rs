//! **A family settings file's edit is a verdict in `grimoire_state.reloads`, once per edit** (#96, #200).
//!
//! A settings file that is valid RON replaces the crate's settings and a file that is not is refused
//! with the last good ones kept; both used to be said only on the status line, so an agent driving the
//! editor over BRP could not ask whether its edit had landed. The verdict names the file under the
//! game's root (`family/bevy_elements.ron`), says why a refusal is one, and goes when the file does.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::family_host::FamilySettings;
use bevy_cauldron::live::EditorReloads;
use serde_json::Value;

use crate::fixtures;

const FILE: &str = "family/bevy_elements.ron";

fn kinds(app: &App) -> Vec<Value> {
    app.world().resource::<EditorReloads>().state_json()["kinds"].as_array().cloned().unwrap_or_default().into_iter().filter(|k| k["kind"] == "family").collect()
}

fn seq(app: &App) -> u64 {
    app.world().resource::<EditorReloads>().state_json()["seq"].as_u64().unwrap()
}

fn until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 5.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never saw: {what}; reloads {}", app.world().resource::<EditorReloads>().state_json());
}

fn pump(app: &mut App, secs: f64) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_settings_edit_is_one_verdict_a_bad_one_is_refused_naming_the_file_and_a_removed_one_is_gone() {
    let root = fixtures::Fixture::new("family_verdicts").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.ron");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "(sun: (hour: 6.0))").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The editor's own `reload_settings` runs in the harness; a second copy would watch the same
    // files with its own state and decide every edit twice.
    app.update();
    assert_eq!(kinds(&app).len(), 1, "a file read at launch is a verdict too: {:?}", kinds(&app));
    assert_eq!(kinds(&app)[0]["file"], FILE);
    assert_eq!(kinds(&app)[0]["outcome"], "applied");

    // A good edit.
    let before = seq(&app);
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: 18.5))").unwrap();
    until(&mut app, "the edit applied", |a| a.world().resource::<FamilySettings>().text["bevy_elements"] == "(sun: (hour: 18.5))");
    assert_eq!(seq(&app), before + 1, "one edit, one decision");
    assert_eq!(kinds(&app)[0]["outcome"], "applied");

    // A bad one: refused, named, the good settings stay, said once however many frames pass.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: ").unwrap();
    until(&mut app, "the refusal", |a| kinds(a).first().is_some_and(|k| k["outcome"] == "refused"));
    let v = kinds(&app)[0].clone();
    assert_eq!(v["file"], FILE, "{v}");
    assert!(!v["reason"].as_str().unwrap().is_empty(), "the verdict says why: {v}");
    assert_eq!(app.world().resource::<FamilySettings>().text["bevy_elements"], "(sun: (hour: 18.5))", "the last good settings stay");
    assert_eq!(app.world().resource::<EditorReloads>().state_json()["ok"], false);
    let refused_at = seq(&app);
    assert_eq!(refused_at, before + 2);
    pump(&mut app, 1.5);
    assert_eq!(seq(&app), refused_at, "nothing was edited, so nothing is decided again");

    // Undoing the typo, back to exactly the settings in force, is no change to the settings, and the
    // refusal still goes.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: 18.5))").unwrap();
    until(&mut app, "the undo applied", |a| kinds(a).first().is_some_and(|k| k["outcome"] == "applied"));
    assert_eq!(app.world().resource::<EditorReloads>().state_json()["ok"], true);

    // A fix to other settings applies, and a removed file is no longer listed.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: ").unwrap();
    until(&mut app, "the second refusal", |a| kinds(a).first().is_some_and(|k| k["outcome"] == "refused"));
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: 7.0))").unwrap();
    until(&mut app, "the fix applied", |a| kinds(a).first().is_some_and(|k| k["outcome"] == "applied"));
    assert_eq!(app.world().resource::<EditorReloads>().state_json()["ok"], true);
    assert_eq!(app.world().resource::<FamilySettings>().text["bevy_elements"], "(sun: (hour: 7.0))");
    std::thread::sleep(Duration::from_millis(50));
    std::fs::remove_file(&file).unwrap();
    until(&mut app, "the verdict gone", |a| kinds(a).is_empty());
}

fn verdict(app: &App, file: &str) -> Value {
    kinds(app).into_iter().find(|k| k["file"] == file).unwrap_or(Value::Null)
}

#[test]
fn a_file_broken_before_the_editor_started_is_named_decided_once_and_goes_when_deleted() {
    let root = fixtures::Fixture::new("family_verdicts_launch").descriptor("site/wall", "alpha").build("m");
    let dir = root.join("family");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bevy_elements.ron"), "(sun: (hour: 6.0))").unwrap();
    std::fs::write(dir.join("bevy_lights.ron"), "(budget: (").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    assert_eq!(kinds(&app).len(), 2, "{:?}", kinds(&app));
    assert_eq!(verdict(&app, "family/bevy_elements.ron")["outcome"], "applied");
    let broken = verdict(&app, "family/bevy_lights.ron");
    assert_eq!(broken["outcome"], "refused", "a file that never read is named from the start: {broken}");
    assert!(!broken["reason"].as_str().unwrap().is_empty(), "{broken}");
    assert_eq!(app.world().resource::<EditorReloads>().state_json()["ok"], false);

    // Another file's edit makes the watch say the broken one is invalid again; it is not decided twice.
    let before = seq(&app);
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(dir.join("bevy_elements.ron"), "(sun: (hour: 9.0))").unwrap();
    until(&mut app, "the other file applied", |a| a.world().resource::<FamilySettings>().text["bevy_elements"] == "(sun: (hour: 9.0))");
    pump(&mut app, 0.6);
    assert_eq!(seq(&app), before + 1, "one edit, one decision: the broken file was not re-decided");
    assert_eq!(kinds(&app).len(), 2);
    assert_eq!(verdict(&app, "family/bevy_lights.ron")["outcome"], "refused");

    // Deleting a file that never read: nothing was in force to be removed, and the refusal still goes.
    std::fs::remove_file(dir.join("bevy_lights.ron")).unwrap();
    until(&mut app, "the refusal gone", |a| verdict(a, "family/bevy_lights.ron") == Value::Null);
    assert_eq!(app.world().resource::<EditorReloads>().state_json()["ok"], true);
}
