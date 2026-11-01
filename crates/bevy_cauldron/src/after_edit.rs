//! **When the project's after-edit checks are due: once per operation an agent applied, never for a
//! person's own edit (#111).**
//!
//! A person editing is not interrupted by a model call, and a finding about their own placement would
//! be a comment nobody asked for. An agent's operation is different: nobody was watching it land, so
//! the editor looks for it. The trigger reads the operation log, the same file the journal's steps are
//! read from, and so sees an agent over BRP and over MCP alike without either side calling it.
//!
//! This module decides *when*; it takes no picture and calls no model. [`AfterEditQueue::pending`] is
//! what a runner drains: the sequence and name of each agent operation whose checks are owed.
//!
//! What triggers: a line whose `source` is `agent`, that was `applied` (it has a `result`), and that is
//! not an undo (an undo puts a state back that was already looked at). What does not: a person's line,
//! a refusal, anything already in the log when the editor opened the project.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use bevy::prelude::*;
use bevy_grimoire_decide::{Checks, Config, Error as DecideError, Evidence, Finding, Http, Transport};

use crate::ops::{Entry, Source};

/// One agent operation whose after-edit checks are owed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentEdit {
    /// The operation log line it is, so a finding hangs on the step that made it.
    pub seq: u64,
    /// The operation's name (`placement_add`).
    pub op: String,
    /// The id the agent cites, which a finding names.
    pub operation_id: String,
    /// What it was called with, which the checks' facts carry.
    pub params: serde_json::Value,
    /// The operation's own sentence about what it did.
    pub result: String,
}

/// **The checks owed, oldest first.**
#[derive(Resource, Default, Debug)]
pub struct AfterEditQueue {
    /// Drained by whatever runs the checks.
    pub pending: Vec<AgentEdit>,
    /// The last log sequence looked at. `None` until the first read, which takes the log as it stands
    /// as already seen: history is not re-checked each time the editor opens.
    seen: Option<u64>,
}

/// The operation name a person's **Check view** run carries, which a finding's line names.
pub const CHECK_VIEW: &str = "check view";

/// The button in the journal panel that runs the after-edit checks on what is on screen now.
#[derive(Component)]
pub struct CheckViewButton;

impl AfterEditQueue {
    /// **A person asked for the checks on the current view.** It is queued like an agent's operation,
    /// so it takes the same picture and the same path; only the person's click makes it, never an edit.
    pub fn request_view(&mut self) {
        self.pending.push(AgentEdit {
            seq: 0,
            op: CHECK_VIEW.into(),
            operation_id: CHECK_VIEW.into(),
            params: serde_json::Value::Null,
            result: String::new(),
        });
    }
}

/// **The journal's Check view button was pressed.** Global observer: any `Activate` anywhere fires it,
/// so the marker query is the guard and the queue is an `Option`.
fn on_check_view(
    activate: On<bevy::ui_widgets::Activate>,
    buttons: Query<(), With<CheckViewButton>>,
    queue: Option<ResMut<AfterEditQueue>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    if let Some(mut queue) = queue {
        queue.request_view();
    }
}

/// **The agent operations in a log's text with a sequence above `after`**, oldest first.
pub fn agent_edits(text: &str, after: Option<u64>) -> Vec<AgentEdit> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .filter(|e| after.is_none_or(|a| e.seq > a))
        .filter(|e| e.source == Source::Agent && e.result.is_some() && e.undoes.is_none())
        .filter_map(|e| {
            Some(AgentEdit {
                seq: e.seq,
                op: e.op,
                operation_id: e.operation_id,
                params: e.params,
                result: e.result?,
            })
        })
        .collect()
}

/// The highest sequence in a log's text.
fn last_seq(text: &str) -> Option<u64> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .map(|e| e.seq)
        .max()
}

/// **Queue the checks owed for agent operations that landed since the last look.**
fn queue_after_edit(
    project: Option<Res<crate::project::Project>>,
    mut queue: ResMut<AfterEditQueue>,
) {
    let Some(project) = project else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(crate::ops::oplog_path(project.as_ref())) else {
        return;
    };
    let Some(newest) = last_seq(&text) else {
        return;
    };
    match queue.seen {
        // First look: what is there already was checked, or was not asked about, in its own session.
        None => queue.seen = Some(newest),
        Some(seen) if newest > seen => {
            let fresh = agent_edits(&text, Some(seen));
            queue.seen = Some(newest);
            queue.pending.extend(fresh);
        }
        Some(_) => {}
    }
}

/// **What the after-edit checks made of one agent operation.**
#[derive(Clone, Debug, PartialEq)]
pub struct EditReport {
    pub seq: u64,
    pub op: String,
    /// Every check that ran and answered, found or not.
    pub findings: Vec<Finding>,
    /// Checks that needed pictures or facts the editor could not give, by name. A check never answers
    /// from less, so these are said rather than guessed.
    pub skipped: Vec<String>,
    /// Checks that could not be asked (the endpoint did not answer), each with the reason.
    pub errors: Vec<String>,
}

impl EditReport {
    /// **The journal lines, advisory and one per thing worth saying.** A clear check is silent.
    pub fn journal_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .findings
            .iter()
            .filter_map(|f| f.message().map(|m| format!("{m} (after {})", self.op)))
            .collect();
        if self.op == CHECK_VIEW && lines.is_empty() && self.skipped.is_empty() && self.errors.is_empty() {
            lines.push(format!("{CHECK_VIEW}: {} checks, nothing found", self.findings.len()));
        }
        if !self.skipped.is_empty() {
            lines.push(format!(
                "after {}: {} not asked, the editor cannot capture what they need yet",
                self.op,
                self.skipped.join(", ")
            ));
        }
        lines.extend(self.errors.iter().map(|e| format!("after {}: {e}", self.op)));
        lines
    }
}

/// **The facts a check may carry for an operation**: what was run and what it said.
fn facts_of(edit: &AgentEdit) -> serde_json::Value {
    serde_json::json!({
        "operation_id": edit.operation_id,
        "operation": edit.op,
        "parameters": edit.params,
        "result": edit.result,
    })
}

/// **Run every after-edit check on each operation.** `capture` supplies the pictures by name; today the
/// editor supplies none, so any check that needs one is reported as skipped. Blocking: call it off the
/// main thread.
pub fn run_due(
    edits: &[AgentEdit],
    checks: &Checks,
    transport: &dyn Transport,
    capture: &dyn Fn(&AgentEdit) -> BTreeMap<String, String>,
) -> Vec<EditReport> {
    edits
        .iter()
        .map(|edit| {
            let evidence = Evidence {
                images: capture(edit),
                facts: Some(facts_of(edit)),
            };
            let mut report = EditReport {
                seq: edit.seq,
                op: edit.op.clone(),
                findings: Vec::new(),
                skipped: Vec::new(),
                errors: Vec::new(),
            };
            for check in checks.checks.iter().filter(|c| c.after_edit) {
                match check.run(transport, &evidence) {
                    Ok(f) => report.findings.push(f),
                    Err(DecideError::MissingEvidence { .. }) => report.skipped.push(check.name.clone()),
                    Err(e) => report.errors.push(format!("{}: {e}", check.name)),
                }
            }
            report
        })
        .collect()
}

/// Reports coming back from the threads that ask.
#[derive(Resource)]
struct Reports {
    tx: Mutex<Sender<Vec<EditReport>>>,
    rx: Mutex<Receiver<Vec<EditReport>>>,
}

impl Default for Reports {
    fn default() -> Self {
        let (tx, rx) = channel();
        Reports { tx: Mutex::new(tx), rx: Mutex::new(rx) }
    }
}

/// **Operations waiting for the editor's picture**, by ticket. A screenshot lands a frame or two after it
/// is asked for, and a headless app never lands one; the wait is bounded so such an operation is still
/// asked about, with the checks that need a picture reported as skipped.
#[derive(Resource, Default, Debug)]
pub struct ShotWait {
    next: u64,
    waiting: BTreeMap<u64, (Vec<AgentEdit>, u32)>,
}

/// Frames a screenshot may take before the operations waiting on it are asked about without it.
pub const SHOT_FRAMES: u32 = 180;

impl ShotWait {
    /// Hold `edits` until their picture lands. Returns the ticket the picture will carry.
    pub fn hold(&mut self, edits: Vec<AgentEdit>) -> u64 {
        let id = self.next;
        self.next += 1;
        self.waiting.insert(id, (edits, 0));
        id
    }

    /// The picture for `ticket` landed: the operations it was for, or none when it already timed out.
    pub fn deliver(&mut self, ticket: u64) -> Option<Vec<AgentEdit>> {
        self.waiting.remove(&ticket).map(|(edits, _)| edits)
    }

    /// One frame passes. Returns what has waited [`SHOT_FRAMES`] and is now released without a picture.
    pub fn tick(&mut self) -> Vec<Vec<AgentEdit>> {
        for (_, f) in self.waiting.values_mut() {
            *f += 1;
        }
        let late: Vec<u64> = self.waiting.iter().filter(|(_, (_, f))| *f >= SHOT_FRAMES).map(|(id, _)| *id).collect();
        late.into_iter().filter_map(|id| self.deliver(id)).collect()
    }
}

/// Pictures coming back from the screenshot observer: a ticket and the pictures, or `None` when the
/// capture could not become a PNG.
#[derive(Resource)]
struct Shots {
    tx: Mutex<Sender<(u64, Option<Pictures>)>>,
    rx: Mutex<Receiver<(u64, Option<Pictures>)>>,
}

impl Default for Shots {
    fn default() -> Self {
        let (tx, rx) = channel();
        Shots { tx: Mutex::new(tx), rx: Mutex::new(rx) }
    }
}

/// Does any after-edit check ask for the editor's own picture (`view`)?
pub fn wants_view(checks: &Checks) -> bool {
    checks.checks.iter().any(|c| c.after_edit && c.images.iter().any(|i| i == "view"))
}

/// Does any after-edit check ask for a picture the editor takes (`view`, or a `closeup` cut from it)?
pub fn wants_picture(checks: &Checks) -> bool {
    wants_view(checks) || checks.checks.iter().any(|c| c.after_edit && c.images.iter().any(|i| matches!(i.as_str(), "closeup" | "before" | "after")))
}

/// **The editor's last picture of the scene**, kept so the next run has a `before`. A run's `view` is
/// the scene after its edit; it is the next run's `before`. The first run has none, so a check that
/// needs `before` is reported as skipped rather than answered from less.
#[derive(Resource, Default, Debug)]
pub struct PictureMemory {
    last: Option<String>,
}

impl PictureMemory {
    /// Remember `view` (the picture after this run's edits) and return the one it replaces.
    pub fn swap(&mut self, view: Option<String>) -> Option<String> {
        match view {
            Some(v) => self.last.replace(v),
            None => self.last.clone(),
        }
    }
}

/// The picture as the data URL a model request takes.
fn png_data_url(image: &Image) -> Option<String> {
    rgba_data_url(&image.clone().try_into_dynamic().ok()?.to_rgba8())
}

fn rgba_data_url(rgba: &image::RgbaImage) -> Option<String> {
    use base64::Engine as _;
    let mut png = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.into_inner())))
}

/// The close-up's side, as a fraction of the picture's shorter side: wide enough to show what a piece
/// stands on and touches, narrow enough that it fills the frame.
pub const CLOSEUP_FRACTION: f32 = 0.4;

/// **Where the close-up of a point is in a picture**, as `[x, y, width, height]` in pixels. `centre` is
/// the point as a fraction of the picture (0..1 each way). The square is [`CLOSEUP_FRACTION`] of the
/// shorter side and is slid inside the picture rather than shrunk, so a piece at the edge still gets a
/// full-size frame. A point off the picture is clamped to its nearest edge.
pub fn closeup_rect(centre: (f32, f32), size: (u32, u32)) -> [u32; 4] {
    let side = ((size.0.min(size.1) as f32 * CLOSEUP_FRACTION).round() as u32).clamp(1, size.0.min(size.1).max(1));
    let place = |c: f32, len: u32| {
        let mid = (c.clamp(0.0, 1.0) * len as f32).round() as i64;
        (mid - (side / 2) as i64).clamp(0, (len - side) as i64) as u32
    };
    [place(centre.0, size.0), place(centre.1, size.1), side, side]
}

/// **Where an agent's `at` is in map space**, from a placement's parameters.
pub fn edit_at(params: &serde_json::Value) -> Option<(f32, f32)> {
    let at = params.get("at")?.as_array()?;
    Some((at.first()?.as_f64()? as f32, at.get(1)?.as_f64()? as f32))
}

/// The crop of `image` named by `rect`, as a data URL.
pub fn crop_data_url(image: &image::DynamicImage, rect: [u32; 4]) -> Option<String> {
    rgba_data_url(&image.crop_imm(rect[0], rect[1], rect[2], rect[3]).to_rgba8())
}

/// What the editor's one screenshot becomes: the whole picture and a close-up for each operation
/// that has a place in it.
#[derive(Debug, Default)]
struct Pictures {
    view: Option<String>,
    before: Option<String>,
    closeups: BTreeMap<u64, String>,
}

/// Where each owed operation is on screen (fractions of the viewport), by log sequence. An operation
/// with no place, or whose piece is not drawn, has no entry and so no close-up.
fn frames_of(
    edits: &[AgentEdit],
    origin: (f32, f32, f32),
    camera: &Camera,
    cam_tf: &GlobalTransform,
    placed: &Query<&GlobalTransform, With<crate::editor::Placement>>,
) -> BTreeMap<u64, (f32, f32)> {
    let Some(vp) = camera.logical_viewport_size().filter(|s| s.x > 0.0 && s.y > 0.0) else {
        return BTreeMap::new();
    };
    edits
        .iter()
        .filter_map(|e| {
            let at = edit_at(&e.params)?;
            let (wx, wz) = (origin.0 + at.0, origin.2 + at.1);
            // The piece the operation put down is the drawn one standing where it said.
            let piece = placed
                .iter()
                .map(|t| (t, (t.translation().x - wx).powi(2) + (t.translation().z - wz).powi(2)))
                .filter(|(_, d)| *d < 0.25)
                .min_by(|a, b| a.1.total_cmp(&b.1))?
                .0;
            let p = camera.world_to_viewport(cam_tf, piece.translation()).ok()?;
            Some((e.seq, (p.x / vp.x, p.y / vp.y)))
        })
        .collect()
}

/// Ask the checks about `edits` on a thread, with the editor's picture and each operation's close-up.
fn spawn_checks(edits: Vec<AgentEdit>, checks: Checks, pictures: Pictures, tx: Sender<Vec<EditReport>>) {
    std::thread::spawn(move || {
        let http = Http::new(Config::from_env());
        let done = run_due(&edits, &checks, &http, &|e| {
            let mut out: BTreeMap<String, String> = pictures.view.iter().map(|v| ("view".to_string(), v.clone())).collect();
            if let Some(v) = &pictures.view {
                out.insert("after".into(), v.clone());
            }
            if let Some(b) = &pictures.before {
                out.insert("before".into(), b.clone());
            }
            if let Some(c) = pictures.closeups.get(&e.seq) {
                out.insert("closeup".into(), c.clone());
            }
            out
        });
        let _ = tx.send(done);
    });
}

/// **Drain the queue onto a thread that asks the checks**: a model call never runs on the frame. When a
/// check needs a picture, it is taken first (the frame after the operation landed) and the thread
/// starts when it arrives.
#[allow(clippy::too_many_arguments)]
fn start_checks(
    mut commands: Commands,
    window: Query<(), With<bevy::window::PrimaryWindow>>,
    project: Option<Res<crate::project::Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<crate::view::MainCamera>>>,
    placed: Query<&GlobalTransform, With<crate::editor::Placement>>,
    mut queue: ResMut<AfterEditQueue>,
    reports: Res<Reports>,
    shots: Res<Shots>,
    mut wait: ResMut<ShotWait>,
    mut journal: ResMut<crate::chrome::Journal>,
    view: Option<Res<crate::checks_view::ChecksView>>,
) {
    // A headless app has no window and so no picture of the editor to ask about: the owed checks stay
    // queued for whoever drains them, rather than every image check being reported as skipped.
    if queue.pending.is_empty() || window.is_empty() {
        return;
    }
    let Some(project) = project else {
        return;
    };
    let edits = std::mem::take(&mut queue.pending);
    let checks = match crate::checks_view::checks_for(view.as_deref(), &project.root) {
        Ok(c) => c,
        Err(e) => {
            journal.record(&format!("after-edit checks not run: {e}"), 1);
            return;
        }
    };
    let Ok(tx) = reports.tx.lock().map(|t| t.clone()) else {
        return;
    };
    if !wants_picture(&checks) {
        spawn_checks(edits, checks, Pictures::default(), tx);
        return;
    }
    let Ok(shot_tx) = shots.tx.lock().map(|t| t.clone()) else {
        return;
    };
    let frames = match (&open, &camera) {
        (Some(open), Some(cam)) => frames_of(&edits, open.map.origin, cam.0, cam.1, &placed),
        _ => BTreeMap::new(),
    };
    let ticket = wait.hold(edits);
    commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(
        move |shot: On<bevy::render::view::screenshot::ScreenshotCaptured>| {
            let mut pictures = Pictures { view: png_data_url(&shot.image), ..Default::default() };
            if let Ok(img) = shot.image.clone().try_into_dynamic() {
                let size = (img.width(), img.height());
                for (seq, centre) in &frames {
                    if let Some(url) = crop_data_url(&img, closeup_rect(*centre, size)) {
                        pictures.closeups.insert(*seq, url);
                    }
                }
            }
            let _ = shot_tx.send((ticket, Some(pictures)));
        },
    );
}

/// **Start the checks for operations whose picture landed, or whose wait ran out.**
fn launch_shot_checks(
    project: Option<Res<crate::project::Project>>,
    reports: Res<Reports>,
    shots: Res<Shots>,
    mut wait: ResMut<ShotWait>,
    mut memory: ResMut<PictureMemory>,
    view: Option<Res<crate::checks_view::ChecksView>>,
) {
    let mut ready: Vec<(Vec<AgentEdit>, Pictures)> = wait.tick().into_iter().map(|e| (e, Pictures::default())).collect();
    if let Ok(rx) = shots.rx.lock() {
        while let Ok((ticket, pictures)) = rx.try_recv() {
            if let Some(edits) = wait.deliver(ticket) {
                let mut pictures = pictures.unwrap_or_default();
                pictures.before = memory.swap(pictures.view.clone());
                ready.push((edits, pictures));
            }
        }
    }
    let Some(project) = project else {
        return;
    };
    for (edits, pictures) in ready {
        let (Ok(checks), Ok(tx)) = (crate::checks_view::checks_for(view.as_deref(), &project.root), reports.tx.lock().map(|t| t.clone())) else {
            continue;
        };
        spawn_checks(edits, checks, pictures, tx);
    }
}

/// **Findings into the journal**, where Cmd+E shows them. Advisory: nothing is blocked or undone.
fn collect_reports(reports: Res<Reports>, mut journal: ResMut<crate::chrome::Journal>) {
    let Ok(rx) = reports.rx.lock() else {
        return;
    };
    while let Ok(done) = rx.try_recv() {
        for report in done {
            for line in report.journal_lines() {
                journal.record(&line, 1);
            }
        }
    }
}

pub struct AfterEditPlugin;

impl Plugin for AfterEditPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_check_view)
            .init_resource::<AfterEditQueue>()
            .init_resource::<Reports>()
            .init_resource::<Shots>()
            .init_resource::<ShotWait>()
            .init_resource::<PictureMemory>()
            // The checks it asks are the held ones, so the view is looked at first, in every door: the
            // checks belong to the project, not to the open map.
            .init_resource::<crate::checks_view::ChecksView>()
            .init_resource::<crate::oplog_view::OplogView>()
            .init_resource::<crate::data_view::DataView>()
            .init_resource::<crate::reference_view::ReferenceView>()
            .init_resource::<crate::settings::SettingsView>()
            .init_resource::<crate::settings::EditorSettings>()
            .add_systems(
                Update,
                (crate::checks_view::sync_checks_view, crate::oplog_view::sync_oplog_view, crate::data_view::sync_data_view, crate::reference_view::sync_reference_view, crate::settings::sync_settings, queue_after_edit, start_checks, launch_shot_checks, collect_reports).chain(),
            );
    }
}
