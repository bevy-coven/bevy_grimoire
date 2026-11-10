//! **A layout the agent proposes is followed as a preview** (AF-05, #361; series agent follow, #356).
//!
//! AF-05 followed `grimoire/operation_preview` and `operation_apply` with `dry_run: true`. The third
//! way an agent proposes something is `layout_propose`: it writes nothing (its own `writes()` is
//! `&[]` and its `undo()` is `Undo::None`), and the layout stands on screen as a *proposal* until
//! `layout_accept` keeps it.
//!
//! It was followed as a **change**, so the person was shown a proposal in the applied colour and read
//! a receipt that did not say it was a proposal — the one difference AF-05 exists to make visible.
//!
//! # What is proved here
//!
//! 1. **The follow says it is showing a proposal**, not a fact.
//! 2. **The camera goes to the proposed area**, projected through the real `MainCamera`, so "it was
//!    framed" cannot pass on a camera that stayed where it was.
//! 3. **Nothing was written**: the map on disk is byte for byte what it was. A proposal becomes map
//!    data only when `layout_accept` keeps it.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::follow::{Following, Manner};
use bevy_cauldron::harness;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

/// One frame of the test clock, seconds: 60 Hz, as the follow's own tests step it.
const FRAME: f32 = 1.0 / 60.0;

/// How much of the viewport the proposed area's own box fills for the frame to count as framed.
const FRAMED_MIN_FILL: f32 = 0.25;

/// The area the proposal is laid in, `[x0, z0, x1, z1]` in map metres.
const AREA: [f32; 4] = [0.0, 0.0, 6.0, 6.0];

fn frames_of(seconds: f32) -> usize {
    (seconds / FRAME).ceil() as usize + 2
}

/// A 32 m map with a 4 x 4 block of pieces near the middle, which is what `learned` reads to lay a
/// layout — the same fixture `a_layout_proposed_in_an_area_leaves_the_rest_of_the_map` uses.
fn room(tag: &str) -> std::path::PathBuf {
    let mut fixture = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(32.0)
        .descriptor("wall", "alpha");
    for i in 0..4 {
        for j in 0..4 {
            fixture = fixture.place("wall", (0.5 + i as f32, 0.5 + j as f32));
        }
    }
    fixture.place("wall", (-5.5, -5.5)).build("m")
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    app.insert_resource(bevy_gooey::MotionSettings::default());
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
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(-14.0, 0.0, -14.0);
    app.world_mut().resource_mut::<Rig>().height = 2.0;
    for _ in 0..3 {
        app.update();
    }
}

/// **The four corners of the proposed area on the ground**, in the map's own frame: what a person
/// sees framed when the camera goes to it. Read from the map's origin, as `footprint_corners` reads a
/// placement's.
fn area_corners(app: &App) -> [Vec3; 4] {
    let origin = app.world().resource::<bevy_cauldron::project::OpenMap>().map.origin;
    [
        Vec3::new(origin.0 + AREA[0], origin.1, origin.2 + AREA[1]),
        Vec3::new(origin.0 + AREA[2], origin.1, origin.2 + AREA[1]),
        Vec3::new(origin.0 + AREA[2], origin.1, origin.2 + AREA[3]),
        Vec3::new(origin.0 + AREA[0], origin.1, origin.2 + AREA[3]),
    ]
}

/// How much of the viewport's shorter side the area's own box fills, `0..=1`.
fn fill(app: &mut App, corners: &[Vec3]) -> f32 {
    let world = app.world_mut();
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(world).expect("one main camera");
    let rect = camera.logical_viewport_rect().expect("a viewport");
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for corner in corners {
        let p = camera
            .world_to_viewport(transform, *corner)
            .unwrap_or_else(|e| panic!("{corner:?} is not in view: {e:?}"));
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (hi.x - lo.x).abs().max((hi.y - lo.y).abs()) / rect.width().min(rect.height())
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
            (
                path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                std::fs::read(&path).unwrap_or_default(),
            )
        })
        .collect();
    found.sort();
    found
}

/// **`layout_propose` is a proposal, so the follow draws it as one: the camera goes to the area it
/// would lay, the map on disk does not move, and the follow says it is showing a proposal.**
#[test]
fn a_layout_proposal_is_followed_as_a_preview() {
    let root = room("proposal_preview");
    let mut app = editor(&root);
    point_away(&mut app);
    let before = rig(&app);
    let before_files = map_files(&root);

    let answer = apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "learned", "seed": 7, "area": AREA}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S + FRAME) {
        app.update();
    }

    // **It is drawn as a proposal**, which is the only thing AF-05 makes visible: a layout that is
    // not map data yet must not wear the applied colour or read as something that happened.
    assert_eq!(
        app.world().resource::<Following>().shown_manner(),
        Manner::Preview,
        "`layout_propose` writes nothing, so the follow must say it is showing a proposal"
    );
    // **And the camera went to it**, measured through the real camera rather than read off the rig.
    let corners = area_corners(&app);
    let filled = fill(&mut app, &corners);
    assert!(
        filled >= FRAMED_MIN_FILL,
        "the proposed area fills {:.0}% of the viewport, so the camera never went to it",
        filled * 100.0
    );
    assert_ne!(rig(&app), before, "the camera did not move at all");
    // **Byte for byte**: a proposal becomes map data only when `layout_accept` keeps it.
    assert_eq!(
        map_files(&root),
        before_files,
        "`layout_propose` wrote to the map. Its own `writes()` is empty and its `undo()` is \
         `Undo::None`: the layout stands on screen as a proposal and nothing more"
    );
    // **And the receipt says it is a proposal**, in the operation's own words rather than a second
    // sentence written for the follow. The assertion is the `the Familiar is about to` prefix a
    // preview's receipt carries — `Gates::follow_a_preview` writes it and a change's receipt does not
    // — because every `layout_propose` answer already contains "proposed" in its own summary, so that
    // word would pass whether the follow were a preview or not.
    let note = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .note_text()
        .to_string();
    assert!(
        note.contains(&format!("{} is about to", bevy_cauldron::follow::FAMILIAR)),
        "the receipt does not say this is only about to happen: {note:?}"
    );

    // **And the ring is the proposal's footprint**, which is the preview path's own drawing: an
    // applied change rings the placement ids it names and leaves `outline_box` empty. Comparing the
    // box rather than reading the manner twice is what makes this a second, independent witness —
    // the manner could be `Preview` while the ring still resolved placements.
    assert_eq!(
        app.world().resource::<Following>().outlined_box(),
        Some(AREA),
        "the ring is not the proposed area's footprint, so the preview was drawn as an applied change"
    );
}
