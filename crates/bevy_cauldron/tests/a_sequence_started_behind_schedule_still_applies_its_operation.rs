//! **A sequence that starts behind its own schedule still applies its operation (E-065).**
//!
//! # What the bug is, and why the item named the wrong cause
//!
//! `E-065` recorded "five sequencer tests fail under machine load: a real-time 20 s deadline spins
//! without sleeping". The deadline is real, but it is not the mechanism: under load those tests reach
//! `Sequencer::start` with a `now` already past the last frame's time, because their warm-up updates
//! took longer than the sequence's own half second.
//!
//! `drive_sequencer` runs in this order:
//!
//! 1. the operation gate — `if !run.applied && run.next >= 1` — so the operation waits for frame 0;
//! 2. the due-frame loop, which asks for one frame per update and stops when it has asked for all;
//! 3. the finish — `if all_asked && (all_written || !can_render)`.
//!
//! **When the first tick's `now` is already past the last frame's time, the gate is skipped** (`next`
//! is still 0, so the operation is not due), the headless branch asks every frame in that one tick,
//! and the finish closes the run with `operation_result: None`. The operation can then never apply,
//! because the run has finished. On a machine at load 30 this is not rare: the test fails, the gate
//! means nothing, and a real regression hides behind it.
//!
//! # What is being proved
//!
//! A run started behind its own schedule applies the operation it carries before it finishes, and
//! records when. The clock is deterministic — a manual duration, not the wall clock — so this is a
//! statement about the algorithm and not about how busy the machine was.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{Operation, SequenceParams, Sequencer};

use crate::fixtures;

/// **A project with one map and one piece.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **An editor whose clock the test controls**, one second per `update()`.
///
/// One second is deliberate: the sequence runs for 0.5 s at 4 fps, so the very first tick after
/// `start` is already past every frame's time — the behind-schedule state, made deterministic
/// instead of waiting for a machine busy enough to produce it.
fn editor(name: &str) -> App {
    let root = project(name);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // `Time<Virtual>` clamps each step to its `max_delta` (250 ms by default) and the sequencer
    // reads that clock, so a one-second step would otherwise advance it a quarter second.
    app.world_mut()
        .resource_mut::<bevy::time::Time<bevy::time::Virtual>>()
        .set_max_delta(Duration::from_secs(1));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(1)));
    app
}

/// **Start a sequence at the clock's own `now`**, which is what a caller under load does.
fn start_behind_schedule(app: &mut App, out: &std::path::Path) -> u64 {
    let params = SequenceParams {
        label: "seq".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 0.5,
        fps: 4,
        operation: Some(Operation {
            name: "placement_set_yaw".to_owned(),
            parameters: serde_json::json!({"id": "site/wall@0", "yaw": 90.0}),
        }),
    };
    let now = app.world().resource::<Time>().elapsed_secs();
    let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
    sequencer.start(&params, now).expect("the sequence starts")
}

/// **A run started behind its schedule applies its operation before it finishes.**
#[test]
fn a_sequence_started_behind_schedule_still_applies_its_operation() {
    let mut app = editor("seqbehind");
    for _ in 0..3 {
        app.update();
    }

    let out = std::env::temp_dir().join(format!("grimoire-seqbehind-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the out directory is made");

    let id = start_behind_schedule(&mut app, &out);

    let mut status = serde_json::Value::Null;
    for _ in 0..10 {
        app.update();
        status = app.world().resource::<Sequencer>().status(id);
        if matches!(
            status.get("status").and_then(|s| s.as_str()),
            Some("done") | Some("failed")
        ) {
            break;
        }
    }

    assert!(
        matches!(
            status.get("status").and_then(|s| s.as_str()),
            Some("done") | Some("failed")
        ),
        "the sequence neither finished nor failed in ten updates: {status}"
    );
    assert_eq!(
        status
            .get("operation_result")
            .and_then(|r| r.get("outcome"))
            .and_then(|s| s.as_str()),
        Some("applied"),
        "the run finished without applying its operation, because the operation gate is skipped when \
         the first tick is already past the last frame's time. The caller is told the sequence is \
         done and the change it asked for never happened. status: {status}"
    );
    assert!(
        status.get("operation_applied_t_s").is_some_and(|t| t.is_number()),
        "the run applied its operation but did not record when, so the manifest cannot say the \
         change is inside frame 0 and not after it. status: {status}"
    );

    let _ = std::fs::remove_dir_all(&out);
}
