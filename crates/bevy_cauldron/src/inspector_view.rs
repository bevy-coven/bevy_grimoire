//! **The inspector: the selection's pose, edited through gooey's reflect inspector** (UI-50, #236).
//!
//! `docs/ui/vision.md` §4 and §8 put the inspector in the right dock, and #236 asks for one thing the
//! hand-built panel does not do: the selection's properties shown by **gooey's reflect inspector**, an
//! edit dispatching the operation that owns that field.
//!
//! # Why a view type, and not `Placed` itself
//!
//! `bevy_gooey::inspector` edits a **reflected component on an entity**. The map's rows live in
//! `bevy_grimoire_core::map::Placed`, which is plain data in an engine-free crate (`engine_free.rs`
//! refuses any `bevy` dependency there), so it cannot carry `Reflect` and must not. It is also not what
//! the inspector should write: a placement's pose is changed by `placement_set_pose` and
//! `placement_set_lift`, which own the wrapping, the collision check and the undo step, and a reflected
//! write straight onto a row would be a second path that skips all three.
//!
//! So [`PoseView`] is a **reflected view of the selection's pose**, spawned on one entity of its own; the
//! inspector edits *it*, and [`pose_edits_are_operations`] turns the change into the operation. That is
//! the same shape `filter.rs` and `anim_tab.rs` already use for their read-only views: the editor's own
//! type, not the crate's, and one writer per fact.
//!
//! # One selection at a time
//!
//! A multi-selection is **not** a view with mixed values yet: the inspector shows the locked piece and
//! says how many others are selected. #236's first acceptance item ("two pieces with different turns
//! show —; typing 90 turns both through one log line") is the next slice, and it needs a
//! `placement_set_pose` that takes a list — an operation change, not an inspector change.

use bevy::prelude::*;
use bevy::reflect::Reflect;

use crate::editor::{EditorState, TargetLock};
use crate::ops::{self, Ctx, Source};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;

/// **The selection's pose, as the reflect inspector edits it.**
///
/// One entity carries it, spawned with [`InspectorPlugin`]. It is written from the map whenever the
/// selection or the map changes, and read *back* only when gooey reports an edit — so the fields are
/// the inspector's state, never the editor's truth.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub struct PoseView {
    /// Where the piece stands, in map metres.
    pub x: f32,
    /// The other floor-plan axis.
    pub z: f32,
    /// The facing, in degrees.
    pub yaw: f32,
    /// The authored lift, in metres, above the height the mount resolves.
    pub lift: f32,
}

impl Default for PoseView {
    fn default() -> Self {
        Self { x: 0.0, z: 0.0, yaw: 0.0, lift: 0.0 }
    }
}

impl PoseView {
    /// The pose of a placement, as the view shows it.
    pub fn of(placed: &bevy_grimoire_core::map::Placed) -> Self {
        Self { x: placed.at.0, z: placed.at.1, yaw: placed.yaw, lift: placed.lift }
    }
}

/// The entity the inspector edits, so a system can find it without a second resource.
#[derive(Resource, Default)]
pub struct PoseInspector(pub Option<Entity>);

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PoseView>()
            .init_resource::<PoseInspector>()
            .add_systems(OnEnter(Screen::Editor), spawn_the_view.after(crate::chrome::FrameSystems))
            .add_systems(
                Update,
                mirror_the_selection.run_if(in_state(Screen::Editor)),
            )
            .add_observer(pose_edits_are_operations);
    }
}

/// **The inspector's own entity.** It is not a panel: the panel that shows it is built elsewhere, and
/// this is the value it reflects.
fn spawn_the_view(mut commands: Commands, mut slot: ResMut<PoseInspector>) {
    let entity = commands.spawn((Name::new("PoseInspector"), PoseView::default())).id();
    slot.0 = Some(entity);
}

/// **The fields follow the selection.** The locked placement (else the only one selected, else nothing)
/// is what the inspector shows; the view is written only when it differs, so a settled selection costs
/// a comparison.
fn mirror_the_selection(
    open: Option<Res<OpenMap>>,
    lock: Option<Res<TargetLock>>,
    mut views: Query<&mut PoseView>,
) {
    let (Some(open), Some(lock)) = (open, lock) else { return };
    let Some(id) = lock.placement_id().map(str::to_owned) else { return };
    let Some(placed) = open.map.placements.iter().find(|p| p.id == id) else { return };
    let want = PoseView::of(placed);
    for mut view in &mut views {
        if *view != want {
            *view = want;
        }
    }
}

/// **An edit of the view is the operation that owns it.**
///
/// `placement_set_pose` carries the facing and the tip together, and `placement_set_lift` the lift: the
/// view has no tip field yet, so a pose edit keeps the placement's own tip as it is. One edit is one
/// dispatch and therefore one undo step, exactly as the key that does the same thing.
///
/// **The map goes through `open`**, not read from disk: a dispatch with an empty `open` resolves the
/// placement against the file, so an edit made between two saves would be refused or applied to the
/// wrong row. The editor's own note dispatch takes the same care (`editor::dispatch_note`).
fn pose_edits_are_operations(
    change: On<bevy_gooey::InspectorChange>,
    inspectors: Res<PoseInspector>,
    views: Query<&PoseView>,
    project: Option<Res<Project>>,
    open: Option<ResMut<OpenMap>>,
    state: Option<ResMut<EditorState>>,
    lock: Option<Res<TargetLock>>,
) {
    let (Some(project), Some(mut open), Some(mut state), Some(lock)) = (project, open, state, lock)
    else {
        return;
    };
    {
        if inspectors.0 != Some(change.entity) {
            return;
        }
        let Ok(view) = views.get(change.entity) else { return };
        let Some(id) = lock.placement_id().map(str::to_owned) else { return };
        let Some(placed) = open.map.placements.iter().find(|p| p.id == id) else { return };
        let (name, params, phrase) = if change.path.ends_with("lift") {
            (
                "placement_set_lift",
                serde_json::json!({ "id": id, "lift": view.lift }),
                format!("lift {id}"),
            )
        } else {
            (
                "placement_set_pose",
                serde_json::json!({ "id": id, "yaw": view.yaw, "tip": placed.tip }),
                format!("turn {id}"),
            )
        };
        let mut ctx = Ctx {
            view: None,
            trust_file: None,
            project: Some(project.clone()),
            open: Some(std::mem::replace(
                &mut *open,
                OpenMap::new(bevy_grimoire_core::map::Map::default(), std::path::PathBuf::new()),
            )),
            source: Source::Human,
            root: None,
            session: ops::session_id(),
            proposal: None,
        };
        let outcome = ops::dispatch(&mut ctx, name, &params);
        if let Some(fresh) = ctx.open.take() {
            *open = fresh;
        }
        match outcome {
            Ok(ops::Applied::Done { outcome, operation_id, .. }) => {
                // **One edit is one undo step**, on the stack Cmd+Z walks: the same line every other
                // dispatch site records (`editor::dispatch_note`), so an inspector edit is not a change
                // the history never saw.
                state.status.note(outcome.summary.clone());
                state.record_logged(operation_id, outcome.summary, false);
            }
            Ok(ops::Applied::Refused(e)) | Err(e) => {
                state.status.raise(&format!("cannot {phrase}: "), e.problem())
            }
        }
    }
}
