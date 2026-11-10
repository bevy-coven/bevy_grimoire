//! **Look is a command, so a person can bind it to a key of their own** (AF-04, #360; series agent
//! follow, #356).
//!
//! The issue's own Decisions name three routes to Look and refuse a fourth: *"`F` is Fill in the Map
//! context, and `keys.rs`'s key budget (`no_context_gains_a_key`) is a protected test. Look is the
//! chip's button, a **Look** button on the AGENT block's newest-change line, and a palette command
//! `view.look_at_agent_change` (`command_run.rs`) with no default shortcut, which a person can bind in
//! the keymap screen."*
//!
//! The chip and the AGENT block were built; the command is what was left. It is the route that works
//! with **no chip on screen** — a person who toggled the follow off and dismissed the chip can still
//! say "show me what the agent changed" — and it is the one an agent can run, because
//! `grimoire/run_command` runs exactly what the chord would.
//!
//! # What is proved here
//!
//! 1. **The command is in the registry** under `view.look_at_agent_change`, with a census row of its
//!    own (so `keys::binding` answers for it and the keymap screen offers it for rebinding).
//! 2. **Running it frames the change the follow is holding back**, with View ▸ Follow the agent off,
//!    where nothing else moves the camera — measured by projecting the placed piece's own corners
//!    through the real `MainCamera`, not by reading a flag.
//! 3. **The chip is still there to say what waits**, so the command is an addition rather than a
//!    replacement for the person's other two routes.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::harness;
use bevy_cauldron::keys::{self, Action};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::settings::EditorSettings;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

/// The command's id, as the palette, a keymap file and an agent all name it.
const LOOK: &str = "view.look_at_agent_change";

/// One frame of the test clock, seconds: 60 Hz.
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
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(-28.0, 0.0, -28.0);
    app.world_mut().resource_mut::<Rig>().height = 4.0;
    for _ in 0..3 {
        app.update();
    }
}

/// The corners of a placement's own footprint, from the map and the project — the measure
/// `the_follow_waits_for_a_person_using_the_view` uses, so "it was framed" cannot pass on a camera
/// that stayed on the whole map.
fn footprint_corners(app: &App, id: &str) -> Option<[Vec3; 4]> {
    let open = app.world().resource::<OpenMap>();
    let project = app.world().resource::<Project>();
    let row = open.map.placements.iter().find(|p| p.id == id)?;
    let descriptor = project.library.get(&row.descriptor)?;
    let (w, d) = bevy_grimoire_core::descriptor::placed_footprint(descriptor)?;
    let origin = open.map.origin;
    let (hw, hd) = (w * 0.5, d * 0.5);
    Some([
        Vec3::new(origin.0 + row.at.0 - hw, origin.1, origin.2 + row.at.1 - hd),
        Vec3::new(origin.0 + row.at.0 + hw, origin.1, origin.2 + row.at.1 - hd),
        Vec3::new(origin.0 + row.at.0 + hw, origin.1, origin.2 + row.at.1 + hd),
        Vec3::new(origin.0 + row.at.0 - hw, origin.1, origin.2 + row.at.1 + hd),
    ])
}

/// How much of the viewport's shorter side the change's own box fills, `0..=1`.
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

/// The id of the placement an apply made, read from the map rather than from the answer.
fn the_one_placed(app: &App) -> String {
    let open = app.world().resource::<OpenMap>();
    let found: Vec<&str> = open
        .map
        .placements
        .iter()
        .map(|p| p.id.as_str())
        .filter(|id| *id != "bed@0")
        .collect();
    assert_eq!(found.len(), 1, "one piece was placed: {found:?}");
    found[0].to_owned()
}

/// **1. The command exists, in the registry and in the key census.**
#[test]
fn look_is_a_registered_command_with_a_census_row_of_its_own() {
    let command = bevy_cauldron::commands::by_id(LOOK)
        .unwrap_or_else(|| panic!("`{LOOK}` is not a command, so nothing can run it or bind it"));
    assert_eq!(
        command.action,
        Action::LookAtAgentChange,
        "the id names the action the census has a row for"
    );
    assert!(!command.title.trim().is_empty(), "a palette row needs words");
    // **Its own row, not another action's.** `binding` falls back to `BINDINGS[0]`, which is
    // indistinguishable from a real answer by chord and description, so the action is compared.
    assert_eq!(
        keys::binding(command.action).action,
        command.action,
        "the census has no row for {LOOK}, so `keys::binding` fell back to another action's"
    );
    // **The keymap screen offers it**, which is the route the issue names for a person who wants a
    // key: every command of the registry is a row there, so being registered is being bindable.
    assert!(
        bevy_cauldron::commands::all().iter().any(|c| c.id == LOOK),
        "the keymap screen and the palette list `commands::all()`, so a command outside it is one \
         nobody can bind"
    );
}

/// **2. Running it frames what the follow is holding back, with the toggle off.**
///
/// With View ▸ Follow the agent off, nothing else moves the camera — so the only thing that can have
/// framed the change is the command. The chip is read as well, because the command must not have
/// thrown away what was waiting for the other two routes.
#[test]
fn running_look_frames_the_change_the_follow_is_holding_back() {
    let root = room("look_command");
    let mut app = editor(&root);
    // **The person's setting**, written where the editor reads it: the follow never moves on its own.
    let settings = app.world().resource::<Project>().project_dir.join("editor.ron");
    std::fs::write(&settings, "(version: 1, follow_agent: Some(false))").expect("editor.ron");
    for _ in 0..frames_of(1.5) {
        app.update();
    }
    assert!(
        !app.world().resource::<EditorSettings>().follow_agent,
        "the setting did not reach the running editor, so this would prove nothing about Look"
    );

    point_away(&mut app);
    let before = rig(&app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = the_one_placed(&app);
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_IDLE_S) {
        app.update();
    }
    assert_eq!(rig(&app), before, "the follow moved the camera with Follow the agent off");
    assert!(
        app.world()
            .resource::<bevy_cauldron::follow::LookChip>()
            .sentence()
            .is_some(),
        "the chip says what is waiting, and the command is the other route to it"
    );

    // **The command, run the way the palette and an agent run it**: by id, through the chord the
    // dispatcher answers to.
    let ran = call(&mut app, "grimoire/run_command", serde_json::json!({"id": LOOK}));
    assert_eq!(ran["outcome"], "ran", "{ran}");
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S + FRAME) {
        app.update();
    }
    let corners = footprint_corners(&app, &placed).expect("the placed bed measures");
    let filled = fill(&mut app, &corners);
    assert!(
        filled >= FRAMED_MIN_FILL,
        "Look did not frame the change: it fills {:.0}% of the viewport",
        filled * 100.0
    );
}
