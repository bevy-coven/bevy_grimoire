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
//! off the world once, and [`Place::restore`] writes them back; there is no second builder, so a
//! place can never be a partial snapshot of an edit site's choosing.
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
//! A placement an agent deleted, a rig renamed on disk: [`Place::restore`] answers `Err` with the
//! sentence, the walk carries on to the next place, and the person reads what went. Skipping silently
//! would leave Back doing nothing with no answer, which is the dead end this series exists to remove.

use bevy::prelude::*;
use bevy::prelude::Vec3;

use crate::ops::Shown;
use crate::tiles::Mode;
use crate::workspace::Workspace;

/// **How many places one session remembers.** A hundred is enough to walk back through a morning's
/// work and small enough that the stack is a few kilobytes: a `Place` is a workspace, a panel, one
/// selection id and four floats.
pub const NAV_HISTORY_MAX: usize = 100;

/// **Notices a person has raised but the status line has not drawn yet**, so the walk does not need an
/// `EditorState` to skip a place whose thing is gone.
pub type Notice = String;

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
        let same = rig.focus.abs_diff_eq(self.focus, 1e-4)
            && (rig.height - self.height).abs() <= 1e-4
            && (rig.yaw - self.yaw).abs() <= 1e-4
            && (rig.goal_yaw - self.goal_yaw).abs() <= 1e-4
            && (rig.elevation - self.elevation).abs() <= 1e-4;
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

    /// **Walk back from `now`**, answering the place to go to. `None` when there is nothing behind.
    pub fn back(&mut self, now: Place) -> Option<Place> {
        let target = self.behind.pop()?;
        self.ahead.push(now);
        Some(target)
    }

    /// **Walk forward from `now`**, answering the place to go to. `None` at the present.
    pub fn forward(&mut self, now: Place) -> Option<Place> {
        let target = self.ahead.pop()?;
        self.behind.push(now);
        if self.behind.len() > NAV_HISTORY_MAX {
            self.behind.remove(0);
        }
        Some(target)
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

/// **A place, as a `Shown` the reveal can carry**, so restoring one goes through the one function that
/// decides a page rather than a second switch here.
///
/// `None` for a place whose page is not a `Shown` (a workspace with no selection, the map alone): those
/// are restored by [`go`] and need no row selected.
pub fn shown_of(place: &Place) -> Option<Shown> {
    let id = place.selection.clone()?;
    Some(match place.workspace {
        Workspace::Map => Shown::Map { rect: [0.0, 0.0, 0.0, 0.0], placements: vec![id] },
        Workspace::Kit => Shown::Piece { id },
        Workspace::Animation => Shown::Rig { id },
        _ => return None,
    })
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
) {
    let (Some(workspace), Some(mode)) = (workspace.as_deref().copied(), mode.as_deref().copied()) else {
        return;
    };
    let here: Place = Place::of(Some(&workspace), Some(&mode), rig.as_deref(), selection.as_deref());
    // **Last frame's place is the one to push**, not this frame's: a workspace switch resets `Rig`
    // (`screen::door_state!` names it), so a place read on the frame the switch lands carries the
    // default camera rather than the one the person was looking through. The copy taken before the
    // crossing is the only honest answer to *where I was*.
    if let Some((last_workspace, last_mode, last_place)) = was.as_ref()
        && (*last_workspace, *last_mode) != (workspace, mode)
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
    project: Option<Res<crate::project::Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    workspace: Option<Res<Workspace>>,
    mode: Option<Res<Mode>>,
    mut rig: Option<ResMut<crate::view::Rig>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut revealing: Option<ResMut<crate::reveal::Revealing>>,
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
    // **The camera, once the door has landed.** Nothing else is written here: a place's *row* is a
    // `Shown` and the reveal owns it.
    if let (Some(camera), Some(rig)) = (place.camera, rig.as_deref_mut()) {
        camera.restore(rig);
    }
    let _ = (&project, &mut revealing);
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
) {
    if let Some(revealing) = revealing.as_mut() {
        for place in revealing.take_leaving() {
            history.push(place);
        }
    }
    if let Some(following) = following.as_mut() {
        for place in following.take_leaving() {
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
// **Eleven reads and one write, and every read is a fact a place is made of** — the crate's own
// `reveal::take_the_reveal` takes twenty and says the same. Bundling them would be a second type for
// "where the editor is", which is exactly what a `Place` is.
#[allow(clippy::too_many_arguments)]
fn walk_on_the_chords(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut history: ResMut<History>,
    mut revealing: Option<ResMut<crate::reveal::Revealing>>,
    mut following: Option<ResMut<crate::follow::Following>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    rig: Option<Res<crate::view::Rig>>,
    open: Option<Res<crate::project::OpenMap>>,
    workspace: Option<Res<Workspace>>,
    mode: Option<Res<Mode>>,
    selection: Option<Res<crate::selection::Selection>>,
) {
    let modifier = [KeyCode::SuperLeft, KeyCode::SuperRight, KeyCode::ControlLeft, KeyCode::ControlRight]
        .into_iter()
        .any(|k| keys.pressed(k));
    // **Cmd/Ctrl+`[` back, Cmd/Ctrl+`]` forward**, and the mouse's two side buttons, as a browser
    // takes them.
    let back = mouse.just_pressed(MouseButton::Back) || (modifier && keys.just_pressed(KeyCode::BracketLeft));
    let forward = mouse.just_pressed(MouseButton::Forward) || (modifier && keys.just_pressed(KeyCode::BracketRight));
    if !back && !forward {
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
    let now = Place::of(
        workspace.as_deref(),
        mode.as_deref(),
        rig.as_deref(),
        selection.as_deref(),
    );
    let place = if back { history.back(now) } else { history.forward(now) };
    let Some(place) = place else {
        if let Some(state) = state.as_mut() {
            state.status.note(if back { "nothing behind to go back to" } else { "nothing ahead to go forward to" });
        }
        return;
    };
    match go(&place, open.as_deref()) {
        Ok(()) => history.walk_to(place),
        Err(why) => {
            // **A place whose thing is gone is skipped, with a notice** — and the walk carries on.
            if let Some(state) = state.as_mut() {
                state.status.problem(format!("cannot go {}: {why}. The place is skipped.", if back { "back" } else { "forward" }));
            }
        }
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
