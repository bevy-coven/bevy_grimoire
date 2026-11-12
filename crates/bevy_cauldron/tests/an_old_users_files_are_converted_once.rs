//! **A person's `settings.ron`, `recent.ron` and `trusted.ron` are converted once, never dropped** (GD-03, #365; review M3).
//!
//! Before the cutover those three files were RON. This build reads `.yaml`, so without the conversion a person who upgrades has
//! their keymap and layout reset, their recents empty and every trusted guide asked again. The test uses scratch folders for the
//! config and data directories.

use std::path::PathBuf;

use bevy_cauldron::legacy;
use bevy_cauldron::settings::{SETTINGS_VERSION, SettingsLayer};
use bevy_grimoire_core::trust::TrustStore;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("old_users_files_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

#[test]
fn the_three_files_are_converted_read_by_their_types_and_the_originals_moved_aside() {
    let (config, data) = (scratch("config"), scratch("data"));
    std::fs::write(config.join("settings.ron"), format!("(version: {SETTINGS_VERSION}, text_scale: Some(1.25))")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(data.join("trusted.ron"), "(version: 1, projects: [\"/a/game\"], commands: [], guides: [])").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(data.join("recent.ron"), "// my projects\n(version: 1, projects: [], changelog_seen: None, window: None, tour_offered: false)").unwrap_or_else(|e| panic!("{e}"));

    let notices = legacy::upgrade_user_files(Some(config.clone()), Some(data.clone()));
    assert_eq!(notices.len(), 3, "one sentence per file: {notices:?}");
    assert!(notices.iter().all(|n| n.contains("was converted")), "{notices:?}");

    let layer = SettingsLayer::read("settings.yaml", &std::fs::read_to_string(config.join("settings.yaml")).unwrap_or_default())
        .unwrap_or_else(|r| panic!("{}", r.reason));
    assert_eq!(layer.text_scale, Some(1.25), "the person's setting survived the upgrade");
    let store = TrustStore::load(&data.join("trusted.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert!(format!("{store:?}").contains("/a/game"), "and so did what they trusted: {store:?}");
    for (dir, stem) in [(&config, "settings"), (&data, "trusted"), (&data, "recent")] {
        assert!(!dir.join(format!("{stem}.ron")).exists(), "{stem}.ron is moved aside");
        assert!(dir.join("migrated-ron").join(format!("{stem}.ron")).exists(), "into migrated-ron/");
    }
    assert!(legacy::upgrade_user_files(Some(config), Some(data)).is_empty(), "a second start has nothing to do");
}

#[test]
fn a_file_that_cannot_be_converted_is_named_and_left_where_it_is() {
    let config = scratch("broken");
    std::fs::write(config.join("settings.ron"), "(version: 1, text_scale: ").unwrap_or_else(|e| panic!("{e}"));
    let notices = legacy::upgrade_user_files(Some(config.clone()), None);
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("settings.ron") && notices[0].contains("could not be converted"), "loud, naming the file: {notices:?}");
    assert!(config.join("settings.ron").exists(), "the original stays");
    assert!(!config.join("settings.yaml").exists(), "and nothing half-written is left");
}

/// **The notice reaches the screen, not only stderr** (GD-03, #365; review M3, restored at a level that runs).
///
/// A bundle launch has no terminal, so a sentence written to stderr is one nobody reads — and the person whose keymap was
/// just carried over should be told rather than discover it. `launcher::say_the_startup_notices` says each notice as a
/// sticky toast on the Projects home, which is the first screen after an upgrade, and says each one once.
///
/// **The app is the editor's own** (`harness::build_headless` + `LauncherPlugin`), because the launcher draws the home's
/// splash and needs the asset server a `MinimalPlugins` app does not have — which is why a bare-plugin version of this test
/// panicked in `Assets<Image>` and was removed rather than left red.
#[test]
fn the_notice_is_a_toast_on_the_projects_home_and_it_is_said_once() {
    use bevy::prelude::*;
    use bevy_cauldron::launcher::{ClipboardOff, LauncherPlugin};
    use bevy_gooey::{Toast, ToastKind};

    use crate::fixtures::Fixture;

    let config = scratch("notice_config");
    std::fs::write(config.join("settings.ron"), format!("(version: {SETTINGS_VERSION}, text_scale: Some(1.5))")).unwrap_or_else(|e| panic!("{e}"));
    let notices = legacy::upgrade_user_files(Some(config), None);
    assert_eq!(notices.len(), 1, "{notices:?}");

    let home = scratch("notice_home");
    let root = Fixture::new(&format!("old_users_notice_{}", std::process::id())).descriptor("alpha/floor", "alpha").place("alpha/floor", (1.0, 1.0)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_cauldron::recent::RecentProjects(home.join("recent.yaml")))
        .insert_resource(ClipboardOff)
        .insert_resource(legacy::StartupNotices(notices.clone()))
        .add_plugins(LauncherPlugin);
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<bevy_cauldron::screen::Screen>>().set(bevy_cauldron::screen::Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }

    let mut q = app.world_mut().query::<&Toast>();
    let said: Vec<(String, ToastKind, bool)> = q.iter(app.world()).map(|t| (t.text().to_owned(), t.kind(), t.is_sticky())).collect();
    assert_eq!(said.len(), 1, "the conversion is on screen: {said:?}");
    assert!(said[0].0.contains("settings.ron") && said[0].0.contains("was converted"), "in its own words: {said:?}");
    assert_eq!(said[0].1, ToastKind::Alert, "an upgrade a person should read is an alert");
    assert!(said[0].2, "and it stands until dismissed");

    for _ in 0..4 {
        app.update();
    }
    let mut q = app.world_mut().query::<&Toast>();
    assert_eq!(q.iter(app.world()).count(), 1, "and it is said once, not every frame");
}
