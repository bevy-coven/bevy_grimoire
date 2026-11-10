//! **A family settings file edited while the editor runs is reloaded within a second** (#96).
//!
//! The loops below step `Time<Real>` with `TimeUpdateStrategy::ManualDuration` instead of sleeping
//! (#279). `FamilyWatch` measures its 0.1 s poll and 0.1 s settle on that clock
//! (`family_host::reload_settings`), so the reload lands after a fixed number of updates whatever the
//! machine's load. The sleeps this replaced were 50 ms per update, and a loaded machine stretched them
//! past the 3 s bound (#121, #195). The bound stays, and the second promise, a reload within one second
//! of the edit, is now asserted on the clock the editor actually measures it on.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{reload_settings, FamilySettings};

use crate::fixtures;

/// One update of the test's clock, in seconds: the 50 ms the loops used to sleep, now stepped.
const STEP_SECS: f32 = 0.05;
/// The editor's promise (#96): an edit is in force within this much clock time.
const PROMISE_SECS: f32 = 1.0;
/// Updates a loop may run before it gives up: five seconds of clock time, as the wall-clock loop allowed.
const MAX_STEPS: u32 = 100;

/// **A real `ElementsSettings`, as `bevy_elements` writes one**, and the same file an hour later.
///
/// These tests wrote `(sun: (hour: 6.0))`, which is not an `ElementsSettings`: the host's typed read
/// refuses it (`Unexpected field named sun`), nothing is stored, and the test was asserting on a
/// fixture the crate throws away (#152). The base is `ElementsSettings::defaults().to_yaml()` with
/// `clock.hour` set; **exactly one line differs** between the two, which is what makes the edit an
/// edit rather than a second file.
const SETTINGS: &str = include_str!("fixtures/bevy_elements.yaml");
const SETTINGS_LATER: &str = include_str!("fixtures/bevy_elements_18.5.yaml");

#[test]
fn an_edited_settings_file_reaches_the_editor_and_a_bad_one_is_a_problem() {
    let root = fixtures::Fixture::new("family_reload").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.yaml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, SETTINGS).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilySettings>().add_systems(Update, reload_settings);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(STEP_SECS)));
    app.update();
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    let start = std::time::Instant::now();
    let mut steps = 0;
    while steps < MAX_STEPS && app.world().resource::<FamilySettings>().text["bevy_elements"] != SETTINGS_LATER {
        app.update();
        steps += 1;
    }
    // **3 s of wall clock, and the 1 s promise on the editor's clock** (owner, 2026-10-29, #121;
    // 2026-11-06, #279). The wall-clock bound is unchanged. Without the sleeps it is only the few
    // `app.update()` calls the stepped clock needs, so load can no longer stretch it past the bound
    // (1.81 s on bmb with 50 ms sleeps, which failed the push gate three times). The promise is
    // measured on `Time<Real>`, the clock `FamilyWatch` polls on, so it holds on a busy machine too.
    assert!(start.elapsed().as_secs_f64() < 3.0, "took {:?}", start.elapsed());
    assert!(
        steps as f32 * STEP_SECS <= PROMISE_SECS,
        "the edit took {steps} updates of {STEP_SECS} s on the editor's clock, past the {PROMISE_SECS} s it promises"
    );
    assert_eq!(
        app.world().resource::<FamilySettings>().text["bevy_elements"],
        SETTINGS_LATER,
        "the edit must be the settings the host holds, verbatim"
    );

    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "{sun: {hour: ").unwrap();
    let mut steps = 0;
    while steps < MAX_STEPS && app.world().resource::<EditorState>().status.problems().is_empty() {
        app.update();
        steps += 1;
    }
    let probs = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>();
    assert!(probs.iter().any(|p| p.contains("bevy_elements") && p.contains("not valid RON")), "{probs:?}");
    assert_eq!(
        app.world().resource::<FamilySettings>().text["bevy_elements"],
        SETTINGS_LATER,
        "a refused edit keeps the last good settings, verbatim"
    );
}
