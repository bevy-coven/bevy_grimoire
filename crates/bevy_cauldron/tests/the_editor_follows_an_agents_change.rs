//! **The editor frames an agent's map change** (AF-02, #358, series agent follow #356).
//!
//! AF-01 gave every operation an answer to *where its change is seen* (`Operation::shows` → `Shown`).
//! This is the reader: when an agent changes the map, the person's view goes to the change. A person's
//! own key does not move the camera — their eyes are already where they are working — so the request
//! is raised in the two BRP handlers an agent comes through and nowhere else.
//!
//! # What is proved here
//!
//! 1. **A placement over BRP, with the camera framed elsewhere, ends with every corner of the placed
//!    footprint inside the viewport.** The corners are read from the map and projected through the
//!    real `MainCamera` with `Camera::world_to_viewport`, so nothing here trusts the framing
//!    arithmetic — `frame_box` is not called by this file.
//! 2. **The same change made by a person does not move the camera.** A click that places a piece is
//!    the person's edit; the view stays where they left it and no follow is queued.
//! 3. **An agent's undo frames the place the undone change was.**
//! 4. **`reduced_motion` jumps**: the camera is there on the next frame, with no ease to wait out.
//! 5. **A change from another door opens the Map first**, and the camera frames it after the crossing.
//! 6. **The receipt names the change in the history's words** — `the Familiar placed …` — and the
//!    change's placements are outlined for `FOLLOW_OUTLINE_S` and then not.

use std::time::Duration;

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PrimaryWindow, Window};
use bevy_gooey::MotionSettings;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::follow::{FOLLOW_OUTLINE_S, Following};
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

/// **How much of the viewport a change framed as a change must fill.** A 1 m piece framed with
/// `FOLLOW_MARGIN_FRAC` fills most of it; the whole 64 m room leaves the piece a few percent. The
/// threshold sits well clear of both, so it fails for a whole-map frame and not for a slightly
/// different margin.
const FRAMED_MIN_FILL: f32 = 0.25;

/// **One frame of the test clock**, seconds: 60 Hz, so a count of frames is a count of the ease's
/// own seconds.
///
/// **A headless editor runs its frames as fast as it can** — measured, an `app.update()` with no
/// window costs about a millisecond — so 30 real frames are 3 % of a 0.35 s ease and an assertion
/// after them reads "the camera is still on its way" for a follow that works. Every test here that
/// waits for the ease steps this clock instead.
const FRAME: f32 = 1.0 / 60.0;

/// **How many frames of [`FRAME`] the ease takes**, and a couple over, so `for` loops are exact
/// rather than a number somebody guessed.
fn frames_of(seconds: f32) -> usize {
    (seconds / FRAME).ceil() as usize + 2
}

/// The fixture's own piece is 1.0 x 1.0 m, so a placement's floor box is half a metre each way. Read
/// from the map's row and the library's measurement rather than from any framing answer.
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

/// **Every corner of a box, projected through the editor's own camera, inside its viewport.** `Err`
/// carries the sentence a failing assertion prints, naming where the corner landed.
///
/// **The viewport is a rect, not a size.** `Camera::world_to_viewport` answers in
/// `logical_viewport_rect`'s own coordinates, which start at `rect.min` — the Map's viewport sits to
/// the right of the palette and below the menu bar, so its min is not the origin. Comparing against
/// `[0, size]` reads a correctly centred box as off-screen by exactly the dock's width; measured,
/// that is what it did.
fn seen(app: &mut App, corners: &[Vec3]) -> Result<(), String> {
    let world = app.world_mut();
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .single(world)
        .expect("the editor has one main camera");
    let rect = camera
        .logical_viewport_rect()
        .ok_or_else(|| "the Map camera has no viewport, so nothing can be asserted".to_owned())?;
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return Err(format!("the camera reports an empty viewport {rect:?}"));
    }
    let mut outside = Vec::new();
    for corner in corners {
        match camera.world_to_viewport(transform, *corner) {
            Ok(p) if p.x >= rect.min.x && p.y >= rect.min.y && p.x <= rect.max.x && p.y <= rect.max.y => {}
            Ok(p) => outside.push(format!("{corner:?} at {p:?}, outside {rect:?}")),
            Err(e) => outside.push(format!("{corner:?} is not in view: {e:?}")),
        }
    }
    if outside.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} corner(s) of the change are outside the viewport: {outside:?}",
            outside.len(),
        ))
    }
}

/// **How much of the viewport's shorter side the change's own box fills**, `0..=1`.
///
/// **The second half of "it was framed".** A whole-map frame contains every piece on it, so
/// `seen` alone passes for a follow that went nowhere near the change — measured: an undo of a place
/// answered `MapWhole`, the camera flew out to the whole 64 m room, and every corner of the piece was
/// still inside the viewport. This is what says the view is *on* the change rather than somewhere
/// it happens to be in.
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
    let (w, h) = ((hi.x - lo.x).abs(), (hi.y - lo.y).abs());
    w.max(h) / rect.width().min(rect.height())
}

/// **A 64 m map with one piece in a corner**, so a change in the far corner is plainly elsewhere.
fn room(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(64.0)
        .descriptor("bed", "alpha")
        .place("bed", (-28.0, -28.0))
        .build("m")
}

/// **The editor with the agent's door open and a clock the test steps.** The clock is the point:
/// the follow eases against `Time<Real>`, and a real-clock headless run makes a frame about a
/// millisecond long, so a fixed count of frames is not a fixed number of seconds.
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
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
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

/// **Reduced motion, set the way a person sets it.** `ui::follow_the_motion_setting` copies
/// `EditorSettings::reduced_motion` into `bevy_gooey::MotionSettings` on every frame, so a test that
/// writes the resource alone has it overwritten before the follow ever reads it.
fn reduce_motion(app: &mut App) {
    app.world_mut()
        .resource_mut::<bevy_cauldron::settings::EditorSettings>()
        .reduced_motion = true;
    app.update();
    assert!(
        app.world().resource::<MotionSettings>().lands_travel(),
        "the reduced-motion setting did not reach gooey's MotionSettings, so the test would prove \
         nothing about the jump"
    );
}

/// **The camera taken somewhere else**, so a follow that does nothing cannot pass.
fn point_away(app: &mut App) {
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(-28.0, 0.0, -28.0);
    app.world_mut().resource_mut::<Rig>().height = 4.0;
    for _ in 0..3 {
        app.update();
    }
}

fn rig(app: &App) -> (Vec3, f32) {
    let rig = app.world().resource::<Rig>();
    (rig.focus, rig.height)
}

/// **A placement over BRP, with the camera framed elsewhere, is framed in view.**
#[test]
fn an_agents_placement_is_framed_so_its_footprint_is_inside_the_viewport() {
    let root = room("follow_place");
    let mut app = editor(&root);
    point_away(&mut app);

    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.id.clone())
        .filter(|i| i != "bed@0")
        .collect::<Vec<_>>();
    assert_eq!(placed.len(), 1, "one piece was placed: {placed:?}");
    let corners = footprint_corners(&app, &placed[0]).expect("the placed bed measures");
    assert!(
        seen(&mut app, &corners).is_err(),
        "the camera already holds the new piece, so the test proves nothing about the follow"
    );

    // The ease lands inside the follow's own time.
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    if let Err(why) = seen(&mut app, &corners) {
        panic!("an agent placed a piece over BRP and the editor did not frame it: {why}");
    }
    let fill = fill(&mut app, &corners);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "the placed piece fills {:.0}% of the viewport, so the camera framed the map and not the \
         change",
        fill * 100.0
    );
}

/// **The same placement made by a person's key moves the camera nowhere.** A click on the map is the
/// person's own edit (`Source::Human`), and their eyes are already there.
#[test]
fn a_person_placing_a_piece_leaves_the_camera_where_it_was() {
    /// One frame's worth of person: the pointer and a mouse press or release.
    #[derive(Default)]
    struct Frame {
        pointer: Option<Vec2>,
        mouse: Option<bool>,
    }

    #[derive(Resource, Default)]
    struct Script {
        next: Option<Frame>,
    }

    fn drive(
        mut script: ResMut<Script>,
        mut mouse: ResMut<ButtonInput<MouseButton>>,
        mut windows: Query<&mut Window, With<PrimaryWindow>>,
    ) {
        let Some(frame) = script.next.take() else {
            return;
        };
        if let Ok(mut window) = windows.single_mut() {
            window.set_cursor_position(frame.pointer);
        }
        match frame.mouse {
            Some(true) => mouse.press(MouseButton::Left),
            Some(false) => mouse.release(MouseButton::Left),
            None => {}
        }
    }

    fn step(app: &mut App, frame: Frame) {
        app.world_mut().resource_mut::<Script>().next = Some(frame);
        app.update();
    }

    fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
        let origin = app.world().resource::<OpenMap>().map.origin;
        let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
        let (camera, transform) = cameras.single(app.world()).expect("one main camera");
        camera
            .world_to_viewport(transform, world)
            .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
    }

    let root = room("follow_person");
    let mut app = editor(&root);
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..6 {
        app.update();
    }
    // The piece is armed as a palette click arms it, so the click is the only thing under test.
    let index = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == "bed")
        .expect("the fixture defines bed");
    app.world_mut().resource_mut::<EditorState>().brush = Some(index);
    app.update();

    let before = rig(&app);
    let at = pixel_over(&mut app, (20.0, 18.0));
    step(&mut app, Frame { pointer: Some(at), mouse: Some(true) });
    step(&mut app, Frame { pointer: Some(at), mouse: Some(false) });
    // A whole ease's worth, so a follow that was wrongly raised had every chance to move the camera.
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }

    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .filter(|p| p.id != "bed@0")
        .count();
    assert_eq!(placed, 1, "the person's click placed one piece, so the test is about the camera");
    assert_eq!(
        rig(&app),
        before,
        "the person's own click moved the camera; the follow is the agent's, and their eyes are \
         already where they are working"
    );
    assert_eq!(
        app.world().resource::<Following>().pending(),
        0,
        "a person's click queued a follow, so the camera would move a frame later"
    );
}

/// **An agent's undo frames the place the undone change was.**
#[test]
fn an_agents_undo_frames_where_the_change_was() {
    let root = room("follow_undo");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let id = answer["operation_id"].as_str().expect("an id").to_owned();
    // Let the place's own follow land first, so the undo's is measured from a settled camera.
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.id.clone())
        .filter(|i| i != "bed@0")
        .collect::<Vec<_>>();
    let corners = footprint_corners(&app, &placed[0]).expect("the placed bed measures");

    // Somewhere else, so the undo has to bring the view back.
    point_away(&mut app);
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    );
    assert_eq!(undone["outcome"], "applied", "the undo did not apply: {undone}");
    assert!(
        seen(&mut app, &corners).is_err(),
        "the camera already holds where the change was, so the test proves nothing"
    );
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    if let Err(why) = seen(&mut app, &corners) {
        panic!("an agent undid a placement and the editor did not frame where it was: {why}");
    }
    // **And it framed the change, not the whole room.** A whole-map frame holds every piece by
    // definition; measured before this assertion existed, that is exactly what an undo of a place did.
    let fill = fill(&mut app, &corners);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "the undo framed the change at {:.0}% of the viewport, which is the whole map rather than \
         where the piece stood — the follow read no place from the undo's answer",
        fill * 100.0
    );
}

/// **With reduced motion on, the camera is there on the next frame.**
#[test]
fn reduced_motion_lands_the_frame_at_once() {
    let root = room("follow_reduced");
    let mut app = editor(&root);
    // **The person's setting, not the library's resource.** `ui::follow_the_motion_setting` copies
    // `EditorSettings::reduced_motion` into `bevy_gooey::MotionSettings` every frame, so a test that
    // writes the resource alone is overwritten before the follow reads it — measured: the ease ran
    // for its full 0.35 s while the resource said `reduced`.
    reduce_motion(&mut app);
    point_away(&mut app);

    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.id.clone())
        .filter(|i| i != "bed@0")
        .collect::<Vec<_>>();
    let corners = footprint_corners(&app, &placed[0]).expect("the placed bed measures");

    // One frame: the request is taken, and with reduced motion it lands rather than eases.
    app.update();
    if let Err(why) = seen(&mut app, &corners) {
        panic!("reduced motion still eased the follow, so the next frame was not there yet: {why}");
    }
    // **And it framed the change, not the whole room** (review of #358, nit e): a whole-map frame
    // holds every piece by definition, so `seen` alone would pass for a camera that jumped to the map
    // rather than to the change. The placement test asserts this; the reduced-motion one is the same
    // rule and needs the same check.
    let fill = fill(&mut app, &corners);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "reduced motion landed on the whole map rather than on the change: it fills {:.0}% of the \
         viewport",
        fill * 100.0
    );
}

/// **The ease is a real one, and it lands.** With motion on, the first frame of the follow is between
/// where the camera was and where it is going, and after `FOLLOW_EASE_S` it is exactly there.
#[test]
fn the_follow_eases_and_then_lands() {
    let root = room("follow_eases");
    let mut app = editor(&root);
    app.insert_resource(MotionSettings::default());
    app.update();
    point_away(&mut app);
    let before = rig(&app);

    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.id.clone())
        .filter(|i| i != "bed@0")
        .collect::<Vec<_>>();
    let corners = footprint_corners(&app, &placed[0]).expect("the placed bed measures");

    // Settle the target so the assertions are about the ease rather than the request being taken.
    app.update();
    let (first, _) = rig(&app);
    assert!(
        (first - before.0).length() > 0.0,
        "the follow did not start: the focus is still {first:?}"
    );
    // Partway through, the corner is not in view yet — the camera is on its way.
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) / 2 - 2 {
        app.update();
    }
    assert!(
        seen(&mut app, &corners).is_err(),
        "halfway through a {:.2} s ease the camera is already there, so nothing is easing",
        bevy_cauldron::follow::FOLLOW_EASE_S
    );
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    if let Err(why) = seen(&mut app, &corners) {
        panic!("the ease never landed: {why}");
    }
}

/// **A change from another door opens the Map first.** The follow is a camera request, and the camera
/// is the Map door's: from the Kit door the editor crosses, then frames.
#[test]
fn a_change_from_another_door_opens_the_map_and_frames_it() {
    let root = room("follow_door");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Kit,
        "the test starts on the Kit door, so a crossing to Map is visible"
    );

    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| p.id.clone())
        .filter(|i| i != "bed@0")
        .collect::<Vec<_>>();
    let corners = footprint_corners(&app, &placed[0]).expect("the placed bed measures");
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) * 4 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Map,
        "the editor stayed on the Kit door, so the person is looking at a panel and not the change"
    );
    if let Err(why) = seen(&mut app, &corners) {
        panic!("the editor crossed to the Map and still did not frame the change: {why}");
    }
}

/// **The receipt names the change in the history's words, and the placements are outlined for
/// `FOLLOW_OUTLINE_S`.**
#[test]
fn the_receipt_names_the_familiar_and_the_places_are_outlined() {
    let root = room("follow_receipt");
    let mut app = editor(&root);
    // The ease does not matter to this test; the ring's own clock does. Reduced motion keeps the
    // camera out of a count that is about the outline.
    reduce_motion(&mut app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().expect("a summary").to_owned();

    let note = app
        .world()
        .resource::<EditorState>()
        .status
        .note_text()
        .to_owned();
    assert!(
        note.contains("the Familiar ") && note.contains(&summary),
        "the receipt does not name the change in the history's words (`the Familiar` and what it \
         did): {note:?}"
    );

    // The outline: the changed rows are named while it lasts, and gone after FOLLOW_OUTLINE_S.
    let outlined = |app: &App| app.world().resource::<Following>().outlined().to_vec();
    let mut seen_outline = false;
    for _ in 0..frames_of(FOLLOW_OUTLINE_S) {
        app.update();
        let ids = outlined(&app);
        if !ids.is_empty() {
            assert!(
                ids.iter().any(|id| id.starts_with("bed@") && id != "bed@0"),
                "the outline names {ids:?}, which is not the row the change placed"
            );
            seen_outline = true;
        }
    }
    assert!(seen_outline, "no placement was ever outlined, so the follow drew nothing");
    for _ in 0..frames_of(FOLLOW_OUTLINE_S) {
        app.update();
    }
    assert!(
        outlined(&app).is_empty(),
        "the outline is still up after {} s: {:?}",
        FOLLOW_OUTLINE_S,
        outlined(&app)
    );
}
