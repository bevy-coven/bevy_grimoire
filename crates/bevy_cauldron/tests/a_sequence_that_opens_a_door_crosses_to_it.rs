//! **A sequence whose operation opens a door crosses to that door.**
//!
//! The regression test for the cycle 8 bug where `grimoire_capture_sequence` answered `applied` for a
//! `door_open` while the editor stayed on its door. The sequencer dispatched the operation through
//! the registry and reported the plan's answer, but nothing stored the door request, so
//! `apply_pending_door` never saw it. The BRP path (`grimoire/operation_apply`) does both steps:
//! `editor::door_refusal` before the plan, and `editor::request_door` after it.
//!
//! # What the test does
//!
//! It runs the real sequencer in a real (headless) editor, with `door_open` to the Kit door as the
//! operation, then lets the editor run a few frames. The assertion is on the **door the editor is
//! on**, which is what the person sees, and not on the sequence's own answer.

use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{SequenceParams, Sequencer};
use bevy_cauldron::tiles::Door;

use crate::fixtures;

#[test]
fn a_sequence_that_opens_the_kit_door_leaves_the_editor_on_the_kit_door() {
    let root = fixtures::Fixture::new("sequence_door_crosses").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_ne!(
        *app.world().resource::<Door>(),
        Door::Kit,
        "the test starts on another door, so a crossing to Kit is visible"
    );

    let out = std::env::temp_dir().join(format!("grimoire-seq-door-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the out directory is made");

    let params = SequenceParams {
        label: "door".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 4,
        operation: Some(bevy_cauldron::sequencer::Operation {
            name: "door_open".to_owned(),
            parameters: serde_json::json!({"door": "kit"}),
        }),
    };
    let id = {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer.start(&params, 0.0).expect("the sequence starts")
    };

    // **Run until the sequence has finished, as the other sequence tests do.** A headless run ends
    // `failed` with the render-world reason; that is expected here and is not the thing under test.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut status = serde_json::Value::Null;
    while std::time::Instant::now() < deadline {
        app.update();
        status = app.world().resource::<Sequencer>().status(id);
        if matches!(status.get("status").and_then(|s| s.as_str()), Some("done") | Some("failed")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // **Let the editor cross the stored request.** A request is crossed in the frame after it is
    // stored, so a few frames after the sequence finished are enough.
    for _ in 0..4 {
        app.update();
    }

    assert_eq!(
        *app.world().resource::<Door>(),
        Door::Kit,
        "the sequence answered {} for door_open, but the editor is still on another door",
        status["operation_result"]
    );
}
