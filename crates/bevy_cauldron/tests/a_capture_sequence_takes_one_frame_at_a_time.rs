//! **A capture sequence asks for at most one new frame per update, and a frame that never lands ends the run.**
//!
//! The regression test for B-038 and B-040. B-038: `grimoire_capture_sequence` at 15 fps dropped frames
//! with "Duplicate render target" and never reported done. B-040: the sequencer waited for each PNG
//! before asking for the next frame, so the readback's latency set the capture rate.
//!
//! # What is being proved
//!
//! **Bevy's duplicate check is per frame.** Two screenshots requested for the same render target
//! in the same frame collide: the second is despawned without its observer firing, so its PNG is
//! never written. The sequencer's frames all go to the same surface image, so it spawns at most one
//! new screenshot per update, and holds up to `MAX_IN_FLIGHT` while the earlier PNGs land. The run
//! waits for every PNG to land before it finishes. A capture that never lands is a refusal with a
//! reason after a bounded wait, not a run that waits forever.
//!
//! # How the test runs without a GPU
//!
//! The harness is headless, so no screenshot can ever be read back. That is the point: nothing
//! lands, so any screenshot spawned and left in the world is one that is *in flight*. The test
//! lets the editor's `Surface` and the `CanRender` flag stand for a real renderer, and counts the
//! `Screenshot` entities the sequencer has spawned. The clock is set by
//! `TimeUpdateStrategy::ManualDuration`, so a big jump in time makes many frames due in one update,
//! which is the condition that broke.

use std::time::Duration;

use bevy::prelude::*;
use bevy::render::view::screenshot::Screenshot;
use bevy::time::TimeUpdateStrategy;
use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{CanRender, SequenceParams, Sequencer};
use bevy_cauldron::surface::Surface;

use crate::fixtures;

/// **A project with one map and one piece.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **An editor that behaves as if it can render**, with its clock under the test's control.
///
/// **The surface is a default handle** — nothing renders it headless, so no readback ever lands,
/// and every screenshot the sequencer spawns stays in the world where the test can count it.
fn rendering_editor(name: &str, step: Duration) -> App {
    let root = project(name);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **`Time<Virtual>` clamps each step to its `max_delta` (250 ms by default)**, and the sequencer
    // reads that clock. Without this line a "one second" step advances the clock a quarter second.
    app.world_mut()
        .resource_mut::<bevy::time::Time<bevy::time::Virtual>>()
        .set_max_delta(step);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(step));
    app.insert_resource(CanRender(true));
    app.insert_resource(Surface {
        image: Handle::default(),
    });
    app
}

/// **How many screenshots are in flight**, which is how many the sequencer has spawned.
fn in_flight(app: &mut App) -> usize {
    app.world_mut()
        .query::<&Screenshot>()
        .iter(app.world())
        .count()
}

fn params(label: &str, seconds: f32, fps: u32) -> SequenceParams {
    SequenceParams {
        label: label.to_owned(),
        out_dir: std::env::temp_dir()
            .join(format!(
                "grimoire-one-at-a-time-{}-{label}",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned(),
        seconds,
        fps,
        operation: None,
    }
}

/// **A jump of a second makes fifteen frames due at once.** The sequencer asks for one new
/// screenshot per update, and holds up to four while their PNGs are on their way.
#[test]
fn a_burst_of_due_frames_is_asked_for_one_per_update() {
    const CAP: usize = 4;
    let mut app = rendering_editor("one_per_update", Duration::from_secs(1));
    for _ in 0..2 {
        app.update();
    }
    {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer
            .start(&params("burst", 2.0, 15), 0.0)
            .expect("the sequence starts");
    }
    // **Exactly one more per update, until the cap.** Four seconds is under the five-second
    // landing timeout, so no frame lands and none is replaced. Bevy's duplicate check is per
    // extraction frame: one new screenshot per update cannot collide with another.
    for update in 1..=4usize {
        app.update();
        let n = in_flight(&mut app);
        assert_eq!(
            n,
            update.min(CAP),
            "after update {update}, {n} screenshots are in flight: the sequence asks for one new \
             one per update and holds up to {CAP} while their PNGs land"
        );
    }
}

/// **A due frame is asked for on the update after the one before it, while that one is still out.**
///
/// The regression test for B-040. At 15 fps the sequencer asked for a frame and then waited for its
/// PNG before asking for the next, so a frame's readback latency set the rate: the capture ran at
/// the speed of one readback, not the speed asked for. Here the clock advances four frames' worth
/// per second exactly (a 250 ms step at 4 fps), so one frame falls due on every update, and nothing
/// lands (headless), so every frame is still out when the next is due.
#[test]
fn a_due_frame_is_requested_while_the_one_before_is_still_out() {
    let mut app = rendering_editor("pipelined", Duration::from_millis(250));
    for _ in 0..2 {
        app.update();
    }
    {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer
            .start(&params("pipelined", 2.0, 4), 0.0)
            .expect("the sequence starts");
    }
    for update in 1..=4usize {
        app.update();
        let n = in_flight(&mut app);
        assert_eq!(
            n, update,
            "after update {update}, {n} frames are out: a frame due on this update must be asked \
             for now, not after the earlier PNG lands"
        );
    }
}

/// **A frame whose PNG never lands ends the run with a reason**, rather than waiting forever.
#[test]
fn a_frame_that_never_lands_ends_the_run_with_a_reason() {
    let mut app = rendering_editor("never_lands", Duration::from_secs(1));
    for _ in 0..2 {
        app.update();
    }
    let id = {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer
            .start(&params("never", 0.2, 5), 0.0)
            .expect("the sequence starts")
    };
    let mut status = serde_json::Value::Null;
    for _ in 0..12 {
        app.update();
        status = app.world().resource::<Sequencer>().status(id);
        if status["status"] != "running" {
            break;
        }
    }
    assert_eq!(
        status["status"], "failed",
        "the run never finished: {status}"
    );
    let reason = status["error"].as_str().unwrap_or_default();
    assert!(
        reason.contains("did not land"),
        "the run failed without saying why the frame is missing: {status}"
    );
    // **And the timed-out capture is gone too.** Its entity still holds a `save_to_disk` observer,
    // so a readback that lands late would write a PNG into a run that has already failed.
    assert_eq!(
        in_flight(&mut app),
        0,
        "a timed-out capture is still alive, so its late readback could write into a failed run"
    );
}
