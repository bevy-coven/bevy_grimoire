//! **The Game workspace: the home for the running game** (NV-09, #386; series getting around, #377).
//!
//! The game keeps its own window (`play`); this is where a person watches and drives it from the editor. It holds
//!
//! - **Play** and **Stop**, which are the existing `play` and `play_stop` commands moved here — **there is no
//!   second launcher** — and **Build** with the build job's progress and its errors (GD-09);
//! - **the game's log**, from the file `play` writes the game's output to ([`crate::ops_play::log_path`]);
//! - **a live, read-only view of the game's state**, read over BRP: the named entities, and with one chosen its
//!   components as the game's own reflection serialises them, and the resources it holds;
//! - **frame time and memory**, the frame time from the game's own `Time<Real>` over BRP and the memory from the
//!   operating system (`ps`);
//! - **Show window**, which brings the game's window to the front.
//!
//! # The state view is read-only
//!
//! A game is changed through its YAML, so the file stays the truth (P3): nothing here writes a component. Each
//! field would carry **Open file** to the file that sets it (NV-07), and that needs the Files workspace (GD-11),
//! which is not built; the reading is here and the link is recorded as owed on the issue.
//!
//! # A crash stays on screen
//!
//! A game that ends without being asked to and without success ([`crate::ops_play::Exit::crashed`]) keeps its last log
//! lines and its exit code on screen, and **the Game icon on the activity bar is badged** until a person plays again or
//! stops it. Watching for that runs in every workspace (a person on the Map must still be told); drawing runs only in
//! this one.
//!
//! # Nothing blocks a frame
//!
//! BRP is read on a thread that answers through a mutex, one poll in flight at a time; the log is a file tail read when
//! its length changes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_gooey::Activate;
use serde_json::{Value, json};

use crate::ops_play::{self, Exit};
use crate::project::Project;
use crate::screen::Screen;
use crate::workspace::Workspace;

/// How many of the game's last log lines are kept and drawn.
pub const LOG_LINES: usize = 40;
/// How many of the last lines a crash keeps on the status line's account, beside its exit code.
pub const CRASH_LOG_LINES: usize = 8;
/// Seconds between looks at the log file's length.
pub const LOG_POLL_S: f32 = 0.25;
/// Seconds between reads of the game's state over BRP.
pub const STATE_POLL_S: f32 = 1.0;
/// Milliseconds a BRP read waits for the game before it is called unanswered.
pub const BRP_TIMEOUT_MS: u64 = 500;
/// The most named entities listed.
pub const ENTITY_ROWS: usize = 30;
/// The most resource names listed.
pub const RESOURCES_SHOWN: usize = 12;
/// The most lines of one entity's components shown.
pub const FIELD_LINES: usize = 16;
/// The longest one component's value is shown, in characters, before it is cut with an ellipsis.
pub const VALUE_CHARS: usize = 120;
/// The most bytes of the log file read at once: a game that prints for an hour must not cost the editor its memory.
pub const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// The full type path of the component whose value is an entity's name.
pub const NAME_COMPONENT: &str = "bevy_ecs::name::Name";
/// The full type path of the game's real-time clock, whose last delta is its frame time.
pub const REAL_TIME_RESOURCE: &str = "bevy_time::time::Time<bevy_time::real::Real>";

/// **One named entity of the game**, as the state view lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityRow {
    /// BRP's id for the entity.
    pub id: u64,
    /// Its `Name`.
    pub name: String,
}

/// **What one read of the game over BRP found.**
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// The game answered. `false` is not an error to hide: the panel says the game is not answering.
    pub answered: bool,
    pub entities: Vec<EntityRow>,
    pub resources: Vec<String>,
    pub frame_ms: Option<f32>,
    pub rss_mb: Option<f32>,
    /// The chosen entity's id and its components, `type path: value`, one line each.
    pub fields: Option<(u64, Vec<String>)>,
}

/// **What the editor knows of the project's game**, and what the Game workspace draws.
///
/// `Project` ownership: a switch between workspaces keeps it (the game runs on), and it names its project's root so
/// another project's game is never shown.
#[derive(Resource, Default)]
pub struct GameView {
    /// The project this describes.
    pub root: Option<PathBuf>,
    /// The game's process, while it runs.
    pub pid: Option<u32>,
    /// How the last run ended.
    pub exit: Option<Exit>,
    /// The log's last [`LOG_LINES`] lines.
    pub log: Vec<String>,
    log_len: u64,
    /// What the last read over BRP found.
    pub snapshot: Snapshot,
    /// The entity whose components are shown.
    pub selected: Option<u64>,
    /// The last build's errors, kept here because a switch of workspace resets the build's own record.
    pub build_errors: Vec<crate::build_view::BuildError>,
    /// The last build's sentence.
    pub build_said: String,
    /// BRP's port, from the project's `play.ron`.
    pub port: u16,
    pub(crate) arrived: Arc<Mutex<Option<Snapshot>>>,
    pub(crate) in_flight: Arc<AtomicBool>,
    pub(crate) since_log: f32,
    pub(crate) since_state: f32,
}

impl GameView {
    /// **Is the game's end a crash?** What the Game icon's badge and the status line both ask.
    pub fn crashed(&self) -> bool {
        self.pid.is_none() && self.exit.is_some_and(|e| e.crashed())
    }

    /// **The sentence that says where the game stands.**
    pub fn status(&self) -> String {
        match (self.pid, self.exit) {
            (Some(pid), _) if self.snapshot.answered => format!("Running (process {pid}), answering on port {}", self.port),
            (Some(pid), _) => format!("Running (process {pid}), not answering over BRP on port {} yet", self.port),
            (None, Some(exit)) if exit.crashed() => match exit.code {
                Some(code) => format!("Crashed: the game ended with exit code {code}"),
                None => "Crashed: the game was ended by a signal".to_owned(),
            },
            (None, Some(exit)) if exit.stopped => "Stopped".to_owned(),
            (None, Some(_)) => "The game closed itself (exit code 0)".to_owned(),
            (None, None) => "Not running. Press Play to start the game on the open map".to_owned(),
        }
    }

    /// The log, with what a crash leaves on screen: its last lines are the log's, so this is the log's tail.
    pub fn log_text(&self) -> String {
        if self.log.is_empty() {
            return "No output yet.".to_owned();
        }
        self.log.join("\n")
    }

    /// The last [`CRASH_LOG_LINES`] lines, which a crash keeps in the sentence beside its code.
    pub fn crash_lines(&self) -> Vec<&str> {
        let start = self.log.len().saturating_sub(CRASH_LOG_LINES);
        self.log[start..].iter().map(String::as_str).collect()
    }
}

// ── the reading ─────────────────────────────────────────────────────────────────────────────────

/// **One BRP call**, blocking, on the polling thread. The error is the sentence the panel could show.
fn brp(agent: &ureq::Agent, port: u16, method: &str, params: Value) -> Result<Value, String> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let mut response = agent.post(format!("http://127.0.0.1:{port}")).send_json(&body).map_err(|e| e.to_string())?;
    let value: Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
    if let Some(error) = value.get("error") {
        return Err(error.get("message").and_then(Value::as_str).unwrap_or("the game refused the request").to_owned());
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

/// The last two segments of a type path: `bevy_transform::components::transform::Transform` → `transform::Transform`.
fn short(path: &str) -> String {
    let mut parts: Vec<&str> = path.rsplit("::").take(2).collect();
    parts.reverse();
    parts.join("::")
}

/// A component's value as one bounded line.
fn bounded(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() <= VALUE_CHARS {
        return text;
    }
    let cut: String = text.chars().take(VALUE_CHARS).collect();
    format!("{cut}…")
}

/// **The name in a `Name` component's JSON**: a string, or an object that carries one.
fn name_of(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned).or_else(|| value.get("name").and_then(Value::as_str).map(str::to_owned))
}

/// The process's resident memory in megabytes, asked of the operating system.
fn rss_mb(pid: u32) -> Option<f32> {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().ok()?;
    let kilobytes: f32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kilobytes / 1024.0)
}

/// **Read the game once**, on the polling thread: its named entities, its resources, its frame time and — for the
/// chosen entity — its components.
fn read_the_game(port: u16, pid: u32, selected: Option<u64>) -> Snapshot {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(BRP_TIMEOUT_MS)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut snapshot = Snapshot { rss_mb: rss_mb(pid), ..Snapshot::default() };
    let Ok(rows) = brp(&agent, port, "world.query", json!({"data": {"components": [NAME_COMPONENT]}})) else {
        return snapshot;
    };
    snapshot.answered = true;
    for row in rows.as_array().into_iter().flatten().take(ENTITY_ROWS) {
        let (Some(id), Some(name)) =
            (row.get("entity").and_then(Value::as_u64), row.pointer(&format!("/components/{NAME_COMPONENT}")).and_then(name_of))
        else {
            continue;
        };
        snapshot.entities.push(EntityRow { id, name });
    }
    if let Ok(names) = brp(&agent, port, "world.list_resources", Value::Null) {
        snapshot.resources = names.as_array().into_iter().flatten().filter_map(Value::as_str).map(short).take(RESOURCES_SHOWN).collect();
    }
    if let Ok(time) = brp(&agent, port, "world.get_resources", json!({"resource": REAL_TIME_RESOURCE})) {
        let value = time.get("value").unwrap_or(&time);
        snapshot.frame_ms = value.get("delta_secs").and_then(Value::as_f64).map(|s| (s * 1000.0) as f32);
    }
    if let Some(id) = selected
        && let Ok(listed) = brp(&agent, port, "world.list_components", json!({"entity": id}))
    {
        let names: Vec<String> = listed.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
        if let Ok(got) = brp(&agent, port, "world.get_components", json!({"entity": id, "components": names})) {
            let components = got.get("components").unwrap_or(&got);
            let mut lines: Vec<String> = components
                .as_object()
                .into_iter()
                .flatten()
                .map(|(path, value)| format!("{}: {}", short(path), bounded(value)))
                .collect();
            lines.sort();
            lines.truncate(FIELD_LINES);
            snapshot.fields = Some((id, lines));
        }
    }
    snapshot
}

/// **The tail of the log file**, or `None` when it has not grown since `seen`. Reads at most [`LOG_TAIL_BYTES`] from the
/// end and drops a first line the cut may have split.
fn read_log_tail(path: &Path, seen: u64) -> Option<(Vec<String>, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length == seen {
        return None;
    }
    let start = length.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let from = lines.len().saturating_sub(LOG_LINES);
    Some((lines.split_off(from), length))
}

// ── the systems ─────────────────────────────────────────────────────────────────────────────────

pub struct GameViewPlugin;

impl Plugin for GameViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameView>()
            // **Watching runs in every workspace**: a crash while a person is on the Map is the one they most need to hear of.
            .add_systems(Update, watch_the_game.run_if(in_state(Screen::Editor)))
            .add_systems(
                Update,
                (read_the_log, poll_the_game, draw_the_game)
                    .chain()
                    .after(watch_the_game)
                    .run_if(in_state(Screen::Editor))
                    .run_if(Workspace::game_is_open),
            )
            .add_systems(OnEnter(Screen::Editor), spawn_game_panel.run_if(Workspace::game_is_open).after(crate::chrome::FrameSystems))
            .add_observer(on_game_click);
    }
}

/// **Follow the game's process and the last build**, every frame and in every workspace: one lookup in a table when no
/// game was ever started, which is the common case.
pub fn watch_the_game(
    project: Option<Res<Project>>,
    build: Option<Res<crate::build_view::BuildState>>,
    mut view: ResMut<GameView>,
) {
    let Some(project) = project else { return };
    let root = project.root.clone();
    if view.root.as_ref() != Some(&root) {
        // Another project's game is not this one's: start again.
        *view = GameView { root: Some(root.clone()), ..GameView::default() };
        view.port = bevy_grimoire_core::play::PlayConfig::load(&root).map_or(bevy_grimoire_core::play::DEFAULT_PORT, |c| c.port);
    }
    let pid = ops_play::running_pid(&root);
    let exit = ops_play::last_exit(&root);
    if view.pid != pid {
        // **A game that has just started is read at once**, not a second from now.
        if pid.is_some() {
            view.since_state = STATE_POLL_S;
            view.since_log = LOG_POLL_S;
        }
        view.pid = pid;
    }
    if view.exit != exit {
        view.exit = exit;
    }
    if let Some(build) = build {
        if build.running.is_some() {
            if !view.build_errors.is_empty() || !view.build_said.is_empty() {
                view.build_errors.clear();
                view.build_said.clear();
            }
        } else {
            if !build.errors.is_empty() && view.build_errors != build.errors {
                view.build_errors.clone_from(&build.errors);
            }
            if !build.said.is_empty() && view.build_said != build.said {
                view.build_said.clone_from(&build.said);
            }
        }
    }
}

/// **Tail the game's log file** when it has grown, a few times a second.
fn read_the_log(time: Res<Time>, project: Option<Res<Project>>, mut view: ResMut<GameView>) {
    view.since_log += time.delta_secs();
    if view.since_log < LOG_POLL_S {
        return;
    }
    view.since_log = 0.0;
    let Some(project) = project else { return };
    let path = ops_play::log_path(&project.root);
    let length = std::fs::metadata(&path).map_or(0, |m| m.len());
    if length == 0 {
        // No file, or `play` has truncated it for a new run: an empty page.
        if !view.log.is_empty() {
            view.log.clear();
        }
        view.log_len = 0;
        return;
    }
    // A shorter file than the one seen is a new run's: read it whole.
    let seen = if length < view.log_len { 0 } else { view.log_len };
    if let Some((lines, length)) = read_log_tail(&path, seen) {
        if view.log != lines {
            view.log = lines;
        }
        view.log_len = length;
    }
}

/// **Read the game over BRP** once a second, on a thread, while it runs. The answer is taken on a later frame.
fn poll_the_game(time: Res<Time>, mut view: ResMut<GameView>) {
    let arrived = view.arrived.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    if let Some(snapshot) = arrived
        && view.snapshot != snapshot
    {
        view.snapshot = snapshot;
    }
    let Some(pid) = view.pid else {
        if view.snapshot != Snapshot::default() {
            view.snapshot = Snapshot::default();
        }
        return;
    };
    view.since_state += time.delta_secs();
    if view.since_state < STATE_POLL_S {
        return;
    }
    if view.in_flight.swap(true, Ordering::AcqRel) {
        return;
    }
    view.since_state = 0.0;
    let (port, selected) = (view.port, view.selected);
    let (arrived, in_flight) = (Arc::clone(&view.arrived), Arc::clone(&view.in_flight));
    std::thread::spawn(move || {
        let snapshot = read_the_game(port, pid, selected);
        *arrived.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(snapshot);
        in_flight.store(false, Ordering::Release);
    });
}

// ── what is drawn ───────────────────────────────────────────────────────────────────────────────

/// The Game panel's root.
#[derive(Component)]
pub struct GameRoot;

/// The sentence that says where the game stands.
#[derive(Component)]
pub struct GameStatus;
/// The game's log.
#[derive(Component)]
pub struct GameLog;
/// The frame time and memory line.
#[derive(Component)]
pub struct GameFrame;
/// The column of entity rows.
#[derive(Component)]
pub struct GameEntities;
/// The chosen entity's components.
#[derive(Component)]
pub struct GameFields;
/// The resources line.
#[derive(Component)]
pub struct GameResources;
/// The last build's errors.
#[derive(Component)]
pub struct GameBuildErrors;
/// Brings the game's window to the front.
#[derive(Component)]
pub struct ShowWindowButton;
/// One row of the state view: choosing it shows the entity's components.
#[derive(Component, Clone, Copy)]
pub struct EntityButton(pub u64);

/// **The Game workspace's panel**: Play, Stop, Build, the status, the log, the state and the frame line.
fn spawn_game_panel(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    use crate::chrome::{self, GAP_TIGHT, Severity, button, section};
    let root = chrome::panel_root(&mut commands, &frame, chrome::Side::Left, chrome::CONTROLS_W, true, false, "GAME").insert(GameRoot).id();
    let dim = chrome::ink().dim;
    let body = chrome::ink().text;
    commands.entity(root).with_children(|p| {
        chrome::title(p, "GAME");
        // FOLLOW-OK: a stack of buttons, lines and fields, not a list the arrows walk: the scroll exists for height.
        chrome::scroll_list(p, GameRootPane).with_children(|p| {
            p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(GAP_TIGHT), ..default() }).with_children(|s| {
                crate::ops_play::spawn_section(s);
                s.spawn(Node { column_gap: Val::Px(GAP_TIGHT), ..default() }).with_children(|r| {
                    button(r, ShowWindowButton, "Show window", Severity::Plain);
                });
                chrome::row_value(s, "", body, GameStatus);
            });
            p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(GAP_TIGHT), ..default() })
                .with_children(|s| {
                    crate::build_view::spawn_section(s);
                    chrome::row_value(s, "", dim, GameBuildErrors);
                });
            p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(GAP_TIGHT), ..default() }).with_children(|s| {
                section(s, "LOG");
                chrome::row_value(s, "", body, GameLog);
            });
            p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(GAP_TIGHT), ..default() }).with_children(|s| {
                section(s, "STATE");
                s.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(GAP_TIGHT), ..default() }, GameEntities));
                chrome::row_value(s, "", body, GameFields);
                chrome::row_value(s, "", dim, GameResources);
                chrome::row_value(s, "", dim, GameFrame);
            });
        });
    });
}

/// The panel's scroll pane.
#[derive(Component)]
pub struct GameRootPane;

/// What [`draw_the_game`] writes into, bundled for the system-parameter limit.
#[derive(SystemParam)]
struct Targets<'w, 's> {
    status: Query<'w, 's, &'static mut Text, With<GameStatus>>,
    log: Query<'w, 's, &'static mut Text, (With<GameLog>, Without<GameStatus>)>,
    frame: Query<'w, 's, &'static mut Text, (With<GameFrame>, Without<GameStatus>, Without<GameLog>)>,
    fields: Query<'w, 's, &'static mut Text, (With<GameFields>, Without<GameStatus>, Without<GameLog>, Without<GameFrame>)>,
    resources: Query<'w, 's, &'static mut Text, (With<GameResources>, Without<GameStatus>, Without<GameLog>, Without<GameFrame>, Without<GameFields>)>,
    errors: Query<
        'w,
        's,
        &'static mut Text,
        (With<GameBuildErrors>, Without<GameStatus>, Without<GameLog>, Without<GameFrame>, Without<GameFields>, Without<GameResources>),
    >,
}

/// Write `want` into the first text of `query` when it differs.
fn put<F: bevy::ecs::query::QueryFilter>(query: &mut Query<&mut Text, F>, want: String) {
    for mut text in query.iter_mut() {
        if text.0 != want {
            text.0.clone_from(&want);
        }
    }
}

/// **Draw the game into the panel**: the status, the log, the build's errors, the frame line, the entity rows and the
/// chosen entity's components. Each text is written only when it changed.
fn draw_the_game(
    view: Res<GameView>,
    mut targets: Targets,
    list: Query<(Entity, Option<&Children>), With<GameEntities>>,
    rows: Query<&EntityButton>,
    mut commands: Commands,
) {
    let mut status = view.status();
    if view.crashed() {
        let lines = view.crash_lines();
        if !lines.is_empty() {
            status.push_str("\nLast output:\n");
            status.push_str(&lines.join("\n"));
        }
    }
    put(&mut targets.status, status);
    put(&mut targets.log, view.log_text());
    let frame = match (view.snapshot.frame_ms, view.snapshot.rss_mb) {
        (Some(ms), Some(mb)) => format!("Frame time {ms:.1} ms · memory {mb:.0} MB"),
        (Some(ms), None) => format!("Frame time {ms:.1} ms"),
        (None, Some(mb)) => format!("Memory {mb:.0} MB"),
        (None, None) => "Frame time and memory appear while the game runs".to_owned(),
    };
    put(&mut targets.frame, frame);
    let fields = match &view.snapshot.fields {
        Some((id, lines)) if view.selected == Some(*id) => {
            let name = view.snapshot.entities.iter().find(|e| e.id == *id).map_or("entity", |e| e.name.as_str());
            format!("{name}\n{}", lines.join("\n"))
        }
        _ if view.selected.is_some() => "Reading the entity…".to_owned(),
        _ if view.snapshot.entities.is_empty() && view.pid.is_some() => "The game has no named entities, or is not answering yet".to_owned(),
        _ if view.pid.is_some() => "Choose an entity to read its components. The view is read-only: the game changes through its files".to_owned(),
        _ => "The game's entities and resources appear while it runs".to_owned(),
    };
    put(&mut targets.fields, fields);
    let resources = if view.snapshot.resources.is_empty() { String::new() } else { format!("Resources: {}", view.snapshot.resources.join(", ")) };
    put(&mut targets.resources, resources);
    let errors = if view.build_errors.is_empty() {
        view.build_said.clone()
    } else {
        view.build_errors.iter().take(crate::build_view::ERRORS_SHOWN).map(|e| format!("{}  {}", e.link(), e.message)).collect::<Vec<_>>().join("\n")
    };
    put(&mut targets.errors, errors);

    // **The entity rows are rebuilt only when the list changed.**
    let Ok((container, children)) = list.single() else { return };
    let drawn: Vec<u64> = children.into_iter().flatten().filter_map(|c| rows.get(*c).ok()).map(|b| b.0).collect();
    let want: Vec<u64> = view.snapshot.entities.iter().map(|e| e.id).collect();
    if drawn == want {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands.entity(container).with_children(|c| {
        for entity in &view.snapshot.entities {
            crate::chrome::button(c, EntityButton(entity.id), &entity.name, crate::chrome::Severity::Plain);
        }
    });
}

/// **A click in the Game panel**: choose an entity to read, or bring the game's window to the front.
fn on_game_click(
    activate: On<Activate>,
    entities: Query<&EntityButton>,
    windows: Query<(), With<ShowWindowButton>>,
    view: Option<ResMut<GameView>>,
    state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(mut view) = view else { return };
    if let Ok(button) = entities.get(activate.entity) {
        if view.selected != Some(button.0) {
            view.selected = Some(button.0);
            // Read it now rather than at the next second.
            view.since_state = STATE_POLL_S;
        }
        return;
    }
    if windows.contains(activate.entity) {
        let Some(mut state) = state else { return };
        match view.pid {
            Some(pid) => {
                if let Err(why) = bring_to_front(pid) {
                    state.status.problem(why);
                }
            }
            None => state.status.problem("the game is not running, so there is no window to show. Press Play first.".to_owned()),
        }
    }
}

/// **Bring the window of process `pid` to the front**, by the platform's own tool. A platform or a missing tool is
/// a sentence that says what to install, never a silent no-op.
pub fn bring_to_front(pid: u32) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let (program, args): (&str, Vec<String>) = (
        "osascript",
        vec![
            "-e".to_owned(),
            format!("tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"),
        ],
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, args): (&str, Vec<String>) = ("xdotool", vec!["search".to_owned(), "--pid".to_owned(), pid.to_string(), "windowactivate".to_owned()]);
    #[cfg(not(unix))]
    let (program, args): (&str, Vec<String>) = ("", Vec::new());
    if program.is_empty() {
        return Err("showing the game's window is not supported on this platform".to_owned());
    }
    match std::process::Command::new(program).args(&args).output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!("`{program}` could not show the game's window: {}", String::from_utf8_lossy(&out.stderr).trim())),
        Err(e) => Err(format!("could not run `{program}` to show the game's window: {e}. Install it, or click the game's own window")),
    }
}
