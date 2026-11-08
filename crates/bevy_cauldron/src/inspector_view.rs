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
//! # A selection of several pieces
//!
//! The view shows the **primary** (the lock's piece), and a field is **mixed** when another selected piece
//! differs from it there: [`PoseWidget`] carries gooey's `InspectMixed` naming those fields, so the row
//! reads "—" instead of one piece's number. An edit of the facing or the lift is applied to **every**
//! selected piece: one `placement_set_pose` per distinct old facing and tip (a batch is undone by one
//! inverse, so it refuses pieces that were not facing alike, and typing a facing must not change a tip),
//! each taking `ids` for the pieces that share one, and the entries those calls leave are collapsed into
//! one undo step (`EditorState::group_turn`). A
//! position is not edited by typing here, so an `x` or `z` edit of several pieces does nothing.
//!
//! # The panel
//!
//! [`spawn_the_block`] puts a heading and gooey's `inspector(...)` over the view at the top of the Map
//! door's INSPECTOR panel (the scroll list `MapControlsPane`), and the block is drawn only while
//! something is selected.

use bevy::prelude::*;
use bevy::reflect::Reflect;

use crate::editor::{EditorState, MapControlsPane, TargetLock};
use crate::ops::{self, Ctx, Source};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::selection::Selection;

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

/// **The block in the INSPECTOR panel**: a heading and the reflect inspector. Drawn only while something
/// is selected, so an empty selection does not show a row of zeroes.
#[derive(Component)]
pub struct PoseBlock;

/// **gooey's reflect inspector over the view.** Its `InspectMixed` names the fields on which the selected
/// pieces differ, and an edit made in it arrives as an `InspectorChange` on this entity.
#[derive(Component)]
pub struct PoseWidget;

/// The fields of [`PoseView`] a mixed selection can differ on, by reflect path.
const POSE_FIELDS: [&str; 4] = ["x", "z", "yaw", "lift"];

/// Two floats are the same pose value when they are within a hair, so a float written and read back
/// is not "mixed" with itself.
const SAME_POSE_EPSILON: f32 = 1e-4;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PoseView>()
            .init_resource::<PoseInspector>()
            .add_systems(OnEnter(Screen::Editor), spawn_the_view.after(crate::chrome::FrameSystems))
            .add_systems(
                Update,
                (spawn_the_block, mirror_the_selection).chain().run_if(in_state(Screen::Editor)),
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

/// **The block goes into the Map door's INSPECTOR panel** the frame the panel appears, first among the
/// panel's rows so it is on screen without scrolling.
fn spawn_the_block(mut commands: Commands, panes: Query<Entity, Added<MapControlsPane>>, slot: Res<PoseInspector>) {
    let Some(view) = slot.0 else { return };
    for pane in &panes {
        let block = commands
            .spawn((
                PoseBlock,
                Name::new("pose block"),
                Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(crate::chrome::GAP_TIGHT), display: Display::None, ..default() },
            ))
            .with_children(|block| {
                crate::chrome::list_heading(block, "POSE");
                block.spawn((
                    PoseWidget,
                    Name::new("pose inspector"),
                    bevy_gooey::inspector(bevy_gooey::InspectTarget::component::<PoseView>(view)),
                    bevy_gooey::InspectMixed::default(),
                ));
            })
            .id();
        commands.entity(pane).insert_children(0, &[block]);
    }
}

/// **The selected pieces, primary first**: the selection when it holds the lock's piece, else the lock's
/// piece alone. A piece that left the map is not in it.
fn selected_ids(lock: &TargetLock, selection: Option<&Selection>, open: &OpenMap) -> Vec<String> {
    let Some(primary) = lock.placement_id().map(str::to_owned) else { return Vec::new() };
    let mut ids: Vec<String> = selection
        .map(|s| s.ids().to_vec())
        .filter(|ids| ids.first() == Some(&primary))
        .unwrap_or_else(|| vec![primary]);
    ids.retain(|id| open.map.placements.iter().any(|p| p.id == *id));
    ids
}

/// **The fields follow the selection.** The locked placement (else the only one selected, else nothing)
/// is what the inspector shows; the view is written only when it differs, so a settled selection costs
/// a comparison. With several pieces selected, the fields on which they differ are named to gooey as
/// mixed, and the block is drawn only while something is selected.
fn mirror_the_selection(
    open: Option<Res<OpenMap>>,
    lock: Option<Res<TargetLock>>,
    selection: Option<Res<Selection>>,
    mut views: Query<&mut PoseView>,
    mut blocks: Query<&mut Node, With<PoseBlock>>,
    mut widgets: Query<(Entity, Option<&bevy_gooey::InspectMixed>), With<PoseWidget>>,
    mut commands: Commands,
) {
    let (Some(open), Some(lock)) = (open, lock) else { return };
    let ids = selected_ids(&lock, selection.as_deref(), &open);
    let display = if ids.is_empty() { Display::None } else { Display::Flex };
    for mut node in &mut blocks {
        if node.display != display {
            node.display = display;
        }
    }
    let Some(primary) = ids.first().and_then(|id| open.map.placements.iter().find(|p| p.id == *id)) else { return };
    let want = PoseView::of(primary);
    for mut view in &mut views {
        if *view != want {
            *view = want;
        }
    }
    let mixed: Vec<String> = POSE_FIELDS
        .iter()
        .filter(|field| {
            ids.iter().skip(1).filter_map(|id| open.map.placements.iter().find(|p| p.id == *id)).any(|other| {
                let other = PoseView::of(other);
                let (a, b) = match **field {
                    "x" => (want.x, other.x),
                    "z" => (want.z, other.z),
                    "yaw" => (want.yaw, other.yaw),
                    _ => (want.lift, other.lift),
                };
                (a - b).abs() > SAME_POSE_EPSILON
            })
        })
        .map(|field| (*field).to_owned())
        .collect();
    for (widget, now) in &mut widgets {
        if now.map(|m| &m.0) != Some(&mixed) {
            commands.entity(widget).insert(bevy_gooey::InspectMixed(mixed.clone()));
        }
    }
}

/// The selected pieces that face one way (`yaw_bits` is the facing's bit pattern, so equal means equal) and are tipped
/// alike: one `placement_set_pose` can take them together and one inverse can put them back.
struct SameStance {
    yaw_bits: u32,
    tip: [u8; 2],
    ids: Vec<String>,
}

/// **An edit of the view is the operation that owns it.**
///
/// `placement_set_pose` carries the facing and the tip together, and `placement_set_lift` the lift: the
/// view has no tip field yet, so a pose edit keeps each piece's own tip as it is. One edit is one
/// dispatch per distinct old facing and tip (one, for pieces that stand the same way) and therefore one undo step,
/// exactly as the key that does the same thing; several pieces' entries are collapsed into one.
///
/// **The map goes through `open`**, not read from disk: a dispatch with an empty `open` resolves the
/// placement against the file, so an edit made between two saves would be refused or applied to the
/// wrong row. The editor's own note dispatch takes the same care (`editor::dispatch_note`).
#[allow(clippy::too_many_arguments)]
fn pose_edits_are_operations(
    change: On<bevy_gooey::InspectorChange>,
    inspectors: Res<PoseInspector>,
    widgets: Query<(), With<PoseWidget>>,
    views: Query<&PoseView>,
    project: Option<Res<Project>>,
    open: Option<ResMut<OpenMap>>,
    state: Option<ResMut<EditorState>>,
    lock: Option<Res<TargetLock>>,
    selection: Option<Res<Selection>>,
    due: Option<ResMut<crate::watch::Due>>,
) {
    let (Some(project), Some(mut open), Some(mut state), Some(lock)) = (project, open, state, lock)
    else {
        return;
    };
    // The change is on the view (a test, or a host that edits it directly) or on gooey's inspector over it.
    let ours = inspectors.0 == Some(change.entity) || widgets.contains(change.entity);
    let Some(view_entity) = inspectors.0.filter(|_| ours) else { return };
    let Ok(view) = views.get(view_entity) else { return };
    let ids = selected_ids(&lock, selection.as_deref(), &open);
    if ids.is_empty() {
        return;
    }
    let lift_edit = change.path.ends_with("lift");
    let facing_edit = change.path.ends_with("yaw");
    // A position is not typed in here, and a batch never moves pieces: only a facing or a lift is applied to a set.
    if ids.len() > 1 && !(lift_edit || facing_edit) {
        return;
    }
    // (operation, its parameters, what to say if it is refused)
    let mut calls: Vec<(&'static str, serde_json::Value, String)> = Vec::new();
    if lift_edit {
        for id in &ids {
            calls.push(("placement_set_lift", serde_json::json!({ "id": id, "lift": view.lift }), format!("lift {id}")));
        }
    } else {
        // **One call per distinct old facing and tip.** A batch is undone by one inverse, so
        // `placement_set_pose` refuses pieces that were not facing the same way; and a typed facing must
        // never change how a piece is tipped, so each group keeps its own tip. The calls are one act
        // again below, in one undo step.
        let mut by_pose: Vec<SameStance> = Vec::new();
        for id in &ids {
            let Some(placed) = open.map.placements.iter().find(|p| p.id == *id) else { continue };
            let (yaw_bits, tip) = (placed.yaw.to_bits(), [placed.tip.0, placed.tip.1]);
            match by_pose.iter_mut().find(|g| g.yaw_bits == yaw_bits && g.tip == tip) {
                Some(group) => group.ids.push(id.clone()),
                None => by_pose.push(SameStance { yaw_bits, tip, ids: vec![id.clone()] }),
            }
        }
        for SameStance { tip, ids: group, .. } in by_pose {
            let (params, phrase) = match group.as_slice() {
                [one] => (serde_json::json!({ "id": one, "yaw": view.yaw, "tip": tip }), format!("turn {one}")),
                many => (serde_json::json!({ "ids": many, "yaw": view.yaw, "tip": tip }), format!("turn {} pieces", many.len())),
            };
            calls.push(("placement_set_pose", params, phrase));
        }
    }
    let mut logged: Vec<String> = Vec::new();
    let mut drawn_stale = false;
    for (name, params, phrase) in calls {
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
                drawn_stale |= outcome.saved;
                logged.push(operation_id.clone());
                state.record_logged(operation_id, outcome.summary, false);
            }
            Ok(ops::Applied::Refused(e)) | Err(e) => {
                state.status.raise(&format!("cannot {phrase}: "), e.problem())
            }
        }
    }
    // Several calls are still one act: Cmd+Z takes the whole edit back.
    state.group_turn(&logged);
    // **The pieces on screen are drawn again**, by the one flag that means "the map changed under the
    // entities": the map is the data, and without this the inspector said 90 while the sofa kept
    // facing the way it did (`editor::redraw_the_map`, the path every other operation's change takes).
    if drawn_stale && let Some(mut due) = due {
        due.map_replaced = true;
    }
}
