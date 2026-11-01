//! **An agent frames the whole map, a placement or a rectangle, and the frame is really in view.**
//!
//! The Map opened on a close-up of the origin, so on a 1120 m town an agent could not see the whole
//! of it, nor move the camera at all (backlog E-017: a review had to inject wheel events). The
//! `grimoire/frame_view` BRP method (MCP `grimoire_frame_view`) frames the map, the locked placement, a
//! placement by id, or a rectangle, through `view::frame_box`. Framing the whole map also needed the
//! zoom ceiling to allow it: `view::max_zoom` was 1.25 x the longer side, which under the 45-degree
//! iso view and a viewport narrower than tall is too low to see the map whole.
//!
//! The assertions read the camera the way the projection does: each corner of a box is taken into
//! the camera's own space and must fall inside the orthographic area that the camera's projection
//! reports. Nothing here trusts the framing code's own arithmetic.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

fn editor(tag: &str, side: f32) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .place("floor", (300.0, 200.0))
        .side(side)
        .build("m");
    booted(&root)
}

/// The floor, and a 50 m tower placed at `at`: a piece whose height is what a frame has to hold.
fn editor_with_tower(tag: &str, at: (f32, f32)) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .tall_descriptor("tower", "alpha", 50.0)
        .place("floor", (300.0, 200.0))
        .place("tower", at)
        .side(1120.0)
        .build("m");
    booted(&root)
}

fn booted(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    app
}

/// Through the registered method, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// The eight corners of a world-space box.
fn corners(centre: Vec3, size: Vec3) -> [Vec3; 8] {
    let half = size * 0.5;
    let mut out = [Vec3::ZERO; 8];
    let mut i = 0;
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                out[i] = centre + half * Vec3::new(sx, sy, sz);
                i += 1;
            }
        }
    }
    out
}

/// Where a box lands in the MainCamera's own orthographic area: the fraction of the area's width
/// and height the box fills, and whether all eight corners are inside the area.
struct Seen {
    inside: bool,
    fill: f32,
}

fn seen(app: &mut App, centre: Vec3, size: Vec3) -> Seen {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<MainCamera>>();
    let (tf, proj) = q.single(world).expect("the editor has one main camera");
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic")
    };
    let area = ortho.area;
    assert!(
        area.width() > 0.0 && area.height() > 0.0,
        "the camera reports an empty orthographic area {area:?}, so nothing can be framed"
    );
    let inverse = tf.affine().inverse();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for corner in corners(centre, size) {
        let local = inverse.transform_point3(corner);
        lo = lo.min(local.xy());
        hi = hi.max(local.xy());
    }
    let inside =
        lo.x >= area.min.x && hi.x <= area.max.x && lo.y >= area.min.y && hi.y <= area.max.y;
    let fill = ((hi.x - lo.x) / area.width()).max((hi.y - lo.y) / area.height());
    Seen { inside, fill }
}

/// Ask for the whole map through the agent's method, then let the camera settle.
fn frame_the_map(app: &mut App) -> serde_json::Value {
    let answer = call(
        app,
        "grimoire/frame_view",
        serde_json::json!({"target": "map"}),
    );
    app.update();
    app.update();
    answer
}

/// **Framing the map puts every corner of the 1120 m map in view, and the map fills most of it.**
/// Failed before the change twice over: `grimoire/frame_view` did not exist, and with it in place the
/// old zoom ceiling (1400 m) held the view below the ~1880 m the map needs, so corners stayed out.
#[test]
fn framing_the_map_shows_the_whole_map() {
    let mut app = editor("frame-map", 1120.0);
    let (centre, size) = bevy_cauldron::editor::map_bounds(app.world().resource::<OpenMap>());
    // The Map now opens framed on the whole map (E-022). This test was written against the 18 m
    // close-up the Map used to open on, so it starts the view there; the call below must do the framing.
    app.world_mut()
        .resource_mut::<bevy_cauldron::view::Rig>()
        .height = 18.0;
    app.update();
    app.update();
    let before = seen(&mut app, centre, size);
    assert!(
        !before.inside,
        "the map is already framed before the call, so the test proves nothing"
    );

    let answer = frame_the_map(&mut app);
    assert_eq!(answer["outcome"], "framed", "{answer}");

    let after = seen(&mut app, centre, size);
    assert!(
        after.inside,
        "after framing, a corner of the 1120 m map is still outside the camera's view"
    );
    assert!(
        after.fill >= 0.6,
        "after framing the map fills only {:.0}% of the view's limiting dimension, so it is visible \
         but not framed",
        after.fill * 100.0
    );
}

/// **The person is told why the view moved**: an agent's frame leaves a receipt naming what was
/// framed and that an agent asked.
#[test]
fn an_agents_frame_says_what_it_framed() {
    let mut app = editor("frame-receipt", 1120.0);
    frame_the_map(&mut app);
    let state = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>();
    let said = state.status.note_text();
    assert!(
        said.contains("agent") && said.contains("whole map") && said.contains("1120"),
        "the receipt does not say what was framed: {said:?}"
    );
}

/// **An agent frames one placement by id, and the placement's own box is in view.** Failed before the
/// change: `grimoire/frame_view` was not registered, so the call could not reach the camera at all.
#[test]
fn an_agent_frames_one_placement_by_id() {
    let mut app = editor("frame-placement", 1120.0);
    let (centre, size) = {
        let project = app.world().resource::<Project>();
        let open = app.world().resource::<OpenMap>();
        bevy_cauldron::editor::placement_bounds(project, open, 0)
            .expect("the placed floor has a measured box, so it can be framed")
    };
    let id = app.world().resource::<OpenMap>().map.placements[0]
        .id
        .clone();

    let answer = call(
        &mut app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"placements": [id.clone()]}}),
    );
    assert_eq!(answer["outcome"], "framed", "{answer}");
    app.update();
    app.update();

    let got = seen(&mut app, centre, size);
    assert!(
        got.inside,
        "the placement `{id}` is not inside the framed view: {answer}"
    );
    assert!(
        got.fill >= 0.6,
        "the placement `{id}` fills only {:.0}% of the view, so it is visible but not framed: {answer}",
        got.fill * 100.0
    );
}

/// **An unknown id is refused, and the camera does not move.** Failed before the change: the method did
/// not exist.
#[test]
fn an_unknown_placement_is_refused_and_nothing_moves() {
    let mut app = editor("frame-refused", 1120.0);
    let before = {
        let rig = app.world().resource::<Rig>();
        (rig.focus, rig.height)
    };
    let answer = call(
        &mut app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"placements": ["no-such-piece@99"]}}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["why"].as_str().unwrap_or_default();
    assert!(
        why.contains("no-such-piece@99"),
        "the refusal does not name the id it could not find, so an agent cannot fix the call: {answer}"
    );
    let rig = app.world().resource::<Rig>();
    assert_eq!(
        (rig.focus, rig.height),
        before,
        "a refused frame moved the camera"
    );
}

/// **A degenerate rectangle is refused, not framed as a point.** Failed before the change: no method.
#[test]
fn a_flat_rectangle_is_refused() {
    let mut app = editor("frame-flat", 1120.0);
    let answer = call(
        &mut app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"rect": [5.0, 5.0, 5.0, 50.0]}}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
}

/// **A rectangle is framed in map space, and the box it names is in view.** Failed before the change: no
/// method.
#[test]
fn an_agent_frames_a_rectangle_in_map_space() {
    let mut app = editor("frame-rect", 1120.0);
    let answer = call(
        &mut app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"rect": [200.0, 100.0, 400.0, 300.0]}}),
    );
    assert_eq!(answer["outcome"], "framed", "{answer}");
    app.update();
    app.update();
    let (pc, ps) = {
        let project = app.world().resource::<Project>();
        let open = app.world().resource::<OpenMap>();
        bevy_cauldron::editor::placement_bounds(project, open, 0)
            .expect("the placed floor has a measured box, so it can be framed")
    };
    let open = app.world().resource::<OpenMap>();
    let o = open.map.origin;
    // The rectangle's own ground is its footprint, and what stands in it is the piece's own box: its
    // centre height and its height (cycle-3 item 1, reviewer gap 3). The map's height is not the
    // rectangle's.
    let centre = Vec3::new(o.0 + 300.0, pc.y, o.2 + 200.0);
    let size = Vec3::new(200.0, ps.y, 200.0);
    let got = seen(&mut app, centre, size);
    assert!(
        got.inside,
        "the rectangle is not inside the framed view: {answer}"
    );
    assert!(
        got.fill >= 0.6,
        "the rectangle fills only {:.0}%: {answer}",
        got.fill * 100.0
    );
}

/// Frame a rectangle through the agent's method, and let the camera settle. Returns the answer.
fn frame_rect(app: &mut App, rect: [f32; 4]) -> serde_json::Value {
    let answer = call(
        app,
        "grimoire/frame_view",
        serde_json::json!({"target": {"rect": rect}}),
    );
    assert_eq!(answer["outcome"], "framed", "{answer}");
    app.update();
    app.update();
    answer
}

/// **A tall piece outside a rectangle does not raise the rectangle's frame.** A rectangle is ground, so
/// what stands in it is the height, and a 50 m tower beside it must leave the view where the plain
/// rectangle put it. This was already true when the test was written; the test pins it.
#[test]
fn a_tall_piece_outside_the_rectangle_does_not_raise_its_frame() {
    let rect = [200.0, 100.0, 400.0, 300.0];
    let plain = frame_rect(&mut editor("frame-rect-plain", 1120.0), rect);
    let with = frame_rect(
        &mut editor_with_tower("frame-rect-tower", (800.0, 800.0)),
        rect,
    );
    assert_eq!(
        plain["height_m"], with["height_m"],
        "a tower outside the rectangle changed its frame's height: plain {plain}, with tower {with}"
    );
    assert_eq!(
        plain["focus"], with["focus"],
        "a tower outside the rectangle moved its frame's focus: plain {plain}, with tower {with}"
    );
}

/// **A piece that straddles a rectangle's edge is framed at its full height.** The partial-overlap rule
/// counts a piece that touches the rectangle, so the frame must enclose all of it, not only the part
/// inside. This was already true when the test was written; the test pins it.
#[test]
fn a_piece_straddling_the_rectangle_edge_is_framed_at_full_height() {
    let mut app = editor_with_tower("frame-rect-straddle", (400.0, 200.0));
    frame_rect(&mut app, [200.0, 100.0, 400.0, 300.0]);
    let (centre, size) = {
        let project = app.world().resource::<Project>();
        let open = app.world().resource::<OpenMap>();
        bevy_cauldron::editor::placement_bounds(project, open, 1)
            .expect("the tower has a measured box, so it can be framed")
    };
    let got = seen(&mut app, centre, size);
    assert!(
        got.inside,
        "the straddling tower's full box is not inside the framed rectangle's view"
    );
}
