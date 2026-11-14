//! **A report about what is wrong on screen: point at it, say what is wrong, save four files.**
//!
//! This module is the *mechanism*: freeze the frame at the key, let a person point at the thing that
//! is wrong, outline what will be recorded, ask for a note, and write the pictures and the record
//! beside each other. Who reads it is not its business: `bevy_grimoire_core::feedback` owns the
//! record and the folders, and the MCP server owns the two readers.
//!
//! # One pick, or several, and the stack under the pointer
//!
//! `Shift+click` accumulates: "this label is under the viewport" is a fact about two elements, and a
//! record that can hold one makes a person file two reports and describe the relationship in prose.
//! A plain click still collapses to a single pick, which is what a person who means one thing does.
//!
//! **The wheel and `Tab` step down the stack *under the point*, not up an ancestor chain.** An
//! overlapping label is usually a **sibling** of what covers it, so an ancestor walk can never reach
//! it: there is nowhere on screen where only the label is under the cursor. [`stack_under`] collects
//! every node the point is in, front-to-back, and `Tab` or the wheel moves within that.
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
use bevy::input_focus::InputFocus;
// `Tab` is this report's own "step down the stack" gesture, so this module owns a focus home for it:
// without one, `bevy_input_focus`'s tab navigation reaches the primary window and warns on every
// press. The reason it is safe — it is focused by `report_overlay`, never by a click — is written out
// there. The marker is on the line above the import because that is where the rule looks.
// FOCUS-DECISION-REOPENED: a focus home for the report's `Tab`, focused by code, never by a click.
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::ui::{CalculatedClip, ComputedNode, UiGlobalTransform, UiStack};
use bevy_grimoire_core::feedback::{
    self, Audience, CameraPose, Element, Pair, PairKind, PathStep, Pick, Picture, Report, Scene,
    Subject, WindowRecord, FEEDBACK_VERSION, TEXT_CAP,
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

/// The border drawn round the subject in the second picture, in picture pixels.
const OUTLINE_W: u32 = 3;
/// A white hairline just inside it, so the outline reads on a light ground as well as a dark one.
const OUTLINE_INNER_W: u32 = 1;
/// A scene mark's arm length, in picture pixels, going out from the click each way.
const MARK_ARM_PX: i32 = 24;
/// **How strong the wash over two picks' shared pixels is**, out of 255. The accent at about a
/// third: strong enough to find at a glance, weak enough that what is underneath still reads.
const OVERLAP_ALPHA: u8 = 90;
/// A pick already taken, drawn on the live overlay: the same arm as the picture's mark, so what a
/// person sees while pointing matches what is saved.
const TAKEN_MARK_ARM_PX: f32 = 8.0;
/// **How strong a taken pick's wash is, as a share of `chrome::ink().snip_wash`.** Half: it is the record
/// of a decision already made, not the one being made now, so the live outline stays the loudest thing
/// on the frozen frame.
const PICK_TAKEN_SHARE: f32 = 0.5;

/// How many lines of the editor's log a report carries.
const LOG_TAIL_LINES: usize = 200;

/// What the hint line says while the person is pointing.
const PICK_HINT: &str =
    "Click what is wrong · Tab or wheel steps the stack · Shift+click adds · Enter describes · Esc cancels";
/// What it says once there is at least one pick and the person may add another.
const MULTI_HINT: &str = "Shift+click adds another · Enter describes them · Esc cancels";
/// What it says once the note is being typed.
const NOTE_HINT: &str = "Type what is wrong, then Save · Esc cancels";

/// **How long the note box's pick line may be**, in characters. Two or three picks is what a person
/// adds; the cap is what stops a dozen from growing the box a paragraph.
const PICKS_LINE_CAP: usize = 160;

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

    /// **Every thing that has been picked**, in the order they were picked. Empty while the person
    /// has not picked anything yet.
    pub fn picks(&self) -> &[Pick] {
        self.open.as_ref().map_or(&[], |o| o.picks.as_slice())
    }

    /// **How each two of the picks sit against each other**, worked out from their rectangles by the
    /// core's [`feedback::pairs`] — the same function the saved record runs, so what the note box
    /// says and what an agent reads cannot disagree.
    pub fn relations(&self) -> Vec<Pair> {
        let Some(open) = self.open.as_ref() else {
            return Vec::new();
        };
        feedback::pairs(&open.picks, &open.rects(), &open.front)
    }

    /// What has been typed so far.
    pub fn note(&self) -> &str {
        self.open.as_ref().map_or("", |o| o.note.as_str())
    }

    /// The box being dragged right now, in surface pixels.
    pub fn drag(&self) -> Option<[f32; 4]> {
        self.open.as_ref().and_then(|o| o.area)
    }

    /// **The stack under the pointer and which entry is picked**: what `Tab` and the wheel move.
    pub fn picked_step(&self) -> Option<(usize, usize)> {
        self.open
            .as_ref()
            .map(|o| (o.at, o.stack.len()))
            .filter(|(_, len)| *len > 0)
    }

    /// The entity of the picked entry of the stack, for a test that asserts which one was stepped to.
    pub fn picked_entity(&self) -> Option<Entity> {
        self.open
            .as_ref()
            .and_then(|o| o.stack.get(o.at).copied())
    }

    /// **A report in the picking phase, for the pointer gate's own test.** Nothing else constructs
    /// one: a real report needs a frame and a window, which is what the integration tests drive.
    #[cfg(test)]
    pub(crate) fn pointing_for_test() -> Self {
        ReportEdit {
            open: Some(Open {
                audience: Audience::Developers,
                phase: Phase::Picking,
                taken_at_ms: 0,
                frozen: None,
                whole: None,
                stack: Vec::new(),
                front: Vec::new(),
                at: 0,
                hover: None,
                area: None,
                from: None,
                picks: Vec::new(),
                note: String::new(),
                noted_on: 0,
                screen: "editor".into(),
                project: None,
                map: None,
                tool: None,
                window: WindowRecord {
                    physical: [1280, 720],
                    logical: [1280.0, 720.0],
                    scale_factor: 1.0,
                    ui_scale: 1.2,
                },
            }),
            save_asked: false,
        }
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
    /// **Every node the pointer is inside, front-to-back**, so `[0]` is what a click would hit.
    /// [`stack_under`] fills it; it is a superset of the ancestor chain, which is why the wheel and
    /// `Tab` can reach a node that is covered rather than containing.
    stack: Vec<Entity>,
    /// **Each entry of [`Self::stack`]'s place in the UI's own stack**, lower being further front.
    /// A pick records its own, so the record can say which of two overlapping elements is on top.
    front: Vec<usize>,
    /// Which entry of the stack is picked. `Tab` and the wheel move it, so a person can say "the
    /// button" rather than "the words on the button", or reach the panel behind the button.
    at: usize,
    /// The outline, in surface pixels — always `stack[at]`'s rectangle.
    hover: Option<[f32; 4]>,
    /// The box being dragged, or the one that was dragged.
    area: Option<[f32; 4]>,
    /// Where the drag began.
    from: Option<Vec2>,
    /// **What has been picked so far.** A plain click replaces it with one entry; `Shift+click`
    /// appends. Enter over an empty list is the whole screen.
    picks: Vec<Pick>,
    note: String,
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

/// **Not in the interface's stack at all**: an area, a whole screen, a point on the map. It is the
/// largest rank there is, so a pair against an element puts the element in front, which is what a
/// person sees — the overlay draws over the world and the frozen frame is behind everything.
pub(crate) const NO_FRONT_RANK: usize = usize::MAX;

impl Open {
    /// Each pick's rectangle, or `None` where its subject has none — what
    /// [`feedback::pairs`] compares. The same reading the save writes into the record's `relations`.
    fn rects(&self) -> Vec<Option<[f32; 4]>> {
        self.picks.iter().map(rect_of_pick).collect()
    }

    /// **Take what was just picked**: `Shift` adds to what is there and stays pointing, so a person
    /// can add a third; a plain click replaces the list and moves on to the note.
    ///
    /// `picks` and `ranks` are parallel: a rank is the pick's **front-ness**, `0` being the node a
    /// click would take and [`NO_FRONT_RANK`] meaning the pick is not a node of the interface.
    fn take(&mut self, picked: Vec<Pick>, ranks: &[usize], shift: bool, frame: u32) {
        if !shift {
            self.picks.clear();
            self.front.clear();
        }
        for (pick, rank) in picked.into_iter().zip(ranks.iter().copied()) {
            self.picks.push(pick);
            self.front.push(rank);
        }
        self.area = None;
        self.hover = None;
        self.from = None;
        if shift {
            // Still pointing, so another can be added. The stack is left alone: the pointer has not
            // moved and `Tab` should keep working from where it was.
            return;
        }
        self.noted_on = frame;
        self.phase = Phase::Note;
    }
}

/// The rectangle a pick answers for, in surface pixels, or `None` for a subject that has none.
///
/// A [`Subject::Screen`] has no rectangle; a [`Subject::Scene`] has a point, not a box, so two of them
/// compare as having nothing in common rather than as one containing the other.
fn rect_of_pick(pick: &Pick) -> Option<[f32; 4]> {
    match &pick.subject {
        Subject::Element(element) => Some(element.rect_px),
        Subject::Area { rect_px } => Some(*rect_px),
        Subject::Screen | Subject::Scene(_) => None,
    }
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
    // **`Option`, because the report is in the shared plugin list and an app may have no screens at
    // all.** `the_surface_and_the_camera_agree` builds the editor's plugin set around a hand-made
    // window and no `Screen` state, and a bare `Res<State<Screen>>` panics its system there — measured
    // 2026-11-05. A report still works without a screen; the record says so with an empty `screen`.
    screen: Option<Res<State<Screen>>>,
    project: Option<Res<crate::project::Project>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    state: Option<Res<crate::editor::EditorState>>,
    surface: Option<Res<crate::surface::Surface>>,
    can_render: Option<Res<crate::sequencer::CanRender>>,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
    ui_scale: Res<UiScale>,
    mut report: ResMut<ReportEdit>,
    terminal: Option<Res<crate::terminal_pane::TerminalFocus>>,
    mut commands: Commands,
) {
    if report.open.is_some() {
        return;
    }
    let (chord, audience) = if project_chord().just_pressed(&keys) {
        (project_chord(), Audience::Project)
    } else if developers_chord().just_pressed(&keys) {
        (developers_chord(), Audience::Developers)
    } else {
        return;
    };
    // **Ctrl+D is the shell's while it has the keyboard** (#318): end-of-file, the way most people leave
    // a shell or end a `cat`. Off a mac the report's chord is that very press, and it used to open a
    // report over the shell while the shell exited. Cmd+D on a mac never reaches a shell, so it still
    // opens a report from anywhere.
    if terminal.as_deref().is_some_and(|focus| focus.owns(&chord)) {
        return;
    }
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
        stack: Vec::new(),
        front: Vec::new(),
        at: 0,
        hover: None,
        area: None,
        from: None,
        picks: Vec::new(),
        note: String::new(),
        noted_on: 0,
        screen: screen
            .as_deref()
            .map(|state| screen_name(*state.get()))
            .unwrap_or_default(),
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

/// **Every UI node the point is inside, back-to-front: `[len - 1]` is what a click would take.**
///
/// The same three checks `bevy_ui`'s own picking backend makes — a non-zero size, a visible node, and
/// a point inside its `CalculatedClip` — applied to **every** node in the stack rather than stopping
/// at the first. The walk is the picking backend's own
/// (`bevy_ui-0.20.0/src/picking_backend.rs:150-230`), front to back, and the result is held **the
/// other way round on purpose**: outermost-and-back first is the order an ancestor chain already has
/// (`path` on an [`Element`]), so a chain is just this list cut down to one path. Stepping *down* the
/// stack is therefore one index lower in both, which is what keeps the wheel's direction true.
///
/// **Why the ancestor chain is not enough.** A chain walk returns the chain of the *topmost* hit
/// node, and the wheel used to move within it. An overlapping label is usually a **sibling** of what
/// covers it, so no ancestor walk can reach it: there is nowhere on screen where only the label is
/// under the cursor. This list contains the ancestor chain of every node it finds, so it subsumes the
/// old behaviour rather than replacing it with something narrower.
#[allow(clippy::type_complexity)]
pub fn stack_under(
    point: Vec2,
    stack: &UiStack,
    nodes: &Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&CalculatedClip>,
        Option<&ChildOf>,
    ), (Without<ReportPart>, Without<bevy_gooey::ToastStack>)>,
) -> Vec<Entity> {
    let mut hits: Vec<Entity> = Vec::new();
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
            // A node is reached twice when a parent and its child both fill the point; the first
            // sighting is the front-most one, and that is the one that answers.
            if hits.contains(entity) {
                continue;
            }
            hits.push(*entity);
        }
    }
    // Collected front-to-back, held back-to-front: see the note above.
    hits.reverse();
    hits
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
        // **Not the report's own layer, and not a toast's stack.** The frozen picture covers the whole
        // window, so the report's overlay would answer every click; and gooey's `ToastStack` is a
        // full-window anchor that holds a toast when there is one — measured live on 2026-11-05, a
        // report pointing at the toolbar picked `ToastStack` with no toast on screen at all. A toast
        // that *is* up still wins, because its own node is inside the stack and is not excluded.
        (
            Without<ReportPart>,
            Without<bevy_gooey::ToastStack>,
        ),
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
    /// **Every node the pointer is inside, front-to-back**, which is what `Tab` and the wheel walk.
    fn stack(&self) -> Vec<Entity> {
        let (Some(point), Some(stack)) = (self.pointer.0, self.stack.as_deref()) else {
            return Vec::new();
        };
        stack_under(point, stack, &self.nodes)
    }

    /// **The ancestor chain of one node, outermost first** — the `path` a pick records, and the way
    /// a click still picks a node *and* says what contains it.
    fn chain_of(&self, entity: Entity) -> Vec<Entity> {
        let mut chain = vec![entity];
        let mut at = entity;
        while let Some(parent) = self.nodes.get(at).ok().and_then(|step| step.4).map(ChildOf::parent) {
            chain.push(parent);
            at = parent;
        }
        chain.reverse();
        chain
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
/// is a box. `Shift` on either accumulates a second pick instead of replacing the first. `Tab` and the
/// wheel step **down the stack under the pointer** — the outlined node is what a click will take —
/// and Enter with nothing picked is the whole screen.
///
/// What a click means, in this order:
///
/// 1. **A floating layer over the map is interface.** A dialog, menu, tooltip, popover or scrim is
///    what the person is pointing at even when it sits over the viewport.
/// 2. **Else a point inside the map camera's viewport is the scene** — the ground under it, its map
///    point, its cell, and the placement the editor's own `pick_at` chose there.
/// 3. **Else the outlined node of the stack is the answer**, which is the front-most one until `Tab`
///    or the wheel says otherwise.
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
    let shift = shift_held(&keys);

    // **Enter takes the whole screen**, the one subject that needs no pointer at all.
    if open.phase == Phase::Picking && keys.just_pressed(KeyCode::Enter) {
        if open.picks.is_empty() {
            open.picks
                .push(Pick {
                    subject: Subject::Screen,
                    at: here.unwrap_or(Vec2::ZERO).to_array(),
                });
            open.front.push(NO_FRONT_RANK);
        }
        open.area = None;
        open.hover = None;
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
            let area = Subject::Area {
                rect_px: clamp_rect(rect, open.window.physical),
            };
            open.take(
                vec![Pick {
                    subject: area,
                    at: here.unwrap_or(Vec2::ZERO).to_array(),
                }],
                &[NO_FRONT_RANK],
                shift,
                frames.0,
            );
            return;
        }
        if from.is_some()
            && let Some(at) = here
        {
            match pick_at_point(at, &world, &open.stack, open.at) {
                Ok((subject, rank)) => {
                    open.take(
                        vec![Pick {
                            subject,
                            at: at.to_array(),
                        }],
                        &[rank],
                        shift,
                        frames.0,
                    );
                }
                Err(why) => {
                    commands.spawn(bevy_gooey::toast_alert(why).sticky());
                }
            }
            return;
        }
    }

    // **The stack follows the pointer while nothing is pressed**, and `Tab` or the wheel moves the
    // pick within it.
    if here.is_none() || world.mouse.pressed(MouseButton::Left) {
        return;
    }
    let stack = world.stack();
    let front_moved = open.stack.first() != stack.first();
    open.stack = stack;
    let len = open.stack.len();
    let steps = steps_from(&keys, notches);
    if steps != 0 && len > 0 {
        // A clamp keeps a long scroll from running off either end.
        open.at = (open.at as i32 + steps).clamp(0, len as i32 - 1) as usize;
    } else if front_moved || open.at >= len {
        // A new stack starts at the node a click would take: the last entry.
        open.at = len.saturating_sub(1);
    }
    open.hover = open
        .stack
        .get(open.at)
        .and_then(|e| world.nodes.get(*e).ok())
        .and_then(|(node, tf, ..)| rect_of(node, tf));
}

/// **`Tab` is one step down the stack; `Shift+Tab` and a wheel notch step the other way.**
///
/// **A negative answer is one step down** — one index *lower* in [`Open::stack`], which is held
/// back-to-front, so the last entry is the one a click would take and stepping down walks toward
/// `[0]`. That is the wheel's own direction, pinned by a standing test: a notch up from a button's
/// words picks the button that holds them.
///
/// Both are read here rather than through [`bevy::input_focus`]'s tab navigation, which this module
/// also has to keep out of the editor's own widgets while a report is pointing — see
/// [`report_overlay`], which gives the pointing overlay the focus so `Tab` cycles inside it.
fn steps_from(keys: &ButtonInput<KeyCode>, notches: f32) -> i32 {
    let mut steps = 0;
    if keys.just_pressed(KeyCode::Tab) {
        steps += if shift_held(keys) { 1 } else { -1 };
    }
    if notches != 0.0 {
        steps += if notches > 0.0 { -1 } else { 1 };
    }
    steps
}

/// Is either shift key down? A person reaching for one does not choose which.
fn shift_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

/// **The subject under `at`, and the stack entry it came from**, or the sentence saying why there is
/// none.
///
/// `stack` is the one the person has been stepping — `entry` says which of it they settled on, so a
/// click takes what the outline shows rather than always taking the front-most node.
///
/// **An empty stack is a click with no hover frame before it** (a press and release in one place, on
/// the first frame a report is pointing): the hit test is run here instead, and the answer is the
/// front-most node, because that is what a click would have taken. Reading the caller's `at` there
/// would answer with whatever index the last look at a *different* stack left behind — measured
/// 2026-11-05: every click on the Projects home picked the `Screen`, never the button on it.
fn pick_at_point(
    at: Vec2,
    world: &ReportWorld,
    stack: &[Entity],
    entry: usize,
) -> Result<(Subject, usize), &'static str> {
    let stepped = !stack.is_empty();
    let stack = if stepped {
        stack.to_vec()
    } else {
        world.stack()
    };
    let floating = stack.iter().any(|e| world.floats(*e));
    if !floating && in_map_viewport(at, world) {
        return Ok((scene_at(at, world), NO_FRONT_RANK));
    }
    let entry = if stepped {
        entry.min(stack.len().saturating_sub(1))
    } else {
        stack.len().saturating_sub(1)
    };
    let Some(picked) = stack.get(entry).copied() else {
        return Err(NOTHING_UNDER_POINTER);
    };
    let (node, tf, ..) = world.nodes.get(picked).map_err(|_| NOTHING_UNDER_POINTER)?;
    let rect_px = rect_of(node, tf).ok_or(NOTHING_UNDER_POINTER)?;
    Ok((
        Subject::Element(Element {
            entity: picked.to_bits(),
            rect_px,
            path: world
                .chain_of(picked)
                .iter()
                .map(|e| world.step_of(*e))
                .collect(),
        }),
        entry,
    ))
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

/// How far short of a cell's edge, in cells, a point still counts as on it: a ten-thousandth, a tenth of a
/// millimetre on a metre-wide cell, well above the f32 noise of a pixel round trip and far below a pixel.
const CELL_EDGE_SNAP: f32 = 1.0e-4;

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
        // **A point a hair short of an edge is on the edge.** The ground point is a pixel put back through the
        // camera, which carries f32 noise (`2.999999` for a piece standing at exactly 3), and a move of the
        // viewport by a fraction of a pixel (a UI library's text sizing) is enough to tip a click on an edge into
        // the cell beside it. `scene.map` keeps the point as it came; only the cell label is snapped.
        scene.cell = Some([
            (map_point.0 / bevy_grimoire_core::grid::TILE + CELL_EDGE_SNAP).floor() as i32,
            (map_point.1 / bevy_grimoire_core::grid::TILE + CELL_EDGE_SNAP).floor() as i32,
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
        rect[0].clamp(0.0, w),
        rect[1].clamp(0.0, h),
        rect[2].clamp(0.0, w),
        rect[3].clamp(0.0, h),
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

/// **Draw the frozen frame with what is picked outlined on it, and hold the keyboard inside it.**
///
/// One root, despawned and respawned every frame — the shape `snip_overlay` has, and for the same
/// reason: the frame a report closes, there is nothing stale left to draw.
///
/// # The focus home
///
/// **`Tab` means "down the stack" here, and Bevy's own tab navigation must not fight it.**
/// `bevy_input_focus::tab_navigation::handle_tab_navigation` is an observer on
/// `FocusedInput<KeyboardInput>`; with nothing focused it reaches the primary window, which has no
/// `TabGroup`, and **warns on every press**. So the pointing root is given a **modal** `TabGroup` and
/// a `TabIndex`, and focused. `Tab` then cycles inside the report — this root is its only tabbable
/// node — so the editor's widgets never see it, while `ButtonInput<KeyCode>` still does, and
/// [`report_pick`] is what actually moves the pick.
///
/// **This reopens `the_sweep_is_finished`'s focus decision, deliberately, and the marker says so.**
/// That test's note assumes a `TabIndex` is how focus traversal creeps in, and its own condition is
/// "whatever gains focus must be reachable without a click". This one is: **this system focuses it**
/// (`focus.set` below) the frame the report starts pointing, and nothing else can — the overlay is
/// `Pickable::IGNORE`, so a click does not land on it at all, and `report_pick` reads the pointer
/// itself rather than going through `Hovered`. FVS-R-25's broken case is click-to-focus following the
/// *window's* cursor, which is a path this overlay does not have. When the report closes the root is
/// despawned and `bevy_input_focus` clears focus with it, so the editor's routing is untouched.
fn report_overlay(
    mut commands: Commands,
    report: Res<ReportEdit>,
    ui_scale: Res<UiScale>,
    mut focus: ResMut<InputFocus>,
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
        Phase::Picking if open.picks.is_empty() => (open.hover, open.area, PICK_HINT),
        Phase::Picking => (open.hover, open.area, MULTI_HINT),
    };
    // **What has already been taken**, so a person adding a second pick can see the first: a box gets
    // the same border in the quieter wash, and a point on the map gets a small cross.
    let taken: Vec<[f32; 4]> = open.picks.iter().filter_map(rect_of_pick).collect();
    let marks: Vec<Vec2> = open
        .picks
        .iter()
        .filter(|pick| matches!(pick.subject, Subject::Scene(_)))
        .map(|pick| Vec2::from_array(pick.at))
        .collect();
    let mut root = commands.spawn((
        ReportOpen,
        ReportPart,
        // The whole frozen frame, drawn over the window rather than in any layout (`cover` lets the pointer through).
        bevy_gooey::cover(),
        GlobalZIndex(REPORT_Z),
        // The focus home: see this function's note. Only while pointing, so the note box's field
        // takes the keyboard the moment it is up.
        TabGroup::modal(),
        // FOCUS-DECISION-REOPENED: the report's `Tab` needs a focus home; see this function's note.
        TabIndex(0),
    ));
    let root_id = root.id();
    root.with_children(|root| {
        if let Some(frozen) = open.frozen.clone() {
            root.spawn((
                ReportPart,
                ImageNode::new(frozen),
                // The frozen frame is the layer everything else is placed against.
                bevy_gooey::cover(),
            ));
        }
        // **The picks already taken, the live outline, and the box being dragged**, drawn the same
        // way: what is outlined is what will be recorded, and a drag is that answer in the making.
        let boxes = taken
            .into_iter()
            .map(|rect| (Some(rect), chrome::wash_at(chrome::ink().snip_wash, PICK_TAKEN_SHARE)))
            .chain([(outline, Color::NONE), (dragged, chrome::ink().snip_wash)]);
        for (rect, wash) in boxes {
            let Some(rect) = rect else { continue };
            root.spawn((
                ReportPart,
                // The outline is placed at the pick over the frozen frame, not in a layout: its corner, its size and a 3 px edge that is exactly 3 px, like the drag's own.
                bevy_gooey::placed(UiRect { left: Val::Px(overlay_px(rect[0])), top: Val::Px(overlay_px(rect[1])), ..UiRect::all(Val::Auto) }),
                bevy_gooey::Sizing {
                    width: Some(bevy_gooey::Extent::Px(overlay_px((rect[2] - rect[0]).max(0.0)))),
                    height: Some(bevy_gooey::Extent::Px(overlay_px((rect[3] - rect[1]).max(0.0)))),
                    ..default()
                },
                bevy_gooey::BorderPx(bevy_gooey::Sides::all(chrome::SNIP_EDGE_W)),
                bevy_gooey::InkOverride { edge: Some(chrome::ink().accent), ..default() },
                BorderColor::all(chrome::ink().accent),
                BackgroundColor(wash),
            ));
        }
        for at in marks {
            let arm = overlay_px(TAKEN_MARK_ARM_PX);
            root.spawn((
                ReportPart,
                // A mark sits on its own pixel of the frozen frame: the horizontal arm.
                bevy_gooey::placed(UiRect { left: Val::Px(overlay_px(at.x) - arm), top: Val::Px(overlay_px(at.y) - arm * 0.2), ..UiRect::all(Val::Auto) }),
                bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(arm * 2.0)), height: Some(bevy_gooey::Extent::Px(arm * 0.4)), ..default() },
                BackgroundColor(chrome::ink().accent),
            ));
            root.spawn((
                ReportPart,
                // The cross's other arm, on the same pixel.
                bevy_gooey::placed(UiRect { left: Val::Px(overlay_px(at.x) - arm * 0.2), top: Val::Px(overlay_px(at.y) - arm), ..UiRect::all(Val::Auto) }),
                bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(arm * 0.4)), height: Some(bevy_gooey::Extent::Px(arm * 2.0)), ..default() },
                BackgroundColor(chrome::ink().accent),
            ));
        }
        root.spawn((
            ReportPart,
            Text::new(hint.to_owned()),
            chrome::typed(chrome::text::LABEL),
            TextColor(chrome::ink().text),
            TextLayout::default().with_no_wrap(),
            // The hint rides the top edge of the frozen frame, not a panel: pinned left and right so it is as wide as the frame, its words centred.
            bevy_gooey::placed(UiRect { left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Px(chrome::GAP_GROUP), bottom: Val::Auto }),
            bevy_gooey::RowLayout { justify: Some(JustifyContent::Center), ..default() },
        ));
    });
    // **Focus the root while pointing**, so `Tab` cycles inside the report rather than warning on the
    // window and reaching the editor's own widgets.
    if open.phase == Phase::Picking && focus.get() != Some(root_id) {
        focus.set(root_id, bevy::input_focus::FocusCause::Navigated);
    }
}

/// **What one pick is, in words**, without the "Picked: " a whole line would repeat.
fn pick_words(subject: &Subject) -> String {
    match subject {
        Subject::Screen => "the whole screen".to_owned(),
        Subject::Area { rect_px } => format!(
            "an area {:.0} × {:.0}",
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
            format!("{} ({})", what, trail.join(" › "))
        }
        Subject::Scene(scene) => {
            let cell = scene
                .cell
                .map(|c| format!(", cell ({}, {})", c[0], c[1]))
                .unwrap_or_default();
            match &scene.placement {
                Some(id) => format!("placed piece `{id}`{cell}"),
                None => format!(
                    "the map at {:.1}, {:.1}{cell}",
                    scene.world[0], scene.world[2]
                ),
            }
        }
    }
}

/// **What a person is about to report, in one line**, so the note box says what Save will record.
///
/// One pick reads `Picked: <what>`. Two read `Picked: <a> + <b>`, which is how a person says the
/// relation out loud. Three or more read `Picked 3 things: <a>, <b>, <c>`. The whole line is capped
/// at [`PICKS_LINE_CAP`] characters, so a dozen picks cannot grow the note box a paragraph.
pub fn picks_line(picks: &[Pick]) -> String {
    let line = match picks {
        [] => "Picked: nothing".to_owned(),
        [one] => format!("Picked: {}", pick_words(&one.subject)),
        [a, b] => format!(
            "Picked: {} + {}",
            pick_words(&a.subject),
            pick_words(&b.subject)
        ),
        many => {
            let each: Vec<String> = many.iter().map(|p| pick_words(&p.subject)).collect();
            format!("Picked {} things: {}", many.len(), each.join(", "))
        }
    };
    if line.chars().count() <= PICKS_LINE_CAP {
        return line;
    }
    // Cut on a character boundary and say so, rather than ending mid-word with no sign it happened.
    let mut cut: String = line.chars().take(PICKS_LINE_CAP.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// **How two picks sit against each other, in the words the note box shows** — the relation, which
/// pick is which, and the number an agent would act on.
///
/// Picks are named by their index in the order they were made, counting from 0, which is the same
/// number the record's `relations` carries.
pub fn relation_line(pair: &Pair) -> String {
    let (a, b) = (pair.a, pair.b);
    match pair.kind {
        PairKind::Contains => format!("contains: {a} and {b}, one inside the other"),
        PairKind::Overlaps => {
            // The one in front is `pair.front`; the other is the one it covers.
            let behind = if pair.front == a { b } else { a };
            format!(
                "overlaps: {} over {behind} by {} px²",
                pair.front,
                thousands(pair.overlap_px2)
            )
        }
        PairKind::Touches => format!("touches: {a} and {b} meet at an edge"),
        PairKind::Apart => format!("apart: {a} and {b} share nothing"),
    }
}

/// **`4312.0` as `4 312`**: a count of pixels read at a glance, with the separator a person groups
/// digits by rather than a machine's. Rounded, because a fraction of a pixel is not a fact.
fn thousands(area: f32) -> String {
    let whole = area.round().max(0.0) as u64;
    let digits = whole.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
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
    let Some(audience) = report.audience() else {
        return;
    };
    let picks = report.picks();
    if picks.is_empty() {
        return;
    }
    let relations = report.relations();
    let heading = match audience {
        Audience::Developers => "Report to those who make Cauldron",
        Audience::Project => "Report about this project",
    };
    let line = picks_line(picks);
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
                .spawn(bevy_gooey::sized_panel(bevy_gooey::CardWidth::Text(crate::launcher::DIALOG_CARD_TEXT_SIZES)))
                .with_children(|card| {
                    card.spawn(bevy_gooey::heading(heading));
                    card.spawn(bevy_gooey::label(line));
                    // **The relation, worked out and stated, never asked.** A person's only decision is
                    // the sentence they type; what the boxes do to each other is geometry.
                    for pair in &relations {
                        card.spawn(bevy_gooey::label(relation_line(pair)));
                    }
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
                    chrome::end_row(card, chrome::GAP_GROUP_STEPS)
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
    if open.picks.is_empty() {
        return;
    }
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
    let picks = open.picks.clone();
    let relations = feedback::pairs(&picks, &open.rects(), &open.front);
    let mut marked = whole.clone();
    // **The wash first, then the outlines**, so an overlap reads as the place two boxes share rather
    // than as a border drawn over a tint. This is the thing that makes "the label is under the view"
    // legible in one glance.
    for pair in &relations {
        if pair.kind != PairKind::Overlaps {
            continue;
        }
        let (Some(ra), Some(rb)) = (rect_of_pick(&picks[pair.a]), rect_of_pick(&picks[pair.b]))
        else {
            continue;
        };
        if let Some(shared) = feedback::overlap_rect(ra, rb) {
            draw_wash(&mut marked, shared, accent_rgb(), OVERLAP_ALPHA);
        }
    }
    // **The newest pick in the danger colour, every earlier one in the accent**, so a person who
    // added three can tell which one they just made. A scene pick has no rectangle: its cross sits
    // where the pointer was.
    for (i, pick) in picks.iter().enumerate() {
        let colour = if i + 1 == picks.len() {
            danger_rgb()
        } else {
            accent_rgb()
        };
        match &pick.subject {
            Subject::Element(element) => draw_outline(&mut marked, element.rect_px, colour),
            Subject::Area { rect_px } => draw_outline(&mut marked, *rect_px, colour),
            Subject::Scene(_) => draw_mark(&mut marked, Vec2::from_array(pick.at), colour),
            Subject::Screen => {}
        }
    }

    let mut writes: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    match encode_png(&whole) {
        Ok(bytes) => writes.push((dir.join(feedback::picture_file(&stem)), bytes)),
        Err(why) => return refuse(report, world, &why),
    }
    // **A whole-screen report has one picture**, because the second would be the first.
    let whole_screen = picks.len() == 1 && picks[0].subject == Subject::Screen;
    let picks_picture = if whole_screen {
        None
    } else {
        match encode_png(&marked) {
            Ok(bytes) => {
                writes.push((dir.join(feedback::picks_picture_file(&stem)), bytes));
                Some(Picture {
                    file: feedback::picks_picture_file(&stem),
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
        picks,
        relations,
        note: open.note.clone(),
        picture: Picture {
            file: feedback::picture_file(&stem),
            size: [w, h],
        },
        picks_picture,
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

/// **A border round `rect` in `colour`, with a white hairline just inside it** — so the outline reads
/// on a dark panel and on a light one, and a reader who opens the picture sees the same rectangle the
/// person was pointing at.
fn draw_outline(image: &mut image::RgbaImage, rect: [f32; 4], colour: [u8; 4]) {
    let (w, h) = image.dimensions();
    let clamp_x = |x: f32| x.round().clamp(0.0, w as f32) as u32;
    let clamp_y = |y: f32| y.round().clamp(0.0, h as f32) as u32;
    let x0 = clamp_x(rect[0]);
    let y0 = clamp_y(rect[1]);
    let x1 = clamp_x(rect[2]).max(x0);
    let y1 = clamp_y(rect[3]).max(y0);
    for band in 0..OUTLINE_W + OUTLINE_INNER_W {
        let colour = if band < OUTLINE_W {
            colour
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

/// **A two-armed cross at `at`**, in `colour` — how a report says "here" about a point in the world,
/// where there is no rectangle to outline.
fn draw_mark(image: &mut image::RgbaImage, at: Vec2, colour: [u8; 4]) {
    let (w, h) = image.dimensions();
    let cx = at.x.round().clamp(0.0, w as f32) as i32;
    let cy = at.y.round().clamp(0.0, h as f32) as i32;
    for d in -MARK_ARM_PX..=MARK_ARM_PX {
        put_signed(image, cx + d, cy, colour);
        put_signed(image, cx, cy + d, colour);
    }
}

/// The accent as RGBA8, so the picture's outline and the on-screen outline are the same colour.
fn accent_rgb() -> [u8; 4] {
    rgba8(chrome::ink().accent)
}

/// **`chrome::ink().danger` as RGBA8**, for the newest pick: the same red the interface uses for "wrong",
/// so a person opening the picture finds the thing they added last.
fn danger_rgb() -> [u8; 4] {
    rgba8(chrome::ink().danger)
}

/// A chrome colour as opaque RGBA8.
fn rgba8(colour: Color) -> [u8; 4] {
    let colour = bevy::color::Srgba::from(colour);
    [
        (colour.red * 255.0).round() as u8,
        (colour.green * 255.0).round() as u8,
        (colour.blue * 255.0).round() as u8,
        255,
    ]
}

/// **A translucent wash over `rect`**, blended over the pixels already there at `alpha` (0–255).
///
/// The overlap between two picks is the one thing a single-pick record could never show, so it is
/// drawn rather than described: where two boxes share pixels, the shared box is tinted.
fn draw_wash(image: &mut image::RgbaImage, rect: [f32; 4], colour: [u8; 4], alpha: u8) {
    if alpha == 0 {
        return;
    }
    let (w, h) = image.dimensions();
    let clamp_x = |x: f32| x.round().clamp(0.0, w as f32) as u32;
    let clamp_y = |y: f32| y.round().clamp(0.0, h as f32) as u32;
    let x0 = clamp_x(rect[0]);
    let y0 = clamp_y(rect[1]);
    let x1 = clamp_x(rect[2]).max(x0);
    let y1 = clamp_y(rect[3]).max(y0);
    let a = f32::from(alpha) / 255.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let under = image.get_pixel(x, y).0;
            let blend = |i: usize| {
                (f32::from(colour[i]) * a + f32::from(under[i]) * (1.0 - a)).round() as u8
            };
            image.put_pixel(
                x,
                y,
                image::Rgba([blend(0), blend(1), blend(2), under[3].max(alpha)]),
            );
        }
    }
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

// **`PickingSettings` has exactly one writer, and it used to have two.**
//
// The snip turns picking off while it is armed (`snip::snip_gates_picking`) and a report turns it off
// while it is pointing. Two systems each saying "I write `is_enabled`, and I am the only one" is the
// second-census failure this crate refuses everywhere else: whichever ran last won, and
// `a_snip_armed_over_a_button_does_not_press_it` said so — measured 2026-11-05, the first time both
// features existed. `bevy_cauldron::picking` is the one writer; each gesture only *asks*.
//
// The rule each gesture states is its own, and they are not the same question: a snip is armed, a
// report is pointing.

/// **A report that starts pointing drops the presses already in flight.** See
/// [`snip::snip_gates_picking`](crate::snip) for the same rule and the measurement behind it: picking
/// off cannot stop a release from completing a press that began before the overlay arrived.
fn report_drops_presses(
    report: Res<ReportEdit>,
    mut in_flight: Query<Entity, With<bevy::ui::Pressed>>,
    mut commands: Commands,
) {
    if report.is_changed() && report.pointing() {
        crate::picking::drop_presses(&mut in_flight, &mut commands);
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
            // **A report that starts pointing drops every press already in flight.** B-032: turning
            // picking off stops *new* presses, and the press that set `Pressed` may have begun before
            // the report opened, so its release would still activate the button under the overlay.
            .add_systems(Update, report_drops_presses);
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

    fn pick(subject: Subject) -> Pick {
        Pick {
            subject,
            at: [0.0, 0.0],
        }
    }

    /// **A subject says what it is, and several say what they are together**, so the note box names
    /// what Save will record without a person having to remember what they clicked.
    #[test]
    fn every_subject_reads_as_a_line() {
        assert_eq!(
            picks_line(&[pick(Subject::Screen)]),
            "Picked: the whole screen"
        );
        assert_eq!(
            picks_line(&[pick(Subject::Area {
                rect_px: [0.0, 0.0, 220.0, 140.0]
            })]),
            "Picked: an area 220 × 140"
        );
        assert_eq!(
            picks_line(&[pick(scene_subject())]),
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
        assert_eq!(
            picks_line(&[pick(element.clone())]),
            "Picked: Save (Panel › Button)"
        );
        // **Two picks are joined with a `+`**, which is how a person says the relation out loud;
        // three or more are counted, because a list of a dozen does not fit a box.
        assert_eq!(
            picks_line(&[
                pick(element.clone()),
                pick(Subject::Area {
                    rect_px: [0.0, 0.0, 220.0, 140.0]
                }),
            ]),
            "Picked: Save (Panel › Button) + an area 220 × 140"
        );
        let three = picks_line(&[
            pick(element.clone()),
            pick(element.clone()),
            pick(Subject::Screen),
        ]);
        assert!(three.starts_with("Picked 3 things: "), "{three}");
        assert!(three.contains("the whole screen"), "{three}");
        assert!(picks_line(&[]).starts_with("Picked: "), "an empty list still answers");
    }

    /// **The line is capped**, so a report of a dozen picks cannot grow the note box a paragraph: the
    /// cap cuts on a character boundary and leaves a mark that it did.
    #[test]
    fn a_picks_line_is_capped() {
        let many: Vec<Pick> = (0..40)
            .map(|_| {
                pick(Subject::Area {
                    rect_px: [0.0, 0.0, 220.0, 140.0],
                })
            })
            .collect();
        let line = picks_line(&many);
        assert_eq!(line.chars().count(), PICKS_LINE_CAP, "{line}");
        assert!(line.ends_with('…'), "the cut says so: {line}");
    }

    /// **A relation is stated, not asked**: the note box says which picks and how many pixels, and the
    /// one in front is named as `over`, so "this label is under the viewport" reads off the box.
    #[test]
    fn a_relation_reads_as_a_sentence() {
        let overlap = Pair {
            a: 0,
            b: 1,
            kind: PairKind::Overlaps,
            overlap_px2: 4312.0,
            front: 1,
        };
        assert_eq!(relation_line(&overlap), "overlaps: 1 over 0 by 4 312 px²");
        let contains = Pair {
            kind: PairKind::Contains,
            ..overlap
        };
        assert_eq!(
            relation_line(&contains),
            "contains: 0 and 1, one inside the other"
        );
        let touches = Pair {
            kind: PairKind::Touches,
            overlap_px2: 0.0,
            ..overlap
        };
        assert_eq!(
            relation_line(&touches),
            "touches: 0 and 1 meet at an edge"
        );
        let apart = Pair {
            kind: PairKind::Apart,
            overlap_px2: 0.0,
            ..overlap
        };
        assert_eq!(relation_line(&apart), "apart: 0 and 1 share nothing");
    }

    /// A count of pixels is grouped the way a person reads digits, and a fraction is not a fact.
    #[test]
    fn an_area_is_grouped_by_thousands() {
        assert_eq!(thousands(0.0), "0");
        assert_eq!(thousands(999.4), "999");
        assert_eq!(thousands(1000.0), "1 000");
        assert_eq!(thousands(4312.4), "4 312");
        assert_eq!(thousands(1_234_567.0), "1 234 567");
    }

    /// **The outline lands where the rectangle is, in the colour asked for**, so a reader who opens
    /// the picture sees the same box the person was pointing at — and nothing inside it.
    #[test]
    fn the_outline_is_drawn_where_the_rectangle_is() {
        let mut picture = image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]));
        draw_outline(&mut picture, [10.0, 8.0, 40.0, 30.0], accent_rgb());
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

        // **The newest pick is the danger colour**, so a person who added three can find the last.
        let mut picture = image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]));
        draw_outline(&mut picture, [10.0, 8.0, 40.0, 30.0], danger_rgb());
        assert_eq!(picture.get_pixel(10, 8).0, danger_rgb());
        assert_ne!(danger_rgb(), accent_rgb(), "the two read as different colours");
    }

    /// **The wash tints the shared box and nothing outside it**, so the place two picks overlap is
    /// the one part of the picture that changed.
    #[test]
    fn an_overlap_is_washed_where_the_two_share() {
        let mut picture = image::RgbaImage::from_pixel(32, 32, image::Rgba([0, 0, 0, 255]));
        draw_wash(&mut picture, [8.0, 8.0, 16.0, 16.0], accent_rgb(), OVERLAP_ALPHA);
        let washed = picture.get_pixel(10, 10).0;
        assert_ne!(washed, [0, 0, 0, 255], "the shared box is tinted");
        assert_eq!(picture.get_pixel(4, 4).0, [0, 0, 0, 255], "outside it is not");
        assert_eq!(picture.get_pixel(16, 16).0, [0, 0, 0, 255], "the far edge is outside");
        // A zero alpha is no wash at all, and never a panic.
        let mut untouched = picture.clone();
        draw_wash(&mut untouched, [0.0, 0.0, 32.0, 32.0], accent_rgb(), 0);
        assert_eq!(untouched, picture);
    }

    /// A scene has no rectangle, so its mark is a cross at the point — in the colour it was given.
    #[test]
    fn a_scene_is_marked_at_the_point() {
        let mut picture = image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]));
        draw_mark(&mut picture, Vec2::new(32.0, 24.0), accent_rgb());
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
