//! **The person's half of E-014: arm, drag a rectangle, type one line, Enter saves a capture.**
//!
//! A snip is a capture, not a registry operation: nothing here changes a map, so no operation is
//! registered for it. `Cmd+\` arms it. A press and a release over the window mark a rectangle, and
//! the release asks for a one-line note. Enter confirms and the window is captured; Escape cancels
//! and writes nothing.
//!
//! **What is written.** `<root>/target/loop/captures/snips/snip-<ms>.png`, the crop of the window
//! inside the rectangle, and `snip-<ms>.json`, its metadata (map, tool, camera, cursor, placements
//! inside the rectangle, the note). Both go through [`bevy_grimoire_snip::write_snip`], the same code
//! `grimoire_snip` uses, and `create_new` means an earlier snip is never replaced. Nothing is written
//! under `assets/grimoire`, which games load.
//!
//! **A headless app cannot write the picture.** It has no render world, so no screenshot is ever
//! read back. A confirmed snip there is refused in `Status::problem`, with the reason, and no file
//! is created. The gesture is proved headless; the bytes on disk are proved by the crate's test and
//! by a live run.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::input::ButtonInput;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
// `Pressed` is the button layer's "a press is in flight" marker; arming the snip clears it — see
// `snip_gates_picking`.
use bevy::ui::Pressed;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::{PrimaryWindow, Window};
use bevy_grimoire_snip::{CameraView, PlacedIn, SnipRecord, write_snip};

use crate::chrome;
use crate::editor::{EditorState, Placement, StampedPiece};
use crate::keys::{self, Action, Live, Phase};
use crate::project::{OpenMap, Project};
use crate::sequencer::CanRender;
use crate::view::{MainCamera, Pointer, cursor_ground};

/// Where snips are written, relative to the project root. Under `target/`, never `assets/grimoire`.
pub const SNIP_DIR: &str = "target/loop/captures/snips";

/// The smallest drag that is a picture. Below this a release is a slip of the hand.
const MIN_SIDE_PX: f32 = 8.0;
/// How far above the drag rectangle's top edge its label rides, in logical px (negative is up).
const SNIP_LABEL_RISE_PX: f32 = -22.0;

/// The longest note the field keeps.
const NOTE_CAP: usize = 200;

/// A rectangle waiting for its note. `rect` is `[x0, y0, x1, y1]` in surface pixels, the editor's render target, which is physical.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub rect: [f32; 4],
    /// Where the button came up, in surface pixels. The record reads the cursor on the ground here, so
    /// moving the pointer while the note is typed does not move the record.
    pub release: Vec2,
    pub note: String,
}

/// Where a snip is, from arming to saving.
#[derive(Resource, Debug, Default, Clone)]
pub struct SnipEdit {
    /// Armed by `Cmd+\`, waiting for a drag.
    pub armed: bool,
    /// Where the drag began, while the button is down.
    pub from: Option<Vec2>,
    /// Released and waiting for its note.
    pub pending: Option<Pending>,
    /// Confirmed with Enter, waiting for [`snip_commit`] to take it in the same frame.
    pub confirmed: Option<Pending>,
}

impl SnipEdit {
    /// Does the snip own the pointer right now? Placement drivers yield to it, so a press that
    /// begins a drag cannot also place a piece.
    pub fn claims_pointer(&self) -> bool {
        self.armed || self.from.is_some() || self.pending.is_some()
    }

    fn disarm(&mut self) {
        self.armed = false;
        self.from = None;
    }
}

/// The rectangle with its corners in order, whichever way the drag went.
fn rect_between(a: Vec2, b: Vec2) -> [f32; 4] {
    [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
}

/// The picture's pixel rectangle. `rect` is the drag in surface pixels, which the pointer reports and which is already physical, so it is rounded once and never scaled again.
fn picture_px(rect: [f32; 4]) -> [i64; 4] {
    rect.map(|v| v.round() as i64)
}

/// Is a point inside a rectangle, edges included? Both are in surface pixels.
fn under(rect: [f32; 4], at: Vec2) -> bool {
    at.x >= rect[0] && at.x <= rect[2] && at.y >= rect[1] && at.y <= rect[3]
}

/// A surface rectangle in the window's logical pixels: the surface is the logical size times the scale factor.
fn logical_px(surface: [f32; 4], scale: f32) -> [f32; 4] {
    surface.map(|v| v / scale)
}

/// `Cmd+\` arms the snip, and pressed again it disarms. Escape disarms while no note is open.
pub fn snip_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<Live>,
    mut snip: ResMut<SnipEdit>,
    mut state: ResMut<EditorState>,
    // **The project, because the key dispatches an operation and the operation is logged**
    // (2026-11-03, #224). Without it the context had no root, `ops::dispatch` could not append its
    // line, `take_back` reported a refusal — and the status painted a red
    // *"the operation could not be logged, so nothing was changed: no project is open, so there is
    // nowhere to write the log."* over the editor on every `Cmd+\`. Measured on the 2026-11-03
    // hero recording: the banner is on every frame after the key, and the tool **was** armed, so
    // the sentence was false in both halves. Every other key path that dispatches an operation
    // carries the project the same way (`editor::environment_key`, `set_yaw`, `set_tip`).
    project: Option<Res<crate::project::Project>>,
) {
    if keys::just_pressed(&keyboard, *live, Action::Snip) {
        // **The key dispatches the operation an agent calls** (E-002, #129). `snip_arm` owns the toggle;
        // this carries the view in and applies the answer. A drag in progress is cancelled first, which
        // is the key's own business and not a value an agent can hold.
        if snip.from.is_some() {
            snip.disarm();
        }
        let open = snip.armed;
        let mut ctx = crate::ops::Ctx {
            project: project.as_deref().cloned(),
            source: crate::ops::Source::Human,
            session: crate::ops::session_id(),
            view: Some(crate::ops_view::View { snip_armed: open, ..Default::default() }),
            ..crate::ops::Ctx::default()
        };
        let outcome = crate::ops::dispatch(&mut ctx, "snip_arm", &serde_json::json!({}));
        if let Some(view) = ctx.view {
            snip.armed = view.snip_armed;
        }
        match outcome {
            Ok(crate::ops::Applied::Done { outcome, .. }) => state.status.note(outcome.summary),
            Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
        }
        return;
    }
    if (snip.armed || snip.from.is_some()) && keyboard.just_pressed(KeyCode::Escape) {
        snip.disarm();
        state
            .status
            .note("snip cancelled, nothing was saved".to_owned());
    }
}

/// A press begins the rectangle and a release ends it. Releasing too small a rectangle is a
/// refusal, in the problem slot, and the snip stays armed so the person can drag again.
pub fn snip_pointer(
    pointer: Res<Pointer>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut snip: ResMut<SnipEdit>,
    mut state: ResMut<EditorState>,
) {
    if !snip.armed && snip.from.is_none() {
        return;
    }
    let Some(here) = pointer.0 else {
        // The pointer has left the window. A button that comes up out there ends the drag, with no
        // picture, and the snip stays armed so the person can drag again inside the window.
        if snip.from.is_some() && mouse.just_released(MouseButton::Left) {
            snip.from = None;
            state.status.problem(
                "the drag ended outside the window, so nothing was snipped. Drag again inside the window."
                    .to_owned(),
            );
        }
        return;
    };
    if snip.from.is_none() {
        if mouse.just_pressed(MouseButton::Left) {
            snip.from = Some(here);
        }
        return;
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(from) = snip.from.take() else {
        return;
    };
    let rect = rect_between(from, here);
    if rect[2] - rect[0] < MIN_SIDE_PX || rect[3] - rect[1] < MIN_SIDE_PX {
        state.status.problem(
            "that area is too small to be a picture, so nothing was snipped. Drag a wider rectangle."
                .to_owned(),
        );
        return;
    }
    snip.armed = false;
    snip.pending = Some(Pending {
        rect,
        release: here,
        note: String::new(),
    });
}

/// While a note is open, every key belongs to it. Enter confirms, Escape cancels and writes
/// nothing, and the rest edit the line.
pub fn snip_note_keys(
    mut events: MessageReader<KeyboardInput>,
    mut snip: ResMut<SnipEdit>,
    mut state: ResMut<EditorState>,
) {
    if snip.pending.is_none() {
        // Drain before leaving, as `pin_reason_keys` does, so the key that opened the note is not
        // read as its first character next frame.
        events.clear();
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                snip.confirmed = snip.pending.take();
                return;
            }
            Key::Escape => {
                snip.pending = None;
                state
                    .status
                    .note("snip cancelled, nothing was saved".to_owned());
                return;
            }
            Key::Backspace => {
                if let Some(p) = snip.pending.as_mut() {
                    p.note.pop();
                }
            }
            Key::Space => {
                if let Some(p) = snip.pending.as_mut()
                    && p.note.chars().count() < NOTE_CAP
                {
                    p.note.push(' ');
                }
            }
            Key::Character(c) => {
                if let Some(p) = snip.pending.as_mut() {
                    for ch in c.chars().filter(|ch| !ch.is_control()) {
                        if p.note.chars().count() < NOTE_CAP {
                            p.note.push(ch);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Every placement, and every stamped row, whose viewport point is inside `rect`, as a snip lists
/// them. A stamped row is listed under its row id with the stamp it belongs to, because the stamp
/// is what an agent acts on (`stamp_remove` takes it).
pub fn placements_under(
    rect: [f32; 4],
    cam: &Camera,
    cam_tf: &GlobalTransform,
    placed: &Query<(&Placement, &GlobalTransform)>,
    stamped: &Query<(&StampedPiece, &GlobalTransform)>,
) -> Vec<PlacedIn> {
    let inside = |at: Vec3| {
        cam.world_to_viewport(cam_tf, at)
            .is_ok_and(|p| under(rect, p))
    };
    let mut out = Vec::new();
    for (placement, tf) in placed {
        if inside(tf.translation()) {
            out.push(PlacedIn {
                id: placement.0.clone(),
                world: tf.translation().to_array(),
                stamp: None,
            });
        }
    }
    for (piece, tf) in stamped {
        if inside(tf.translation()) {
            out.push(PlacedIn {
                id: piece.row.clone(),
                world: tf.translation().to_array(),
                stamp: Some(piece.stamp.clone()),
            });
        }
    }
    out
}

/// Takes a confirmed snip, gathers its metadata from the editor as it stands, and asks the
/// window for a picture. The file is written when the readback lands, by the observer.
///
/// A headless app has no render world ([`CanRender`] is false or absent), so the snip is refused
/// here, before anything is asked of the renderer.
#[allow(clippy::too_many_arguments)]
pub fn snip_commit(
    mut snip: ResMut<SnipEdit>,
    mut state: ResMut<EditorState>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    can_render: Option<Res<CanRender>>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    placed: Query<(&Placement, &GlobalTransform)>,
    stamped: Query<(&StampedPiece, &GlobalTransform)>,
    mut commands: Commands,
) {
    let Some(pending) = snip.confirmed.take() else {
        return;
    };
    let Some(project) = project.as_deref() else {
        state
            .status
            .problem("no project is open, so nothing was saved.".to_owned());
        return;
    };
    if !can_render.is_some_and(|c| c.0) {
        state.status.problem(
            "this editor has no render world, so no picture could be taken; nothing was saved."
                .to_owned(),
        );
        return;
    }
    let Some(window) = window else {
        state
            .status
            .problem("there is no window to take a picture of; nothing was saved.".to_owned());
        return;
    };
    let scale = window.resolution.scale_factor();
    let r = pending.rect;
    let physical = picture_px(r);

    let taken_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let stem = format!("snip-{taken_at_ms}");
    let map = open
        .as_deref()
        .map(|o| o.map.name.clone())
        .unwrap_or_default();

    let mut camera_view = None;
    let mut cursor_world = None;
    let mut placements = Vec::new();
    if let Some(camera) = camera.as_deref() {
        let (cam, cam_tf) = *camera;
        camera_view = Some(CameraView {
            translation: cam_tf.translation().to_array(),
            forward: cam_tf.forward().as_vec3().to_array(),
        });
        cursor_world = cursor_ground(Some(pending.release), cam, cam_tf).map(|v| v.to_array());
        placements = placements_under(r, cam, cam_tf, &placed, &stamped);
    }

    let record = SnipRecord {
        stem: stem.clone(),
        taken_at_ms,
        map,
        active_tool: format!("{:?}", state.tool),
        note: pending.note,
        rect_window_px: logical_px(r, scale),
        scale_factor: scale,
        rect_picture_px: physical,
        size: [0, 0],
        picture: [0, 0],
        picture_file: String::new(),
        camera: camera_view,
        cursor_world,
        placements,
    };
    let dir = project.root.join(SNIP_DIR);
    let root: PathBuf = project.root.clone();

    if let Err(e) = bevy_grimoire_snip::write_pending(&dir, &record) {
        state.status.problem(e);
        return;
    }
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut state: ResMut<EditorState>| {
            let rgba = match shot.image.clone().try_into_dynamic() {
                Ok(dynamic) => dynamic.to_rgba8(),
                Err(e) => {
                    bevy_grimoire_snip::clear_pending(&dir, &stem);
                    state.status.problem(format!(
                        "the capture cannot become a picture ({e:?}); nothing was saved."
                    ));
                    return;
                }
            };
            let (w, h) = rgba.dimensions();
            match write_snip(
                &dir,
                &stem,
                (w, h, rgba.into_raw()),
                physical,
                record.clone(),
            ) {
                Ok(files) => {
                    bevy_grimoire_snip::clear_pending(&dir, &stem);
                    let shown = files
                        .png
                        .strip_prefix(&root)
                        .unwrap_or(&files.png)
                        .display()
                        .to_string();
                    state.status.note(format!("snip saved to {shown}"));
                }
                Err(e) => {
                    bevy_grimoire_snip::clear_pending(&dir, &stem);
                    state.status.problem(e)
                }
            }
        },
    );
}

/// The one overlay entity the drag and the note are drawn with. Despawned and respawned each
/// frame, so it is gone before the frame a capture reads.
#[derive(Component)]
pub struct SnipOverlay;

/// Draws the rectangle while it is dragged, and the note while it is typed. Runs after the systems
/// that change the snip, so the frame a note is confirmed the overlay is already gone.
pub fn snip_overlay(
    mut commands: Commands,
    snip: Res<SnipEdit>,
    pointer: Res<Pointer>,
    ui_scale: Res<UiScale>,
    old: Query<Entity, With<SnipOverlay>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let (rect, label) = match (&snip.pending, snip.from, pointer.0) {
        (Some(p), _, _) => (
            p.rect,
            format!("note: {}|  (Enter saves, Escape cancels)", p.note),
        ),
        (None, Some(from), Some(here)) => (
            rect_between(from, here),
            "drag over the area to snip".to_owned(),
        ),
        _ => return,
    };
    commands
        .spawn((
            SnipOverlay,
            // The drag rectangle is placed at the pointer over the whole window, so it is not a panel and has no layout to join: its corner, its size and a 3 px edge
            // that is exactly 3 px (`BorderPx` is not snapped to device pixels the way a skin border is).
            bevy_gooey::placed(UiRect {
                left: Val::Px(chrome::overlay_px(rect[0], ui_scale.0)),
                top: Val::Px(chrome::overlay_px(rect[1], ui_scale.0)),
                ..UiRect::all(Val::Auto)
            }),
            bevy_gooey::Sizing {
                width: Some(bevy_gooey::Extent::Px(chrome::overlay_px(rect[2] - rect[0], ui_scale.0))),
                height: Some(bevy_gooey::Extent::Px(chrome::overlay_px(rect[3] - rect[1], ui_scale.0))),
                ..default()
            },
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(chrome::SNIP_EDGE_W)),
            bevy_gooey::InkOverride { edge: Some(chrome::ink().accent), ..default() },
            BorderColor::all(chrome::ink().accent),
            BackgroundColor(chrome::ink().snip_wash),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(label),
                chrome::typed(chrome::text::LABEL),
                TextColor(chrome::ink().text),
                // One line, so a narrow drag cannot wrap the label down across the box's top edge.
                TextLayout::default().with_no_wrap(),
                // A label riding the drag rectangle's top edge, placed against the rectangle it names.
                bevy_gooey::placed(UiRect { left: Val::Px(0.0), top: Val::Px(SNIP_LABEL_RISE_PX), ..UiRect::all(Val::Auto) }),
            ));
        });
}

/// **While the snip is armed, no button sees the pointer.** Bevy fires a button's `Activate` on a click,
/// so a drag that starts and ends on a button would press it. Picking is off while the snip is armed and
/// back on once it is done — [`crate::picking`] is the one writer of that setting, and this system only
/// drops the presses already in flight.
///
/// **Turning picking off is not enough on its own** — B-032. `button_on_pointer_click`
/// (`bevy_ui_widgets-0.20.0/src/button.rs:60-72`) activates a button that carries [`Pressed`] when a
/// `PointerClick` arrives, and the press that set `Pressed` may have happened *before* the snip
/// armed. Picking off stops new presses; it cannot stop a release from completing one already in
/// flight, so the person presses a button, decides mid-press to snip instead, and the button fires
/// anyway. Arming therefore also **drops every `Pressed`**, which is the state the release looks for.
/// The press the person abandoned is the press they abandoned.
fn snip_gates_picking(
    snip: Res<SnipEdit>,
    mut in_flight: Query<Entity, With<Pressed>>,
    mut commands: Commands,
) {
    // **On the frame it becomes armed, not every armed frame.** `Res::is_changed` is true once, when
    // the arming written this frame arrives; sweeping every frame would instead keep clearing
    // presses the button layer makes later, and a press started on a *disarmed* editor would never
    // finish. One clear at the transition is exactly the press the person abandoned.
    //
    // **The setting itself is `crate::picking`'s**, and that is not a detail: this system and the
    // report's both wrote `PickingSettings::is_enabled` until 2026-11-05, and whichever ran last won.
    if snip.is_changed() && snip.armed {
        crate::picking::drop_presses(&mut in_flight, &mut commands);
    }
}

/// **Clears the pending records an earlier editor left, once a project is open.** A record present
/// on the first frame with a project was written by an editor that ended before its picture landed,
/// so no readback is still waiting on it. It runs on a frame with a project, not at plugin build,
/// because the project is a resource the editor may not have yet.
fn sweep_stale_pending(project: Option<Res<Project>>, mut swept: Local<bool>) {
    if *swept {
        return;
    }
    let Some(project) = project else {
        return;
    };
    bevy_grimoire_snip::clear_all_pending(&project.root.join(SNIP_DIR));
    *swept = true;
}

/// Registers the snip. Its own plugin so the editor's `Update` tuple stays under Bevy's cap.
pub struct SnipPlugin;

impl Plugin for SnipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SnipEdit>()
            .add_systems(Update, snip_key.in_set(Phase::Act))
            .add_systems(Update, snip_pointer.in_set(Phase::Act).after(snip_key))
            .add_systems(Update, snip_note_keys.in_set(Phase::Text))
            .add_systems(
                Update,
                snip_commit.in_set(Phase::Text).after(snip_note_keys),
            )
            .add_systems(
                Update,
                snip_overlay
                    .after(snip_pointer)
                    .after(snip_commit)
                    .after(snip_note_keys),
            )
            .add_systems(Update, snip_gates_picking)
            .add_systems(Update, sweep_stale_pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **At scale 2 the crop is the drag, in surface pixels.** The drag is `(200,120)`→`(420,260)` in
    /// logical pixels, so the surface rectangle is `[400,240,840,520]`. The crop must be that rectangle,
    /// not twice it: the surface is already physical.
    #[test]
    fn the_crop_is_the_drag_in_surface_pixels_at_scale_two() {
        let surface = rect_between(Vec2::new(400.0, 240.0), Vec2::new(840.0, 520.0));
        assert_eq!(picture_px(surface), [400, 240, 840, 520]);
    }

    /// A fractional drag rounds once, to the nearest surface pixel.
    #[test]
    fn a_fractional_drag_rounds_once() {
        assert_eq!(
            picture_px([400.4, 240.6, 840.0, 520.0]),
            [400, 241, 840, 520]
        );
    }

    /// **A placement under the drag is listed.** Its viewport point is in surface pixels, as the drag is,
    /// and the edges of the rectangle count as under it.
    #[test]
    fn a_placement_under_the_drag_is_listed() {
        let rect = [400.0, 240.0, 840.0, 520.0];
        assert!(under(rect, Vec2::new(500.0, 300.0)));
        assert!(under(rect, Vec2::new(840.0, 520.0)));
        assert!(!under(rect, Vec2::new(100.0, 100.0)));
    }

    /// **The overlay is drawn over the drag under UiScale.** The layout multiplies `Val::Px` by UiScale, so a
    /// surface pixel is `1/UiScale` of `Val::Px`, as compose.rs places its rectangles.
    #[test]
    fn the_overlay_is_divided_by_ui_scale() {
        assert!((chrome::overlay_px(440.0, 2.4) - 440.0 / 2.4).abs() < 1e-4);
        assert_eq!(chrome::overlay_px(440.0, 0.0), 440.0);
    }

    /// **The rectangle the record calls a window rectangle is in logical pixels.** A surface rectangle
    /// divided by the scale factor is what the window's logical coordinates read, so an agent that scales
    /// it back by the scale factor gets the surface rectangle again.
    #[test]
    fn the_window_rectangle_is_in_logical_pixels() {
        assert_eq!(
            logical_px([400.0, 240.0, 840.0, 520.0], 2.0),
            [200.0, 120.0, 420.0, 260.0]
        );
    }
}
