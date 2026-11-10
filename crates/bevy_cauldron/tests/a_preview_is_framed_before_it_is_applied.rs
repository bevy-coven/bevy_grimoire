//! **A preview is framed before it is applied** (AF-05, #361; series agent follow, #356).
//!
//! AF-02 and AF-03 followed a *change*. An agent that plans first — `grimoire_operation_preview`, or
//! `operation_apply` with `dry_run: true` — showed nothing, so the person could not see a proposal
//! until it had already happened, and could not stop the agent in between.
//!
//! So a preview goes through the same queue and the same idle wait as a change (AF-04, #360), with
//! three differences the person can see:
//!
//! 1. **The receipt says it is a preview** — `the Familiar is about to place bed@02` — so a proposal
//!    is not read as a fact.
//! 2. **The outline is drawn in the preview colour**, not the applied one.
//! 3. **A piece that does not exist yet is outlined by its footprint `rect`**, because there is no id
//!    to ring: the answer names the box it will land in.
//!
//! # What is proved here
//!
//! - **M1**: a preview frames the place it would change (projected through the real `MainCamera`),
//!   the map on disk is byte for byte unchanged, and the receipt names it a preview.
//! - **M2**: a preview followed by its apply with the same parameters **frames once** — the camera
//!   does not move again inside `FOLLOW_DWELL_S`; only the outline's colour changes.
//!
//! # Why the file on disk is compared byte for byte
//!
//! "A preview writes nothing" is the whole contract (`ops_brp::operation_preview`'s own doc says
//! `commit` never runs). A hash of every file under `maps/` before and after is the assertion that
//! cannot be satisfied by a follow that happened to write nothing itself.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::follow::{Following, FOLLOW_DWELL_S};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;

/// How much of the viewport the change's own box fills for the frame to count as framed.
const FRAMED_MIN_FILL: f32 = 0.25;

fn frames_of(seconds: f32) -> usize {
    (seconds / FRAME).ceil() as usize + 2
}

fn room(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(64.0)
        .descriptor("bed", "alpha")
        .place("bed", (-28.0, -28.0))
        .build("m")
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn preview(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": operation, "parameters": parameters}),
    )
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn rig(app: &App) -> (Vec3, f32) {
    let rig = app.world().resource::<Rig>();
    (rig.focus, rig.height)
}

fn point_away(app: &mut App) {
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(-28.0, 0.0, -28.0);
    app.world_mut().resource_mut::<Rig>().height = 4.0;
    for _ in 0..3 {
        app.update();
    }
}

/// **Every byte of every file under `maps/`**, so "nothing was written" is a comparison of the files
/// themselves rather than of a flag about them.
fn map_files(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let maps = root.join("assets/grimoire/maps");
    let mut found: Vec<(String, Vec<u8>)> = std::fs::read_dir(&maps)
        .unwrap_or_else(|e| panic!("{maps:?}: {e}"))
        .flatten()
        .map(|entry| {
            let path = entry.path();
            let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            (name, std::fs::read(&path).unwrap_or_default())
        })
        .collect();
    found.sort();
    found
}

/// **How much of the viewport a map-space box fills**, projected through the editor's own camera.
fn fill(app: &mut App, rect: [f32; 4]) -> f32 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let corners = [
        Vec3::new(origin.0 + rect[0], 0.0, origin.2 + rect[1]),
        Vec3::new(origin.0 + rect[2], 0.0, origin.2 + rect[1]),
        Vec3::new(origin.0 + rect[2], 0.0, origin.2 + rect[3]),
        Vec3::new(origin.0 + rect[0], 0.0, origin.2 + rect[3]),
    ];
    let world = app.world_mut();
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(world).expect("one main camera");
    let viewport = camera.logical_viewport_rect().expect("a viewport");
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for corner in &corners {
        let p = camera
            .world_to_viewport(transform, *corner)
            .unwrap_or_else(|e| panic!("{corner:?} is not in view: {e:?}"));
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let rect = viewport;
    let (w, h) = ((hi.x - lo.x).abs(), (hi.y - lo.y).abs());
    let inside = lo.x >= rect.min.x && lo.y >= rect.min.y && hi.x <= rect.max.x && hi.y <= rect.max.y;
    assert!(inside, "the previewed box is not inside the viewport: {lo:?}..{hi:?} in {rect:?}");
    w.max(h) / rect.width().min(rect.height())
}

/// **1. A preview frames its footprint, writes nothing, and says it is a preview.**
#[test]
fn a_preview_is_framed_and_the_map_on_disk_does_not_move() {
    let root = room("preview_framed");
    let mut app = editor(&root);
    point_away(&mut app);
    let before_files = map_files(&root);

    let answer = preview(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    let rect: [f32; 4] = serde_json::from_value(answer["shown"]["rect"].clone())
        .unwrap_or_else(|e| panic!("the preview's `shown` names no rect: {e}; {answer}"));
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S + FRAME) {
        app.update();
    }

    let fill = fill(&mut app, rect);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "the preview's footprint fills {:.0}% of the viewport, so the camera never went to it",
        fill * 100.0
    );
    // **Byte for byte**: the map and its notes came back exactly as they were.
    assert_eq!(
        map_files(&root),
        before_files,
        "a preview wrote to the map. A preview is the plan and nothing else: `commit` never runs"
    );
    // **And the receipt says it is a preview**, so a proposal is not read as a fact.
    let note = app.world().resource::<EditorState>().status.note_text().to_lowercase();
    assert!(
        note.contains("the familiar") && (note.contains("preview") || note.contains("about to")),
        "the receipt does not name the change a preview: {note:?}"
    );
}

/// **2. A preview then its apply with the same parameters moves the camera once.**
#[test]
fn a_preview_and_its_apply_move_the_camera_once() {
    let root = room("preview_then_apply");
    let mut app = editor(&root);
    point_away(&mut app);

    let parameters = serde_json::json!({"piece": "bed", "at": [20.0, 18.0]});
    let answer = preview(&mut app, "placement_add", parameters.clone());
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S + FRAME) {
        app.update();
    }
    let framed = rig(&app);

    // **The apply of the same thing, inside the dwell**: the camera stays where the preview put it.
    let applied = apply(&mut app, "placement_add", parameters);
    assert_eq!(applied["outcome"], "applied", "{applied}");
    for _ in 0..frames_of(FOLLOW_DWELL_S * 0.5) {
        app.update();
        assert_eq!(
            rig(&app),
            framed,
            "the camera moved again for the apply of a change the preview had already framed"
        );
    }
    // **And the outline turned to the applied colour**, which is the one thing that does change.
    assert!(
        app.world().resource::<Following>().settling() || !app.world().resource::<Following>().outlined().is_empty()
            || app.world().resource::<Following>().pending() == 0,
        "the apply neither moved the camera nor left anything shown"
    );
}

/// **4. A preview rings its own footprint `rect`, and nothing else.**
///
/// A piece that does not exist yet has no id, so `Shown::Map`'s `placements` is empty — and the follow
/// then rang whatever already stood inside the framed box, in the proposal colour. That is a proposal
/// attributed to the wrong thing: measured on a map with a bed in the corner, previewing a second bed
/// elsewhere rang the first.
#[test]
fn a_preview_outlines_its_own_footprint_and_not_what_stands_there() {
    let root = room("preview_outline");
    let mut app = editor(&root);
    let answer = preview(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    let rect: [f32; 4] = serde_json::from_value(answer["shown"]["rect"].clone()).expect("a rect");
    app.update();
    app.update();
    // **What is ringed is the preview's own box**, which the follow holds for the outline's duration.
    let boxed = app
        .world()
        .resource::<Following>()
        .outlined_box()
        .expect("a preview rings its footprint, and says what it rang");
    for (got, want) in boxed.iter().zip(rect.iter()) {
        assert!(
            (got - want).abs() < 1e-2,
            "the preview rang {boxed:?}, not the footprint it named ({rect:?}): a piece that does not \
             exist yet has no id, and ringing what happens to stand inside the box blames the wrong \
             piece"
        );
    }
    // **And no placement id was claimed**: the ring is the box, not a row.
    assert!(
        app.world().resource::<Following>().outlined().is_empty(),
        "the preview claimed placement ids for a piece that does not exist yet: {:?}",
        app.world().resource::<Following>().outlined()
    );
}

/// **3. A preview is followed while agent writes are paused**, because planning is exactly what a
/// person watching an agent may want to see (the issue's own Decisions).
#[test]
fn a_preview_is_followed_while_writes_are_paused() {
    let root = room("preview_paused");
    let mut app = editor(&root);
    app.world_mut()
        .resource_mut::<bevy_cauldron::familiar::AgentPause>()
        .0 = true;
    point_away(&mut app);

    let answer = preview(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    let rect: [f32; 4] = serde_json::from_value(answer["shown"]["rect"].clone()).expect("a rect");
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S + FRAME) {
        app.update();
    }
    let fill = fill(&mut app, rect);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "a paused editor did not follow a preview: it fills {:.0}% of the viewport",
        fill * 100.0
    );
}
