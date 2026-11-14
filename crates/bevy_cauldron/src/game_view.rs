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
/// Seconds between checks that the game's log is still under its cap.
pub const LOG_CAP_CHECK_S: f32 = 1.0;
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

/// **What one read of the game over BRP found**, and what it could not: every source that failed says why, so a refused method, a
/// timeout or a platform without `ps` is never drawn as "no data".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// The game answered. `false` comes with [`Self::error`].
    pub answered: bool,
    /// Why the game did not answer the first question (the named entities): the connection's or the game's own sentence.
    pub error: Option<String>,
    pub entities: Vec<EntityRow>,
    pub resources: Vec<String>,
    /// Why the resources could not be listed.
    pub resources_error: Option<String>,
    pub frame_ms: Option<f32>,
    /// Why the frame time could not be read.
    pub frame_error: Option<String>,
    pub rss_mb: Option<f32>,
    /// Why the memory could not be read.
    pub memory_error: Option<String>,
    /// The chosen entity's id and its components, `type path: value`, one line each.
    pub fields: Option<(u64, Vec<String>)>,
    /// Why the chosen entity's components could not be read.
    pub fields_error: Option<String>,
}

/// **What the editor knows of the project's game**, and what the Game workspace draws.
///
/// `Project` ownership: a switch between workspaces keeps it (the game runs on), and it names its project's root so
/// another project's game is never shown.
#[derive(Resource, Default)]
pub struct GameView {
    /// The project this describes.
    pub root: Option<PathBuf>,
    /// **The game's process, while it runs.** It is the pid of what `play.yaml` started — the launcher when the command is
    /// `cargo run` — so the memory read and Show window act on that process and its window, which is the game's only when the
    /// command runs the game itself.
    pub pid: Option<u32>,
    /// How the last run ended.
    pub exit: Option<Exit>,
    /// The log's last [`LOG_LINES`] lines.
    pub log: Vec<String>,
    log_len: u64,
    /// Why the log file could not be read while the game runs.
    pub log_error: Option<String>,
    pub(crate) window: Arc<Mutex<Option<Result<(), String>>>>,
    /// What the last read over BRP found.
    pub snapshot: Snapshot,
    /// The entity whose components are shown.
    pub selected: Option<u64>,
    /// The last build's errors, kept here because a switch of workspace resets the build's own record.
    pub build_errors: Vec<crate::build_view::BuildError>,
    /// The last build's sentence.
    pub build_said: String,
    /// BRP's port, from the project's `play.yaml`.
    pub port: u16,
    pub(crate) arrived: Arc<Mutex<Option<Snapshot>>>,
    pub(crate) in_flight: Arc<AtomicBool>,
    pub(crate) since_log: f32,
    pub(crate) since_state: f32,
    pub(crate) since_cap: f32,
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
            (Some(pid), _) => format!(
                "Running (process {pid}), not answering over BRP on port {}: {}",
                self.port,
                self.snapshot.error.as_deref().unwrap_or("no answer yet")
            ),
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
        if let Some(why) = &self.log_error {
            return format!("Log unavailable: {why}");
        }
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

/// The process's resident memory in megabytes, asked of the operating system, or why that cannot be done here.
fn rss_mb(pid: u32) -> Result<f32, String> {
    #[cfg(unix)]
    {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .map_err(|e| format!("could not run `ps`: {e}"))?;
        if !out.status.success() {
            return Err(format!("`ps` does not know process {pid}"));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let kilobytes: f32 = text.trim().parse().map_err(|_| format!("`ps` answered `{}`, not a number", text.trim()))?;
        Ok(kilobytes / 1024.0)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Err("this platform has no `ps`".to_owned())
    }
}

/// **Read the game once**, on the polling thread: its named entities, its resources, its frame time and — for the
/// chosen entity — its components. **Every source that fails says why** in the snapshot.
fn read_the_game(port: u16, pid: u32, selected: Option<u64>) -> Snapshot {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(BRP_TIMEOUT_MS)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut snapshot = Snapshot::default();
    match rss_mb(pid) {
        Ok(mb) => snapshot.rss_mb = Some(mb),
        Err(why) => snapshot.memory_error = Some(why),
    }
    let rows = match brp(&agent, port, "world.query", json!({"data": {"components": [NAME_COMPONENT]}})) {
        Ok(rows) => rows,
        Err(why) => {
            snapshot.error = Some(why);
            return snapshot;
        }
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
    match brp(&agent, port, "world.list_resources", Value::Null) {
        Ok(names) => {
            snapshot.resources = names.as_array().into_iter().flatten().filter_map(Value::as_str).map(short).take(RESOURCES_SHOWN).collect();
        }
        Err(why) => snapshot.resources_error = Some(why),
    }
    match brp(&agent, port, "world.get_resources", json!({"resource": REAL_TIME_RESOURCE})) {
        Ok(time) => {
            let value = time.get("value").unwrap_or(&time);
            match value.get("delta_secs").and_then(Value::as_f64) {
                Some(secs) => snapshot.frame_ms = Some((secs * 1000.0) as f32),
                None => snapshot.frame_error = Some("the game's clock has no `delta_secs`".to_owned()),
            }
        }
        Err(why) => snapshot.frame_error = Some(why),
    }
    if let Some(id) = selected {
        let read = brp(&agent, port, "world.list_components", json!({"entity": id})).and_then(|listed| {
            let names: Vec<String> = listed.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
            brp(&agent, port, "world.get_components", json!({"entity": id, "components": names}))
        });
        match read {
            Ok(got) => {
                let components = got.get("components").unwrap_or(&got);
                let mut lines: Vec<String> =
                    components.as_object().into_iter().flatten().map(|(path, value)| format!("{}: {}", short(path), bounded(value))).collect();
                lines.sort();
                lines.truncate(FIELD_LINES);
                snapshot.fields = Some((id, lines));
            }
            Err(why) => snapshot.fields_error = Some(why),
        }
    }
    snapshot
}

/// **The tail of the log file**: `Ok(None)` when it has not grown since `seen`, an error when it cannot be read. Reads at most
/// [`LOG_TAIL_BYTES`] from the end and drops a first line the cut may have split.
fn read_log_tail(path: &Path, seen: u64) -> Result<Option<(Vec<String>, u64)>, String> {
    use std::io::{Read, Seek, SeekFrom};
    let cannot = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut file = std::fs::File::open(path).map_err(cannot)?;
    let length = file.metadata().map_err(cannot)?.len();
    if length == seen {
        return Ok(None);
    }
    let start = length.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).map_err(cannot)?;
    let mut bytes = Vec::new();
    file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes).map_err(cannot)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let from = lines.len().saturating_sub(LOG_LINES);
    Ok(Some((lines.split_off(from), length)))
}

// ── the systems ─────────────────────────────────────────────────────────────────────────────────

pub struct GameViewPlugin;

impl Plugin for GameViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameView>()
            // **Watching runs in every workspace**: a crash while a person is on the Map is the one they most need to hear of.
            .add_systems(Update, (watch_the_game, report_the_window).run_if(in_state(Screen::Editor)))
            // **Closing the project removes its game log** when no game runs (not a switch of workspace, which keeps the game and
            // its log), and **quitting the editor removes it** whatever runs: the game writes to the file itself, so it is not
            // hurt by the file going.
            .add_systems(OnExit(Screen::Editor), remove_the_log.before(crate::screen::close_the_door))
            .add_systems(Last, remove_the_log_on_quit.run_if(on_message::<AppExit>))
            .add_systems(
                Update,
                (read_the_log, poll_the_game, draw_the_game, draw_the_field_files)
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
    time: Res<Time>,
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
    // **The log is bounded while the game runs**: checked once a second, the cut is the editor's.
    if pid.is_some() {
        view.since_cap += time.delta_secs();
        if view.since_cap >= LOG_CAP_CHECK_S {
            view.since_cap = 0.0;
            if let Err(why) = ops_play::enforce_log_cap(&root)
                && view.log_error.as_deref() != Some(why.as_str())
            {
                view.log_error = Some(why);
            }
        }
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

/// **Closing the project removes the game's log file** — when no game runs: a game that runs on keeps its log until it ends or the
/// editor quits. A switch of workspace is not a close ([`crate::screen::Crossing`]).
fn remove_the_log(project: Option<Res<Project>>, crossing: Option<Res<crate::screen::Crossing>>) {
    if crossing.is_some() {
        return;
    }
    if let Some(project) = project
        && ops_play::running_pid(&project.root).is_none()
    {
        ops_play::remove_log(&project.root);
    }
}

/// **Quitting removes the log**, so a normal quit leaves nothing in the temporary directory. The game writes to the file itself, so it
/// is not hurt by the file going.
fn remove_the_log_on_quit(project: Option<Res<Project>>) {
    if let Some(project) = project {
        ops_play::remove_log(&project.root);
    }
}

/// **Tail the game's log file** when it has grown, a few times a second. A file that cannot be read while the game runs is said
/// ([`GameView::log_error`]); one that is gone after the game ended (stopped, or the project closed) leaves what was read.
fn read_the_log(time: Res<Time>, project: Option<Res<Project>>, mut view: ResMut<GameView>) {
    view.since_log += time.delta_secs();
    if view.since_log < LOG_POLL_S {
        return;
    }
    view.since_log = 0.0;
    let Some(project) = project else { return };
    let path = ops_play::log_path(&project.root);
    let running = view.pid.is_some();
    let length = match std::fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(e) if !running && e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            let why = format!("{}: {e}", path.display());
            if view.log_error.as_deref() != Some(why.as_str()) {
                view.log_error = Some(why);
            }
            return;
        }
    };
    if view.log_error.is_some() {
        view.log_error = None;
    }
    if length == 0 {
        // `play` has truncated it for a new run: an empty page.
        if running && !view.log.is_empty() {
            view.log.clear();
        }
        view.log_len = 0;
        return;
    }
    // A shorter file than the one seen is a new run's, or a cut past the cap: read it whole.
    let seen = if length < view.log_len { 0 } else { view.log_len };
    match read_log_tail(&path, seen) {
        Ok(Some((lines, length))) => {
            if view.log != lines {
                view.log = lines;
            }
            view.log_len = length;
        }
        Ok(None) => {}
        Err(why) => view.log_error = Some(why),
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
/// **The column of Open file rows under the state view's components** (NV-09, #386; NV-07, #384): one for each component of the chosen entity that
/// an entity file of the project sets.
#[derive(Component)]
pub struct GameFieldFiles;
/// **An Open file button of that column**: the entity file and the line that sets the component (0-based).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct FieldFileButton {
    pub rel: String,
    pub line: usize,
}

/// **The Game workspace's panel**: Play, Stop, Build, the status, the log, the state and the frame line.
fn spawn_game_panel(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    use crate::chrome::{self, GAP_TIGHT_STEPS, Severity, button, section};
    let root = chrome::panel_root(&mut commands, &frame, chrome::Side::Left, chrome::CONTROLS_W, true, false, "GAME").insert(GameRoot).id();
    let dim = chrome::ink().dim;
    let body = chrome::ink().text;
    commands.entity(root).with_children(|p| {
        chrome::title(p, "GAME");
        // FOLLOW-OK: a stack of buttons, lines and fields, not a list the arrows walk: the scroll exists for height.
        chrome::scroll_list(p, GameRootPane).with_children(|p| {
            p.spawn(bevy_gooey::column(GAP_TIGHT_STEPS)).with_children(|s| {
                crate::ops_play::spawn_section(s);
                s.spawn(bevy_gooey::row(GAP_TIGHT_STEPS)).with_children(|r| {
                    button(r, ShowWindowButton, "Show window", Severity::Plain);
                });
                chrome::row_value(s, "", body, GameStatus);
            });
            p.spawn(bevy_gooey::column(GAP_TIGHT_STEPS))
                .with_children(|s| {
                    crate::build_view::spawn_section(s);
                    chrome::row_value(s, "", dim, GameBuildErrors);
                });
            p.spawn(bevy_gooey::column(GAP_TIGHT_STEPS)).with_children(|s| {
                section(s, "LOG");
                chrome::row_value(s, "", body, GameLog);
            });
            p.spawn(bevy_gooey::column(GAP_TIGHT_STEPS)).with_children(|s| {
                section(s, "STATE");
                s.spawn((bevy_gooey::column(GAP_TIGHT_STEPS), GameEntities));
                chrome::row_value(s, "", body, GameFields);
                s.spawn((bevy_gooey::column(GAP_TIGHT_STEPS), GameFieldFiles));
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
#[allow(clippy::type_complexity)]
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
    let answering = view.snapshot.answered;
    let frame_part = match (view.snapshot.frame_ms, &view.snapshot.frame_error) {
        (Some(ms), _) => format!("Frame time {ms:.1} ms"),
        (None, Some(why)) => format!("Frame time unavailable: {why}"),
        (None, None) if view.pid.is_some() && !answering => {
            format!("Frame time unavailable: the game is not answering over BRP ({})", view.snapshot.error.as_deref().unwrap_or("no answer yet"))
        }
        (None, None) => "Frame time appears while the game runs".to_owned(),
    };
    let memory_part = match (view.snapshot.rss_mb, &view.snapshot.memory_error) {
        (Some(mb), _) => format!("memory {mb:.0} MB"),
        (None, Some(why)) => format!("memory unavailable: {why}"),
        (None, None) => "memory appears while the game runs".to_owned(),
    };
    put(&mut targets.frame, format!("{frame_part} · {memory_part}"));
    let fields = match &view.snapshot.fields {
        Some((id, lines)) if view.selected == Some(*id) => {
            let name = view.snapshot.entities.iter().find(|e| e.id == *id).map_or("entity", |e| e.name.as_str());
            format!("{name}\n{}", lines.join("\n"))
        }
        _ if view.selected.is_some() && view.snapshot.fields_error.is_some() => {
            format!("The entity's components could not be read: {}", view.snapshot.fields_error.as_deref().unwrap_or_default())
        }
        _ if view.selected.is_some() => "Reading the entity…".to_owned(),
        _ if view.snapshot.entities.is_empty() && view.pid.is_some() => match &view.snapshot.error {
            Some(why) => format!("The game's state could not be read: {why}"),
            None => "The game has no named entities".to_owned(),
        },
        _ if view.pid.is_some() => "Choose an entity to read its components. The view is read-only: the game changes through its files".to_owned(),
        _ => "The game's entities and resources appear while it runs".to_owned(),
    };
    put(&mut targets.fields, fields);
    let resources = match (&view.snapshot.resources_error, view.snapshot.resources.is_empty()) {
        (Some(why), _) => format!("Resources unavailable: {why}"),
        (None, true) => String::new(),
        (None, false) => format!("Resources: {}", view.snapshot.resources.join(", ")),
    };
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

/// **How the state view separates a component's name from its value**: `module::Type: value`, the engine's own wording.
pub const STATE_VIEW_VALUE_SEP: &str = ": ";

/// **How the engine separates a type's module from its own name.**
pub const MODULE_SEP: &str = "::";

/// **How often the entity files are read again for the chosen entity**, in seconds: they are read for the line each component stands on, and a file
/// edited under the panel moves a line.
pub const FIELD_FILES_POLL_S: f32 = 1.5;

/// **Draw an Open file row for each component the project's entity files set**, for the chosen entity. The state view stays read-only (the file is
/// the truth, P3): a row does not change the game, it opens the YAML that does, at the line.
/// **What the Open file column last drew**: the entity and its fields it was drawn for, the rows it drew (component, file, line), and the
/// seconds since it last looked. A named type, so the column's own memory reads as one thing.
type Drawn = (Option<(u64, Vec<String>)>, Vec<(String, String, usize)>, f32);

fn draw_the_field_files(
    view: Res<GameView>,
    project: Option<Res<Project>>,
    time: Res<Time>,
    list: Query<(Entity, Option<&Children>), With<GameFieldFiles>>,
    mut commands: Commands,
    mut shown: Local<Drawn>,
) {
    let Ok((container, children)) = list.single() else { return };
    let key = view.snapshot.fields.clone().filter(|(id, _)| view.selected == Some(*id));
    shown.2 += time.delta_secs();
    let unfilled = !shown.1.is_empty() && children.is_none_or(|c| c.is_empty());
    if shown.0 == key && shown.2 < FIELD_FILES_POLL_S && !unfilled {
        return;
    }
    shown.2 = 0.0;
    let want: Vec<(String, String, usize)> = match (&key, project.as_deref()) {
        (Some((id, lines)), Some(project)) => view.snapshot.entities.iter().find(|e| e.id == *id).map_or_else(Vec::new, |entity| {
            lines
                .iter()
                .filter_map(|line| {
                    // The state view writes a component's name as the engine words it; the entity file's key is the type's own
                    // name, so both of the view's own separators are named here rather than spelled out.
                    let component = line.split(STATE_VIEW_VALUE_SEP).next()?.rsplit(MODULE_SEP).next()?;
                    crate::crosslinks::component_line(&project.root, &entity.name, component).map(|(rel, at)| (component.to_owned(), rel, at))
                })
                .collect()
        }),
        _ => Vec::new(),
    };
    shown.0 = key;
    if shown.1 == want && !unfilled {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands.entity(container).with_children(|c| {
        for (component, rel, line) in &want {
            crate::chrome::gapped_row(c, crate::chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::Center)
                .with_children(|r| {
                    r.spawn((Text::new(component.clone()), TextColor(crate::chrome::ink().dim), crate::chrome::typed(crate::chrome::text::BODY)));
                    crate::chrome::button(r, FieldFileButton { rel: rel.clone(), line: *line }, "Open file", crate::chrome::Severity::Plain);
                });
        }
    });
    shown.1 = want;
}

/// **A click in the Game panel**: choose an entity to read, or bring the game's window to the front.
fn on_game_click(
    activate: On<Activate>,
    fields: Query<&FieldFileButton>,
    entities: Query<&EntityButton>,
    windows: Query<(), With<ShowWindowButton>>,
    view: Option<ResMut<GameView>>,
    state: Option<ResMut<crate::editor::EditorState>>,
    mut commands: Commands,
) {
    // **Open file on a component** goes to the entity file at the line that sets it.
    if let Ok(button) = fields.get(activate.entity) {
        let (rel, line) = (button.rel.clone(), button.line);
        commands.queue(move |world: &mut World| crate::crosslinks::open_in_files(world, &rel, Some(line)));
        return;
    }
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
                // **Off the UI thread**: `osascript` may sit on a permission prompt and `xdotool` may hang. The answer is taken by
                // [`report_the_window`] on a later frame.
                let answer = Arc::clone(&view.window);
                std::thread::spawn(move || {
                    let result = bring_to_front(pid);
                    *answer.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(result);
                });
            }
            None => state.status.problem("the game is not running, so there is no window to show. Press Play first.".to_owned()),
        }
    }
}

/// **Say how Show window went**, on the status line: a refusal with its reason, or a note that the window was asked to the front.
fn report_the_window(view: Res<GameView>, state: Option<ResMut<crate::editor::EditorState>>) {
    let answer = view.window.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    let (Some(answer), Some(mut state)) = (answer, state) else { return };
    match answer {
        Ok(()) => state.status.note("asked the game's window to come to the front".to_owned()),
        Err(why) => state.status.problem(why),
    }
}

/// **Bring the window of process `pid` to the front**, by the platform's own tool. A platform or a missing tool is
/// a sentence that says what to install, never a silent no-op. Blocking: call it on a thread.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// **Show window's answer reaches the status line**: the thread's error, with the tool's own words, is a problem; a success is a note.
    #[test]
    fn the_window_threads_answer_is_said_on_the_status_line() {
        let mut app = App::new();
        app.init_resource::<GameView>().init_resource::<crate::editor::EditorState>().add_systems(Update, report_the_window);
        *app.world().resource::<GameView>().window.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(Err("could not run `xdotool`: not found".to_owned()));
        app.update();
        let problems: Vec<String> = app.world().resource::<crate::editor::EditorState>().status.problems().iter().map(|p| p.text.clone()).collect();
        assert!(problems.iter().any(|p| p.contains("xdotool")), "the thread's error is on the status line: {problems:?}");
        *app.world().resource::<GameView>().window.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Ok(()));
        app.update();
        let note = app.world().resource::<crate::editor::EditorState>().status.note_text().to_owned();
        assert!(note.contains("window"), "a success is said: {note:?}");
    }
}
