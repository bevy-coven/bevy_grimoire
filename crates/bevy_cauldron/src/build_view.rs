//! **When Rust must change: build, watch it, and relaunch where you were** (GD-09, #371).
//!
//! A project's game is data, and most of what a person or an agent changes reaches it without a
//! compiler. A **new component type or a new system** does not, so the rebuild is made a step the
//! person can watch rather than a wait they endure outside the editor:
//!
//! - **`game_build`** runs the build command `play.ron` names (`build`), behind the same trust gate as
//!   `play`: a file that names a program names this one too, and a person allows it the once.
//! - **The build is a background job** ([`crate::jobs`], UI-22). **Cargo is run, never
//!   reimplemented**: the command is the file's own, with `--message-format=json-diagnostic-rendered-ansi`
//!   added when the file does not already say how it wants its messages, and every
//!   `compiler-artifact` message is one crate finished.
//! - **Every compiler error becomes a problem with its file, its line and its message**, and a `file:
//!   line` to go to, so a person reads the compiler's own sentence rather than a wall of cargo output.
//! - **The running game keeps running until the new build succeeds**, and only then is it replaced:
//!   stopped and started on the same map at the same camera through the two operations a person's Play
//!   and Stop press. A failed build leaves the old game up, beside the errors.
//!
//! # What is deliberately not here
//!
//! **Hotpatching is out of scope** (owner, 2026-11-09): scripts and configuration reload live, and
//! everything else is compiled. There is no second path where a half-built binary is spliced into a
//! running game.
//!
//! # The count is what cargo reported, and no more
//!
//! Cargo's messages say which crates it has **finished** and never how many there are, so the job
//! counts crates and draws the band's spinner: a `3 of 12` would be a denominator this editor made up,
//! and a bar that fills to a number nobody measured is worse than no bar.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::prelude::*;
use bevy_gooey::Activate;
use bevy_grimoire_core::play::PlayConfig;
use serde_json::{Value, json};

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};
use crate::project::Project;
use crate::screen::Screen;

/// **The producer's key in [`crate::jobs::Jobs`]**, so the band, the popover and a stop request all
/// name the same job.
pub const BUILD_JOB: &str = "game_build";

/// **The flag cargo is given for machine-readable messages**, when `play.ron`'s own build command does
/// not name one. A command that names its own `--message-format` is run as written.
///
/// # Why not the issue's `json-render-diagnostics`
///
/// **Assumed decision** (#371's review, must-fix 1): the issue names
/// `--message-format=json-render-diagnostics`, and that mode does not do what its own acceptance
/// criteria need — measured on cargo 1.99.1 against a file with one type error, its stdout carries
/// `compiler-artifact` and `build-finished` and **no `compiler-message` at all**, while the
/// diagnostics go to stderr as rendered text (`error[E0308]: mismatched types`). An editor reading that
/// stream would show a progress bar and never a compiler error. The two modes that do put the
/// diagnostics on stdout as `compiler-message` JSON are `json` and `json-diagnostic-rendered-ansi`;
/// this build asks for the second, because it also carries the `rendered` text a person reads whole.
pub const CARGO_JSON_FLAG: &str = "--message-format=json-diagnostic-rendered-ansi";

/// **What a build has done so far**, as the band says it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// The crates cargo says are finished. Its messages name no total, so this is the whole count.
    pub compiled: u32,
}

impl Progress {
    /// The words after the job's title: `3 crates`, or nothing before the first one.
    pub fn words(&self) -> String {
        match self.compiled {
            0 => String::new(),
            1 => "1 crate".to_owned(),
            n => format!("{n} crates"),
        }
    }
}

/// **How many compiler errors become problems**, which is the status log's own cap
/// ([`crate::chrome::MAX_PROBLEMS`]): a build with two hundred errors is one broken file, and a log
/// that held all of them would bury every other thing the editor had to say. The count is still the
/// build's own, so the sentence says how many there were.
pub const ERRORS_SHOWN: usize = crate::chrome::MAX_PROBLEMS;

/// **One compiler error**, as a person reads it and as the problem carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildError {
    /// The file the compiler named, as the compiler named it (`src/people.rs`).
    pub file: String,
    /// The line, 1-based.
    pub line: u32,
    /// The compiler's own sentence (`cannot find type Wander in this scope`).
    pub message: String,
}

impl BuildError {
    /// **The problem a person is shown**: the compiler's sentence, where, and the place to go.
    pub fn problem(&self) -> bevy_grimoire_core::problem::Problem {
        use bevy_grimoire_core::problem::{Detail, Location, Problem};
        Problem::with(
            self.message.clone(),
            Detail::default()
                .at(Location::file(self.file.clone()))
                .because(if self.line == 0 { "no line named".to_owned() } else { format!("line {}", self.line) })
                .fix(format!("open {}", self.link())),
        )
    }

    /// **The place a person goes to**: `file:line`, which a terminal and every editor turn into a jump.
    pub fn link(&self) -> String {
        format!("{}:{}", self.file, self.line)
    }
}

/// **What one line of the build says**, as the editor acts on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildEvent {
    /// One crate is compiled.
    Compiled(String),
    /// One compiler error, with the place it is about.
    Error(BuildError),
    /// The build is over, and whether it succeeded.
    Finished { success: bool },
}

/// **One line of cargo's own output**, read into what a person is shown.
///
/// `None` for the lines that are not about this build: a warning (not a reason a build failed), a
/// `progress` message, or anything that is not JSON at all. **Read from the message's own fields**, so
/// a file name with a colon in it, a path with spaces and a message over several lines are all the
/// compiler's own values rather than a regexp's guess.
pub fn read_line(line: &str) -> Option<BuildEvent> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("reason")?.as_str()? {
        "compiler-artifact" => {
            let name = value.get("target").and_then(|t| t.get("name")).and_then(Value::as_str)?;
            Some(BuildEvent::Compiled(name.to_owned()))
        }
        "compiler-message" => {
            let message = value.get("message")?;
            if message.get("level").and_then(Value::as_str) != Some("error") {
                return None;
            }
            let text = message.get("message").and_then(Value::as_str).unwrap_or("a compiler error").to_owned();
            // **The primary span is the place the error is about.** A message with none — cargo's own
            // refusal, a link error — is reported by its sentence, never by the first `note`'s span.
            let span = message
                .get("spans")
                .and_then(Value::as_array)?
                .iter()
                .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))?;
            // **A span with no file is a message about the build, not a line in it** (a linker error
            // naming a crate). It is still the compiler's sentence, and saying so is better than
            // naming a file the person would open and find nothing wrong in.
            let Some(file) = span.get("file_name").and_then(Value::as_str).filter(|f| !f.is_empty()) else {
                return Some(BuildEvent::Error(BuildError {
                    file: String::new(),
                    line: 0,
                    message: text,
                }));
            };
            // **No line is `0`, which `BuildError::link` and `problem` both read as "no line"**, rather
            // than a `1` a person would go to and find nothing at.
            let line = span.get("line_start").and_then(Value::as_u64).unwrap_or(0) as u32;
            Some(BuildEvent::Error(BuildError {
                file: file.to_owned(),
                line,
                message: text,
            }))
        }
        "build-finished" => Some(BuildEvent::Finished {
            success: value.get("success").and_then(Value::as_bool).unwrap_or(false),
        }),
        _ => None,
    }
}

/// **What cargo said the build did**, as its own `build-finished` message worded it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Verdict {
    /// `build-finished` has not been read yet.
    #[default]
    Unsaid,
    /// Cargo said the build succeeded.
    Succeeded,
    /// Cargo said the build failed.
    Failed,
}

/// **How many of a build's two output pipes have been read to the end.**
///
/// **The build is over when both are drained, not when the child exits.** Cargo's compiler is a child
/// process that inherits the pipe, so its last lines — and `build-finished` itself — can arrive after
/// `cargo` has exited; a poll that trusted `try_wait` alone read an empty error list and reported a
/// failed build as a success.
const PIPES: u8 = 2;

/// **What the reader threads and the editor both hold.** Shared rather than moved, because the threads
/// write it while the editor draws it, and a child's output arrives on nobody's frame.
#[derive(Default)]
struct Shared {
    progress: Mutex<Progress>,
    errors: Mutex<Vec<BuildError>>,
    /// What cargo's `build-finished` said.
    verdict: Mutex<Verdict>,
    /// **The last line cargo wrote to stderr**, which is what a build that never reached a compiler
    /// says instead of a span (`error: no such command: build`). Bounded: a failing build can say
    /// megabytes, and one line is what a person reads.
    refusal: Mutex<Option<String>>,
    /// **The pipes read to the end**, of [`PIPES`]. The build is over at `PIPES`, whatever `try_wait`
    /// said.
    drained: AtomicU32,
    /// **The build was stopped by the person**, so its exit is not reported as a compiler failure.
    stopped: AtomicBool,
}

impl Shared {
    /// **Whether both pipes have been read to the end**, which is when a build's result is complete.
    fn fully_read(&self) -> bool {
        self.drained.load(Ordering::Acquire) == u32::from(PIPES)
    }

    /// **One pipe is finished**, said by the reader that had it.
    fn a_pipe_finished(&self) {
        self.drained.fetch_add(1, Ordering::AcqRel);
    }
}

/// **One build this editor started**, by project root.
struct Running {
    child: Child,
    shared: Arc<Shared>,
    /// The map and camera the game is relaunched on, read when the build started.
    map: String,
    camera: Option<[f32; 3]>,
    /// **Who asked for the build**, so the relaunch is logged as theirs.
    source: crate::ops::Source,
}

static RUNNING: Mutex<Option<std::collections::HashMap<PathBuf, Running>>> = Mutex::new(None);

fn with_table<T>(f: impl FnOnce(&mut std::collections::HashMap<PathBuf, Running>) -> T) -> T {
    let mut guard: MutexGuard<'_, Option<std::collections::HashMap<PathBuf, Running>>> =
        RUNNING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(guard.get_or_insert_with(Default::default))
}

/// **Whether a build is running for this project.**
pub fn is_building(root: &Path) -> bool {
    with_table(|t| t.contains_key(root))
}

/// **What an editor is shown about builds**: the one running, and what the last one found.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildState {
    /// The project a build is running for and how far it is.
    pub running: Option<BuildView>,
    /// **The last build's errors**, kept after the job leaves the band so they can still be read.
    pub errors: Vec<BuildError>,
    /// **The last build's count**, which is cargo's own: how many crates it compiled.
    pub progress: Progress,
    /// **The last build's sentence**: `built, and playing m (pid 41)`, or what stopped it.
    pub said: String,
}

/// One running build, as the editor and a test read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildView {
    pub root: PathBuf,
    pub progress: Progress,
    pub errors: Vec<BuildError>,
}

/// **Whether a build is running in this editor**, which is what the Build button and a test ask.
pub fn building(app: &App) -> bool {
    app.world().get_resource::<BuildState>().is_some_and(|state| state.running.is_some())
}

// ── the operation ───────────────────────────────────────────────────────────────────────────────

/// **What a build needs**, read once so the plan and the commit cannot disagree: the project's root, the
/// file's command, and the map the game is relaunched on.
struct Ready {
    root: PathBuf,
    config: PlayConfig,
    map: String,
}

/// **`game_build`: run the project's build command, as a job.**
pub struct GameBuild;

impl GameBuild {
    /// The project's root, its command and its map — or why it cannot be built.
    fn prepare(ctx: &Ctx) -> Result<Ready, Refusal> {
        let Some(root) = ctx.root_or_project() else {
            return Err(Refusal::new("no project is open, so there is nothing to build."));
        };
        let map = ctx
            .open
            .as_ref()
            .map(|open| open.map.name.clone())
            .ok_or_else(|| Refusal::new("no map is open, so there is no game to relaunch when the build succeeds."))?;
        if is_building(&root) {
            return Err(Refusal::new("a build is already running. Wait for it, or stop it in the status band."));
        }
        let config = PlayConfig::load(&root).map_err(Refusal::new)?;
        // **The command must exist before anything starts**, so a project that has none is told what
        // to write rather than shown a job that fails a second later.
        config.build_argv(&root).map_err(Refusal::new)?;
        // **The same gate Play is behind**, on the build's own command: a `play.ron` that names a
        // program runs nothing until a person has allowed it, and the build is a second program the
        // same file names.
        if let Some(spec) = config.build_spec()
            && let Some(request) = crate::trusted::needs_answer(ctx.trust_file.as_deref(), &root, &spec).map_err(Refusal::new)?
        {
            return Err(Refusal::new(bevy_grimoire_core::trust::TrustStore::untrusted_sentence(&request.root, &request.spec)));
        }
        Ok(Ready { root, config, map })
    }
}

impl Operation for GameBuild {
    fn name(&self) -> &'static str {
        "game_build"
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        // **The band's job is where this is seen**: a build writes no map, no file and no setting.
        Shown::Job { key: BUILD_JOB.to_owned() }
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "camera": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3,
                    "description": "Where the editor's camera is, [x, y, z], handed to the game when it relaunches. Default: the editor's own camera."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, _params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let ready = Self::prepare(ctx)?;
        let argv = ready.config.build_argv(&ready.root).map_err(Refusal::new)?;
        Ok(json!({"argv": argv, "cwd": ready.config.dir(&ready.root).display().to_string()}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let camera = params
            .get("camera")
            .and_then(|c| serde_json::from_value::<[f32; 3]>(c.clone()).ok());
        let ready = Self::prepare(ctx)?;
        let started = start(&ready.root, &ready.config, &ready.map, camera, ctx.source).map_err(Refusal::new)?;
        Ok(Outcome {
            summary: format!("building {}: {}", ready.map, started.iter().skip(1).map(String::as_str).collect::<Vec<_>>().join(" ").trim()),
            affected: 0,
            saved: false,
            inverse: None,
            after: Some(json!({"state": "building", "command": started, "map": ready.map})),
        })
    }
}

/// **Start the project's build**, and answer with the command at once: it is a job, not a call that
/// blocks, and the child's output is read on a thread so no frame waits on a compiler.
pub fn start(root: &Path, config: &PlayConfig, map: &str, camera: Option<[f32; 3]>, source: crate::ops::Source) -> Result<Vec<String>, String> {
    if is_building(root) {
        return Err(format!("a build of {} is already running.", root.display()));
    }
    let mut argv = config.build_argv(root)?;
    // **Cargo's messages, unless the file already chose a format.** A command that names its own
    // `--message-format` is run as written: the file is the project's and its author may have a reason.
    if !argv.iter().any(|arg| arg.starts_with("--message-format")) {
        argv.push(CARGO_JSON_FLAG.to_owned());
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(config.dir(root))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // **No stdin.** A build that decided to ask a question would hold its own pipe open and the
        // job would never end; a build that needs input is not one a person can watch.
        .stdin(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd.spawn().map_err(|e| format!("could not start `{}`: {e}. Nothing is being built.", argv[0]))?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        return Err(format!("`{}` gave no pipes to read, so its build could not be watched.", argv[0]));
    };
    let shared = Arc::new(Shared::default());
    // **Two threads, one pipe each.** Reading stdout to its end and only then starting on stderr is a
    // deadlock waiting for a big build: once stderr passes the pipe's buffer (~64 KB) cargo blocks on
    // its own write, so stdout never closes and neither reader ever finishes.
    let (out, err) = (Arc::clone(&shared), Arc::clone(&shared));
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            match read_line(&line) {
                Some(BuildEvent::Compiled(_)) => {
                    out.progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner).compiled += 1;
                }
                Some(BuildEvent::Error(error)) => {
                    out.errors.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(error);
                }
                Some(BuildEvent::Finished { success }) => {
                    *out.verdict.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                        if success { Verdict::Succeeded } else { Verdict::Failed };
                }
                None => {}
            }
        }
        out.a_pipe_finished();
    });
    std::thread::spawn(move || {
        // **Drained to the end, even though only the last line is kept**: a reader that stopped early
        // would leave cargo blocked on a full stderr and the build would never end.
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                *err.refusal.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(trimmed.to_owned());
            }
        }
        err.a_pipe_finished();
    });
    with_table(|t| {
        t.insert(
            root.to_path_buf(),
            Running { child, shared: Arc::clone(&shared), map: map.to_owned(), camera, source },
        );
    });
    Ok(argv)
}

/// **Leaving the project stops its build**, and drops the record with the rest of the door's state. **A switch of
/// workspace does not** (NV-09, #386): the build is watched in the Game workspace, and a person who goes to the Map
/// while it compiles has not asked for it to stop. A switch marks itself with [`crate::screen::Crossing`], which this
/// reads before `close_the_door` takes it away.
fn stop_the_build(project: Option<Res<Project>>, crossing: Option<Res<crate::screen::Crossing>>) {
    if crossing.is_some() {
        return;
    }
    if let Some(root) = project.as_ref().map(|p| p.root.clone()) {
        stop(&root);
    }
}

/// **Stop the build this editor started for `root`**, by its process group, so a cargo that spawned
/// rustc takes rustc with it. `false` when nothing was running.
pub fn stop(root: &Path) -> bool {
    let Some(mut running) = with_table(|t| t.remove(root)) else { return false };
    running.shared.stopped.store(true, Ordering::Release);
    let pid = running.child.id();
    #[cfg(unix)]
    let _ = Command::new("kill").args(["-KILL", &format!("-{pid}")]).stderr(Stdio::null()).status();
    let _ = running.child.kill();
    let _ = running.child.wait();
    true
}

// ── the plugin ──────────────────────────────────────────────────────────────────────────────────

/// Adds the build's record, its polling, and the PLAY section's Build button.
pub struct BuildPlugin;

impl Plugin for BuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildState>()
            .add_systems(Update, (poll_the_build, follow_the_build_line).run_if(in_state(Screen::Editor)))
            // **A door change takes the build with it**: the job names this project's map and camera, and
            // a compiler for a project nobody is looking at is not this editor's work. Stopped by its
            // group, so rustc goes with cargo.
            .add_systems(OnExit(Screen::Editor), stop_the_build.before(crate::screen::close_the_door));
    }
}

/// **Follow the running build**: its progress into the band's job, its errors into problems, and its
/// end into a relaunch when it worked — or a refusal that leaves the running game alone.
fn poll_the_build(
    jobs: Option<Res<crate::jobs::Jobs>>,
    mut state: Option<ResMut<BuildState>>,
    project: Option<Res<Project>>,
    mut editor: Option<ResMut<crate::editor::EditorState>>,
    // **Where the person is looking**, which is the camera the relaunch keeps when the build named none.
    camera: Query<&Transform, With<crate::view::MainCamera>>,
    mut commands: Commands,
) {
    let Some(state) = state.as_mut() else { return };
    let Some(root) = project.as_ref().map(|p| p.root.clone()) else {
        // **A project closed under a running build**: its job leaves the band, and the child keeps no
        // record here — a build whose result nobody is looking at is not this editor's step any more.
        state.running = None;
        return;
    };
    if state.running.is_none() && !is_building(&root) {
        return;
    }
    // **Where the build stands**, in one look under the lock: still running (with what it has said so
    // far), or over with everything it said.
    //
    // **Over means both pipes have been read to the end, not that the child has exited.** Cargo's
    // compiler is a child of cargo that inherits the pipe, so its last lines — and `build-finished`
    // itself — arrive after `cargo` exits. Stopping at `try_wait` read an empty error list and called a
    // failed build a success (measured: a build whose last line comes after its process, which is
    // `build_late` in the test).
    enum Where {
        Running(BuildView),
        Over { shared: Arc<Shared>, status_ok: bool, map: String, camera: Option<[f32; 3]>, source: crate::ops::Source },
        Gone,
    }
    let where_ = with_table(|t| {
        let Some(running) = t.get_mut(&root) else { return Where::Gone };
        let view = BuildView {
            root: root.clone(),
            progress: running.shared.progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
            errors: running.shared.errors.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
        };
        let exited = running.child.try_wait();
        // **A child that has exited with its pipes still open is still building**, as far as what a
        // person is told goes: the lines it has not finished writing are the ones that matter.
        if !matches!(exited, Ok(Some(_))) || !running.shared.fully_read() {
            return Where::Running(view);
        }
        let status_ok = matches!(exited, Ok(Some(status)) if status.success());
        let shared = Arc::clone(&running.shared);
        let (map, camera, source) = (running.map.clone(), running.camera, running.source);
        t.remove(&root);
        Where::Over { shared, status_ok, map, camera, source }
    });
    match where_ {
        Where::Gone => {
            state.running = None;
        }
        Where::Running(view) => {
            if let Some(jobs) = jobs.as_deref() {
                jobs.set(BUILD_JOB, "Building the game", view.progress.words(), None);
                // **The person's stop is the group's**, and its report is a note, not a compiler
                // failure: asking a compiler to stop is a decision, not an error it made.
                if jobs.cancel_requested(BUILD_JOB) {
                    stop(&root);
                    jobs.finish(BUILD_JOB);
                    state.running = None;
                    state.said = "you stopped it".to_owned();
                    if let Some(editor) = editor.as_mut() {
                        editor.status.note("the build was stopped".to_owned());
                    }
                    return;
                }
            }
            if state.running.as_ref() != Some(&view) {
                state.running = Some(view);
            }
        }
        Where::Over { shared, status_ok, map, camera: camera_now, source } => {
            if let Some(jobs) = jobs.as_deref() {
                jobs.finish(BUILD_JOB);
            }
            state.running = None;
            let stopped = shared.stopped.load(Ordering::Acquire);
            let verdict = *shared.verdict.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let errors = shared.errors.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            let refusal = shared.refusal.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            state.errors.clone_from(&errors);
            state.progress = shared.progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            if stopped {
                state.said = "you stopped it".to_owned();
                return;
            }
            // **`build-finished` is cargo's own answer**; a command that never said it (one that is not
            // cargo at all) is judged by its status, which is the only word it gave.
            let success = match verdict {
                Verdict::Succeeded => true,
                Verdict::Failed => false,
                Verdict::Unsaid => status_ok,
            };
            if success {
                // **The game that was running keeps running until here**, and only now is it replaced:
                // the same map at the same camera, the two `play` already substitutes.
                //
                // **Written out first, and that is the reload** (GD-09's acceptance). The restarted game
                // reads the project's files as it starts, so a map the editor held unsaved would be
                // replaced by the one the disk has — the build would then show less than the person had
                // been looking at. Saving here is the same write Cmd+S makes, and the same one a restart
                // after a save reads, so the game comes up on what the editor holds.
                // **Queued on the world, because saving is the open map's own write** and this system
                // holds it only to read. The relaunch is queued after it, so a save that fails stops
                // the restart rather than replacing a game with the state the disk had.
                let (map_to_play, camera_to_play, trust_file, of_the_build) = (
                    map.clone(),
                    camera_now.or_else(|| camera.iter().next().map(|t| t.translation.to_array())),
                    editor.as_deref().and_then(|editor| editor.trust.file.clone()),
                    source,
                );
                commands.queue(move |world: &mut World| {
                    // **Nothing to write is the same as written**: a map already saved is exactly what
                    // the game reads when it starts, so `save_the_open_map`'s `Ok` covers both.
                    let said = match crate::app_verbs::save_the_open_map(world) {
                        Ok(()) => relaunch(&root, &map_to_play, camera_to_play, trust_file, of_the_build)
                            .map_err(|e| format!("the game did not restart: {e}")),
                        Err(why) => Err(format!("{why}, so nothing was replaced")),
                    };
                    let (line, failed) = match said {
                        Ok(said) => (format!("built, and {said}"), false),
                        Err(e) => (format!("built, but {e}"), true),
                    };
                    if let Some(mut state) = world.get_resource_mut::<BuildState>() {
                        state.said.clone_from(&line);
                    }
                    if let Some(mut editor) = world.get_resource_mut::<crate::editor::EditorState>() {
                        if failed {
                            editor.status.problem(line);
                        } else {
                            editor.status.note(line);
                        }
                    }
                });
                return;
            }
            // **A failure: the game that was running is left running.** Each error becomes a problem
            // with its file and its line, which is what a person needs to go and fix it.
            let said = if errors.is_empty() {
                refusal.unwrap_or_else(|| "the build failed and named no error".to_owned())
            } else {
                format!("{} compiler error(s)", errors.len())
            };
            state.said = said.clone();
            if let Some(editor) = editor.as_mut() {
                for error in errors.iter().take(ERRORS_SHOWN) {
                    editor.status.raise("the build failed: ", error.problem());
                }
                if errors.is_empty() {
                    editor.status.problem(format!("the build failed: {said}"));
                } else if errors.len() > ERRORS_SHOWN {
                    editor.status.problem(format!(
                        "the build failed: {} more error(s) than the {ERRORS_SHOWN} listed here",
                        errors.len() - ERRORS_SHOWN
                    ));
                }
            }
        }
    }
}

/// **Run `game_build` in a context**, and answer with its summary — [`crate::editor::play_game`]'s
/// shape, so the Build button, a key and an agent reach one implementation of "build this project".
pub fn build_game(ctx: &mut Ctx, params: serde_json::Value) -> Result<String, String> {
    match crate::ops::dispatch(ctx, "game_build", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **Stop the editor's game for `root` and start it again on `map`, at `camera`**, through the two
/// operations a person's Stop and Play press dispatch. The stop is best-effort: with no game running
/// there is nothing to stop, which is not a failure of the relaunch.
fn relaunch(root: &Path, map: &str, camera: Option<[f32; 3]>, trust_file: Option<PathBuf>, source: crate::ops::Source) -> Result<String, String> {
    // **Nothing was running**, which the sentence says: a build that succeeds over a game the person
    // had already stopped *starts* one, and a receipt that read "playing m" without saying so would
    // look like a relaunch of a game that was not there.
    let was_running = crate::ops_play::running_pid(root).is_some();
    // **The person's own store**, so a relaunch runs the command they allowed a moment ago and not a
    // command nobody has allowed. `play` and `play_stop` are behind that gate, and they should be.
    //
    // **And the source the build was started with**: a rebuild an agent asked for relaunches the game
    // as the agent's, so the log and the history say who did it rather than crediting the person.
    let mut ctx = Ctx {
        root: Some(root.to_path_buf()),
        session: crate::ops::session_id(),
        trust_file,
        source,
        ..Ctx::default()
    };
    let _ = crate::ops::dispatch(&mut ctx, "play_stop", &json!({}));
    let mut params = json!({"map": map});
    if let Some(camera) = camera {
        params["camera"] = json!(camera);
    }
    match crate::ops::dispatch(&mut ctx, "play", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => Ok(if was_running {
            outcome.summary
        } else {
            format!("started the game, which was not running: {}", outcome.summary)
        }),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **Would `game_build` have to ask a person first?** The Build button asks before it dispatches, the
/// way the Play button does, so the question is raised instead of only a refusal. `Ok(None)` when the
/// build is allowed, or when the project names no build command (the operation says so itself).
pub fn untrusted_request(ctx: &Ctx, root: &Path) -> Result<Option<crate::trusted::TrustRequest>, String> {
    match PlayConfig::load(root) {
        Ok(config) => match config.build_spec() {
            Some(spec) => crate::trusted::needs_answer(ctx.trust_file.as_deref(), root, &spec),
            None => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

// ── what a person sees ───────────────────────────────────────────────────────────────────────────

/// The PLAY section's Build button, which dispatches `game_build` through the operation an agent calls.
#[derive(Component)]
pub struct BuildButton;

/// The build's own line, beside the button: what it is doing, or what it last said.
#[derive(Component)]
pub struct BuildLine;

/// **The PLAY section's BUILD row**: the button, and the line that says what it is doing.
///
/// It sits beside Play and Stop, because it is the step that must come before a Play that would
/// otherwise show a game missing the change: a person who presses Play after a change that needs Rust
/// finds the rebuild in the one place they are already looking.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::{GAP_TIGHT, Severity, button, section};
    section(s, "BUILD");
    s.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(GAP_TIGHT), flex_shrink: 0.0, ..default() })
        .with_children(|row| {
            button(row, BuildButton, "Build", Severity::Plain).observe(on_build_click);
            crate::chrome::row_value(row, "", crate::chrome::ink().dim, BuildLine);
        });
}

/// **The Build button asks for a build**, through the same operation an agent calls, so the job, the
/// errors and the relaunch are one path.
///
/// **A build no person has allowed is not started**: the question is raised, exactly as Play raises it
/// for the game, because `play.ron` names both programs.
fn on_build_click(
    activate: On<Activate>,
    buttons: Query<(), With<BuildButton>>,
    project: Option<Res<Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    camera: Query<&Transform, With<crate::view::MainCamera>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(state) = state.as_mut() else { return };
    let mut ctx = Ctx {
        project: project.as_deref().cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        trust_file: state.trust.file.clone(),
        ..Ctx::default()
    };
    if let Some(root) = ctx.root_or_project() {
        match untrusted_request(&ctx, &root) {
            Ok(Some(request)) => {
                state.status.problem(bevy_grimoire_core::trust::TrustStore::untrusted_sentence(&request.root, &request.spec));
                state.trust.pending = Some(request);
                return;
            }
            Ok(None) => {}
            Err(e) => return state.status.problem(e),
        }
    }
    let mut params = json!({});
    if let Some(camera) = camera.iter().next().map(|t| t.translation.to_array()) {
        params["camera"] = json!(camera);
    }
    let _ = open;
    match build_game(&mut ctx, params) {
        Ok(summary) => state.status.note(summary),
        Err(e) => state.status.problem(e),
    }
}

/// **The line says what the build is doing or last said**, and nothing while it has never run.
fn follow_the_build_line(state: Option<Res<BuildState>>, mut lines: Query<&mut Text, With<BuildLine>>) {
    let Some(state) = state else { return };
    let said = match &state.running {
        Some(view) => {
            let words = view.progress.words();
            if words.is_empty() { "building".to_owned() } else { format!("building, {words}") }
        }
        None => state.said.clone(),
    };
    for mut text in &mut lines {
        if text.0 != said {
            text.0 = said.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cargo_artifact_message_is_one_compiled_crate() {
        let line = r#"{"reason":"compiler-artifact","target":{"name":"bevy_ecs","kind":["lib"]},"fresh":false,"filenames":[]}"#;
        assert_eq!(read_line(line), Some(BuildEvent::Compiled("bevy_ecs".to_owned())));
    }

    #[test]
    fn a_compiler_error_is_its_file_its_line_and_its_sentence() {
        let line = r#"{"reason":"compiler-message","message":{"level":"error","message":"cannot find type `Wander` in this scope","spans":[{"file_name":"src/people.rs","line_start":42,"is_primary":false},{"file_name":"src/wander.rs","line_start":7,"is_primary":true}]}}"#;
        assert_eq!(
            read_line(line),
            Some(BuildEvent::Error(BuildError {
                file: "src/wander.rs".to_owned(),
                line: 7,
                message: "cannot find type `Wander` in this scope".to_owned(),
            }))
        );
    }

    #[test]
    fn a_warning_is_not_a_problem_and_a_line_that_is_not_cargo_is_not_an_event() {
        let warning = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused","spans":[]}}"#;
        assert_eq!(read_line(warning), None);
        assert_eq!(read_line("Compiling bevy_ecs v0.20.0"), None);
        assert_eq!(read_line(""), None);
    }

    #[test]
    fn the_progress_counts_crates_and_never_claims_a_total_cargo_did_not_state() {
        assert_eq!(Progress::default().words(), "");
        assert_eq!(Progress { compiled: 1 }.words(), "1 crate");
        assert_eq!(Progress { compiled: 12 }.words(), "12 crates");
    }

    #[test]
    fn an_error_reads_as_its_file_its_line_and_its_sentence() {
        let error = BuildError {
            file: "src/people.rs".to_owned(),
            line: 42,
            message: "cannot find type `Wander` in this scope".to_owned(),
        };
        assert_eq!(error.link(), "src/people.rs:42");
        let line = error.problem().line();
        assert!(line.contains("src/people.rs"), "{line}");
        assert!(line.contains("42"), "{line}");
        assert!(line.contains("Wander"), "{line}");
    }
}
