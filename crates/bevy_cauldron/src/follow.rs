//! **The editor follows the Familiar** (AF-02, #358; series agent follow, #356).
//!
//! An agent works at a speed no person can watch: it places three pieces while they are reading a
//! panel, and by the time they look up the view is somewhere else entirely. So after every change an
//! agent makes **on the map**, the view goes to it — and a person's own key never moves the camera,
//! because their eyes are already where they are working.
//!
//! # Raised where the agent comes in, never inside `dispatch`
//!
//! [`crate::ops::dispatch`] is the one function both front ends call, so a follow raised there would
//! move the camera for every key press and every palette click. The request is raised in the two BRP
//! handlers an agent comes through ([`crate::ops_brp`]'s `operation_apply` and `operation_undo`), and
//! nowhere else. A command an agent runs with `grimoire_run_command` counts as a key press and moves
//! the camera only as that key would, which is the same rule.
//!
//! # One framing computation
//!
//! A request goes through [`crate::editor::resolve_frame`] and [`crate::editor::apply_frame`], exactly
//! as `grimoire_frame_view` does, with [`FOLLOW_MARGIN_FRAC`] as the margin so the change arrives with
//! some of its surroundings in view. There is no second framer: a second one would disagree with the
//! first about what a rectangle means, and the disagreement would be a camera landing somewhere the
//! answer did not name.
//!
//! # The ease, and reduced motion
//!
//! The ease is [`FOLLOW_EASE_S`]. A camera that jumps across a map reads as a glitch; one that eases
//! reads as a hand moving the view. **With reduced motion on it jumps**, because a person who asked
//! for no travel asked for exactly this — and `bevy_gooey::MotionSettings::lands_travel` is the one
//! answer to "does anything travel", so this module asks it rather than keeping a flag of its own.
//!
//! # Requests are a plain FIFO, one per frame
//!
//! An agent applies three operations faster than three frames go by. The queue holds them and the
//! camera takes one a frame, so each change is seen instead of the last one winning. AF-04 (#359)
//! adds the idle wait and the pacing; this slice is deliberately the queue alone.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::editor::{FrameTarget, MapFraming, apply_frame, resolve_frame};
use crate::project::{OpenMap, Project};
use crate::tiles::Door;
use crate::view::{MainCamera, Rig};

/// **How long the camera takes to reach a change**, seconds. Long enough to read as one movement,
/// short enough that a person does not wait for it.
pub const FOLLOW_EASE_S: f32 = 0.35;

/// **How much of the surroundings a follow keeps in view**, as a multiplier on the change's own box:
/// `1.25` leaves an eighth of the box each side. The change lands centred with its edges away from
/// the viewport's border, which is what tells a person *where* it is as well as *what* it is — a
/// piece framed edge to edge says nothing about the room it went into.
pub const FOLLOW_MARGIN_FRAC: f32 = 1.25;

/// **How long the changed pieces stay ringed**, seconds. Long enough to catch the eye once the camera
/// has arrived, short enough not to become decoration.
pub const FOLLOW_OUTLINE_S: f32 = 1.5;

/// **What the history calls an agent**, so the receipt and the history row name the same actor. A
/// person reading either reads "the Familiar".
pub const FAMILIAR: &str = "the Familiar";

/// **A change in the history's words**: `the Familiar placed bed@1`. The receipt the person reads
/// after the camera moves is that sentence, so the row and the receipt cannot describe one change two
/// ways.
pub fn as_the_familiar(summary: &str) -> String {
    format!("{FAMILIAR} {summary}")
}

/// **A place the camera is asked to go**, and the pieces to ring once it is there.
pub struct FollowRequest {
    /// What to frame: the change's own box, or the whole map when it is too large to frame closer.
    pub target: FrameTarget,
    /// The placements to outline, by the ids the map holds. Empty is an ordinary answer.
    pub ids: Vec<String>,
}

/// **The request a `Shown` makes**, or `None` when the change is not on the map.
///
/// A library piece, a composition, a rig, a note or a project file is a place on another screen, and
/// a follow is a camera: those are #359's question, not this one. `MapWhole` frames the map, which is
/// what "a change too large to frame any closer" means.
pub fn request_of(shown: &crate::ops::Shown) -> Option<FollowRequest> {
    let (target, ids) = match shown {
        crate::ops::Shown::Map { rect, placements } => {
            (FrameTarget::Rect(*rect), placements.clone())
        }
        crate::ops::Shown::MapWhole => (FrameTarget::Map, Vec::new()),
        _ => return None,
    };
    Some(FollowRequest { target, ids })
}

/// **Where the camera is going, and how far it has come.** Taken by the ease a frame at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ease {
    from_focus: Vec3,
    from_height: f32,
    to_focus: Vec3,
    to_height: f32,
    /// Seconds spent so far, against [`FOLLOW_EASE_S`].
    elapsed: f32,
}

/// **The follows waiting, the one being eased, and the pieces still ringed.**
///
/// **A `Session` resource** (`screen::OWNERSHIP`): a follow raised on another door opens the Map and
/// is carried out there, and a door change resets the door's own state — a request kept there would
/// be erased by the very crossing it asked for.
#[derive(Resource, Default)]
pub struct Following {
    queue: VecDeque<FollowRequest>,
    ease: Option<Ease>,
    /// The pieces to ring while [`Self::outline_until`] has not passed.
    outline: Vec<String>,
    /// The `Time<Real>` second the ring comes down at.
    outline_until: f64,
    /// **Whether the Map has been asked for on this request's behalf.** A `Door::Map` that does not
    /// follow drops the request instead of asking again every frame.
    asked_for_the_map: bool,
}

impl Following {
    /// **Ask the camera to go somewhere.** Queued, so three changes in one frame are three follows.
    pub fn request(&mut self, request: FollowRequest) {
        self.queue.push_back(request);
    }

    /// **How many follows are waiting for their frame.** The ease in flight is not one of them: the
    /// camera is already moving.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// **The pieces currently ringed**, in the order the request named them.
    pub fn outlined(&self) -> &[String] {
        &self.outline
    }

    /// **Whether the camera is on its way somewhere.**
    pub fn settling(&self) -> bool {
        self.ease.is_some()
    }
}

/// **A new map ends the follow of the old one.**
///
/// **The ease and the ring are about a place on one map**: a focus in map metres, heights measured
/// against its bounds, and placement ids, which are only unique on the map that minted them. Carried
/// onto another map they name a spot that means nothing, so the camera would ease to a metre figure
/// from the previous map and the ring would try to find ids that are not there. **The queue is kept**
/// — those requests are still what the person asked to see, and `take_the_follow_request` re-resolves
/// each against the map open when its turn comes.
fn clear_the_follow_when_the_map_changes(
    open: Option<Res<OpenMap>>,
    mut following: ResMut<Following>,
    mut seen: Local<Option<std::path::PathBuf>>,
) {
    let path = open.as_deref().map(|open| open.map_path.clone());
    if *seen == path {
        return;
    }
    *seen = path;
    following.ease = None;
    following.outline.clear();
}

/// **The plugin that owns the follow**: the resource, the system that carries one request a frame,
/// and the ring drawn round the changed pieces.
pub struct FollowPlugin;

impl Plugin for FollowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Following>().add_systems(
            Update,
            (
                // **Before the camera is written**, so the frame a follow moves on is the frame it is
                // drawn on: `view::drive` reads the rig every frame to build the transform.
                //
                // **And before the whole-map frame on entry**, which is one shot per door: a change
                // that arrives just after a crossing would otherwise be framed by the follow and
                // then overwritten by the whole map a frame later.
                // **Before the request is taken**, so a follow never carries the last map's ease or
                // ring onto the new map.
                clear_the_follow_when_the_map_changes.before(take_the_follow_request),
                take_the_follow_request
                    .before(crate::view::drive)
                    .before(crate::editor::frame_map_on_open),
                draw_the_follow_outline,
            )
                .run_if(in_state(crate::screen::Screen::Editor)),
        );
    }
}

/// **`smoothstep`**, so the ease starts and ends at rest rather than at a constant speed. A camera
/// that arrives at full speed stops with a jolt.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// **Where along the way a camera is**: the focus and height the ease has reached.
fn at(ease: &Ease) -> (Vec3, f32) {
    let t = smoothstep(ease.elapsed / FOLLOW_EASE_S);
    (
        ease.from_focus.lerp(ease.to_focus, t),
        ease.from_height + (ease.to_height - ease.from_height) * t,
    )
}

/// **Carry out one follow a frame**, and advance the one in flight.
///
/// **A change made from another door opens the Map first.** The camera is the Map door's, so the
/// request waits while the editor crosses — through [`crate::editor::DoorRequest`], the same
/// mechanism `door_open` uses, so a question on screen holds both the same way. A question open, or a
/// crossing already waiting, holds the request rather than stacking a second one.
#[allow(clippy::too_many_arguments)]
fn take_the_follow_request(
    mut following: ResMut<Following>,
    mut rig: ResMut<Rig>,
    mut framing: ResMut<MapFraming>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    door: Option<Res<Door>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    confirm: Option<Res<crate::confirm::Confirm>>,
    cameras: Query<&Camera, With<MainCamera>>,
    time: Res<Time<Real>>,
    motion: Option<Res<bevy_gooey::MotionSettings>>,
) {
    let now = time.elapsed_secs_f64();
    // **The ring comes down on its own clock**, whether or not another follow is queued.
    if !following.outline.is_empty() && now >= following.outline_until {
        following.outline.clear();
    }
    // **A person who asked for no travel gets none**: the camera lands in the frame it was asked,
    // so there is no ease to advance. One answer for the whole editor
    // (`bevy_gooey::MotionSettings::lands_travel`), not a flag of this module's own.
    let lands = motion.as_deref().is_none_or(bevy_gooey::MotionSettings::lands_travel);
    if let Some(ease) = following.ease.as_mut() {
        ease.elapsed += time.delta_secs();
        if lands || ease.elapsed >= FOLLOW_EASE_S {
            rig.focus = ease.to_focus;
            rig.height = ease.to_height;
            following.ease = None;
        } else {
            let (focus, height) = at(ease);
            rig.focus = focus;
            rig.height = height;
        }
        // **One follow at a time**: the camera reaches this change before it is asked for the next,
        // or three changes in three frames would be one blur. The queue is what makes them three
        // arrivals.
        return;
    }
    if following.queue.is_empty() {
        return;
    }
    // **The camera is the Map door's.** A crossing is the editor's own request mechanism, so a
    // follow from another door asks for it and waits.
    //
    // **Once per request, and never for ever** (review of #358, nit g). The request below is asked
    // for on the frame the queue first has something to carry, and marked asked; a frame that then
    // finds the door still not the Map — the crossing was refused, or a question on screen held it —
    // **drops the request** rather than asking again every frame. Asking again would be a crossing
    // asked for by a camera move nobody is waiting on any more, and it would never end.
    let on_the_map = door.as_deref().is_none_or(|door| *door == Door::Map);
    if !on_the_map {
        if following.asked_for_the_map {
            following.queue.pop_front();
            following.asked_for_the_map = false;
            return;
        }
        let asking = state.as_deref().is_none_or(|state| {
            state.door_request.is_some() || state.leaving || confirm.as_deref().is_some_and(|c| c.is_open())
        });
        if asking {
            // **Held, not asked for**: a question is on screen, or the editor is already leaving, and
            // this frame is not one that can start a crossing. The request waits for a frame that can.
            return;
        }
        if let Some(state) = state.as_mut() {
            state.door_request = Some(crate::editor::DoorRequest {
                door: Door::Map,
                tab: Some(crate::tiles::Mode::Map),
            });
            following.asked_for_the_map = true;
        }
        return;
    }
    following.asked_for_the_map = false;
    let (Some(project), Some(open)) = (project.as_deref(), open.as_deref()) else {
        // A follow with no map has nothing to look at. It is dropped rather than held: the editor is
        // between projects, and the next request is not this one's business.
        following.queue.clear();
        return;
    };
    let Some(request) = following.queue.pop_front() else {
        return;
    };
    let settled = match resolve_frame(&request.target, project, open, None) {
        Ok(settled) => settled,
        Err(why) => {
            if let Some(state) = state.as_mut() {
                state.status.problem(format!(
                    "{FAMILIAR}'s change could not be framed: {why}"
                ));
            }
            return;
        }
    };
    let Some(aspect) = cameras.single().ok().and_then(crate::editor::aspect_of) else {
        if let Some(state) = state.as_mut() {
            state.status.problem(
                "the Map camera has no viewport yet, so the Familiar's change was not framed",
            );
        }
        return;
    };
    // **Where the ease starts from**, read before `apply_frame` writes the rig.
    let from_focus = rig.focus;
    let from_height = rig.height;
    let (to_focus, to_height) =
        apply_frame(&mut rig, &settled, aspect, open, FOLLOW_MARGIN_FRAC);
    // **The map's own open-frame must not fight this one.** `frame_map_on_open` frames the whole map
    // once per door entry; a change that arrives just after a crossing would be framed by the follow
    // and then overwritten by the whole map.
    framing.settle();
    if lands {
        // `apply_frame` wrote the destination, and reduced motion means it is already there.
    } else {
        // **The ease takes its first step on the frame it is raised**, so the camera begins to move
        // the frame the person's follow is asked for rather than a frame later.
        let mut ease = Ease {
            from_focus,
            from_height,
            to_focus,
            to_height,
            elapsed: time.delta_secs(),
        };
        let (focus, height) = if ease.elapsed >= FOLLOW_EASE_S {
            // A frame longer than the whole ease (a stalled editor, a test clock) lands rather than
            // easing through a step nobody would see.
            ease.elapsed = FOLLOW_EASE_S;
            (to_focus, to_height)
        } else {
            at(&ease)
        };
        rig.focus = focus;
        rig.height = height;
        if ease.elapsed < FOLLOW_EASE_S {
            following.ease = Some(ease);
        }
    }
    // **The pieces to ring, and a place names none.** `Shown::Map`'s `placements` is empty when the
    // id is minted at commit — a `placement_add` names the box it will land in and no id — so the
    // ring is whatever the map now holds inside the box that was just framed. Read after the change,
    // which is why this is here rather than in `request_of`: the answer describes a map that did not
    // exist when the request was raised.
    let ids = if request.ids.is_empty() {
        inside_the_framed_box(project, open, &settled)
    } else {
        request.ids
    };
    if ids.is_empty() {
        following.outline.clear();
    } else {
        following.outline = ids;
        following.outline_until = now + f64::from(FOLLOW_OUTLINE_S);
    }
}

/// **The pieces the framed box holds**, by id, in the map's own row order.
///
/// **Only used when the answer named none** — a place mints its id at commit. A change that framed a
/// rectangle the map cannot place anything inside rings nothing rather than ringing the whole map.
fn inside_the_framed_box(project: &Project, open: &OpenMap, framing: &crate::editor::Framing) -> Vec<String> {
    let half = framing.size * 0.5;
    let (lo, hi) = (framing.centre - half, framing.centre + half);
    (0..open.map.placements.len())
        .filter_map(|index| {
            let (centre, size) = crate::editor::placement_bounds(project, open, index)?;
            let (half, centre) = (size * 0.5, centre);
            // **Fully inside**, so a piece only half in the box is not claimed as the change: the
            // box came from the change's own footprint, so the change is in it whole.
            let inside = centre.x - half.x >= lo.x - INSIDE_SLACK_M
                && centre.x + half.x <= hi.x + INSIDE_SLACK_M
                && centre.z - half.z >= lo.z - INSIDE_SLACK_M
                && centre.z + half.z <= hi.z + INSIDE_SLACK_M;
            inside.then(|| open.map.placements[index].id.clone())
        })
        .collect()
}

/// **How far outside a framed box a piece may reach and still count as inside it**, metres: the
/// rectangle comes from the placement's own turned footprint, so the two agree to float rounding and
/// no further.
const INSIDE_SLACK_M: f32 = 1e-3;

/// **Ring the pieces a follow just brought into view**, for [`FOLLOW_OUTLINE_S`].
///
/// A gizmo rather than a spawned marker, for `draw_edge_faults`'s reason: it is derived from a
/// resource that is already there, so there is nothing to keep in step and nothing to clean up when
/// the ring comes down.
fn draw_the_follow_outline(
    following: Res<Following>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut gizmos: Gizmos,
) {
    if following.outline.is_empty() {
        return;
    }
    let (Some(project), Some(open)) = (project.as_deref(), open.as_deref()) else {
        return;
    };
    for id in &following.outline {
        let Some(index) = open.map.placements.iter().position(|p| &p.id == id) else {
            continue;
        };
        // **The piece's own drawn box, measured the way the frame measured it**, so the ring and the
        // frame cannot disagree about where the change is. **A piece with no measurement is skipped**,
        // as an unknown id is: a ring drawn at the row's origin and a half metre square would point at
        // a place the editor does not know the piece is, which is worse than no ring.
        let Some((centre, size)) = crate::editor::placement_bounds(project, open, index)
            .map(|(centre, size)| (centre, Vec2::new(size.x, size.z)))
        else {
            continue;
        };
        gizmos.rect(
            Isometry3d::new(
                centre + Vec3::Y * OUTLINE_LIFT_M,
                Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            ),
            size.max(Vec2::splat(OUTLINE_MIN_M)),
            crate::chrome::ACCENT,
        );
    }
}

/// **How far above the piece's centre the ring is drawn**, metres: enough to clear a flat piece's
/// surface, so the line is not buried inside the mesh it surrounds.
const OUTLINE_LIFT_M: f32 = 0.02;

/// **The smallest ring drawn**, metres. A piece too small to measure still reads as a mark rather
/// than as a few pixels of line.
const OUTLINE_MIN_M: f32 = 0.5;
