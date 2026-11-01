//! **The editor's settings are three layers, and a change to a file changes the running editor** (UI-70, #248).
//!
//! Defaults, then the user's `settings.ron`, then the project's `assets/grimoire/editor.ron`: per key,
//! the later layer wins. The editor looks at both files while it runs, so deleting the project's key
//! falls back to the user's value without a restart, and a file that does not read keeps the last good
//! value and is refused, naming the file and the field.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::live::EditorReloads;
use bevy_cauldron::project::Project;
use bevy_cauldron::settings::{EditorSettings, Theme, UserSettingsFile};

use crate::fixtures;

struct Rig {
    app: App,
    user: PathBuf,
    project: PathBuf,
}

fn rig(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_config_{tag}_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..3 {
        app.update();
    }
    let project = app.world().resource::<Project>().project_dir.join("editor.ron");
    Rig { app, user, project }
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
    // Distinct modification times, so the editor sees every edit.
    std::thread::sleep(Duration::from_millis(30));
}

fn settings(app: &App) -> EditorSettings {
    app.world().resource::<EditorSettings>().clone()
}

fn until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 6.0 {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never saw: {what}; settings {:?}", settings(app));
}

fn verdicts(app: &App) -> Vec<(String, String, Option<String>, String)> {
    app.world()
        .resource::<EditorReloads>()
        .kinds()
        .iter()
        .filter(|k| k.kind == "editor_settings")
        .map(|k| (k.file.clone(), k.outcome.word().to_owned(), k.field.clone(), k.reason.clone()))
        .collect()
}

#[test]
fn no_file_is_the_defaults() {
    let mut r = rig("settings_defaults");
    for _ in 0..3 {
        r.app.update();
    }
    assert_eq!(settings(&r.app), EditorSettings::default());
    assert!(verdicts(&r.app).is_empty(), "no file, nothing to refuse or apply");
}

#[test]
fn a_key_set_in_both_layers_takes_the_projects_and_deleting_it_falls_back_to_the_users_live() {
    let mut r = rig("settings_layers");
    write(&r.user, "(version: 1, text_scale: Some(1.5), reduced_motion: Some(true))");
    until(&mut r.app, "the user's layer", |a| settings(a).text_scale == 1.5);
    assert!(settings(&r.app).reduced_motion, "a key only the user's file sets reaches the editor");

    write(&r.project, "(version: 1, text_scale: Some(2.0), theme: Some(Parchment))");
    until(&mut r.app, "the project's layer", |a| settings(a).text_scale == 2.0);
    let s = settings(&r.app);
    assert_eq!(s.theme, Theme::Parchment);
    assert!(s.reduced_motion, "a key the project leaves out still comes from the user's file");

    write(&r.project, "(version: 1, theme: Some(Parchment))");
    until(&mut r.app, "the user's text_scale back", |a| settings(a).text_scale == 1.5);
    assert_eq!(settings(&r.app).theme, Theme::Parchment);

    std::fs::remove_file(&r.project).unwrap();
    until(&mut r.app, "the theme falls back to the default", |a| settings(a).theme == Theme::ToolDark);
    assert_eq!(settings(&r.app).text_scale, 1.5);
    assert!(verdicts(&r.app).iter().all(|v| v.1 == "applied"), "{:?}", verdicts(&r.app));
}

#[test]
fn a_file_that_does_not_read_keeps_the_last_good_value_and_is_refused_naming_the_field() {
    let mut r = rig("settings_refused");
    write(&r.project, "(version: 1, text_scale: Some(1.25))");
    until(&mut r.app, "the good file", |a| settings(a).text_scale == 1.25);

    // A value no editor can use, a key that is not one, a version this build does not read, and text that is not RON.
    for (text, field) in [
        ("(version: 1, text_scale: Some(9.0))", Some("text_scale")),
        ("(version: 1, text_sale: Some(2.0))", None),
        ("(version: 7, text_scale: Some(2.0))", Some("version")),
        ("(version: 1, text_scale: Some(", None),
    ] {
        write(&r.project, text);
        until(&mut r.app, &format!("a refusal of {text}"), |a| verdicts(a).iter().any(|v| v.1 == "refused"));
        let v = verdicts(&r.app).remove(0);
        assert_eq!((v.0.as_str(), v.1.as_str()), ("editor.ron", "refused"), "{text}: {v:?}");
        if let Some(field) = field {
            assert_eq!(v.2.as_deref(), Some(field), "{text}: {v:?}");
        }
        assert!(v.3.contains("last good"), "the refusal says what stays: {v:?}");
        assert_eq!(settings(&r.app).text_scale, 1.25, "{text}: the last good value stays");
        let problems = r.app.world().resource::<EditorState>().status.problems().len();
        assert!(problems >= 1, "{text}: a problem is on the status line");
        write(&r.project, "(version: 1, text_scale: Some(1.25))");
        until(&mut r.app, "applied again", |a| verdicts(a).iter().all(|v| v.1 == "applied"));
    }

    write(&r.project, "(version: 1, text_scale: Some(1.75))");
    until(&mut r.app, "a repaired file applies", |a| settings(a).text_scale == 1.75);
}

#[test]
fn an_old_file_with_fewer_keys_still_reads() {
    let mut r = rig("settings_old_file");
    write(&r.user, "(version: 1)");
    for _ in 0..3 {
        r.app.update();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(settings(&r.app), EditorSettings::default());
    assert!(verdicts(&r.app).iter().all(|v| v.1 == "applied"), "{:?}", verdicts(&r.app));
}
