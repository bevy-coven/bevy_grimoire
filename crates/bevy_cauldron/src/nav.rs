//! **Back and Forward: every reveal pushes the place you left** (NV-06, #383; series getting around,
//! #377).
//!
//! Go to anything (NV-05, #382) and a cross-link (NV-07, #384) both end in one call —
//! `crate::reveal`'s reveal — and so does a clicked problem and a workspace key. Each of them moves a
//! person away from where they were looking, and each of them is a place they want back. So the
//! **place** is one value, the **stack** is one resource, and the **push** happens in one system: the
//! one that knows a view is about to move.
//!
//! # What a place is
//!
//! A workspace, a panel of it, what is selected there, and the camera. Four fields because those are
//! the four things a person can be looking at when they say *"put me back"*. [`Place::of`] reads them
//! off the world once, and [`settle_the_place`] writes them back once the door has landed — the camera
//! through [`Camera::restore`], the selection through the editor's one lock — so there is no second
//! builder, and a place can never be a partial snapshot of an edit site's choosing.
//!
//! **A place that held no selection leaves the selection as it is.** A follow records a place without
//! reading one, and clearing what a person has selected to undo a camera move would be a second yank.
//!
//! # Back undoes a yank, and a follow's own steps are not places
//!
//! The agent follow (AF-02) moves the camera without the person asking. When it does, **their place is
//! pushed first**, so one Back returns them to what they were reading — and the follow's own travel is
//! not pushed at all, because a history full of an agent's steps would take a hundred Backs to undo one
//! arrival. That is why the push lives in `reveal::take_the_reveal` for a reveal and in
//! `follow`'s idle-gate system for a follow: **one push, at the moment the view is about to move for
//! the person**, never one per interpolation frame.
//!
//! # Bounded, and not saved
//!
//! [`NAV_HISTORY_MAX`] places per session. The history is **not written to a file**: it is not a
//! document, it is what this sitting was looking at, and a place from yesterday names a selection the
//! project may no longer hold.
//!
//! # A place whose thing is gone is skipped, with a notice
//!
//! A placement an agent deleted: [`go`] answers `Err` with the sentence, [`History::walk`] drops that
//! place and carries on to the next one, and `walk_on_the_chords` raises each sentence as a problem so
//! the person reads what went. Skipping silently would leave Back doing nothing with no answer, which
//! is the dead end this series exists to remove. (A rig renamed on disk is not checked: a place holds a
//! rig's name only as a selection on the Map, and the Animation bench has no row to restore.)

use bevy::prelude::*;
use bevy::prelude::Vec3;

use crate::keys::{Context, Holder, Live};
use crate::tiles::Mode;
use crate::workspace::Workspace;

/// **How many places one session remembers.** A hundred is enough to walk back through a morning's
/// work and small enough that the stack is a few kilobytes: a `Place` is a workspace, a panel, one
/// selection id and four floats.
pub const NAV_HISTORY_MAX: usize = 100;

/// **How near two cameras must be to be the same view**: a tenth of a millimetre for the focus and
/// the height (metres) and the same figure for the angles (radians). A camera restored to within this
/// is where it was, and `Camera::restore` writes nothing for it.
const SAME_VIEW: f32 = 1e-4;

/// **Where a person was looking**: the two axes of navigation, the selection, and the camera.
///
/// `Debug` by hand rather than derived: `tiles::Mode` is a `keys::enumerated!` type that carries no
/// `Debug` of its own (`keys.rs:359` is a macro that generates the enum from a table), so deriving it
/// here would be a request the type cannot answer. A `Place` is printed by the history panel and by a
/// failing assertion, which is what [`Mode::label`] is for.
#[derive(Clone, PartialEq)]
pub struct Place {
    pub workspace: Workspace,
    pub mode: Mode,
    /// **The selection, by the id the page names it with** — a library id, a placement id, a rig's
    /// name, a note's. Empty for a page whose subject is the page itself.
    pub selection: Option<String>,
    /// The camera, when the place has one: only the workspaces that show the map do.
    pub camera: Option<Camera>,
}

/// **The map camera as a place holds it.** `Rig` already is this; the copy exists so a `Place` does
/// not borrow the resource it came from, and so the fields named here are the ones Back promises.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub focus: Vec3,
    /// The orthographic viewport height in metres — the zoom.
    pub height: f32,
    pub yaw: f32,
    pub goal_yaw: f32,
    pub elevation: f32,
}

impl Camera {
    /// The camera a rig holds.
    pub fn of(rig: &crate::view::Rig) -> Camera {
        Camera { focus: rig.focus, height: rig.height, yaw: rig.yaw, goal_yaw: rig.goal_yaw, elevation: rig.elevation }
    }

    /// Write this camera back into `rig`, and answer whether it moved.
    pub fn restore(&self, rig: &mut crate::view::Rig) -> bool {
        let same = rig.focus.abs_diff_eq(self.focus, SAME_VIEW)
            && (rig.height - self.height).abs() <= SAME_VIEW
            && (rig.yaw - self.yaw).abs() <= SAME_VIEW
            && (rig.goal_yaw - self.goal_yaw).abs() <= SAME_VIEW
            && (rig.elevation - self.elevation).abs() <= SAME_VIEW;
        if same {
            return false;
        }
        rig.focus = self.focus;
        rig.height = self.height;
        rig.yaw = self.yaw;
        rig.goal_yaw = self.goal_yaw;
        rig.elevation = self.elevation;
        true
    }
}

impl std::fmt::Debug for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Place")
            .field("workspace", &self.workspace.title())
            .field("mode", &self.mode.label())
            .field("selection", &self.selection)
            .field("camera", &self.camera)
            .finish()
    }
}

impl Place {
    /// **Where the editor is standing now**, read from the world once so every caller records the same
    /// four things.
    pub fn of(
        workspace: Option<&Workspace>,
        mode: Option<&Mode>,
        rig: Option<&crate::view::Rig>,
        selection: Option<&crate::selection::Selection>,
    ) -> Place {
        Place {
            workspace: workspace.copied().unwrap_or_default(),
            mode: mode.copied().unwrap_or_default(),
            // **A placement selection is the one a page can restore by id**: a library piece is
            // `ImportState`'s, and that resource is the page's own state rather than a place — which
            // is why a reveal of a piece is a place of the *page*, with no selection, and the piece's
            // row is `Select`'s job (see `reveal::apply_selection`).
            selection: selection
                .and_then(|s| s.ids().first())
                .filter(|id| !id.is_empty())
                .cloned(),
            camera: rig.filter(|_| workspace.is_some_and(|w| w.shows_the_map())).map(Camera::of),
        }
    }
}

/// **Whether a place can still be gone to**: a selection naming a placement that is no longer on the
/// map is the one thing this can check without a page, and it is the common case (an agent deleted the
/// row a person was looking at).
pub fn still_there(place: &Place, open: Option<&crate::project::OpenMap>) -> Result<(), String> {
    let (Some(id), Some(open)) = (place.selection.as_deref(), open) else { return Ok(()) };
    if place.workspace.edits_the_map() && !open.map.placements.iter().any(|p| p.id == id) {
        return Err(format!("`{id}` is no longer on this map"));
    }
    Ok(())
}

/// **How a file stop is written** in a place's `selection`: `file:<path relative to the project>:<line, 0-based>` (NV-06, #383). The
/// field is "the selection, by the id the page names it with"; in the Files workspace a page's selection is the file open in it and
/// where the caret is, so it needs no field of its own (and a place built by hand, as a test builds one, still has four).
pub const FILE_STOP: &str = "file:";

/// **How a cue stop is written**: `cue:<name>`, the cue the Sound workspace's list marks.
pub const CUE_STOP: &str = "cue:";

/// **What a place holds in the two workspaces whose selection is not a placement**: the open file and its line, the marked cue. Read
/// where a place is recorded ([`Stops::stamp`]) and where one is walked to ([`Stops::gone`]), so the two cannot disagree about what a
/// stop is.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stops<'w, 's> {
    session: Option<Res<'w, crate::session::Session>>,
    cue: Option<Res<'w, crate::world_lists::SelectedCue>>,
    project: Option<Res<'w, crate::project::Project>>,
    workspace: Option<Res<'w, Workspace>>,
    editors: Query<'w, 's, &'static bevy_gooey::CodeEditor, With<crate::files::FileEditor>>,
}

impl Stops<'_, '_> {
    /// **Write the stop the person is standing on into `place`**, when `place` is where they are now: the Files workspace's open file and
    /// caret line, the Sound workspace's marked cue. A place in any other workspace keeps its own selection. A place of one of those two
    /// workspaces that the person has already left holds no stop (the door crossed before it was read), and the placement id `Place::of`
    /// read for it names nothing there, so it is dropped rather than kept as a selection of the wrong page.
    pub fn stamp(&self, place: &mut Place) {
        if self.workspace.as_deref().is_some_and(|w| *w != place.workspace) {
            if matches!(place.workspace, Workspace::Files | Workspace::Sound) {
                place.selection = None;
            }
            return;
        }
        match place.workspace {
            Workspace::Files => {
                place.selection = self.session.as_deref().and_then(crate::files::active_file).map(|rel| {
                    let line = self.editors.iter().next().map_or(0, |e| e.buffer().caret().line);
                    format!("{FILE_STOP}{rel}:{line}")
                });
            }
            Workspace::Sound => {
                place.selection = self.cue.as_deref().and_then(|c| c.0.as_ref()).map(|name| format!("{CUE_STOP}{name}"));
            }
            _ => {}
        }
    }

    /// **Whether the thing a stop names is still there**: a file deleted from the project, a cue taken out of the sound file. `Err` is the
    /// sentence the walk raises when it skips the place.
    pub fn gone(&self, place: &Place) -> Result<(), String> {
        let (Some(stop), Some(project)) = (place.selection.as_deref(), self.project.as_deref()) else { return Ok(()) };
        if let Some(rest) = stop.strip_prefix(FILE_STOP) {
            let rel = rest.rsplit_once(':').map_or(rest, |(rel, _)| rel);
            if !project.root.join(rel).is_file() {
                return Err(format!("`{rel}` is no longer in the project"));
            }
        } else if let Some(name) = stop.strip_prefix(CUE_STOP)
            && !crate::world_lists::cue_names_of(project).is_ok_and(|names| names.iter().any(|n| n == name))
        {
            return Err(format!("the cue `{name}` is no longer in the project's sound file"));
        }
        Ok(())
    }
}

/// **Which way a walk goes.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Back,
    Forward,
}

impl Direction {
    /// The word a sentence says it with.
    pub const fn word(self) -> &'static str {
        match self {
            Direction::Back => "back",
            Direction::Forward => "forward",
        }
    }
}

/// **What a walk found**: the place to go to, and a sentence for each place it skipped on the way.
#[derive(Debug)]
pub struct Walked {
    pub to: Option<Place>,
    pub skipped: Vec<String>,
}

/// **The places a person has left, and the ones they walked back through.**
///
/// **Two stacks, not a cursor in one.** A push is *the place being left*, and the place arrived at is
/// not known to the pusher — a reveal knows the page it is going to but a workspace key does not — so
/// a single list with a cursor in it cannot say which entry is the present. Two stacks can: `behind`
/// is everything walked away from, `ahead` everything Back has stepped through. Walking Back moves
/// the present onto `ahead`; walking Forward moves it back onto `behind`. That is also why a new push
/// clears `ahead`: the person chose a new branch.
///
/// A `Session` resource (`screen::OWNERSHIP`): walking back is walking through the sitting, and a
/// workspace change is one of the moves a person wants back.
#[derive(Resource, Default)]
pub struct History {
    /// **Oldest first**, `NAV_HISTORY_MAX` at most.
    behind: Vec<Place>,
    /// **The places Back stepped through**, most recent first: where Forward goes.
    ahead: Vec<Place>,
    /// **The place being walked to**, and how many frames it has waited for its door.
    ///
    /// A workspace change resets `Rig` and the panels, so a place is written *after* the change lands —
    /// `reveal::take_the_reveal`'s two-step, for the same reason and with the same shape. `asked` makes
    /// the request once, so a switch the editor refused does not become a request every frame.
    pending: Option<(Place, bool)>,
}

impl History {
    /// **Push the place being left.**
    ///
    /// A push **clears the forward stack**: a person who walked back twice and then opened something
    /// has chosen a new branch, and the places they did not go back to are not ahead of them any more.
    ///
    /// **One place is not two**: pushing where the person already stands would make Back a press that
    /// does nothing, and a person who presses a key that does nothing learns the key is broken.
    pub fn push(&mut self, place: Place) {
        self.ahead.clear();
        if self.behind.last().is_some_and(|last| *last == place) {
            return;
        }
        self.behind.push(place);
        if self.behind.len() > NAV_HISTORY_MAX {
            // Oldest first, so the front is what falls off.
            self.behind.remove(0);
        }
    }

    /// Everything walked away from, oldest first — what a test and the history panel read.
    pub fn places(&self) -> &[Place] {
        &self.behind
    }

    /// The places Back stepped through, most recent first.
    pub fn ahead(&self) -> &[Place] {
        &self.ahead
    }

    /// How many places are behind the person.
    pub fn behind(&self) -> usize {
        self.behind.len()
    }

    /// **Walk from `now` to the nearest place `usable` accepts**, in `direction`.
    ///
    /// A place `usable` refuses — its thing is gone — is dropped from the history and its sentence is
    /// kept, and the walk carries on to the next one. `now` goes onto the opposite stack once, and only
    /// when a place was found, so a walk that finds nothing leaves the history as it was but for the
    /// places that were gone.
    pub fn walk(
        &mut self,
        direction: Direction,
        now: Place,
        usable: impl Fn(&Place) -> Result<(), String>,
    ) -> Walked {
        let mut skipped = Vec::new();
        loop {
            let from = match direction {
                Direction::Back => &mut self.behind,
                Direction::Forward => &mut self.ahead,
            };
            let Some(target) = from.pop() else {
                return Walked { to: None, skipped };
            };
            if let Err(why) = usable(&target) {
                skipped.push(why);
                continue;
            }
            match direction {
                Direction::Back => self.ahead.push(now),
                Direction::Forward => {
                    self.behind.push(now);
                    if self.behind.len() > NAV_HISTORY_MAX {
                        // Oldest first, so the front is what falls off.
                        self.behind.remove(0);
                    }
                }
            }
            return Walked { to: Some(target), skipped };
        }
    }

    /// **The place a walk is waiting to settle on**, for a test and for [`settle_the_place`].
    pub fn pending(&self) -> Option<&Place> {
        self.pending.as_ref().map(|(place, _)| place)
    }

    /// **Ask for a place to be settled.**
    pub fn walk_to(&mut self, place: Place) {
        self.pending = Some((place, false));
    }

    /// **A project change ends the history**: a place is a door and a row, and both belong to the
    /// project that has them.
    pub fn clear(&mut self) {
        self.behind.clear();
        self.ahead.clear();
        self.pending = None;
    }
}

/// **Step to a place, or say what is gone.** The one function both directions use to decide whether a
/// place can still be gone to.
///
/// It does not write the camera or the door itself: a workspace change **resets `Rig`**
/// (`screen::OWNERSHIP`), so a camera written on the frame the change is asked for is erased by the
/// change it was for. [`settle_the_place`] writes both, on the frame the door has landed — the same
/// two-step `reveal::take_the_reveal` takes for a row, one value over.
pub fn go(place: &Place, open: Option<&crate::project::OpenMap>) -> Result<(), String> {
    still_there(place, open)
}

/// **A workspace or panel key leaves the place behind** (NV-06, #383).
///
/// The issue names four moves that push: Go to anything, a cross-link, a clicked problem, and **a
/// workspace or tab switch**. The first three go through `reveal`'s door request, which records its own
/// place; a key goes straight to `open_door`, so this is the one that watches the two navigation
/// resources and records where the person was on the last frame they were elsewhere.
///
/// **`History::push` refuses a place it already holds**, so a key that follows a reveal — the reveal's
/// own push, then this one watching the same move — is one place and not two.
fn note_when_the_workspace_moves(
    workspace: Option<Res<Workspace>>,
    mode: Option<Res<Mode>>,
    rig: Option<Res<crate::view::Rig>>,
    selection: Option<Res<crate::selection::Selection>>,
    mut history: ResMut<History>,
    mut was: Local<Option<(Workspace, Mode, Place)>>,
    stops: Stops,
) {
    let (Some(workspace), Some(mode)) = (workspace.as_deref().copied(), mode.as_deref().copied()) else {
        return;
    };
    let mut here: Place = Place::of(Some(&workspace), Some(&mode), rig.as_deref(), selection.as_deref());
    stops.stamp(&mut here);
    // **Last frame's place is the one to push**, not this frame's: a workspace switch resets `Rig`
    // (`screen::door_state!` names it), so a place read on the frame the switch lands carries the
    // default camera rather than the one the person was looking through. The copy taken before the
    // crossing is the only honest answer to *where I was*.
    //
    // **Not while a walk is settling**: Back and Forward cross doors themselves, and pushing the place
    // a walk leaves would put the step just taken back on the stack — Back twice would then land where
    // Back once did.
    if let Some((last_workspace, last_mode, last_place)) = was.as_ref()
        && (*last_workspace, *last_mode) != (workspace, mode)
        && history.pending.is_none()
    {
        history.push(last_place.clone());
    }
    *was = Some((workspace, mode, here));
}

/// **Write the place a walk is waiting on, once its door has landed.**
///
/// `Rig` is the door's own state and a workspace change resets it, so the camera goes in *after* the
/// switch — and the selection is the reveal's to write, through the one `Shown` that names a row.
#[allow(clippy::too_many_arguments)]
fn settle_the_place(
    mut history: ResMut<History>,
    open: Option<Res<crate::project::OpenMap>>,
    workspace: Option<Res<Workspace>>,
    mode: Option<Res<Mode>>,
    mut rig: Option<ResMut<crate::view::Rig>>,
    mut framing: Option<ResMut<crate::editor::MapFraming>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut cue: Option<ResMut<crate::world_lists::SelectedCue>>,
    mut commands: Commands,
) {
    let Some((place, asked)) = history.pending.clone() else { return };
    if go(&place, open.as_deref()).is_err() {
        history.pending = None;
        return;
    }
    let here = workspace.as_deref().copied();
    let on = mode.as_deref().copied();
    let arrived = here == Some(place.workspace) && (on == Some(place.mode) || !place.workspace.shows(place.mode));
    if !arrived {
        if !asked {
            if let Some(state) = state.as_mut()
                && state.door_request.is_none()
                && !state.leaving
            {
                state.door_request = Some(crate::editor::DoorRequest {
                    workspace: place.workspace,
                    tab: Some(place.mode),
                });
                history.pending = Some((place, true));
            }
        } else if let Some(state) = state.as_mut() {
            state.status.problem(format!(
                "could not go back to the {} workspace, so the place is skipped",
                place.workspace.title()
            ));
            history.pending = None;
        }
        return;
    }
    // **The camera and the selection, once the door has landed.** The page itself is the door's.
    if let (Some(camera), Some(rig)) = (place.camera, rig.as_deref_mut()) {
        camera.restore(rig);
        // **The map's own entry frame must not take the camera back**: it is one shot per door and
        // runs a frame or two after the door lands, once the viewport has stopped changing size — so
        // it would put the whole map over the camera just restored. The follow settles it for the
        // same reason (`MapFraming::settle`).
        if let Some(framing) = framing.as_mut() {
            framing.settle();
        }
    }
    // **The selection is the Map's** (a placement id), so it is restored where the Map is edited, and
    // by the editor's one lock: the selection follows it a frame later, so the two cannot disagree.
    if place.workspace.edits_the_map()
        && let Some(id) = place.selection.clone()
    {
        commands.queue(move |world: &mut World| {
            crate::editor::lock_on_placement(world, &id);
        });
    }
    // **The stop, once the door has landed**: the file the person was reading, at the line they were on, and the cue the list marked. Both
    // are the workspace's own state, and a switch resets the workspace's panels, so they are written after it (as the camera is).
    if let Some(stop) = place.selection.as_deref() {
        if place.workspace == Workspace::Files
            && let Some(rest) = stop.strip_prefix(FILE_STOP)
        {
            let (rel, line) = rest.rsplit_once(':').map_or((rest, 0), |(rel, line)| (rel, line.parse().unwrap_or(0)));
            let rel = rel.to_owned();
            commands.queue(move |world: &mut World| crate::crosslinks::open_in_files(world, &rel, Some(line)));
        } else if place.workspace == Workspace::Sound
            && let Some(name) = stop.strip_prefix(CUE_STOP)
            && let Some(cue) = cue.as_mut()
        {
            cue.0 = Some(name.to_owned());
        }
    }
    history.pending = None;
}

/// **Push the places a reveal or a follow has just left** (NV-06, #383).
///
/// Two writers record a place rather than pushing it — `reveal::take_the_reveal` and `follow`'s framing
/// — because both are at Bevy's parameter cap and neither may take a third `ResMut`. This is the one
/// system that pushes them, in the order they were left, before the walk below reads the stack.
fn push_the_places_being_left(
    mut history: ResMut<History>,
    mut revealing: Option<ResMut<crate::reveal::Revealing>>,
    mut following: Option<ResMut<crate::follow::Following>>,
    stops: Stops,
) {
    if let Some(revealing) = revealing.as_mut() {
        for mut place in revealing.take_leaving() {
            stops.stamp(&mut place);
            history.push(place);
        }
    }
    if let Some(following) = following.as_mut() {
        for mut place in following.take_leaving() {
            stops.stamp(&mut place);
            history.push(place);
        }
    }
}

/// The plugin: the stack, the two chords, the two mouse buttons, and the project reset.
pub struct NavPlugin;

impl Plugin for NavPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<History>()
            .add_systems(
                Update,
                (
                    push_the_places_being_left,
                    walk_on_the_chords,
                    clear_the_history_when_the_project_changes,
                    note_when_the_workspace_moves,
                    settle_the_place,
                )
                    .chain()
                    .run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}

/// **Cmd/Ctrl+[ back, Cmd/Ctrl+] forward**, and the mouse's two side buttons — as `bevy_input` names
/// them (`MouseButton::Back`, `MouseButton::Forward`).
///
/// **Only while the tab has the keys.** With a text field focused (the finder's, a filter box, a
/// shell's) `Ctrl+[` is the field's — it is Esc in a shell on Linux — and a question on screen has the
/// keys too; the guard is the command palette's own (`command_palette::toggle`).
// **Thirteen reads and one write, and every read is a fact a place is made of or a gate on the keys**
// — the crate's own `reveal::take_the_reveal` takes twenty and says the same. Bundling them would be a
// second type for "where the editor is", which is exactly what a `Place` is.
#[allow(clippy::too_many_arguments)]
fn walk_on_the_chords(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    live: Res<Live>,
    confirm: Option<Res<crate::confirm::Confirm>>,
    mut history: ResMut<History>,
    mut revealing: Option<ResMut<crate::reveal::Revealing>>,
    mut following: Option<ResMut<crate::follow::Following>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    rig: Option<Res<crate::view::Rig>>,
    open: Option<Res<crate::project::OpenMap>>,
    workspace: Option<Res<Workspace>>,
    mode: Option<Res<Mode>>,
    selection: Option<Res<crate::selection::Selection>>,
    stops: Stops,
) {
    let modifier = [KeyCode::SuperLeft, KeyCode::SuperRight, KeyCode::ControlLeft, KeyCode::ControlRight]
        .into_iter()
        .any(|k| keys.pressed(k));
    // **Cmd/Ctrl+`[` back, Cmd/Ctrl+`]` forward**, and the mouse's two side buttons, as a browser
    // takes them.
    // **Not with Shift**: Cmd/Ctrl+Shift+`[` and `]` close and open a fold in the file editor (GD-12, #374), and one chord must
    // not do two things.
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let back = mouse.just_pressed(MouseButton::Back) || (modifier && !shifted && keys.just_pressed(KeyCode::BracketLeft));
    let forward = mouse.just_pressed(MouseButton::Forward) || (modifier && !shifted && keys.just_pressed(KeyCode::BracketRight));
    if !back && !forward {
        return;
    }
    if live.0 == Context::Typing || live.2 != Holder::Tab || confirm.as_deref().is_some_and(|c| c.is_open()) {
        return;
    }
    // **The person reached for the view**: whatever the finder or the follow had queued is theirs now,
    // and a walk that landed on a page while a reveal was crossing it would be two moves for one press.
    if let Some(revealing) = revealing.as_mut() {
        revealing.clear();
    }
    if let Some(following) = following.as_mut() {
        following.look_now();
    }
    // **The place being left is the present**, recorded by the walk itself so a Back from a page no
    // reveal ever pushed still has somewhere to return to — the editor that opened on the Map and was
    // walked away from by a workspace key.
    let mut now = Place::of(
        workspace.as_deref(),
        mode.as_deref(),
        rig.as_deref(),
        selection.as_deref(),
    );
    stops.stamp(&mut now);
    let direction = if back { Direction::Back } else { Direction::Forward };
    // **A place whose thing is gone is skipped, with a notice** — and the walk carries on.
    let walked = history.walk(direction, now, |place| go(place, open.as_deref()).and_then(|()| stops.gone(place)));
    if let Some(state) = state.as_mut() {
        for why in &walked.skipped {
            state.status.problem(format!("cannot go {}: {why}. The place is skipped.", direction.word()));
        }
        if walked.to.is_none() && walked.skipped.is_empty() {
            state.status.note(match direction {
                Direction::Back => "nothing behind to go back to",
                Direction::Forward => "nothing ahead to go forward to",
            });
        }
    }
    if let Some(place) = walked.to {
        history.walk_to(place);
    }
}

/// **Another project ends the history.**
fn clear_the_history_when_the_project_changes(
    project: Option<Res<crate::project::Project>>,
    mut history: ResMut<History>,
    mut seen: Local<Option<std::path::PathBuf>>,
) {
    let root = project.as_deref().map(|p| p.root.clone());
    if *seen == root {
        return;
    }
    *seen = root;
    history.clear();
}
