//! **A report about what is wrong on screen: point at it, say what is wrong, save four files.**
//!
//! This module is the *mechanism*: freeze the frame at the key, let a person point at the thing that
//! is wrong, outline what will be recorded, ask for a note, and write the pictures and the record
//! beside each other. Who reads it is not its business: `bevy_grimoire_core::feedback` owns the
//! record and the folders, and the MCP server owns the two readers.
//!
//! # Two channels, one path
//!
//! `Cmd+D` reports to **the people who make Cauldron**; it works on every screen, the Projects home
//! included, and needs no project open. `Cmd+Shift+D` reports about **the project you have open**,
//! and is refused with a sentence where there is none. [`Audience`] is the only difference: it picks
//! the folder and it is written into the record. A second copy of the gesture in a second module is
//! how the two would drift apart; this is one path with two destinations.
//!
//! **Not the snip** (`snip.rs`) and **not a map note** (`bevy_grimoire_core::notes`). A snip is a
//! crop a person marks for an agent working the map; a note marks a tile or a placement and lives
//! beside the map. Neither changes, and each keeps its own key.
//!
//! # The chords are outside the key census
//!
//! [`crate::keys::BINDINGS`] is the table of what a *tab* does. These chords work on every screen,
//! including the Projects home where there is no tab at all — the precedent is
//! `command_palette::chords`. A census row would also move `no_context_gains_a_key` for a chord the
//! census does not describe.
//!
//! # What a headless app can and cannot prove
//!
//! A headless editor has no render world, so no screenshot is ever read back and no PNG can be
//! written: a save there is refused, naming the renderer, and nothing is created. The gesture — the
//! chord, the pick, the outline, the note — is all proved headless, driven the way a person drives
//! it. The bytes on disk are proved by a live run under Xvfb.

use std::path::PathBuf;

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::ui::{CalculatedClip, ComputedNode, UiGlobalTransform, UiStack};
use bevy_grimoire_core::feedback::{
    self, Audience, CameraPose, Element, PathStep, Picture, Report, Scene, Subject, WindowRecord,
    FEEDBACK_VERSION, TEXT_CAP,
};
use bevy_gooey::{Modifiers, Shortcut};

use crate::chrome;
use crate::screen::Screen;
use crate::view::Pointer;

/// **`Cmd+D`: report to the people who make Cauldron.** Every screen, no project needed.
pub fn developers_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyD)
}

/// **`Cmd+Shift+D`: report about the project that is open.**
pub fn project_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyD)
}

/// **Above [`chrome::MODAL_Z`] (900).** A report has to be able to point at a dialog, so the frozen
/// picture it draws over must cover one.
pub const REPORT_Z: i32 = 1000;
/// **The note box, above the frozen picture it is written about.**
const NOTE_Z: i32 = REPORT_Z + 10;

/// **The smallest drag that is a box.** Below it, a press and release is a click on what is under it.
const MIN_PICK_PX: f32 = 8.0;

/// The longest note the field keeps, in characters.
pub const NOTE_MAX_CHARS: usize = 2000;

/// How wide the note box is, in logical pixels: wide enough for a sentence, narrow enough that a
/// 1280 px window still shows the thing being reported on around it.
const NOTE_W: f32 = 520.0;

/// The border drawn round the subject in the second picture, in picture pixels.
const OUTLINE_W: u32 = 3;
/// A white hairline just inside it, so the outline reads on a light ground as well as a dark one.
const OUTLINE_INNER_W: u32 = 1;
/// A scene mark's arm length, in picture pixels, going out from the click each way.
const MARK_ARM_PX: i32 = 24;

/// How many lines of the editor's log a report carries.
const LOG_TAIL_LINES: usize = 200;

/// What the hint line says while the person is pointing.
const PICK_HINT: &str =
    "Click what is wrong · drag a box · wheel changes the pick · Enter = whole screen · Esc cancels";
/// What it says once a subject is picked and the note is being typed.
const NOTE_HINT: &str = "Type what is wrong, then Save · Esc cancels";

/// Refusals and receipts, as sentences a person can act on.
const NO_PROJECT: &str = "no project is open, so nothing was saved. Cmd+D reports to the people who make Cauldron; Cmd+Shift+D reports on the project you have open.";
const NO_RENDER: &str =
    "this editor has no render world, so no picture could be taken; nothing was saved.";
const NO_DATA_DIR: &str =
    "this machine has no data directory, so there is nowhere to save the report; nothing was saved.";
const NOTHING_UNDER_POINTER: &str = "nothing is under the pointer, so there is nothing to report on. Point at something, or press Enter to report the whole screen.";
const CANCELLED: &str = "report cancelled, nothing was saved";

/// Where a report is, from the key to the files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The key was pressed and the frame is being read back. Nothing is drawn yet.
    Capturing,
    /// The frozen picture is up and the person is pointing at what is wrong.
    Picking,
    /// A subject is chosen and the note box is up.
    Note,
}

/// A report in progress.
#[derive(Resource, Default)]
pub struct ReportEdit {
    open: Option<Open>,
    /// **The person pressed Save.** An observer cannot do the writing — Bevy refuses an exclusive
    /// system as an observer, and a save touches the filesystem, draws pictures and spawns a receipt
    /// — so the observers *ask* and [`report_save`] does it, in `Update`, like every other act here.
    save_asked: bool,
}

impl ReportEdit {
    /// Is a report taking the pointer? Every placement driver yields to it, so the press that picks
    /// a thing cannot also place a piece behind the overlay.
    pub fn claims_pointer(&self) -> bool {
        self.open.is_some()
    }

    /// **Is the person still pointing?** This — not [`Self::claims_pointer`] — is what turns picking
    /// off, and the difference is load-bearing. Measured live on 2026-11-05: with picking off for the
    /// whole report, the note box's own Save and Cancel buttons never received a click, so a report
    /// could be picked and described and then not saved. The note box is a gooey `dialog`: it draws a
    /// scrim over the whole window and holds the keyboard, so nothing behind it can be pressed while
    /// it is up, and its buttons need picking exactly as a snip's note field needs the keyboard.
    pub fn pointing(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| open.phase != Phase::Note)
    }

    /// Where the report is, for a test or an agent.
    pub fn phase(&self) -> Option<Phase> {
        self.open.as_ref().map(|o| o.phase)
    }

    /// Which channel this report belongs to.
    pub fn audience(&self) -> Option<Audience> {
        self.open.as_ref().map(|o| o.audience)
    }

    /// What has been picked, once it has been.
    pub fn subject(&self) -> Option<&Subject> {
        self.open.as_ref().and_then(|o| o.subject.as_ref())
    }

    /// What has been typed so far.
    pub fn note(&self) -> &str {
        self.open.as_ref().map_or("", |o| o.note.as_str())
    }

    /// The box being dragged right now, in surface pixels.
    pub fn drag(&self) -> Option<[f32; 4]> {
        self.open.as_ref().and_then(|o| o.area)
    }

    /// The chain under the pointer and which step of it is picked: what the wheel moves.
    pub fn picked_step(&self) -> Option<(usize, usize)> {
        self.open
            .as_ref()
            .map(|o| (o.at, o.chain.len()))
            .filter(|(_, len)| *len > 0)
    }

    /// Is a save waiting to be done?
    pub fn save_asked(&self) -> bool {
        self.save_asked
    }

    /// Shut the report, whether it saved or not. An ask goes with it.
    fn close(&mut self) {
        self.open = None;
        self.save_asked = false;
    }

    /// **Shut the report and take its box with it, in this frame.**
    ///
    /// `report_box` despawns the box on the frame the report is closed, which is one frame too late
    /// for a person: the note box is a gooey `dialog`, so between the save and the despawn it still
    /// covers the whole window with a scrim and answers every click. Measured live on 2026-11-05 —
    /// after a save, the next report's click picked `Scrim`, not the control under the pointer.
    /// So this is the one way to close: the state goes and the entity goes with it.
    fn close_now(&mut self, commands: &mut Commands, boxes: &Query<Entity, With<ReportBox>>) {
        for entity in boxes {
            commands.entity(entity).despawn();
        }
        self.close();
    }
}

/// The report in flight.
struct Open {
    audience: Audience,
    phase: Phase,
    /// The chord's millisecond, which names the files. Never the save's: a slow pick must not
    /// collide with a report taken after it.
    taken_at_ms: u64,
    /// The whole frame, to draw under the outline.
    frozen: Option<Handle<Image>>,
    /// The same pixels, for the two PNGs.
    whole: Option<image::RgbaImage>,
    /// The chain under the pointer, outermost first.
    chain: Vec<Entity>,
    /// Which of the chain is picked. The wheel moves it, so a person can say "the button" rather
    /// than "the words on the button".
    at: usize,
    /// The outline, in surface pixels — always `chain[at]`'s rectangle.
    hover: Option<[f32; 4]>,
    /// The box being dragged, or the one that was dragged.
    area: Option<[f32; 4]>,
    /// Where the drag began.
    from: Option<Vec2>,
    subject: Option<Subject>,
    note: String,
    /// Where the pointer was when the subject was chosen: the scene mark in the second picture.
    mark: Option<Vec2>,
    /// **The frame the subject was chosen on.** The `Enter` that chose it, or the click's release,
    /// is still in this frame's input: the note box has `AutoFocus`, so without this the same key
    /// arrives as the first newline of the note. Measured live on 2026-11-05 — every report taken
    /// with Enter had a note that began with one.
    noted_on: u32,
    /// What the screen was, gathered when the key was pressed.
    screen: String,
    project: Option<String>,
    map: Option<String>,
    tool: Option<String>,
    window: WindowRecord,
}

/// The report's overlay root. Its presence is what makes the keyboard census quiet, so "is a report
/// open" has one answer and it is the drawn thing itself.
#[derive(Component)]
pub struct ReportOpen;

/// The note box, so it closes with the report and a test can find it.
#[derive(Component)]
pub struct ReportBox;

/// Everything this module draws, so a test can find the outline and the hit test can ignore its own
/// layer: the frozen picture covers the whole window, so without this every click would answer "a
/// picture of the editor".
#[derive(Component)]
pub struct ReportPart;

/// **The key was pressed: read the frame back and start pointing.**
///
/// The project chord is tested **first**. `Shortcut::just_pressed` compares the whole modifier set
/// exactly, so `Shift+Cmd+D` cannot also satisfy `Cmd+D` — but the two are one key apart and the
/// cost of being wrong is a report filed in the wrong folder, so the order says so too.
///
/// The frame comes from [`crate::surface::Surface`], **not from the window**: it is what the editor
/// draws, it needs no focus, and it cannot come back black because a dialog covered the window.
/// With no render world there is nothing to read, so the report opens ready to point and its save is
/// refused — a headless editor can therefore be driven through the whole gesture.
#[allow(clippy::too_many_arguments)]
fn report_key(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<Screen>>,
    project: Option<Res<crate::project::Project>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    state: Option<Res<crate::editor::EditorState>>,
    surface: Option<Res<crate::surface::Surface>>,
    can_render: Option<Res<crate::sequencer::CanRender>>,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
    ui_scale: Res<UiScale>,
    mut report: ResMut<ReportEdit>,
    mut commands: Commands,
) {
    if report.open.is_some() {
        return;
    }
    let audience = if project_chord().just_pressed(&keys) {
        Audience::Project
    } else if developers_chord().just_pressed(&keys) {
        Audience::Developers
    } else {
        return;
    };
    if audience == Audience::Project && project.is_none() {
        commands.spawn(bevy_gooey::toast_alert(NO_PROJECT).sticky());
        return;
    }
    let renders = can_render.as_deref().is_some_and(|c| c.0) && surface.is_some();
    let (logical, scale_factor) = window.as_deref().map_or(([0.0, 0.0], 1.0), |w| {
        (
            [w.resolution.width(), w.resolution.height()],
            w.resolution.scale_factor(),
        )
    });
    let physical = [
        (logical[0] * scale_factor).round().max(0.0) as u32,
        (logical[1] * scale_factor).round().max(0.0) as u32,
    ];
    report.open = Some(Open {
        audience,
        phase: if renders {
            Phase::Capturing
        } else {
            Phase::Picking
        },
        taken_at_ms: now_ms(),
        frozen: None,
        whole: None,
        chain: Vec::new(),
        at: 0,
        hover: None,
        area: None,
        from: None,
        subject: None,
        note: String::new(),
        mark: None,
        noted_on: 0,
        screen: screen_name(*screen.get()),
        project: project.as_deref().map(|p| p.root.display().to_string()),
        map: open_map.as_deref().map(|o| o.map.name.clone()),
        tool: state.as_deref().map(|s| format!("{:?}", s.tool)),
        window: WindowRecord {
            physical,
            logical,
            scale_factor,
            ui_scale: ui_scale.0,
        },
    });
    if let Some(surface) = surface.filter(|_| renders) {
        commands
            .spawn(Screenshot::image(surface.image.clone()))
            .observe(on_captured);
    }
}

/// The frame came back: keep it, and start pointing.
fn on_captured(
    shot: On<ScreenshotCaptured>,
    mut report: ResMut<ReportEdit>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let Some(open) = report.open.as_mut() else {
        return;
    };
    let rgba = match shot.image.clone().try_into_dynamic() {
        Ok(dynamic) => dynamic.to_rgba8(),
        Err(e) => {
            let why = format!("the picture cannot become a report ({e:?}); nothing was saved.");
            report.close();
            commands.spawn(bevy_gooey::toast_alert(why).sticky());
            return;
        }
    };
    // **A copy in the asset store, not the surface's handle**: the editor keeps drawing into the
    // surface while the person points, and a frozen frame that followed it would be no freeze.
    open.frozen = Some(images.add(shot.image.clone()));
    open.whole = Some(rgba);
    open.phase = Phase::Picking;
}

/// The milliseconds since the epoch, or 0 for a clock set before it.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn screen_name(screen: Screen) -> String {
    match screen {
        Screen::Launcher => "launcher",
        Screen::Loading => "loading",
        Screen::Project => "project",
        Screen::Editor => "editor",
    }
    .to_owned()
}

/// **The topmost UI node under `point`, and its ancestor chain outermost-first.**
///
/// The same three checks `bevy_ui`'s own picking backend makes
/// (`bevy_ui-0.20.0/src/picking_backend.rs:150-230`): a non-zero size, a visible node, and a point
/// inside its `CalculatedClip`. Nothing else is consulted — in particular not `Hovered`, which
/// follows the *window's* cursor and which an agent's injected pointer never moves.
///
/// Iterating back-to-front means the first hit is the one drawn on top, which is the one the person
/// can see.
#[allow(clippy::type_complexity)]
pub fn chain_under(
    point: Vec2,
    stack: &UiStack,
    nodes: &Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&CalculatedClip>,
        Option<&ChildOf>,
    ), Without<ReportPart>>,
) -> Vec<Entity> {
    for slice in stack.partition.iter().rev() {
        for entity in stack.uinodes[slice.clone()].iter().rev() {
            let Ok((node, tf, visible, clip, _)) = nodes.get(*entity) else {
                continue;
            };
            if node.size() == Vec2::ZERO || !visible.get() {
                continue;
            }
            if !node.contains_point(*tf, point) {
                continue;
            }
            if clip.is_some_and(|clip| !clip.contains_point(point)) {
                continue;
            }
            let mut chain = vec![*entity];
            let mut at = *entity;
            while let Some(parent) = nodes.get(at).ok().and_then(|step| step.4).map(ChildOf::parent)
            {
                chain.push(parent);
                at = parent;
            }
            chain.reverse();
            return chain;
        }
    }
    Vec::new()
}

/// The rectangle a laid-out node occupies, in surface pixels.
fn rect_of(node: &ComputedNode, tf: &UiGlobalTransform) -> Option<[f32; 4]> {
    let size = node.size();
    (size != Vec2::ZERO).then(|| {
        let centre = tf.translation;
        [
            centre.x - size.x * 0.5,
            centre.y - size.y * 0.5,
            centre.x + size.x * 0.5,
            centre.y + size.y * 0.5,
        ]
    })
}

/// `text` trimmed, and cut at [`TEXT_CAP`] characters.
fn short_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(TEXT_CAP).collect())
}

/// What the pick and the save read, bundled so neither system goes past clippy's argument limit.
///
/// The two queries spell their items out rather than naming them: a `type` alias for a query item
/// tuple has to carry the `&` and the lifetime that make it an item, and an alias without them is not
/// a query at all — measured on 2026-11-05, it was twelve "not valid to request as data" errors. So
/// the tuple stands and clippy's complexity lint is answered here instead.
#[allow(clippy::type_complexity)]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct ReportWorld<'w, 's> {
    pointer: Res<'w, Pointer>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    stack: Option<Res<'w, UiStack>>,
    camera: Option<Single<'w, 's, (&'static Camera, &'static GlobalTransform), With<crate::view::MainCamera>>>,
    project: Option<Res<'w, crate::project::Project>>,
    open_map: Option<Res<'w, crate::project::OpenMap>>,
    /// Every laid-out node: the hit test's three checks, and a parent to walk up.
    ///
    /// **The report's own layer is excluded**, and that is load-bearing: the frozen picture covers
    /// the whole window at [`REPORT_Z`], so a hit test that walked the stack blindly would find the
    /// overlay itself and answer "a picture of the editor" to every click a person made on it.
    nodes: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static InheritedVisibility,
            Option<&'static CalculatedClip>,
            Option<&'static ChildOf>,
        ),
        Without<ReportPart>,
    >,
    /// What a node is called, for the record's path: never a type name, always what a reader acts on.
    described: Query<
        'w,
        's,
        (
            Option<&'static Name>,
            Option<&'static bevy_gooey::Role>,
            Option<&'static chrome::Control>,
            Option<&'static Text>,
            Option<&'static bevy_gooey::Dialog>,
            Option<&'static Children>,
        ),
    >,
}

impl ReportWorld<'_, '_> {
    /// The chain under the pointer right now, or nothing when there is no pointer or no layout.
    fn chain(&self) -> Vec<Entity> {
        let (Some(point), Some(stack)) = (self.pointer.0, self.stack.as_deref()) else {
            return Vec::new();
        };
        chain_under(point, stack, &self.nodes)
    }

    /// Is `entity` inside a floating layer — a dialog, or a node carrying one of the floating roles?
    fn floats(&self, entity: Entity) -> bool {
        self.described.get(entity).is_ok_and(|(_, role, _, _, dialog, _)| {
            dialog.is_some() || role.is_some_and(|r| is_floating(*r))
        })
    }

    /// One step of a chain, described by what a reader can act on.
    fn step_of(&self, entity: Entity) -> PathStep {
        let Ok((name, role, control, text, _, children)) = self.described.get(entity) else {
            return PathStep {
                name: None,
                role: None,
                control: None,
                text: None,
            };
        };
        PathStep {
            name: name.map(|n| n.as_str().to_owned()),
            role: role.map(|r| format!("{r:?}")),
            control: control.map(|c| format!("{:?}", c.0)),
            text: text
                .and_then(|t| short_text(&t.0))
                .or_else(|| self.labels_of(children)),
        }
    }

    /// **The words a control draws**, for a node that has none of its own: a button's label is its
    /// child, and a report that said only `Button` would not tell a reader what was pressed.
    ///
    /// **Only a step that is a control has this**, and only its own children are searched one level
    /// of nesting down — never the whole subtree. Searching every subtree gave a container the words
    /// of whatever it happened to contain: measured live on 2026-11-05, every unnamed step of the
    /// chain from the home's `Screen` to the `Button` reported `Welcome to Cauldron`, so a path of
    /// eleven steps said the same sentence seven times and named nothing.
    fn labels_of(&self, children: Option<&Children>) -> Option<String> {
        // The child of a button is its words; the child of a row is another row.
        children?.iter().find_map(|child| {
            let (_, role, _, text, _, _) = self.described.get(child).ok()?;
            let is_control = role.is_some_and(|role| {
                matches!(
                    role,
                    bevy_gooey::Role::ButtonText
                        | bevy_gooey::Role::Button
                        | bevy_gooey::Role::Label
                        | bevy_gooey::Role::Heading
                )
            });
            if !is_control {
                return None;
            }
            text.and_then(|t| short_text(&t.0))
        })
    }
}

/// **Point at it.**
///
/// A press and a release in one place is a click; a press, a move of [`MIN_PICK_PX`] and a release
/// is a box. Enter with nothing picked is the whole screen. The wheel walks the chain, so a person
/// can pick the button rather than the words on it.
///
/// What a click means, in this order:
///
/// 1. **A floating layer over the map is interface.** A dialog, menu, tooltip, popover or scrim is
///    what the person is pointing at even when it sits over the viewport.
/// 2. **Else a point inside the map camera's viewport is the scene** — the ground under it, its map
///    point, its cell, and the placement the editor's own `pick_at` chose there.
/// 3. **Else the node chain is the answer.**
fn report_pick(
    mut report: ResMut<ReportEdit>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    frames: Res<bevy::diagnostic::FrameCount>,
    world: ReportWorld,
    mut commands: Commands,
) {
    let notches: f32 = wheel.read().map(|w| w.y).sum();
    let Some(open) = report.open.as_mut() else {
        return;
    };
    if open.phase == Phase::Capturing {
        return;
    }
    let here = world.pointer.0;

    // **Enter takes the whole screen**, the one subject that needs no pointer at all.
    if open.phase == Phase::Picking && keys.just_pressed(KeyCode::Enter) {
        open.subject = Some(Subject::Screen);
        open.area = None;
        open.hover = None;
        open.mark = here;
        open.noted_on = frames.0;
        open.phase = Phase::Note;
        return;
    }
    if open.phase == Phase::Note {
        return;
    }

    if open.from.is_none() && world.mouse.just_pressed(MouseButton::Left) {
        open.from = here;
    }
    if let (Some(from), Some(now)) = (open.from, here)
        && (now - from).abs().max_element() >= MIN_PICK_PX
    {
        open.area = Some(rect_between(from, now));
    }
    if world.mouse.just_released(MouseButton::Left) {
        let from = open.from.take();
        if let Some(rect) = open.area.take() {
            open.subject = Some(Subject::Area {
                rect_px: clamp_rect(rect, open.window.physical),
            });
            open.hover = None;
            open.mark = here;
            open.noted_on = frames.0;
            open.phase = Phase::Note;
            return;
        }
        if from.is_some()
            && let Some(at) = here
        {
            match pick_at_point(at, &world) {
                Ok(subject) => {
                    open.subject = Some(subject);
                    open.area = None;
                    open.hover = None;
                    open.mark = Some(at);
                    open.noted_on = frames.0;
                    open.phase = Phase::Note;
                }
                Err(why) => {
                    commands.spawn(bevy_gooey::toast_alert(why).sticky());
                }
            }
            return;
        }
    }

    // **The chain follows the pointer while nothing is pressed**, and the wheel moves the pick.
    if here.is_none() || world.mouse.pressed(MouseButton::Left) {
        return;
    }
    open.chain = world.chain();
    if notches != 0.0 && !open.chain.is_empty() {
        open.at = if notches > 0.0 {
            open.at.saturating_sub(1)
        } else {
            (open.at + 1).min(open.chain.len() - 1)
        };
    } else if open.chain.len() != open.at.saturating_add(1) {
        // A new chain starts at the innermost node, which is the one under the pointer.
        open.at = open.chain.len().saturating_sub(1);
    }
    open.hover = open
        .chain
        .get(open.at)
        .and_then(|e| world.nodes.get(*e).ok())
        .and_then(|(node, tf, ..)| rect_of(node, tf));
}

/// The subject under `at`, or the sentence saying why there is none.
fn pick_at_point(at: Vec2, world: &ReportWorld) -> Result<Subject, &'static str> {
    let chain = world.chain();
    let floating = chain.iter().any(|e| world.floats(*e));
    if !floating && in_map_viewport(at, world) {
        return Ok(scene_at(at, world));
    }
    let Some(picked) = chain.last().copied() else {
        return Err(NOTHING_UNDER_POINTER);
    };
    let (node, tf, ..) = world.nodes.get(picked).map_err(|_| NOTHING_UNDER_POINTER)?;
    let rect_px = rect_of(node, tf).ok_or(NOTHING_UNDER_POINTER)?;
    Ok(Subject::Element(Element {
        entity: picked.to_bits(),
        rect_px,
        path: chain.iter().map(|e| world.step_of(*e)).collect(),
    }))
}

/// **Is this a `bevy_gooey` role that floats over the world?** A dialog, menu, tooltip, popover,
/// scrim and card are interface even when what they cover is the map.
fn is_floating(role: bevy_gooey::Role) -> bool {
    matches!(
        role,
        bevy_gooey::Role::Popover
            | bevy_gooey::Role::Menu
            | bevy_gooey::Role::Tooltip
            | bevy_gooey::Role::Scrim
            | bevy_gooey::Role::Card
    )
}

/// Is `at` inside the map camera's viewport? `cursor_ray` and `cursor_ground` make the same test.
fn in_map_viewport(at: Vec2, world: &ReportWorld) -> bool {
    world
        .camera
        .as_deref()
        .and_then(|(cam, _)| cam.logical_viewport_rect())
        .is_some_and(|r| r.contains(at))
}

/// The scene under `at`: the ground point, the map point, the cell, and the placement there.
fn scene_at(at: Vec2, world: &ReportWorld) -> Subject {
    let Some((cam, cam_tf)) = world.camera.as_deref() else {
        return Subject::Scene(Scene {
            world: [0.0; 3],
            map: None,
            cell: None,
            placement: None,
            camera: CameraPose {
                translation: [0.0; 3],
                forward: [0.0; 3],
            },
        });
    };
    let ground = crate::view::cursor_ground(Some(at), cam, cam_tf).unwrap_or(Vec3::ZERO);
    let mut scene = Scene {
        world: ground.to_array(),
        map: None,
        cell: None,
        placement: None,
        camera: CameraPose {
            translation: cam_tf.translation().to_array(),
            forward: cam_tf.forward().as_vec3().to_array(),
        },
    };
    if let (Some(project), Some(open_map)) = (world.project.as_deref(), world.open_map.as_deref()) {
        let map_point = open_map.map.to_map_space((ground.x, ground.z));
        scene.map = Some([map_point.0, map_point.1]);
        scene.cell = Some([
            (map_point.0 / bevy_grimoire_core::grid::TILE).floor() as i32,
            (map_point.1 / bevy_grimoire_core::grid::TILE).floor() as i32,
        ]);
        scene.placement = crate::editor::pick_at(project, open_map, map_point)
            .and_then(|i| open_map.map.placements.get(i))
            .map(|p| p.id.clone());
    }
    Subject::Scene(scene)
}

/// The rectangle with its corners in order, whichever way the drag went.
fn rect_between(a: Vec2, b: Vec2) -> [f32; 4] {
    [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
}

/// A box clamped to the frame, so a drag past the edge is the part of the screen that exists.
fn clamp_rect(rect: [f32; 4], size: [u32; 2]) -> [f32; 4] {
    let (w, h) = (size[0] as f32, size[1] as f32);
    [
        rect[0].clamp(0.0, w.max(rect[0])),
        rect[1].clamp(0.0, h.max(rect[1])),
        rect[2].clamp(rect[0].clamp(0.0, w), w.max(rect[2])),
        rect[3].clamp(rect[1].clamp(0.0, h), h.max(rect[3])),
    ]
}

/// **`Esc` closes a report that has no note box to be its `Cancel`.**
///
/// That is one rule with two cases. While the person is pointing there is no box at all, so nothing
/// else would hear the key. And there is a single frame in which the box exists but has not been
/// spawned yet — the frame after a subject is chosen, held back so the `Enter` that chose it is not
/// its first newline (`report_box`) — during which an `Esc` would otherwise fall between the two.
fn report_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut report: ResMut<ReportEdit>,
    boxes: Query<Entity, With<ReportBox>>,
    mut commands: Commands,
) {
    if report.open.is_none() || !boxes.is_empty() || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    report.close_now(&mut commands, &boxes);
    commands.spawn(bevy_gooey::toast(CANCELLED));
}

/// **Draw the frozen frame with the pick outlined on it.**
///
/// One root, despawned and respawned every frame — the shape `snip_overlay` has, and for the same
/// reason: the frame a report closes, there is nothing stale left to draw.
fn report_overlay(
    mut commands: Commands,
    report: Res<ReportEdit>,
    ui_scale: Res<UiScale>,
    old: Query<Entity, With<ReportOpen>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let Some(open) = report.open.as_ref() else {
        return;
    };
    let overlay_px = |px: f32| chrome::overlay_px(px, ui_scale.0);
    let (outline, dragged, hint) = match open.phase {
        Phase::Capturing => return,
        Phase::Note => (open.hover, None, NOTE_HINT),
        Phase::Picking => (open.hover, open.area, PICK_HINT),
    };
    commands
        .spawn((
            ReportOpen,
            ReportPart,
            Node {
                // PLACES-ITSELF-OK: the whole frozen frame, drawn over the window rather than in any layout.
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(REPORT_Z),
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            if let Some(frozen) = open.frozen.clone() {
                root.spawn((
                    ReportPart,
                    ImageNode::new(frozen),
                    Node {
                        // PLACES-ITSELF-OK: the frozen frame is the layer everything else is placed against.
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }
            // **The outline, and the box being dragged**, drawn the same way: what is outlined is
            // what will be recorded, and a drag is that answer in the making.
            for (rect, wash) in [(outline, Color::NONE), (dragged, chrome::SNIP_WASH)] {
                let Some(rect) = rect else { continue };
                root.spawn((
                    ReportPart,
                    Node {
                        // PLACES-ITSELF-OK: the outline is placed at the pick over the frozen frame, not in a layout.
                        position_type: PositionType::Absolute,
                        left: Val::Px(overlay_px(rect[0])),
                        top: Val::Px(overlay_px(rect[1])),
                        width: Val::Px(overlay_px((rect[2] - rect[0]).max(0.0))),
                        height: Val::Px(overlay_px((rect[3] - rect[1]).max(0.0))),
                        border: UiRect::all(Val::Px(chrome::SNIP_EDGE_W)),
                        ..default()
                    },
                    BorderColor::all(chrome::ACCENT),
                    BackgroundColor(wash),
                    Pickable::IGNORE,
                ));
            }
            root.spawn((
                ReportPart,
                Text::new(hint.to_owned()),
                chrome::font(chrome::text::LABEL),
                TextColor(chrome::TEXT),
                TextLayout::default().with_no_wrap(),
                Node {
                    // PLACES-ITSELF-OK: the hint rides the top edge of the frozen frame, not a panel.
                    position_type: PositionType::Absolute,
                    top: Val::Px(chrome::GAP_GROUP),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });
}

/// **What a person is about to report, in one line**, so the note box says what Save will record.
pub fn subject_line(subject: &Subject) -> String {
    match subject {
        Subject::Screen => "Picked: the whole screen".to_owned(),
        Subject::Area { rect_px } => format!(
            "Picked: an area {:.0} × {:.0}",
            (rect_px[2] - rect_px[0]).max(0.0),
            (rect_px[3] - rect_px[1]).max(0.0)
        ),
        Subject::Element(element) => {
            let what = element
                .path
                .last()
                .and_then(|step| {
                    step.text
                        .clone()
                        .or_else(|| step.role.clone())
                        .or_else(|| step.name.clone())
                })
                .unwrap_or_else(|| "a node".to_owned());
            let trail: Vec<&str> = element
                .path
                .iter()
                .filter_map(|step| step.role.as_deref().or(step.name.as_deref()))
                .collect();
            format!("Picked: {} ({})", what, trail.join(" › "))
        }
        Subject::Scene(scene) => {
            let cell = scene
                .cell
                .map(|c| format!(", cell ({}, {})", c[0], c[1]))
                .unwrap_or_default();
            match &scene.placement {
                Some(id) => format!("Picked: placed piece `{id}`{cell}"),
                None => format!(
                    "Picked: the map at {:.1}, {:.1}{cell}",
                    scene.world[0], scene.world[2]
                ),
            }
        }
    }
}

/// **The note box: a gooey dialog**, so Escape is its `Cancel` and the keyboard is held inside it
/// while a person types.
///
/// Enter inside the field is a new line; `Cmd+Enter` ends the edit and the field reports one final
/// `ValueChange`, which saves — so the button and the chord reach the same function. A box that
/// arrives after the report has already closed (the frame a save runs) is never spawned.
fn report_box(
    mut commands: Commands,
    report: Res<ReportEdit>,
    frames: Res<bevy::diagnostic::FrameCount>,
    boxes: Query<Entity, With<ReportBox>>,
) {
    // **A box spawned on the frame its subject was chosen would take that frame's `Enter` as its
    // first newline**, because it carries `AutoFocus` and gooey hands the field the keyboard at once.
    // One frame later the key that chose the subject is gone from the queue.
    let noting = report
        .open
        .as_ref()
        .is_some_and(|open| open.phase == Phase::Note && open.noted_on != frames.0);
    if !noting {
        for entity in &boxes {
            commands.entity(entity).despawn();
        }
        return;
    }
    if !boxes.is_empty() {
        return;
    }
    let (Some(subject), Some(audience)) = (report.subject(), report.audience()) else {
        return;
    };
    let heading = match audience {
        Audience::Developers => "Report to those who make Cauldron",
        Audience::Project => "Report about this project",
    };
    let line = subject_line(subject);
    commands
        .spawn((
            // **`dialog()` carries `GlobalZIndex(100)` of its own**, and a bundle with one twice is a
            // spawn panic — so the box raises that component instead of adding a second.
            bevy_gooey::dialog(),
            ReportBox,
            Name::new("feedback report"),
            bevy_gooey::observe(
                |_: On<bevy_gooey::Cancel>,
                 mut report: ResMut<ReportEdit>,
                 boxes: Query<Entity, With<ReportBox>>,
                 mut commands: Commands| {
                    report.close_now(&mut commands, &boxes);
                    commands.spawn(bevy_gooey::toast(CANCELLED));
                },
            ),
        ))
        .insert(GlobalZIndex(NOTE_Z))
        .with_children(|scrim| {
            scrim
                .spawn(bevy_gooey::panel())
                .insert(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(NOTE_W),
                    max_width: Val::Percent(100.0),
                    row_gap: Val::Px(chrome::GAP_ROW),
                    ..default()
                })
                .with_children(|card| {
                    card.spawn(bevy_gooey::heading(heading));
                    card.spawn(bevy_gooey::label(line));
                    card.spawn((
                        bevy_gooey::text_area("What is wrong", "", 6),
                        bevy::input_focus::AutoFocus,
                        bevy_gooey::TextLimit(NOTE_MAX_CHARS),
                        bevy_gooey::observe(
                            |change: On<bevy_gooey::ValueChange<String>>,
                             mut report: ResMut<ReportEdit>| {
                                if let Some(open) = report.open.as_mut() {
                                    open.note = change.value.clone();
                                }
                                // **`Cmd+Enter` ends the edit** and the field reports one final
                                // change; the button and the chord reach the same save.
                                if change.is_final {
                                    report.save_asked = true;
                                }
                            },
                        ),
                    ));
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: Val::Px(chrome::GAP_GROUP),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            bevy_gooey::button("Cancel"),
                            bevy_gooey::observe(
                                |_: On<bevy_gooey::Activate>,
                                 mut report: ResMut<ReportEdit>,
                                 boxes: Query<Entity, With<ReportBox>>,
                                 mut commands: Commands| {
                                    report.close_now(&mut commands, &boxes);
                                    commands.spawn(bevy_gooey::toast(CANCELLED));
                                },
                            ),
                        ));
                        row.spawn((
                            bevy_gooey::button("Save report"),
                            bevy_gooey::observe(
                                |_: On<bevy_gooey::Activate>, mut report: ResMut<ReportEdit>| {
                                    report.save_asked = true;
                                },
                            ),
                        ));
                    });
                });
        });
}

/// **The save the box asked for**, done in `Update` where a system may touch the world.
fn report_save(world: &mut World) {
    let asked = world
        .get_resource::<ReportEdit>()
        .is_some_and(ReportEdit::save_asked);
    if !asked {
        return;
    }
    let mut report = std::mem::take(&mut *world.resource_mut::<ReportEdit>());
    save(&mut report, world);
    *world.resource_mut::<ReportEdit>() = report;
}

/// **Write the four files, then close.**
///
/// The record describes the screen as it was when the key was pressed, not as it is now: the note box
/// is on screen at this moment, and a report that photographed itself would be a report about a
/// report. Every path and every byte is decided before anything is created, so a refusal leaves no
/// folder behind.
fn save(report: &mut ReportEdit, world: &mut World) {
    let Some(open) = report.open.as_ref() else {
        return;
    };
    let Some(subject) = open.subject.clone() else {
        return;
    };
    let dir = match open.audience {
        Audience::Developers => match bevy_grimoire_core::user_dirs::feedback_dir() {
            Some(dir) => dir,
            None => return refuse(report, world, NO_DATA_DIR),
        },
        Audience::Project => match &open.project {
            Some(root) => PathBuf::from(root).join(bevy_grimoire_core::feedback::PROJECT_DIR),
            None => return refuse(report, world, NO_PROJECT),
        },
    };
    let Some(whole) = open.whole.clone() else {
        // No readback ever landed: the headless editor's answer, at the moment it would matter.
        return refuse(report, world, NO_RENDER);
    };

    let stem = feedback::stem(open.taken_at_ms);
    let (w, h) = whole.dimensions();
    let mut marked = whole.clone();
    match &subject {
        Subject::Element(element) => draw_outline(&mut marked, element.rect_px),
        Subject::Area { rect_px } => draw_outline(&mut marked, *rect_px),
        Subject::Scene(_) => {
            if let Some(at) = open.mark {
                draw_mark(&mut marked, at);
            }
        }
        Subject::Screen => {}
    }

    let mut writes: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    match encode_png(&whole) {
        Ok(bytes) => writes.push((dir.join(feedback::picture_file(&stem)), bytes)),
        Err(why) => return refuse(report, world, &why),
    }
    // **A whole-screen report has one picture**, because the second would be the first.
    let subject_picture = if subject == Subject::Screen {
        None
    } else {
        match encode_png(&marked) {
            Ok(bytes) => {
                writes.push((dir.join(feedback::subject_picture_file(&stem)), bytes));
                Some(Picture {
                    file: feedback::subject_picture_file(&stem),
                    size: [w, h],
                })
            }
            Err(why) => return refuse(report, world, &why),
        }
    };
    let log = log_tail(&stem);
    if let Some((path, text)) = &log {
        writes.push((path.clone(), text.as_bytes().to_vec()));
    }

    let record = Report {
        version: FEEDBACK_VERSION,
        stem: stem.clone(),
        taken_at_ms: open.taken_at_ms,
        audience: open.audience,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        screen: open.screen.clone(),
        project: open.project.clone(),
        map: open.map.clone(),
        tool: open.tool.clone(),
        window: open.window,
        subject,
        note: open.note.clone(),
        picture: Picture {
            file: feedback::picture_file(&stem),
            size: [w, h],
        },
        subject_picture,
        log: log
            .as_ref()
            .and_then(|(path, _)| path.file_name().map(|n| n.to_string_lossy().into_owned())),
    };
    let text = match serde_json::to_string_pretty(&record) {
        Ok(text) => text,
        Err(e) => {
            let why = format!("the report did not encode as JSON ({e}); nothing was saved.");
            return refuse(report, world, &why);
        }
    };
    writes.push((dir.join(feedback::record_file(&stem)), text.into_bytes()));

    for (path, bytes) in &writes {
        if let Err(e) = bevy_grimoire_core::atomic_file::write_new(path, bytes) {
            let why = format!("could not write {}: {e}", path.display());
            return refuse(report, world, &why);
        }
    }
    close_the_box(report, world);
    let shown = crate::launcher::shown_folder(&dir);
    world.spawn(bevy_gooey::toast(format!("report saved to {shown}")));
}

/// **Close the report and despawn its box, in one act.** A save runs inside an exclusive system, so
/// the box is despawned on the spot rather than through a command that lands a frame later — see
/// [`ReportEdit::close_now`] for what one frame of a full-window scrim costs.
fn close_the_box(report: &mut ReportEdit, world: &mut World) {
    let boxes: Vec<Entity> = world
        .query_filtered::<Entity, With<ReportBox>>()
        .iter(world)
        .collect();
    for entity in boxes {
        world.despawn(entity);
    }
    report.close();
}

/// Close the report and say why nothing was written.
fn refuse(report: &mut ReportEdit, world: &mut World, why: &str) {
    close_the_box(report, world);
    world.spawn(bevy_gooey::toast_alert(why).sticky());
}

/// The editor's log tail as `(the file it is written as, its text)`, when there is a log to read.
///
/// The tail goes beside the record under the report's own name, so a reader that finds a report finds
/// what the editor was saying at that moment — and it is a copy, never a link: the log rolls over.
fn log_tail(stem: &str) -> Option<(PathBuf, String)> {
    let path = bevy_grimoire_core::user_dirs::log_file()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let tail = feedback::last_lines(&text, LOG_TAIL_LINES);
    if tail.is_empty() {
        return None;
    }
    let dir = bevy_grimoire_core::user_dirs::feedback_dir()?;
    Some((dir.join(feedback::log_file(stem)), tail))
}

/// A picture as PNG bytes.
fn encode_png(image: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| format!("the picture did not encode as a PNG ({e}); nothing was saved."))?;
    Ok(out.into_inner())
}

/// **A border round `rect`, in [`chrome::ACCENT`], with a white hairline just inside it** — so the
/// outline reads on a dark panel and on a light one, and a reader who opens the picture sees the same
/// rectangle the person was pointing at.
fn draw_outline(image: &mut image::RgbaImage, rect: [f32; 4]) {
    let (w, h) = image.dimensions();
    let clamp_x = |x: f32| x.round().clamp(0.0, w as f32) as u32;
    let clamp_y = |y: f32| y.round().clamp(0.0, h as f32) as u32;
    let x0 = clamp_x(rect[0]);
    let y0 = clamp_y(rect[1]);
    let x1 = clamp_x(rect[2]).max(x0);
    let y1 = clamp_y(rect[3]).max(y0);
    let accent = accent_rgb();
    for band in 0..OUTLINE_W + OUTLINE_INNER_W {
        let colour = if band < OUTLINE_W {
            accent
        } else {
            [255, 255, 255, 255]
        };
        let ax = x0.saturating_add(band);
        let ay = y0.saturating_add(band);
        let bx = x1.saturating_sub(band).max(ax);
        let by = y1.saturating_sub(band).max(ay);
        // The last row and column are the inside edge: one short, so the border is `band` wide.
        let far_x = bx.saturating_sub(1).max(ax);
        let far_y = by.saturating_sub(1).max(ay);
        for x in ax..bx {
            put(image, x, ay, colour);
            put(image, x, far_y, colour);
        }
        for y in ay..by {
            put(image, ax, y, colour);
            put(image, far_x, y, colour);
        }
    }
}

/// A two-armed cross at `at`, in the accent — how a report says "here" about a point in the world,
/// where there is no rectangle to outline.
fn draw_mark(image: &mut image::RgbaImage, at: Vec2) {
    let (w, h) = image.dimensions();
    let cx = at.x.round().clamp(0.0, w as f32) as i32;
    let cy = at.y.round().clamp(0.0, h as f32) as i32;
    let colour = accent_rgb();
    for d in -MARK_ARM_PX..=MARK_ARM_PX {
        put_signed(image, cx + d, cy, colour);
        put_signed(image, cx, cy + d, colour);
    }
}

/// The accent as RGBA8, so the picture's outline and the on-screen outline are the same colour.
fn accent_rgb() -> [u8; 4] {
    let accent = bevy::color::Srgba::from(chrome::ACCENT);
    [
        (accent.red * 255.0).round() as u8,
        (accent.green * 255.0).round() as u8,
        (accent.blue * 255.0).round() as u8,
        255,
    ]
}

fn put(image: &mut image::RgbaImage, x: u32, y: u32, colour: [u8; 4]) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, image::Rgba(colour));
    }
}

fn put_signed(image: &mut image::RgbaImage, x: i32, y: i32, colour: [u8; 4]) {
    if x >= 0 && y >= 0 {
        put(image, x as u32, y as u32, colour);
    }
}

/// **While a report is open, no button sees the pointer.** Bevy fires a button's `Activate` on a
/// click, so the press that picks a button as the report's subject would also press it.
///
/// Copied from `snip_gates_picking`, B-032 and all: turning picking off stops *new* presses, so the
/// transition also **drops every `Pressed`**, because the press that set one may have begun before
/// the report opened and its release would otherwise still activate the button.
fn report_gates_picking(
    report: Res<ReportEdit>,
    settings: Option<ResMut<bevy::picking::PickingSettings>>,
    mut in_flight: Query<Entity, With<bevy::ui::Pressed>>,
    mut commands: Commands,
) {
    if report.is_changed() && report.pointing() {
        for entity in &mut in_flight {
            commands.entity(entity).remove::<bevy::ui::Pressed>();
        }
    }
    let Some(mut settings) = settings else {
        return;
    };
    let wanted = !report.pointing();
    if settings.is_enabled != wanted {
        settings.is_enabled = wanted;
    }
}

/// **The report's own keys, its pick, its overlay and its note box.**
///
/// In the **shared** plugin list ([`crate::harness::add_editor_plugins`]), not in `EditorPlugin`: a
/// report about the editor has to work on the Projects home, where the editor's own plugins have not
/// opened a door.
pub struct ReportPlugin;

impl Plugin for ReportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReportEdit>()
            .add_systems(Update, report_key.in_set(crate::keys::Phase::Act))
            .add_systems(
                Update,
                (report_pick, report_escape)
                    .in_set(crate::keys::Phase::Act)
                    .after(report_key),
            )
            .add_systems(
                Update,
                (report_overlay, report_box)
                    .chain()
                    .after(report_pick)
                    .after(report_escape),
            )
            // **The save is exclusive**, so it is its own system: an observer cannot be one, and
            // the box's two buttons and the field's final change all reach it by asking.
            .add_systems(Update, report_save.after(report_box))
            .add_systems(Update, report_gates_picking);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_subject() -> Subject {
        Subject::Scene(Scene {
            world: [1.0, 0.0, 2.0],
            map: Some([1.0, 2.0]),
            cell: Some([1, 2]),
            placement: Some("chair_3".into()),
            camera: CameraPose {
                translation: [0.0, 12.0, 12.0],
                forward: [0.0, -1.0, -1.0],
            },
        })
    }

    /// **The two chords are one key apart, and only the project one holds Shift.**
    #[test]
    fn the_two_chords_differ_by_shift_alone() {
        assert_eq!(developers_chord().key, project_chord().key);
        assert!(!developers_chord().modifiers.shift);
        assert!(project_chord().modifiers.shift);
        assert_ne!(developers_chord(), project_chord());
    }

    /// The rectangle a drag makes is the same whichever corner it started at.
    #[test]
    fn a_backwards_drag_is_the_same_rectangle() {
        assert_eq!(
            rect_between(Vec2::new(40.0, 90.0), Vec2::new(10.0, 20.0)),
            [10.0, 20.0, 40.0, 90.0]
        );
    }

    /// A drag past the edge is the part of the screen that exists, never a rectangle outside it.
    #[test]
    fn a_drag_past_the_edge_is_clamped_to_the_frame() {
        assert_eq!(
            clamp_rect([-30.0, -10.0, 900.0, 4000.0], [1280, 720]),
            [0.0, 0.0, 900.0, 720.0]
        );
    }

    /// Every subject says what it is in one line, so the note box names what Save will record.
    #[test]
    fn every_subject_reads_as_a_line() {
        assert_eq!(subject_line(&Subject::Screen), "Picked: the whole screen");
        assert_eq!(
            subject_line(&Subject::Area {
                rect_px: [0.0, 0.0, 220.0, 140.0]
            }),
            "Picked: an area 220 × 140"
        );
        assert_eq!(
            subject_line(&scene_subject()),
            "Picked: placed piece `chair_3`, cell (1, 2)"
        );
        let element = Subject::Element(Element {
            entity: 7,
            rect_px: [0.0; 4],
            path: vec![
                PathStep {
                    name: None,
                    role: Some("Panel".into()),
                    control: None,
                    text: None,
                },
                PathStep {
                    name: None,
                    role: Some("Button".into()),
                    control: None,
                    text: Some("Save".into()),
                },
            ],
        });
        assert_eq!(subject_line(&element), "Picked: Save (Panel › Button)");
    }

    /// **The outline lands where the rectangle is, in the accent**, so a reader who opens the second
    /// picture sees the same box the person was pointing at — and nothing inside it.
    #[test]
    fn the_outline_is_drawn_where_the_rectangle_is() {
        let mut picture = image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]));
        draw_outline(&mut picture, [10.0, 8.0, 40.0, 30.0]);
        let accent = accent_rgb();
        assert_eq!(picture.get_pixel(10, 8).0, accent, "the top-left corner");
        assert_eq!(picture.get_pixel(39, 8).0, accent, "the top edge");
        assert_eq!(picture.get_pixel(10, 29).0, accent, "the left edge");
        assert_eq!(
            picture.get_pixel(25, 20).0,
            [0, 0, 0, 255],
            "and nothing inside it"
        );
        // The white hairline is the last band, one pixel in from the accent.
        assert_eq!(picture.get_pixel(10 + OUTLINE_W, 8 + OUTLINE_W).0, [255, 255, 255, 255]);
    }

    /// A scene has no rectangle, so its mark is a cross at the point.
    #[test]
    fn a_scene_is_marked_at_the_point() {
        let mut picture = image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]));
        draw_mark(&mut picture, Vec2::new(32.0, 24.0));
        let accent = accent_rgb();
        assert_eq!(picture.get_pixel(32, 24).0, accent, "the point");
        assert_eq!(picture.get_pixel(32 - 10, 24).0, accent, "the arm");
        assert_eq!(picture.get_pixel(0, 0).0, [0, 0, 0, 255], "and elsewhere");
    }

    /// **A floating layer over the map is interface.** A tooltip over the viewport must be pickable;
    /// without this, pointing at one recorded the ground under it.
    #[test]
    fn the_floating_roles_are_interface() {
        for role in [
            bevy_gooey::Role::Popover,
            bevy_gooey::Role::Menu,
            bevy_gooey::Role::Tooltip,
            bevy_gooey::Role::Scrim,
            bevy_gooey::Role::Card,
        ] {
            assert!(is_floating(role), "{role:?} floats");
        }
        for role in [
            bevy_gooey::Role::Panel,
            bevy_gooey::Role::Button,
            bevy_gooey::Role::Label,
        ] {
            assert!(!is_floating(role), "{role:?} is in the layout");
        }
    }

    /// A text is trimmed and cut, so a paragraph never becomes the record.
    #[test]
    fn a_text_is_trimmed_and_capped() {
        assert_eq!(short_text("  Save  "), Some("Save".to_owned()));
        assert_eq!(short_text("   "), None);
        let long = "x".repeat(TEXT_CAP + 40);
        assert_eq!(short_text(&long).unwrap().chars().count(), TEXT_CAP);
    }
}
