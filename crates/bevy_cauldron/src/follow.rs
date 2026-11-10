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

/// **How long a person's own use of the view holds the follow back**, seconds (AF-04, #360).
///
/// A person who orbited, panned, dragged, clicked in the viewport or typed is *reading the view*, and
/// moving the camera under them is the yank the whole series exists to prevent. Two seconds is long
/// enough to cover a gesture and the look that follows it, and short enough that a person who stopped
/// is not left wondering why nothing happened.
pub const FOLLOW_IDLE_S: f32 = 2.0;

/// **How long one change is shown before the next**, seconds (AF-04, #360).
///
/// Twenty changes in one second are twenty views rather than a blur: the queue is what makes them
/// separate arrivals, and this is what makes each one readable. It is the *floor*, so a longer ease
/// (a change far across the map) is never cut short.
pub const FOLLOW_DWELL_S: f32 = 0.6;

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

impl FollowRequest {
    /// **What the request is for, in words** — what the look chip says when the caller gave no
    /// sentence of its own. `a place on the map with 1 piece` reads as what a person will see, not as
    /// the name of a field.
    pub fn words(&self) -> String {
        match &self.target {
            FrameTarget::Map => "the whole map".to_owned(),
            FrameTarget::Selection => "the selected piece".to_owned(),
            FrameTarget::Placements(ids) => match ids.len() {
                1 => format!("`{}`", ids[0]),
                n => format!("{n} pieces"),
            },
            FrameTarget::Rect(_) => "a place on the map".to_owned(),
        }
    }
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
    /// Each request with the words the chip says it in, or `None` for one raised without a sentence.
    queue: VecDeque<(FollowRequest, Option<String>)>,
    ease: Option<Ease>,
    /// **Whether the person is using the view** (AF-04, #360): input within [`FOLLOW_IDLE_S`].
    ///
    /// **Input, not motion.** The ease writes `Rig` itself, so "did the camera move" would see the
    /// follow's own travel and hold it back for ever; this is set by the two buttons the person
    /// presses and by nothing else.
    busy: bool,
    /// **The `Time<Real>` second the person last touched the view**, for [`FOLLOW_IDLE_S`].
    busy_until: f64,
    /// **The `Time<Real>` second the change on screen was framed.** A follow waits until
    /// [`FOLLOW_DWELL_S`] has passed since it, so a burst is a series of views rather than a blur.
    shown_at: f64,
    /// **The change being waited on**, as the look chip says it: the one in flight, else the oldest
    /// queued. `None` when there is nothing to show.
    waiting: Option<String>,
    /// **Whether the person was working last frame**, so a gesture's *start* is told from a held
    /// button. See [`a_persons_input_takes_the_camera_back`]: a new gesture drops what was queued, and
    /// a held one only keeps the follow waiting (AF-04, #360).
    working: bool,
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
    ///
    /// A request with no sentence is still a request: the chip says what it can, that the Familiar
    /// changed something. [`Self::request_named`] is the form an operation's own receipt fills in.
    pub fn request(&mut self, request: FollowRequest) {
        self.queue.push_back((request, None));
        self.note_what_waits();
    }

    /// **Ask the camera to go somewhere, and say what changed** — `the Familiar placed bed@1`, the
    /// receipt's own words (AF-04, #360).
    ///
    /// **A method rather than a field on [`FollowRequest`]**, because the field would be one a caller
    /// must fill in even when there is nothing to say, and the struct is built by a test that has no
    /// sentence to give it.
    pub fn request_named(&mut self, request: FollowRequest, summary: impl Into<String>) {
        self.queue.push_back((request, Some(summary.into())));
        self.note_what_waits();
    }

    /// **What the look chip says**, or `None` when nothing is waiting to be shown (AF-04, #360).
    ///
    /// The change in flight, else the oldest queued: a person reading a chip wants the next thing they
    /// will see, not the last. Empty until a request names a change, so a chip is never drawn for a
    /// follow with nothing to say.
    pub fn waiting(&self) -> Option<&str> {
        self.waiting.as_deref()
    }

    /// **Record that the person is using the view**, which holds every follow back for
    /// [`FOLLOW_IDLE_S`]. Called by the input system and by nothing else.
    fn note_a_person_at_work(&mut self, now: f64) {
        self.busy = true;
        self.busy_until = now + f64::from(FOLLOW_IDLE_S);
    }

    /// **Look now**: stop waiting on the person and on the dwell, so the next frame shows the oldest
    /// change waiting (AF-04, #360).
    ///
    /// **It moves the camera; it does not clear the queue.** The remaining changes are still what the
    /// person asked to see, so they follow one after another as they would have — the chip's count is
    /// the number left, not a number that a Look throws away.
    pub fn look_now(&mut self) {
        self.busy = false;
        self.busy_until = 0.0;
        self.shown_at = 0.0;
    }

    /// **Carry the chip's words from whatever is next**: the change in flight, else the queue's head.
    fn note_what_waits(&mut self) {
        self.waiting = self.queue.front().map(|(request, said)| {
            said.clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("{FAMILIAR} changed {}", request.words()))
        });
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

/// **A person's own input takes the camera back, and the follow yields.**
///
/// **#358's acceptance says a person is never yanked** — the follow exists so someone reading a panel
/// can see what the agent just did, and it stops being a kindness the moment they are doing something
/// themselves. So a key press or a mouse button **drops the queue, stops the ease where the camera
/// already is, and takes the ring down**, rather than moving the view out from under the gesture in
/// progress.
///
/// **Measured, not supposed.** `a_person_removes_a_piece_and_undoes_it`'s
/// `an_agents_undo_of_a_removal_waits_while_the_person_has_moved_a_piece_since` reads where a piece
/// is on screen with `Camera::world_to_viewport`, then presses `B` and clicks those pixels. With the
/// follow still queued the `B` frame also carried the camera from the whole-map frame to the
/// removal's box, so the pixels the person aimed at were a frame stale and the piece did not move.
/// A person's press is theirs; the camera is theirs from that moment.
///
/// **What this does not touch**: an agent's operation raises no input event, so an agent's change is
/// still followed, and a person who does nothing is still shown it.
fn a_persons_input_takes_the_camera_back(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    time: Res<Time<Real>>,
    mut following: ResMut<Following>,
    // **A reveal is the same kindness**, so it stops on the same press (review of #359, M3): a queued
    // reveal still crossed a door while the person was working, which is the yank one screen over.
    // One rule, one system, two subjects — a second input system would be a second answer to "is the
    // person working", and the two would disagree the first time one of them changed.
    mut revealing: Option<ResMut<crate::reveal::Revealing>>,
) {
    // **`pressed`, not `just_pressed`**: the press state is what survives the frame it was made on,
    // and a person holding a key is working just as much as one who tapped it. It is also the only
    // honest answer in a headless run — `ButtonInput`'s `just_pressed` is cleared in `PreUpdate`, so
    // a test that writes the resource before its frame would be measured a frame late.
    let working = keys.as_deref().is_some_and(|keys| {
        keys.get_pressed().next().is_some() || keys.get_just_released().next().is_some()
    });
    let clicking = mouse.as_deref().is_some_and(|mouse| {
        mouse.get_pressed().next().is_some() || mouse.get_just_released().next().is_some()
    });
    let at_work = working || clicking;
    if !at_work {
        following.working = false;
        return;
    }
    // **The person is using the view** (AF-04, #360): the follow waits [`FOLLOW_IDLE_S`] after they
    // stop, so a gesture and the look that follows it are not interrupted. This is the one writer of
    // that clock — `busy` is about input, never about motion, because the ease writes `Rig` itself and
    // a "did the camera move" test would hold the follow back for ever.
    following.note_a_person_at_work(time.elapsed_secs_f64());
    // **A gesture's FIRST frame takes the camera back; the ones after it only hold the follow.**
    //
    // The distinction is what makes two rules into one. #358's rule is that a person's press is
    // theirs *from that moment*, so what was queued when they reached for the view is dropped rather
    // than moved out from under the gesture (`the_follow_never_moves_a_persons_camera` measures
    // exactly that, with `pending() == 0`). #360's rule is that a person *working* is shown what the
    // agent did once they are done, so a change that arrives mid-drag must wait rather than be thrown
    // away. Dropping on every held frame would be the first rule applied to the second case, and it
    // would make the idle wait a no-op for any gesture longer than one frame — measured: a drag held
    // across the agent's change left the queue empty and the chip silent.
    if following.working {
        return;
    }
    following.working = true;
    // **The camera stays where it is**: `rig` already holds the ease's latest step, so clearing the
    // ease leaves the view exactly where the person's eyes left it — no snap to the ease's end,
    // which would be the yank this exists to prevent.
    following.ease = None;
    following.outline.clear();
    following.queue.clear();
    following.asked_for_the_map = false;
    following.waiting = None;
    if let Some(revealing) = revealing.as_mut() {
        revealing.clear();
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

/// **The look chip's words** (AF-04, #360): what the Familiar changed and has not shown yet.
///
/// **A resource rather than a spawned node's text**, for `chrome::Follow`'s reason: a test can ask what
/// the chip says without a window, and the system that draws it compares before writing. The two
/// cannot disagree, because the drawing is one read of this.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct LookChip {
    /// The sentence, or `None` when nothing is waiting.
    pub sentence: Option<String>,
    /// How many changes are still queued behind the one the sentence names.
    pub queued: usize,
}

impl LookChip {
    /// **The line a person reads**, or `None` when there is nothing to show.
    pub fn sentence(&self) -> Option<&str> {
        self.sentence.as_deref()
    }

    /// **What the chip says for `following`**, as one value: the drawing system and a test both read
    /// this, so the picture and the assertion cannot drift.
    pub fn of(following: &Following) -> LookChip {
        match following.waiting() {
            Some(what) => LookChip {
                sentence: Some(format!(
                    "{what}{} · Look",
                    match following.pending().saturating_sub(1) {
                        0 => String::new(),
                        n => format!(" (+{n} more)"),
                    }
                )),
                queued: following.pending(),
            },
            None => LookChip {
                sentence: None,
                queued: following.pending(),
            },
        }
    }
}

/// **The look chip itself**, the node its sentence is drawn in.
#[derive(Component)]
pub struct LookChipNode;

/// **The look chip's Look button** (AF-04, #360), so a person can look now and a test can press it.
#[derive(Component)]
pub struct LookButton;

/// **Look now, from the chip's button.**
///
/// **One answer to "show it"**: this and the AGENT block's Look both call
/// [`Following::look_now`], so a click and a button cannot wait on different things.
pub fn look_at_the_change(
    activate: On<bevy_gooey::Activate>,
    buttons: Query<(), With<LookButton>>,
    mut following: ResMut<Following>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    following.look_now();
    if let Some(state) = state.as_mut() {
        state.status.note(format!("looking at what {FAMILIAR} changed"));
    }
}

/// **The plugin that owns the follow**: the resource, the system that carries one request a frame,
/// and the ring drawn round the changed pieces.
pub struct FollowPlugin;

impl Plugin for FollowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Following>()
            .init_resource::<LookChip>()
            .add_systems(
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
                // **And before it too**, so the frame a person presses a key on is a frame the
                // camera does not move on: the press and the follow would otherwise land together
                // and the click after it would aim at where things used to be.
                a_persons_input_takes_the_camera_back.before(take_the_follow_request),
                take_the_follow_request
                    .before(crate::view::drive)
                    .before(crate::editor::frame_map_on_open),
                // **The chip is read off the queue after the frame's request was taken**, so it says
                // what is still waiting rather than what has just been shown.
                follow_the_chip.after(take_the_follow_request),
                // **The chip's words are drawn beside the change that is waiting**, so its button and
                // the sentence it sits on are one picture.
                draw_the_look_chip.after(follow_the_chip),
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
    // **View ▸ Follow the agent** (AF-04, #360). Off, the camera never moves for an agent's change and
    // the queue is kept rather than dropped: the look chip still offers Look, and that is the whole
    // point of the switch being a *pause* of the moving rather than a mute of the following.
    settings: Option<Res<crate::settings::EditorSettings>>,
) {
    let now = time.elapsed_secs_f64();
    // **The person's hands off the view for `FOLLOW_IDLE_S`**: the follow may move again. Cleared
    // here rather than by a timer so the clock has one writer per direction — `busy_until` is set by
    // the input system and read here, and nothing else touches either.
    if following.busy && now >= following.busy_until {
        following.busy = false;
    }
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
        following.waiting = None;
        return;
    }
    // **A person using the view holds every follow back** (AF-04, #360). The queue is untouched while
    // they are, so the change is shown once they stop rather than dropped — which is the difference
    // between a considerate follow and one that skips what it happened to miss.
    if following.busy {
        following.note_what_waits();
        return;
    }
    // **The dwell is what separates two changes, and nothing before the first.** A single change is
    // shown the frame it arrives — the ordinary case, and what an editor with one agent change to
    // show must not pay 0.6 s to see. A burst is where the pacing matters: each change is held for
    // `FOLLOW_DWELL_S` *after* it is shown, so five changes are five views rather than a blur.
    if following.shown_at != 0.0 && now < following.shown_at + f64::from(FOLLOW_DWELL_S) {
        following.note_what_waits();
        return;
    }
    // **The toggle is off: nothing moves, and nothing is lost.** The chip keeps naming the oldest
    // change and `Following::look_now` is what shows it.
    if settings.as_deref().is_some_and(|s| !s.follow_agent) {
        following.note_what_waits();
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
    let Some((request, _)) = following.queue.pop_front() else {
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
    // **The dwell starts now**, so the next change waits [`FOLLOW_DWELL_S`] behind this one.
    following.shown_at = now;
    following.note_what_waits();
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

/// **Keep the look chip's words equal to what is waiting.** Compared before writing, which is the
/// whole reason this is a resource: a chip rewritten every frame dirties whatever draws it.
fn follow_the_chip(following: Res<Following>, mut chip: ResMut<LookChip>) {
    let want = LookChip::of(&following);
    if *chip != want {
        *chip = want;
    }
}

/// **Draw the look chip over the Map viewport** while a change waits (AF-04, #360).
///
/// **Spawned once and shown or hidden**, not respawned: a node rebuilt sixty times a second is sixty
/// times the work for one sentence, and the chip is on screen for far longer than a frame. The words
/// come from [`LookChip`], which is the one place they are decided, so what is drawn and what a test
/// asserts cannot drift.
///
/// `Display` is compared before writing — the rule `no_system_writes_every_frame` enforces.
fn draw_the_look_chip(
    mut commands: Commands,
    chip: Res<LookChip>,
    open: Option<Res<OpenMap>>,
    // **Where the map is drawn**, from the node the camera renders into — the same rect
    // `view::drive` asks of a wheel, read the other way round.
    viewport: Option<Single<(&ComputedNode, &UiGlobalTransform), With<crate::chrome::ViewportSlot>>>,
    mut nodes: Query<(Entity, &mut Node, &Children), With<LookChipNode>>,
    mut words: Query<&mut Text>,
) {
    if let Some(mut node) = nodes.iter_mut().next() {
        let (_, style, children) = (&node.0, &mut node.1, node.2);
        let want = if chip.sentence.is_some() { Display::Flex } else { Display::None };
        if style.display != want {
            style.display = want;
        }
        let said = chip.sentence.clone().unwrap_or_default();
        for kid in children.iter() {
            if let Ok(mut text) = words.get_mut(kid)
                && text.0 != said
            {
                text.0.clone_from(&said);
            }
        }
        return;
    }
    // **Only while there is a map to say it about**, and only in a frame that knows where the viewport
    // is: a chip for a change on a map nobody has open is a chip with nothing behind it.
    let (Some(_open), Some(viewport)) = (open, viewport) else {
        return;
    };
    let (node, transform) = viewport.into_inner();
    let at = (transform.translation - node.size() * 0.5) * node.inverse_scale_factor;
    let root = commands
        .spawn((
            LookChipNode,
            Node {
                // PLACES-ITSELF-OK: the chip stands over the viewport rather than in a flow, which is
                // what "over the viewport" means — the same placement `compass` makes for its gizmo.
                position_type: PositionType::Absolute,
                left: Val::Px(at.x + CHIP_INSET),
                top: Val::Px(at.y + CHIP_INSET),
                display: if chip.sentence.is_some() { Display::Flex } else { Display::None },
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(crate::chrome::GAP_ROW),
                padding: crate::chrome::CHIP_PAD,
                border_radius: BorderRadius::all(Val::Px(crate::chrome::RADIUS_ROW)),
                ..default()
            },
            BackgroundColor(crate::chrome::PANEL_BG),
            GlobalZIndex(crate::chrome::COMPASS_Z),
            // A readout that eats clicks is a readout that steals the map underneath it.
            bevy::picking::Pickable::IGNORE,
        ))
        .with_children(|chip_node| {
            chip_node.spawn((
                Text::new(chip.sentence.clone().unwrap_or_default()),
                TextColor(crate::chrome::TEXT),
                crate::chrome::font(crate::chrome::text::BODY),
                bevy::picking::Pickable::IGNORE,
            ));
            // **Look**, the button the issue names: it shows the oldest change now rather than waiting
            // out the person's gesture. `chrome::button` puts it on the `Activate` bus the handler
            // reads.
            let mut look = crate::chrome::button(
                chip_node,
                LookButton,
                "Look",
                crate::chrome::Severity::Primary,
            );
            look.entry::<bevy::picking::Pickable>();
        })
        .id();
    let _ = root;
}

/// How far in from the viewport's top-left corner the chip stands, pixels.
const CHIP_INSET: f32 = 12.0;

/// **How far above the piece's centre the ring is drawn**, metres: enough to clear a flat piece's
/// surface, so the line is not buried inside the mesh it surrounds.
const OUTLINE_LIFT_M: f32 = 0.02;

/// **The smallest ring drawn**, metres. A piece too small to measure still reads as a mark rather
/// than as a few pixels of line.
const OUTLINE_MIN_M: f32 = 0.5;
