//! **A map whose stacking does not resolve still opens framed on the whole map.**
//!
//! The regression check for B-018. `resolve_frame` resolved every placement's Y before it read the
//! target, so a map carrying one bad stack answered `Err` — and `MapFraming` put that in the status
//! line and left the camera at `Rig::default()`'s 18 m close-up. Opening such a map gave a person a
//! patch of ground at the origin and a toast about stacking, when the whole-map box needs no stack
//! heights at all.
//!
//! **Two assertions, and the second is what keeps the fix honest.** The map is framed, *and* the
//! stacking problem is still reported: a fix that framed the map by forgetting the bad stack would
//! turn a reported authoring error into silence, which is worse than the close-up.
//!
//! The camera is read the way `the_map_opens_framed.rs` reads it — every corner of the map's box
//! taken into the MainCamera's own space must fall inside the orthographic area it reports — so
//! nothing here trusts the framing arithmetic.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

/// **A 1120 m map with one lamp seated on a piece that cannot hold it up.**
///
/// `Map::validate` already refuses a placement whose host does not exist, so the reachable
/// unresolvable stack is a host that is there and offers nothing the guest needs. `resolve_y`
/// refuses it by name — *"`lamp@2` needs a `worktop` surface and rests on `floor@0`, which offers
/// none"* — so `heights` fails and every frame that resolved heights eagerly failed with it.
fn editor_with_a_broken_stack(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .surface_descriptor("table", "alpha", "worktop")
        .mounted_descriptor("lamp", "alpha", "worktop")
        .side(1120.0)
        .place("floor", (300.0, 200.0))
        // The floor is an ordinary `OnFloor` piece: it offers no surfaces, so the lamp cannot sit on
        // it and the stack has no answer.
        .place("table", (310.0, 205.0))
        .place_on("lamp", "floor@0", (310.0, 205.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

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

/// Whether every corner of a box is inside the MainCamera's orthographic area, and how much of the
/// area's limiting dimension the box fills.
fn seen(app: &mut App, centre: Vec3, size: Vec3) -> (bool, f32) {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<MainCamera>>();
    let (tf, proj) = q.single(world).expect("the editor has one main camera");
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic")
    };
    let area = ortho.area;
    assert!(
        area.width() > 0.0 && area.height() > 0.0,
        "the camera reports an empty orthographic area {area:?}"
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
    (inside, fill)
}

/// **Opening the map frames it, even though its stacking does not resolve.**
///
/// Failed before the change: the Rig stayed at its 18 m default, so a corner of the 1120 m map was
/// outside the view on the first frame a person sees.
#[test]
fn a_map_with_a_broken_stack_opens_framed_on_the_whole_map() {
    let mut app = editor_with_a_broken_stack("broken-stack-opens-framed");
    let (centre, size) = bevy_cauldron::editor::map_bounds(app.world().resource::<OpenMap>());
    let (inside, fill) = seen(&mut app, centre, size);
    assert!(
        inside,
        "on opening, a corner of the 1120 m map is outside the camera's view — a map whose \
         stacking does not resolve still opens on an 18 m close-up of the origin"
    );
    assert!(
        fill >= 0.6,
        "on opening, the map fills only {:.0}% of the view's limiting dimension",
        fill * 100.0
    );
}

/// **And the stacking problem is still said.**
///
/// The other half of the fix: framing the map must not be bought by dropping the reason its stack is
/// wrong. The status line is where a person learns it, so this reads the line.
#[test]
fn a_map_with_a_broken_stack_still_reports_the_stack() {
    let app = editor_with_a_broken_stack("broken-stack-reports");
    let state = app.world().resource::<bevy_cauldron::editor::EditorState>();
    let said = state
        .status
        .problems()
        .iter()
        .map(|p| p.line())
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        said.contains("floor@0") || said.contains("rests on"),
        "a map whose stacking does not resolve said nothing about the stack, so framing it \
         silenced the authoring error. The status holds: `{said}`"
    );
}
