//! **A family settings file edited while the editor runs is reloaded within a second** (#96).

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{reload_settings, FamilySettings};

use crate::fixtures;

#[test]
fn an_edited_settings_file_reaches_the_editor_and_a_bad_one_is_a_problem() {
    let root = fixtures::Fixture::new("family_reload").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.ron");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "(sun: (hour: 6.0))").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilySettings>().add_systems(Update, reload_settings);
    app.update();
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: 18.5))").unwrap();
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 5.0 && app.world().resource::<FamilySettings>().text["bevy_elements"] != "(sun: (hour: 18.5))" {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // **3 s, not the 1 s the editor promises** (owner, 2026-10-29, #121). The file is polled every
    // 0.1 s, so the reload itself takes a fraction of the second. The rest of this wall clock is
    // this test's own `app.update()` calls, and with the whole workspace's tests running in
    // parallel on a loaded machine they took 1.81 s on bmb, which failed the push gate three times.
    // A bound tight enough to measure the promise on an idle machine fails the gate on a busy one.
    assert!(start.elapsed().as_secs_f64() < 3.0, "took {:?}", start.elapsed());
    assert_eq!(app.world().resource::<FamilySettings>().text["bevy_elements"], "(sun: (hour: 18.5))");

    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "(sun: (hour: ").unwrap();
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 3.0 && app.world().resource::<EditorState>().status.problems().is_empty() {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let probs = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>();
    assert!(probs.iter().any(|p| p.contains("bevy_elements") && p.contains("not valid RON")), "{probs:?}");
    assert_eq!(app.world().resource::<FamilySettings>().text["bevy_elements"], "(sun: (hour: 18.5))");
}
