//! **A person's own map change does not move the camera, even through `dispatch`** (#358's review,
//! must-fix B), **and a follow never outlives the map it was made for** (nits f and g).
//!
//! # Why a second file
//!
//! `the_editor_follows_an_agents_change.rs` proves a person's click places a piece and the camera does
//! not move. **A click never goes through `ops::dispatch`** — the map's own click path writes the
//! placement directly — so that test would still pass if the follow were raised inside `dispatch`,
//! which is the one place the decision forbids. The rule is *"the request is raised in the two BRP
//! handlers an agent comes through and nowhere else"*, and the only way to test it is with a person's
//! change that **does** go through `dispatch`.
//!
//! The inspector is exactly that: `inspector_view::pose_edits_are_operations` builds a `Ctx` with
//! `Source::Human` and calls `ops::dispatch`. So a person turning a piece in the inspector is a
//! dispatch that must leave the camera alone and queue no follow.
//!
//! # What is proved here
//!
//! 1. **An inspector edit — a `Source::Human` dispatch — leaves the rig where it was and queues no
//!    follow.**
//! 2. **A person's own key press takes the camera back**: a follow already queued is dropped and the
//!    camera stays exactly where their eyes left it. This is the rule the gate found broken by #358
//!    (`a_person_removes_a_piece_and_undoes_it::an_agents_undo_of_a_removal_waits_while_the_person_has_moved_a_piece_since`):
//!    a follow queued by an agent's change was carried out on the frame the person pressed `B`, so the
//!    pixels they then clicked were a frame stale.
//! 3. **A new map clears the ease and the ring**, which name places and row ids of the map they were
//!    made on, and keeps the queue.
//! 4. **A follow that cannot be carried out does not ask for the Map every frame**: the request is
//!    asked for once, and dropped when the answer did not cross.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::follow::{FollowRequest, Following};
use bevy_cauldron::harness;
use bevy_cauldron::inspector_view::{PoseInspector, PoseView};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::Rig;

use crate::fixtures;

/// **One frame of the test clock**, seconds. A headless frame is about a millisecond of real time, so
/// a loop counts frames only with this clock behind it.
const FRAME: f32 = 1.0 / 60.0;

/// **A room with one piece and a camera pointed at a corner of it.**
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

/// **The camera away from the middle**, so a follow would visibly move it.
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

/// **Put the camera where it can see one small piece**, which is what a follow would leave.
fn frame_a_piece(app: &mut App, id: &str) {
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("`{id}` is on the map"))
        .at;
    let origin = app.world().resource::<OpenMap>().map.origin;
    let mut rig = app.world_mut().resource_mut::<Rig>();
    rig.focus = Vec3::new(origin.0 + placed.0, 0.0, origin.2 + placed.1);
    rig.height = 3.0;
}

/// **Put a piece on the map through one human dispatch**, and answer its id.
///
/// **The dispatch is real**: a `Ctx` whose `source` is `Source::Human`, calling `ops::dispatch` — the
/// same call the inspector makes and the same one a click's path does not. What it proves is that a
/// human change reaching `dispatch` raises no follow.
fn place(app: &mut App, piece: &str, at: (f32, f32)) -> String {
    let mut ctx = bevy_cauldron::ops::Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        source: bevy_cauldron::ops::Source::Human,
        session: bevy_cauldron::ops::session_id(),
        ..bevy_cauldron::ops::Ctx::default()
    };
    let applied = bevy_cauldron::ops::dispatch(
        &mut ctx,
        "placement_add",
        &serde_json::json!({"piece": piece, "at": [at.0, at.1]}),
    );
    assert!(
        matches!(applied, Ok(bevy_cauldron::ops::Applied::Done { .. })),
        "the human placement applies"
    );
    let fresh = ctx.open.take().expect("the dispatch gives the map back");
    let id = fresh.map.placements.last().expect("a placement").id.clone();
    app.insert_resource(fresh);
    id
}

/// **1. A person's change through `dispatch` moves the camera nowhere and queues no follow.**
///
/// **This is the rule the decision states**, and the one a click cannot test: the inspector's pose
/// edit is a real `ops::dispatch` with `Source::Human`, so raising the follow inside `dispatch` would
/// move the camera here and fail. The assertion is the rig *and* the queue, because a follow raised
/// and then dropped would move nothing on this frame and everything on the next.
#[test]
fn a_persons_dispatch_change_leaves_the_camera_and_the_queue_alone() {
    let root = room("follow_human_dispatch");
    let mut app = editor(&root);
    let id = place(&mut app, "bed", (10.0, 10.0));
    frame_a_piece(&mut app, &id);
    point_away(&mut app);
    // **The camera on the piece it is not changing**, so a follow would have somewhere to go.
    let before = rig(&app);
    assert!(
        app.world().resource::<Following>().pending() == 0,
        "nothing has been asked of the camera yet"
    );

    // **The selection, then the inspector's own edit**: gooey writes the field and sends
    // `InspectorChange`, and `pose_edits_are_operations` dispatches `placement_set_pose` from a
    // `Ctx` whose `source` is `Source::Human`.
    assert!(
        bevy_cauldron::editor::lock_on_placement(app.world_mut(), &id),
        "the piece locks, which is what the inspector reads"
    );
    for _ in 0..3 {
        app.update();
    }
    let inspector = app
        .world()
        .get_resource::<PoseInspector>()
        .and_then(|slot| slot.0)
        .expect("the inspector has its own entity");
    {
        let mut view = app.world_mut().get_mut::<PoseView>(inspector).expect("the pose view");
        view.yaw = 90.0;
    }
    app.world_mut().trigger(bevy_gooey::InspectorChange {
        entity: inspector,
        path: "yaw".to_owned(),
    });
    app.update();
    app.update();

    // **The change happened**, so this is not a test of a refused edit.
    let turned = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.id == id)
        .expect("the bed is still on the map");
    assert_eq!(turned.yaw, 90.0, "the inspector's edit went through dispatch: {turned:?}");

    assert_eq!(
        rig(&app),
        before,
        "a person's own change moved the camera. The follow is raised in the BRP handlers an agent \
         comes through and nowhere else: a request inside `dispatch` moves the view under the hands \
         of the person whose eyes are already there."
    );
    assert_eq!(
        app.world().resource::<Following>().pending(),
        0,
        "a person's own change queued a follow. It may not move the camera this frame and still \
         move it on the next."
    );
}

/// **1b. A person's key press takes the camera back, and the queued follow is dropped.**
///
/// **This is #358's acceptance, "a person is never yanked", in the form the gate found broken.** An
/// agent's change raises a follow; the person then reaches for the keyboard, and the frame their press
/// lands on used to be a frame the camera moved on — so the click after it aimed where things used to
/// be. Measured on the removal test: the rig went from height 17.06 (the whole-map frame) to 1.02 on
/// the `B` frame, and its two clicks then missed.
///
/// The assertion is the camera's identity, not a tolerance: a follow that eases a fraction of the way
/// and then stops is still a camera that moved under the person's hands.
#[test]
fn a_persons_key_press_drops_the_queued_follow_and_keeps_the_camera() {
    let root = room("follow_person_takes_it_back");
    let mut app = editor(&root);
    // **Travel on**, so a follow has an ease to be in flight and a person's press has something to
    // interrupt (the harness builds with motion off, which lands every follow in its own frame).
    app.insert_resource(bevy_gooey::MotionSettings::default());
    app.update();
    let id = place(&mut app, "bed", (20.0, 20.0));
    point_away(&mut app);
    let before = rig(&app);

    // **An agent's change**, through the same path the BRP handlers use.
    app.world_mut().resource_mut::<Following>().request(FollowRequest {
        target: bevy_cauldron::editor::FrameTarget::Placements(vec![id]),
        ids: Vec::new(),
    });
    assert_eq!(
        app.world().resource::<Following>().pending(),
        1,
        "the follow is queued, which is what the person's press has to drop"
    );

    // **The person presses a key.** `ButtonInput<KeyCode>` is what the editor's own keys read, so
    // this is the same event a keyboard makes.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyB);

    assert_eq!(
        rig(&app),
        before,
        "the person pressed a key and the camera moved on that frame: the follow is a kindness to \
         someone reading a panel, and it stops being one the moment they are working"
    );
    assert_eq!(
        app.world().resource::<Following>().pending(),
        0,
        "the person's press must drop the queued follow, or the camera is moved on the next frame \
         instead — which is the same yank, one frame later"
    );
}

/// **2. A new map clears the ease and the ring, and keeps the queue.**
///
/// **The ease's focus is in map metres and the height is measured against that map's bounds**, and the
/// ring's ids are rows of that map's library. Carried onto the next map they name a spot and a piece
/// that are not there, so the camera would ease to a place from another map. **The queue stays**: those
/// requests are still what the person asked to see, and each is resolved against the map open when its
/// turn comes.
///
/// **The other door is what makes the queue observable**: on `Door::Kit` a follow asks for the Map and
/// waits, so a queued request is still queued when the map changes. On the Map door the very same
/// frame would take it, and an empty queue would prove nothing either way.
#[test]
fn a_new_map_clears_the_ease_and_the_ring_and_keeps_the_queue() {
    let root = room("follow_new_map");
    // **A second map of the same project**, so the change is the map and not the project.
    fixtures::Fixture::write_map(&root, "n", 64.0, &[]);
    let mut app = editor(&root);
    // **Travel on**, which is the ordinary setting: the harness builds with
    // `MotionSettings::off()`, and a reduced-motion follow lands in its own frame, so there would be
    // no ease to carry onto the next map and nothing for this test to prove.
    app.insert_resource(bevy_gooey::MotionSettings::default());
    app.update();
    let id = place(&mut app, "bed", (12.0, 12.0));

    // **A follow carried out**: the camera is on its way and the change is ringed.
    app.world_mut().resource_mut::<Following>().request(FollowRequest {
        target: bevy_cauldron::editor::FrameTarget::Placements(vec![id.clone()]),
        ids: vec![id.clone()],
    });
    app.update();
    assert!(
        app.world().resource::<Following>().settling(),
        "the ease must be in flight before the map changes, or this test proves nothing"
    );
    assert_eq!(
        app.world().resource::<Following>().outlined(),
        std::slice::from_ref(&id),
        "the change must be ringed before the map changes, or this test proves nothing"
    );

    // **Now on another door**, where a follow cannot be carried out and so stays queued.
    app.insert_resource(bevy_cauldron::tiles::Door::Kit);
    app.world_mut().resource_mut::<Following>().request(FollowRequest {
        target: bevy_cauldron::editor::FrameTarget::Map,
        ids: Vec::new(),
    });
    assert_eq!(app.world().resource::<Following>().pending(), 1, "one follow waits");

    // **Another map opens.** `map_open` is the dispatch a crossing makes, and what the follow reads
    // is the `OpenMap` resource, so it is replaced the way the operation leaves it.
    let mut ctx = bevy_cauldron::ops::Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        source: bevy_cauldron::ops::Source::Human,
        session: bevy_cauldron::ops::session_id(),
        ..bevy_cauldron::ops::Ctx::default()
    };
    let opened = bevy_cauldron::ops::dispatch(&mut ctx, "map_open", &serde_json::json!({"name": "n"}));
    assert!(
        matches!(opened, Ok(bevy_cauldron::ops::Applied::Done { .. })),
        "the second map opens"
    );
    let fresh = ctx.open.take().expect("map_open gives the map back");
    assert_eq!(fresh.map.name, "n", "the open map is the second one");
    app.insert_resource(fresh);
    app.update();

    let following = app.world().resource::<Following>();
    assert!(
        !following.settling(),
        "the ease carried over from the previous map: its focus is a metre figure of that map and \
         its height was measured against that map's bounds"
    );
    assert!(
        following.outlined().is_empty(),
        "the ring carried over from the previous map: its ids are rows of that map's library, and \
         the new map has no such piece"
    );
    assert_eq!(
        following.pending(),
        1,
        "the queue is what the person asked to see, so a map change keeps it: each request is \
         resolved against the map open when its turn comes"
    );
}

/// **3. A follow that cannot reach the Map asks for it once and is then dropped.**
///
/// **The nit the review named**: `take_the_follow_request` asked for the Map on every frame while the
/// request waited, and nothing dropped the request when the answer did not come. That is a crossing
/// asked for by a camera move nobody is waiting on any more, and it never ends. The request is asked
/// for once; a frame that finds the door still not the Map drops it.
#[test]
fn a_follow_asks_for_the_map_once_and_is_dropped_if_it_does_not_cross() {
    let root = room("follow_once");
    let mut app = editor(&root);
    // **On another door**, which is where a follow has to ask for the Map.
    app.insert_resource(bevy_cauldron::tiles::Door::Kit);
    app.world_mut().resource_mut::<Following>().request(FollowRequest {
        target: bevy_cauldron::editor::FrameTarget::Map,
        ids: Vec::new(),
    });
    assert_eq!(app.world().resource::<Following>().pending(), 1, "one follow waits");

    // **The first frame asks**, and the queue is held while the crossing is in flight.
    app.update();
    let asked = app
        .world()
        .get_resource::<bevy_cauldron::editor::EditorState>()
        .and_then(|state| state.door_request);
    assert_eq!(
        asked.map(|r| r.door),
        Some(bevy_cauldron::tiles::Door::Map),
        "the follow did not ask the editor to open the Map"
    );
    assert_eq!(
        app.world().resource::<Following>().pending(),
        1,
        "the request is held while the crossing is in flight"
    );

    // **The crossing is taken away without crossing** — a refused crossing, a question that held it —
    // and the door is still not the Map. The request is dropped rather than asked for again.
    if let Some(mut state) = app.world_mut().get_resource_mut::<bevy_cauldron::editor::EditorState>() {
        state.door_request = None;
    }
    app.update();
    assert_eq!(
        app.world().resource::<Following>().pending(),
        0,
        "the follow is dropped when the Map did not open: asking again every frame would be a \
         crossing nobody is waiting on, and it would never end"
    );
    assert!(
        app.world()
            .get_resource::<bevy_cauldron::editor::EditorState>()
            .is_none_or(|state| state.door_request.is_none()),
        "the request was asked for again after the answer did not come"
    );
}
