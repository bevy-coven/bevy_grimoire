//! **An agent's turn inside a capture sequence reaches the picture.**
//!
//! # What is being proved
//!
//! `a862342` made `grimoire/operation_apply` and `grimoire/operation_undo` set `Due.map_replaced`,
//! so a saved change is redrawn. The capture sequencer dispatches its own operation through
//! `ops::dispatch` and never sets that flag. The result was measured on 2026-10-22: a sequence
//! with `placement_set_yaw` wall_02_2 0 -> 90 wrote the map to disk and reported `applied`, yet
//! all 12 frames were pixel-identical to frame 0.
//!
//! # What the test does
//!
//! It runs the real sequencer in a real headless editor and applies `placement_set_yaw` through
//! it. The redraw sweeps every standing piece and spawns it again, so the piece's entity changes.
//! That is the mechanism that puts the change on screen, and it is asserted directly, because a
//! headless run has no picture to read.

use bevy::prelude::*;
use bevy_cauldron::editor::Placement;
use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{Operation, SequenceParams, Sequencer};

use crate::fixtures;

/// **A project with one map, one piece, and a map that has been saved.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **A placed piece's entity and rotation, by its id.**
fn piece(app: &mut App, id: &str) -> Option<(Entity, Quat)> {
    let mut q = app.world_mut().query::<(Entity, &Placement, &Transform)>();
    q.iter(app.world())
        .find(|(_, placement, _)| placement.0 == id)
        .map(|(entity, _, transform)| (entity, transform.rotation))
}

#[test]
fn an_operation_inside_a_sequence_is_redrawn_on_the_screen() {
    let root = project("seqredraw");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let (before_entity, before_rot) = piece(&mut app, "site/wall@0").expect("the piece is spawned");

    let out = std::env::temp_dir().join(format!("grimoire-seqredraw-{}", std::process::id()));
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
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = std::fs::remove_dir_all(&out);

    // **The turn applied, so the piece is turned in the state.** A refused turn leaves the picture
    // unchanged, so the picture alone cannot tell a refusal from a redraw: the sequence's own
    // answer is read, and it must say the turn was applied.
    let finished = status.get("status").and_then(|s| s.as_str());
    assert!(
        matches!(finished, Some("done") | Some("failed")),
        "the sequence did not finish: {status}"
    );
    assert_eq!(
        status["operation_result"]["outcome"], "applied",
        "the sequence's turn was not applied: {status}"
    );

    for _ in 0..8 {
        app.update();
    }
    let (after_entity, after_rot) =
        piece(&mut app, "site/wall@0").expect("the piece is still spawned");

    // **The redraw is the thing that reaches the screen.** A respawned piece is a piece the
    // picture was rebuilt from; an unchanged entity means nothing on screen was touched.
    assert_ne!(
        before_entity, after_entity,
        "the sequence's turn was saved, but the piece on screen was never respawned, so the \
         picture still shows the old yaw. An agent's turn inside a sequence must reach the screen."
    );

    let (_, angle) = (after_rot * before_rot.inverse()).to_axis_angle();
    assert!(
        (angle.to_degrees() - 90.0).abs() < 0.5,
        "the piece turned {:.1} degrees, expected 90",
        angle.to_degrees()
    );
}
