//! **A family settings file edited while the editor runs is reloaded within a second** (#96).

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{reload_settings, FamilySettings};

use crate::fixtures;

/// **A real `ElementsSettings`, as `bevy_elements` writes one**, and the same file an hour later.
///
/// These tests wrote `(sun: (hour: 6.0))`, which is not an `ElementsSettings`: the host's typed read
/// refuses it (`Unexpected field named sun`), nothing is stored, and the test was asserting on a
/// fixture the crate throws away (#152). The base is `ElementsSettings::defaults().to_ron()` with
/// `clock.hour` set; **exactly one line differs** between the two, which is what makes the edit an
/// edit rather than a second file.
const SETTINGS: &str = include_str!("fixtures/bevy_elements.ron");
const SETTINGS_LATER: &str = include_str!("fixtures/bevy_elements_18.5.ron");

#[test]
fn an_edited_settings_file_reaches_the_editor_and_a_bad_one_is_a_problem() {
    let root = fixtures::Fixture::new("family_reload").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.ron");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, SETTINGS).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilySettings>().add_systems(Update, reload_settings);
    app.update();
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 5.0 && app.world().resource::<FamilySettings>().text["bevy_elements"] != SETTINGS_LATER {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // **3 s, not the 1 s the editor promises** (owner, 2026-10-29, #121). The file is polled every
    // 0.1 s, so the reload itself takes a fraction of the second. The rest of this wall clock is
    // this test's own `app.update()` calls, and with the whole workspace's tests running in
    // parallel on a loaded machine they took 1.81 s on bmb, which failed the push gate three times.
    // A bound tight enough to measure the promise on an idle machine fails the gate on a busy one.
    assert!(start.elapsed().as_secs_f64() < 3.0, "took {:?}", start.elapsed());
    assert_eq!(
        app.world().resource::<FamilySettings>().text["bevy_elements"],
        SETTINGS_LATER,
        "the edit must be the settings the host holds, verbatim"
    );

    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: ").unwrap();
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 3.0 && app.world().resource::<EditorState>().status.problems().is_empty() {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let probs = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>();
    assert!(probs.iter().any(|p| p.contains("bevy_elements") && p.contains("not valid RON")), "{probs:?}");
    assert_eq!(
        app.world().resource::<FamilySettings>().text["bevy_elements"],
        SETTINGS_LATER,
        "a refused edit keeps the last good settings, verbatim"
    );
}
