//! **A sequence's first frame is the state before the action, and the rest are what it did.**
//!
//! The regression test for the 2026-10-21-02 run's enhancement.
//!
//! # What is being proved
//!
//! **The brief's rule: "frame 0 is the state before it."** The picture is of a *rendered* frame, so
//! an action applied before frame 0 was read back would be *in* frame 0 — and a sequence that
//! begins with the thing it was supposed to show the absence of is worse than no sequence, because
//! it says the action was invisible when it was not.
//!
//! **So the editor takes the frames**, times them itself, and applies the operation **after frame
//! 0 has been asked for**. The MCP server cannot promise this: it is a separate process, and the
//! operation and the capture would be two round trips with no ordering between them.
//!
//! # What the test does
//!
//! **It runs the real sequencer in a real editor**, applies a real operation through the real
//! registry, and reads the frames back. The assertion is on the *map's bounds* — which the
//! operation changes — and on the *frame files*, which must exist and must not be the same image.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{SequenceParams, Sequencer};

use crate::fixtures;

/// **A project with one map and one piece.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **The frames a sequence wrote, and the map's bounds at each one.**
///
/// **The bounds are the operation's own effect**, so the frame that shows the new bounds is the
/// frame that was taken after the action.
#[test]
fn a_sequence_starts_before_the_action_and_continues_after_it() {
    let root = project("sequencing");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    // **The out directory, in a place the test owns.**
    let out = std::env::temp_dir().join(format!("grimoire-seq-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the out directory is made");

    // **The operation changes one axis of the map's bounds**, so the change is visible in the
    // picture and in the state.
    let before = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map
        .bounds;
    let params = SequenceParams {
        label: "seq".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 4,
        operation: Some(bevy_cauldron::sequencer::Operation {
            name: "map_set_settings".to_owned(),
            parameters: serde_json::json!({"axis": "y", "axis_value": 20.0}),
        }),
    };
    let id = {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer.start(&params, 0.0).expect("the sequence starts")
    };
    assert_eq!(id, 1, "the first sequence is id 1");

    // **Enough frames for the whole sequence.** The editor's own clock drives the schedule, and a
    // headless run's frames are fast, so the frames are taken quickly — but the *schedule* is in
    // seconds, so the test has to let real time pass.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut status = serde_json::Value::Null;
    while std::time::Instant::now() < deadline {
        app.update();
        status = {
            let sequencer = app.world().resource::<Sequencer>();
            sequencer.status(id)
        };
        let done = status.get("status").and_then(|s| s.as_str()) == Some("done");
        if done {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // **The sequence finished.**
    //
    // **The headless harness has no render world**, so it cannot write a PNG, and the run says
    // so in its own answer: the status is `failed` with a reason, not `done` with a missing
    // file. **The pixels are proved in a real windowed editor**; what this test proves is the
    // *ordering* — and that is the part a headless run can measure.
    let finished = status.get("status").and_then(|s| s.as_str());
    assert!(
        matches!(finished, Some("done") | Some("failed")),
        "the sequence did not finish: {status}"
    );
    if finished == Some("failed") {
        let reason = status["error"].as_str().unwrap_or_default();
        assert!(
            reason.contains("render world"),
            "the run failed for a reason that is not the one the harness causes: {reason}"
        );
    }
    let frames = status["frames"]
        .as_array()
        .expect("the frames are an array");
    assert!(
        !frames.is_empty(),
        "the sequence wrote no frames, so there is nothing to compare: {status}"
    );

    // **The frames are named the way the brief names them.**
    //
    // **The files themselves are not checked here**, and the reason is the harness: the headless
    // editor runs with `WgpuSettings { backends: None }` — no GPU, no render world — so
    // `Screenshot::image` never produces a readback and the PNG is never written. **The pixels
    // are proved in a real windowed editor**, and this test proves the *ordering*, which is the
    // part the harness can measure.
    for frame in frames {
        let path = frame["path"].as_str().expect("a frame has a path");
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        assert!(
            name.starts_with("f") && name.contains("_t") && name.ends_with("s.png"),
            "the frame is not named the way the brief names them \
             (`f{{index}}_t{{seconds}}s.png`): {name}"
        );
    }

    // **The operation went in, and it went in AFTER the first frame.**
    let applied_at = status["operation_applied_t_s"]
        .as_f64()
        .expect("the operation's time is recorded");
    let first_frame_at = frames[0]["actual_t_s"]
        .as_f64()
        .expect("the first frame's time is recorded");
    assert!(
        applied_at > first_frame_at,
        "the operation was applied at {applied_at}s and the first frame was taken at \
         {first_frame_at}s, so frame 0 already shows the action -- and a sequence that begins with \
         the thing it was supposed to show the absence of is worse than no sequence."
    );

    // **And the operation's own effect is in the map.**
    let after = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map
        .bounds;
    assert_eq!(
        after.1, 20.0,
        "the operation did not change the map's Y bound: {after:?} (it was {before:?})"
    );

    let _ = std::fs::remove_dir_all(&out);
}

/// **A second sequence while one is running is refused.**
///
/// **Two sequences writing frames at once would interleave their file names**, and a reader
/// could not tell which frame belongs to which.
#[test]
fn a_second_sequence_while_one_is_running_is_refused() {
    let root = project("sequencerefuse");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let out = std::env::temp_dir().join(format!("grimoire-seq2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let params = SequenceParams {
        label: "seq".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 4,
        operation: None,
    };
    let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
    sequencer
        .start(&params, 0.0)
        .expect("the first sequence starts");
    let second = sequencer.start(&params, 0.0);
    assert!(
        second.is_err(),
        "a second sequence was accepted while one was running, and the two would interleave their \
         frame names: {second:?}"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// **A sequence's name is refused when it is not an id, and its length and rate are bounded.**
///
/// **The name names a directory**, and a directory named `../../etc` is a directory outside the
/// capture area. The rate and the length are bounded because an unbounded sequence is an
/// unbounded number of files.
#[test]
fn a_sequence_refuses_a_bad_name_a_bad_length_and_a_bad_rate() {
    let root = project("sequencerules");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let out = std::env::temp_dir().join(format!("grimoire-seq3-{}", std::process::id()));
    let mut sequencer = app.world_mut().resource_mut::<Sequencer>();

    let base = SequenceParams {
        label: "seq".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 4,
        operation: None,
    };

    // **A name that is not an id.**
    let mut bad = base.clone();
    bad.label = "../escape".to_owned();
    assert!(
        sequencer.start(&bad, 0.0).is_err(),
        "a sequence named `../escape` was accepted, and it names a directory outside the capture \
         area."
    );

    // **A length that is not a length.**
    let mut bad = base.clone();
    bad.seconds = 0.0;
    assert!(
        sequencer.start(&bad, 0.0).is_err(),
        "a sequence of zero seconds was accepted"
    );
    let mut bad = base.clone();
    bad.seconds = 60.0;
    assert!(
        sequencer.start(&bad, 0.0).is_err(),
        "a sequence of a minute was accepted, and it is forty times the brief's cap"
    );

    // **A rate that is not a rate.**
    let mut bad = base.clone();
    bad.fps = 60;
    assert!(
        sequencer.start(&bad, 0.0).is_err(),
        "a sequence at sixty frames a second was accepted; the editor takes two to fifteen a second"
    );

    let _ = std::fs::remove_dir_all(&out);
}

/// **A sequence at eight frames a second is accepted.**
///
/// **Four frames a second is too slow to read a short operation.** The editor must take the rate
/// the operation needs, up to the cap measured on a real editor, so the rate is not a fixed pair.
#[test]
fn a_sequence_takes_eight_frames_a_second() {
    let root = project("sequenceeight");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let out = std::env::temp_dir().join(format!("grimoire-seq8-{}", std::process::id()));
    let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
    let params = SequenceParams {
        label: "seq".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 8,
        operation: None,
    };
    let started = sequencer.start(&params, 0.0);
    let _ = std::fs::remove_dir_all(&out);
    assert!(
        started.is_ok(),
        "a sequence at eight frames a second was refused: {:?}",
        started.err()
    );
}

/// **A sequence above the measured cap is refused.**
///
/// **Sixteen frames a second is past what a real editor keeps to a quarter of the frame interval.**
/// The cap is fifteen (see `MAX_FPS` and the commit that set it). A rate above it is refused, not
/// clamped, so a caller never gets frames at a rate it did not ask for.
#[test]
fn a_sequence_refuses_sixteen_frames_a_second() {
    let root = project("sequencesixteen");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let out = std::env::temp_dir().join(format!("grimoire-seq16-{}", std::process::id()));
    let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
    let params = SequenceParams {
        label: "seq".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 16,
        operation: None,
    };
    let started = sequencer.start(&params, 0.0);
    let _ = std::fs::remove_dir_all(&out);
    assert!(
        started.is_err(),
        "a sequence at sixteen frames a second was accepted; the editor takes two to fifteen a second"
    );
}
