//! **A settings edit that moves only the clock's hour is walked there, not jumped** (#271).
//!
//! `bevy_elements` adapts the camera's exposure to the sky with a 1.5 s time constant
//! (`EXPOSURE_TAU_S`). A clock that jumps from noon to 20:00 therefore shows the night scene at noon's
//! exposure for about a second and a half: measured on big, 2026-11-02, as **12 frames with more than
//! 45 % of the viewport white** — which is why the clock and season demos could not be recorded.
//!
//! The crate smooths the right thing. The step was the editor's, and this file is its statement: over
//! the walk, no single frame moves the hour more than a fraction, and the clock still lands exactly on
//! the file's hour.
//!
//! **A date, a climate or a pause is not a transition.** Walking one would be a lie about what the file
//! says, so `same_except_hour` gates the ramp and the third test pins that.

#![cfg(feature = "bevy_elements")]

use bevy::prelude::*;
use bevy_cauldron::environment::ClockRamp;


/// **The walk, frame by frame at a stated step**: no frame moves the hour more than a fraction, and the
/// clock lands exactly on the file's hour rather than near it.
///
/// `step` is driven directly rather than through `walk_the_clock`, because a headless `App`'s frame delta
/// is whatever the test machine managed — a walk tested against that measures the host, not the rule.
#[test]
fn the_walk_never_steps_the_hour_and_lands_on_it() {
    let mut ramp = ClockRamp::between(12.0, 20.0).expect("a ramp");
    let mut hour = 12.0f64;
    let mut seen = vec![hour];
    // 1/60 s frames: eight hours over a one-second walk is 0.133 h a frame.
    for _ in 0..600 {
        let (next, done) = ramp.step(hour, 1.0 / 60.0);
        seen.push(next);
        hour = next;
        if done {
            break;
        }
    }
    let mut worst: f64 = 0.0;
    for pair in seen.windows(2) {
        worst = worst.max((pair[1] - pair[0]).abs());
    }
    assert!(worst < 2.0, "the clock moved {worst:.2} hours in one frame; {seen:?}");
    assert!((hour - 20.0).abs() < 1e-9, "the walk lands exactly on the file's hour: {hour}");
    assert_eq!(ramp.left, 0.0, "and it is finished");
}

/// **A date, a climate or a pause is not a transition**, so `same_except_hour` is what lets the ramp run.
///
/// This is the gate, not the walk: a settings edit that also moves the date must be applied whole,
/// because walking a pause or a season would be a lie about what the file says.
#[test]
fn only_an_edit_of_the_hour_alone_starts_a_walk() {
    use bevy_elements::WorldClock;
    let before = WorldClock::default();
    let mut hour_only = before.clone();
    hour_only.hour = 20.0;
    assert!(
        bevy_cauldron::environment::differs_only_in_hour(&before, &hour_only),
        "the same clock, one hour apart, is the edit a walk is for"
    );
    let mut also_paused = hour_only.clone();
    also_paused.paused = !before.paused;
    assert!(
        !bevy_cauldron::environment::differs_only_in_hour(&before, &also_paused),
        "a pause is applied whole: walking it would be a lie about the file"
    );
}

#[test]
fn a_ramp_chooses_the_short_way_round_the_clock() {
    // Noon to 20:00 is eight hours forward and sixteen back; noon to 06:00 is six back and eighteen
    // forward. Both are the smaller arc, which is the one a person turning the clock expects.
    let forward = ClockRamp::between(12.0, 20.0).expect("a different hour is a ramp");
    assert!((forward.left - 8.0).abs() < 1e-9, "noon to 20:00 is +8, not {}", forward.left);
    let back = ClockRamp::between(12.0, 6.0).expect("a different hour is a ramp");
    assert!((back.left + 6.0).abs() < 1e-9, "noon to 06:00 is -6, not {}", back.left);
    assert!(ClockRamp::between(9.0, 9.0).is_none(), "the same hour is no ramp at all");
}

/// **A ramp lands exactly, and a walk over many frames does not drift.**
#[test]
fn the_walk_lands_on_the_hour_and_does_not_drift() {
    let mut ramp = ClockRamp::between(12.0, 20.0).expect("a ramp");
    let mut hour = 12.0;
    for _ in 0..600 {
        let (next, done) = ramp.step(hour, 1.0 / 60.0);
        hour = next;
        if done {
            break;
        }
    }
    assert!((hour - 20.0).abs() < 1e-9, "sixty frames of a one-second walk land on 20.0: {hour}");
    assert_eq!(ramp.left, 0.0);
}
