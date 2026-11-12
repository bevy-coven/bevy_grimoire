//! **A settings reload is measured on `Time<Real>`, so a test can drive it** (#279).
//!
//! `family_settings_reload.rs` has to *wait* for a reload: `FamilyWatch::poll` takes `now_secs` and
//! never asked where it came from, but `reload_settings` passed it `held.started.elapsed()`, an
//! `Instant` the system created itself. So the one knob that decides when a reload lands was pinned
//! to the wall clock, a test had to sleep the poll and the settle interval out in real time, and on a
//! loaded machine `app.update()` cost more than the 3 s the assertion allowed — which failed the push
//! gate three times (#121 recorded the first).
//!
//! # What was measured, and which of the two tests is the proof
//!
//! **At `9ff6ed8`, with the wall clock in place** (`cargo test -p bevy_cauldron --test editor
//! a_reload_is_measured_on_the_clock_the_editor_exposes::`, own `CARGO_TARGET_DIR`):
//!
//! | test | on the wall clock | on `Time<Real>` |
//! |---|---|---|
//! | `stepping_the_clock_past_the_poll_and_the_settle_reloads_the_file` | **FAILED** — left the old text | passes |
//! | `a_clock_that_does_not_advance_reloads_nothing` | passed | passes |
//!
//! So the **first** test is the failing-first proof: three fast `app.update()` calls are 0 s of real
//! time, the private `Instant` never crosses `FamilyWatch`'s 0.1 s poll, and the edit never lands —
//! a test can only see the reload by *sleeping* real time out, which is precisely what
//! `family_settings_reload.rs` does and what load stretches past its bound (#121, #279).
//!
//! **The second test is the one that stays true under load, and that is its value.** It passed at
//! `9ff6ed8` on an idle machine for the wrong reason — real time had not advanced *enough* yet — and
//! it is the assertion that would go red on a loaded one, where the same 40 frames take longer than
//! the poll and the settle. On the injected clock it cannot: a zero step means the poll interval is
//! never crossed, whatever the machine is doing. It is a regression guard rather than the proof, and
//! it says so instead of claiming a failure it does not have.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{reload_settings, FamilySettings};

use crate::fixtures;

const SETTINGS: &str = include_str!("fixtures/bevy_elements.yaml");
const SETTINGS_LATER: &str = include_str!("fixtures/bevy_elements_18.5.yaml");

/// One step of the test's clock, in seconds. `FamilyWatch` is built with `SETTINGS_POLL_SECS` and
/// `SETTINGS_SETTLE_SECS` both 0.1, so a step of 0.2 crosses both in a single update.
const STEP: f32 = 0.2;

/// **The editor the host runs in, on a clock this test owns.**
///
/// `TimeUpdateStrategy::ManualDuration` is how `bevy_time`'s own tests drive time and how
/// `a_capture_sequence_takes_one_frame_at_a_time.rs` drives the capture clock, so this is the
/// established shape rather than a new one.
fn editor(root: &std::path::Path, step: f32) -> App {
    let mut app = bevy_cauldron::harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>()
        .init_resource::<FamilySettings>()
        .add_systems(Update, reload_settings);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(step)));
    app
}

/// The settings the host holds right now, as text.
fn held(app: &App) -> String {
    app.world().resource::<FamilySettings>().text.get("bevy_elements").cloned().unwrap_or_default()
}

/// **A clock that does not advance reloads nothing**, however many frames run.
///
/// This is the assertion a private `Instant` cannot satisfy: real time passes through these frames,
/// so on the wall clock the reload lands and this fails. `FamilyWatch` needs 0.1 s of poll and 0.1 s
/// of settle, and a frozen clock never gives it either.
#[test]
fn a_clock_that_does_not_advance_reloads_nothing() {
    let root = fixtures::Fixture::new("reload_frozen_clock").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.yaml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, SETTINGS).unwrap();

    // A step of zero: `Time<Real>` advances by nothing however often it is updated.
    let mut app = editor(&root, 0.0);
    app.update();
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    for _ in 0..40 {
        app.update();
    }

    assert_eq!(
        held(&app),
        SETTINGS,
        "the clock never advanced, so the watch never polled and never settled — the edit must not \
         be in force. Seeing the new text here means the reload is still measured on the wall clock, \
         which is the whole of #279: on a loaded machine that clock is what the gate's bound is \
         fighting, and no test can drive it"
    );
    assert!(
        app.world().resource::<EditorState>().status.problems().is_empty(),
        "and a file that is merely not yet read is not a problem"
    );
}

/// **The same edit lands once the clock is stepped past the poll and the settle interval.**
///
/// The positive half, and the one that keeps the assertion above from being satisfied by a reload
/// path that never fires at all.
#[test]
fn stepping_the_clock_past_the_poll_and_the_settle_reloads_the_file() {
    let root = fixtures::Fixture::new("reload_stepped_clock").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.yaml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, SETTINGS).unwrap();

    let mut app = editor(&root, STEP);
    app.update();
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    // Three steps is 0.6 s: one poll sees the new mtime, the next crosses the settle interval.
    // No `sleep` anywhere — that is the point, and the reason this test cannot be stretched by load.
    for _ in 0..3 {
        app.update();
    }

    assert_eq!(
        held(&app),
        SETTINGS_LATER,
        "three steps of 0.2 s cross the watch's 0.1 s poll and 0.1 s settle, so the edit must be the \
         settings the host holds, verbatim"
    );
}
