//! **The settings screen lists every setting, finds one by a word and resets it to its default** (UI-71, #249).
//!
//! Cmd/Ctrl+, opens it. Each setting shows its control, what it does, its default and a *Modified* marker
//! while the file being edited sets it; *Reset* takes the key out of that file, so the layer below decides
//! it again. A change writes one key to the user's file or the project's, and the running editor follows the
//! file as it follows an edit made by hand.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy_cauldron::project::Project;
use bevy_cauldron::setting_catalogue::{self, CATALOGUE, Value};
use bevy_cauldron::settings::{EditorSettings, SettingsLayer, UserSettingsFile};
use bevy_cauldron::settings_screen::{OpenSettings, Scope, SettingsDialog};
use bevy_gooey::testing::{click, find_by_label, type_text};

use crate::fixtures;

struct Rig {
    app: App,
    user: PathBuf,
    project: PathBuf,
}

fn rig(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
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
    std::thread::sleep(Duration::from_millis(30));
}

fn layer(path: &Path) -> SettingsLayer {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    SettingsLayer::read("test", &text).unwrap_or_else(|r| panic!("{}", r.reason))
}

/// Opens the screen the way the chord and the launcher's button do.
fn open(app: &mut App) {
    app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        app.update();
    }
}

fn shows(app: &mut App, text: &str) -> bool {
    find_by_label(app, text).is_ok()
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

#[test]
fn searching_grid_lists_only_the_grid_settings_and_reset_takes_the_key_out_of_the_file() {
    let mut r = rig("settings_screen_search");
    write(&r.user, "(version: 1, show_grid: Some(false), text_scale: Some(1.5))");
    open(&mut r.app);

    for every in ["Theme", "Text size", "Interface size", "Reduced motion", "Grid lines", "Grid strength"] {
        assert!(shows(&mut r.app, every), "with nothing searched, the All tab lists `{every}`");
    }

    let search = find_by_label(&mut r.app, "Search settings").expect("the screen has a search field");
    click(&mut r.app, search);
    type_text(&mut r.app, "grid");
    updates(&mut r.app, 3);

    assert!(shows(&mut r.app, "Grid lines") && shows(&mut r.app, "Grid strength"), "the two grid settings are listed");
    for gone in ["Theme", "Text size", "Interface size", "Reduced motion"] {
        assert!(!shows(&mut r.app, gone), "`{gone}` is not a grid setting, so the search hides it");
    }

    // `show_grid` is modified (the file sets it) and `grid_strength` is not: only the first has a Reset.
    assert!(shows(&mut r.app, "Reset Grid lines"), "a setting the file sets can be reset");
    assert!(!shows(&mut r.app, "Reset Grid strength"), "a setting the file leaves out has nothing to reset");
    let reset = find_by_label(&mut r.app, "Reset Grid lines").unwrap();
    click(&mut r.app, reset);
    updates(&mut r.app, 3);

    let after = layer(&r.user);
    assert_eq!(after.show_grid, None, "Reset removes the key from the file");
    assert_eq!(after.text_scale, Some(1.5), "and leaves every other key as it was");
    assert!(!std::fs::read_to_string(&r.user).unwrap().contains("show_grid"), "the word is gone from the text, not set to a default");
    assert!(!shows(&mut r.app, "Reset Grid lines"), "once reset, the setting is no longer modified");
    let running = r.app.world().resource::<EditorSettings>().clone();
    assert!(running.show_grid, "the running editor follows the file back to the default");
}

#[test]
fn a_switch_writes_one_key_and_the_running_editor_follows_the_file() {
    let mut r = rig("settings_screen_switch");
    assert!(r.app.world().resource::<EditorSettings>().show_grid);
    open(&mut r.app);

    let grid = find_by_label(&mut r.app, "Grid lines").unwrap();
    click(&mut r.app, grid);
    updates(&mut r.app, 3);
    assert_eq!(layer(&r.user).show_grid, Some(false), "the click wrote the key to the user's file");
    for _ in 0..80 {
        r.app.update();
        std::thread::sleep(Duration::from_millis(15));
        if !r.app.world().resource::<EditorSettings>().show_grid {
            break;
        }
    }
    assert!(!r.app.world().resource::<EditorSettings>().show_grid, "the running editor reads the file the screen wrote");
    assert!(shows(&mut r.app, "Reset Grid lines"), "and the row now says it is modified");
}

#[test]
fn the_project_scope_writes_the_projects_file_and_shows_what_it_inherits() {
    let mut r = rig("settings_screen_scope");
    write(&r.user, "(version: 1, show_grid: Some(false))");
    open(&mut r.app);
    {
        let world = r.app.world_mut();
        let mut q = world.query::<&mut SettingsDialog>();
        let mut dialog = q.single_mut(world).unwrap();
        dialog.scope = Scope::Project;
    }
    updates(&mut r.app, 3);
    assert!(!shows(&mut r.app, "Reset Grid lines"), "the project's file sets nothing, so nothing is modified there");

    let strength = find_by_label(&mut r.app, "Grid strength").unwrap();
    r.app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: strength, value: 0.5_f32, is_final: true });
    updates(&mut r.app, 3);
    let project = layer(&r.project);
    assert_eq!(project.grid_strength, Some(0.5), "a change in the project scope lands in editor.ron");
    assert_eq!(layer(&r.user).grid_strength, None, "and not in the user's file");
    assert_eq!(layer(&r.user).show_grid, Some(false), "which is as it was");
}

#[test]
fn a_file_that_does_not_read_is_said_and_never_written_over() {
    let mut r = rig("settings_screen_unreadable");
    let broken = "(version: 1, show_grid: Some(maybe";
    write(&r.user, broken);
    open(&mut r.app);
    updates(&mut r.app, 3);

    let grid = find_by_label(&mut r.app, "Grid lines").unwrap();
    click(&mut r.app, grid);
    updates(&mut r.app, 3);
    assert_eq!(std::fs::read_to_string(&r.user).unwrap(), broken, "a file a person typed that does not read is left exactly as it is");
    let mut said = r.app.world_mut().query::<&Text>();
    let lines: Vec<String> = said.iter(r.app.world()).map(|t| t.0.clone()).collect();
    assert!(lines.iter().any(|t| t.contains("does not read") && t.contains("settings.ron")), "the screen names the file: {lines:?}");
}

#[test]
fn every_key_a_settings_file_can_set_is_listed_on_the_screen() {
    let mut everything = SettingsLayer::default();
    for setting in CATALOGUE {
        let default = setting_catalogue::default_value(setting.key).unwrap_or_else(|| panic!("`{}` has no default", setting.key));
        assert!(everything.set(setting.key, default), "`{}` takes its own default", setting.key);
        assert_eq!(everything.get(setting.key), Some(default));
        let said = setting_catalogue::say(setting, default);
        assert!(!said.is_empty() && said != "?", "`{}` says its default: {said}", setting.key);
    }
    let text = ron::ser::to_string(&everything).unwrap();
    let keys = text.matches(':').count() - 1; // `version` is not a setting
    assert_eq!(keys, CATALOGUE.len(), "a settings file has a key the screen does not list (or the other way round): {text}");
    let mut labels: Vec<_> = CATALOGUE.iter().map(|s| s.label).collect();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), CATALOGUE.len(), "two settings share a label, so a row could not be found by its words");
    let _ = Value::On(true);
}
