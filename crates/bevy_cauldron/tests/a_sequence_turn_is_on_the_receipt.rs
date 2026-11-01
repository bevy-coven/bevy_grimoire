//! **An agent's operation run inside a capture sequence is on the person's receipt (B-020, the
//! sequencer's half).**
//!
//! `grimoire/operation_apply` writes the receipt, and so does the capture sequencer when it applies the
//! operation it carries: `receipt_agent_change(op.name, summary)`. The sequencer is a second
//! front end for the same change, so the person must see the same receipt from either. The screen
//! half is in `an_agent_turn_in_a_sequence_stands_on_the_screen.rs`; this checks the receipt.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{Operation, SequenceParams, Sequencer};

use crate::fixtures;

fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

#[test]
fn an_operation_inside_a_sequence_writes_the_receipt() {
    let root = project("seqreceipt");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    let out = std::env::temp_dir().join(format!("grimoire-seqreceipt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the out directory is made");

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
    let id = {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer.start(&params, 0.0).expect("the sequence starts")
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut status = serde_json::Value::Null;
    while std::time::Instant::now() < deadline {
        app.update();
        status = app.world().resource::<Sequencer>().status(id);
        if matches!(
            status.get("status").and_then(|s| s.as_str()),
            Some("done") | Some("failed")
        ) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&out);

    // **Headless, the run ends "failed" with "no render world"**: no frame can be written, and the
    // sequencer says so rather than report success (the refusal-over-degradation rule). The
    // operation is what this test is about, and it ran before the frames did, so its result is read.
    assert_eq!(
        status
            .get("operation_result")
            .and_then(|r| r.get("outcome"))
            .and_then(|s| s.as_str()),
        Some("applied"),
        "the sequence did not apply its operation: {status}"
    );
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(
        note.contains("agent: placement_set_yaw:"),
        "the sequence applied placement_set_yaw but the person's receipt does not name it. note: {note:?}"
    );
}
