//! **When a change needs Rust, the rebuild is a step the person can watch** (#371, GD-09).
//!
//! A project's `play.ron` names how the game runs. When the change a person wants needs a new type or
//! a new system, that command has to be rebuilt before the game can show it — so `game_build` runs
//! the project's **build** command, as a background job (UI-22) whose progress comes from cargo's own
//! JSON messages, and turns every compiler error into a problem with its file and its line.
//!
//! # What is being proved
//!
//! 1. **The job is a background job**, so the band says what it is doing and how far along it is, and
//!    the count is cargo's own: how many of the crates it must compile are compiled.
//! 2. **A compiler error becomes a problem with the file and the line**, and a build that fails leaves
//!    the running game running.
//! 3. **A build that succeeds relaunches the game on the same map with the same camera**, and only
//!    then: the game that was running is replaced by a new one, and the map name in its arguments is
//!    the map the editor has open.
//! 4. **The errors name a file the editor can go to**, so a person reads the compiler's sentence and
//!    the place, not a wall of cargo's own text.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

/// **A stub `cargo build` that says what a real one says**, in the same JSON, and exits with the
/// status the test asks for: the artifact lines a person's progress comes from, and one compiler
/// error with a file and a line when the project holds a `build_fail` file.
///
/// It is a stub rather than a real build because the point under test is the editor's reading of
/// cargo's messages — a real build would take minutes and would prove rustc works.
const STUB_BUILD: &str = r#"
import json, os, subprocess, sys, time
# **The directory the build was started in**, which is what `play.ron`'s `cwd` means: the editor runs
# the command there, so a stub reads its project from its own working directory and never from an
# environment variable the test process happens to carry.
root = os.getcwd()
def say(**kw): print(json.dumps(kw), flush=True)
for name in ["bevy_ecs", "bevy_app", "the_game"]:
    say(reason="compiler-artifact", target={"name": name, "kind": ["lib"]}, fresh=False,
        filenames=[os.path.join(root, "target/lib" + name + ".rlib")])
    # **A compiler takes time**, so the bar a person watches has frames to move on. The stub waits the
    # same way a real rustc does between one crate and the next.
    time.sleep(0.12)
late_error = {
    "reason": "compiler-message",
    "message": {
        "level": "error",
        "message": "cannot find type `Wander` in this scope",
        "spans": [{"file_name": "src/people.rs", "line_start": 42, "line_end": 42, "is_primary": True,
                   "text": [{"text": "    let wander: Wander = Wander::new();"}]}],
        "rendered": "error[E0412]: cannot find type `Wander` in this scope\n  --> src/people.rs:42:17",
    },
}
if os.path.exists(os.path.join(root, "build_late")):
    # **The last words arrive after this process has exited**, which is what a real build does: its
    # compiler is a child that keeps the inherited pipe open a moment longer than `cargo` itself. A
    # reader that stops at the first `try_wait` that answers loses every line after it.
    subprocess.Popen([sys.executable, "-c", (
        "import json,sys,time\n"
        "time.sleep(0.5)\n"
        "print(json.dumps(" + repr(late_error) + "), flush=True)\n"
        "print(json.dumps({'reason': 'build-finished', 'success': False}), flush=True)\n"
    )])
    # **This process is gone before those lines exist**, which is the race: `try_wait` answers "exited"
    # while the pipe is still open and the reader still has lines to come.
    sys.exit(0)
if os.path.exists(os.path.join(root, "build_fail")):
    say(**late_error)
    say(reason="build-finished", success=False)
    sys.exit(101)
say(reason="build-finished", success=True)
"#;

/// **A stub game**: it writes the map and camera it was started with beside itself, so a relaunch can
/// be told from the launch before it, and it stays up until the editor stops it.
const STUB_GAME: &str = r#"
import os, sys, time
root = os.environ["GRIMOIRE_PLAY_ROOT"]
map_name = os.environ["GRIMOIRE_PLAY_MAP"]
# **What the game reads when it starts**, and the editor's own test reads the same thing to compare
# against: the bytes of the map file at the moment the game came up. A relaunch that did not write the
# editor's unsaved map starts on the older bytes, which is the difference under test.
path = os.path.join(root, "assets/grimoire/maps", map_name + ".map.ron")
text = open(path).read() if os.path.exists(path) else ""
open(os.path.join(root, "launches.txt"), "a").write(
    os.environ["GRIMOIRE_PLAY_MAP"] + " " + os.environ.get("GRIMOIRE_PLAY_CAMERA", "")
    + " " + str(len(text)) + "\n")
time.sleep(60)
"#;

/// **A game this test started is killed on both paths**, because a leaked game holds the runner's
/// output pipe open and its `git push` never returns (`play_starts_and_stops_the_game.rs` measured it).
struct KillGroupOnDrop(u32);

impl Drop for KillGroupOnDrop {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &format!("-{}", self.0)])
            .stderr(std::process::Stdio::null())
            .status();
    }
}

/// The interpreter the stubs run on: the system's `/usr/bin/python3` where there is one.
fn fixtures_python() -> &'static str {
    if std::path::Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}

/// **A project whose game and build both run the stubs**, with both commands trusted as a person
/// would have allowed them.
fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place_as("wall@0", "site/wall", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("build_stub.py"), STUB_BUILD).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("game_stub.py"), STUB_GAME).unwrap_or_else(|e| panic!("{e}"));
    let py = fixtures_python();
    std::fs::write(
        root.join("play.ron"),
        format!(
            r#"(version: 2, command: ["{py}", "game_stub.py"], build: ["{py}", "build_stub.py"])"#,
        ),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    root
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    // **The person has trusted this project**, which the fixture's store writes: `play.ron` names two
    // programs now — the game and its build — and both are behind the same gate. The editor holds the
    // store, which is where the relaunch reads it when the build succeeds.
    let store = fixtures::trusted_store(root);
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().trust.file = Some(store.clone());
    for _ in 0..6 {
        app.update();
    }
    // **The store the test's own dispatches use**, the person's.
    app.insert_resource(Trusted(store));
    app
}

/// The person's trust store, so a test dispatches the two operations an agent would be refused.
#[derive(Resource, Clone)]
struct Trusted(std::path::PathBuf);

/// **A context over this project with the person's store**, which is what a person's press builds.
fn person(app: &App) -> Ctx {
    // **Everything a person's own press carries**: the project, the map on screen, and the store their
    // answers went in. A context missing one of these would refuse for a reason nobody sees.
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    let trust_file = app.world().resource::<Trusted>().0.clone();
    Ctx {
        project: Some(project),
        open: Some(open),
        source: ops::Source::Human,
        session: ops::session_id(),
        trust_file: Some(trust_file),
        ..Ctx::default()
    }
}

/// **Run an operation as the person would**, and answer with its summary.
fn dispatch(app: &App, operation: &str, params: serde_json::Value) -> Result<String, String> {
    let mut ctx = person(app);
    match ops::dispatch(&mut ctx, operation, &params) {
        Ok(Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **Step the editor until `done` or the budget runs out**, so a background job is given the frames a
/// person's editor would give it.
fn until(app: &mut App, budget: usize, mut done: impl FnMut(&App) -> bool) -> bool {
    for _ in 0..budget {
        app.update();
        if done(app) {
            return true;
        }
    }
    false
}

/// **Step the editor until the build has ended**, which is the condition a person waits for: the job
/// leaves the band and the poll has read its last words. `true` when it ended inside the budget.
fn build_ends(app: &mut App, root: &std::path::Path) -> bool {
    until(app, 800, |_| !bevy_cauldron::build_view::is_building(root))
}

fn launches(root: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(root.join("launches.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// **The problems the status line is holding**, oldest first.
fn problems(app: &App) -> Vec<String> {
    app.world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .problems()
        .iter()
        .map(|p| p.text.clone())
        .collect()
}

/// **1 and 2. A failing build is a job with the compiler's progress, and its error is a problem with
/// the file and the line — and the game that was running keeps running.**
#[test]
fn a_failing_build_shows_its_progress_and_its_errors_and_leaves_the_game_running() {
    let root = project(&format!("build_fail_{}", std::process::id()));
    // **This build fails**, the way a change that needs new Rust fails when it is wrong.
    std::fs::write(root.join("build_fail"), "").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    // **A game is already up**, so "the game keeps running" has a game to keep.
    dispatch(&app, "play", json!({"map": "m"})).unwrap_or_else(|e| panic!("play must start the fixture game: {e}"));
    let running = bevy_cauldron::ops_play::running_pid(&root).expect("the game the test started");
    // **A failing test still ends the game it started**, or it holds the runner's pipe open.
    let _game = KillGroupOnDrop(running);

    // **A job, not a blocking call**: the dispatch answers while the compiler is still working, so
    // the child is in the table before the first frame that could report it.
    let summary = dispatch(&app, "game_build", json!({})).unwrap_or_else(|e| panic!("a build must start: {e}"));
    assert!(
        bevy_cauldron::build_view::is_building(&root),
        "the build is not running after it answered `{summary}`"
    );

    // **The progress is cargo's own count**: the stub reports three compiled targets exactly as cargo
    // does, and the band's job says so while the build runs.
    // **The count is read while the build runs**, which is the frame the band draws it on. The stub
    // writes its three artifacts at once, so a finished build may show the last of them; a count of
    // three is what a person sees at the end either way.
    let mut seen = Vec::new();
    let mut app_seen = Vec::new();
    for _ in 0..800 {
        app.update();
        if let Some(state) = app.world().get_resource::<bevy_cauldron::build_view::BuildState>() {
            if let Some(view) = &state.running {
                seen.push(view.progress.compiled);
            }
            app_seen.push(state.said.clone());
        }
        if !bevy_cauldron::build_view::is_building(&root) {
            break;
        }
    }
    assert!(
        seen.iter().any(|compiled| *compiled >= 1),
        "the job never counted a crate cargo compiled: seen {seen:?}, said {app_seen:?}"
    );
    // **And the whole count is cargo's own**: three artifacts were reported, three were counted.
    let counted = app
        .world()
        .get_resource::<bevy_cauldron::build_view::BuildState>()
        .map(|state| state.progress.compiled)
        .unwrap_or_default();
    assert_eq!(counted, 3, "the job did not count every crate cargo said it compiled: said {app_seen:?}");

    // **The build ends**, and the poll that reads its last words is the frame after it.
    let ended = until(&mut app, 800, |app| !problems(app).is_empty());
    assert!(
        ended,
        "the failed build reported no problem. still building={} said={:?} problems={:?}",
        bevy_cauldron::build_view::is_building(&root),
        app.world().get_resource::<bevy_cauldron::build_view::BuildState>().map(|s| (s.said.clone(), s.progress.clone(), s.errors.clone())),
        problems(&app),
    );
    let said = problems(&app);
    let error = said
        .iter()
        .find(|p| p.contains("Wander"))
        .unwrap_or_else(|| panic!("the compiler's error is not a problem: {said:?}"));
    assert!(error.contains("src/people.rs"), "the problem does not name the file: {error:?}");
    assert!(error.contains("42"), "the problem does not name the line: {error:?}");

    assert_eq!(
        bevy_cauldron::ops_play::running_pid(&root),
        Some(running),
        "a failed build replaced the running game, and the old one is what a person was watching"
    );
    assert_eq!(launches(&root).len(), 1, "a failed build launched a second game: {:?}", launches(&root));
    let _ = dispatch(&app, "play_stop", json!({}));
    let _ = std::fs::remove_dir_all(&root);
}

/// **3. A build that succeeds relaunches the game on the same map, and only then.**
#[test]
fn a_build_that_succeeds_relaunches_the_game_on_the_same_map() {
    let root = project(&format!("build_ok_{}", std::process::id()));
    let mut app = editor(&root);
    dispatch(&app, "play", json!({"map": "m", "camera": [4.0, 5.0, 6.0]}))
        .unwrap_or_else(|e| panic!("play must start the fixture game: {e}"));
    let first_pid = bevy_cauldron::ops_play::running_pid(&root).expect("the game the test started");
    // **The stub writes its launch line at once**, but it is a process: give it its own moment.
    assert!(
        until(&mut app, 100, |_| !launches(&root).is_empty()),
        "the game the test started recorded no launch: {:?}",
        launches(&root)
    );
    // **The first game, and whatever replaced it, is ended by the test**, so a failure leaves nothing
    // running: `play` puts the game in its own process group, whose id is its pid.
    let _game = KillGroupOnDrop(first_pid);
    let first = launches(&root);
    assert_eq!(first.len(), 1, "the first launch is recorded: {first:?}");

    // **The camera the editor is looking through**, which is what the Build button passes and what the
    // relaunch keeps.
    dispatch(&app, "game_build", json!({"camera": [4.0, 5.0, 6.0]})).unwrap_or_else(|e| panic!("a build must start: {e}"));
    assert!(build_ends(&mut app, &root), "the build never finished, so nothing was shown");
    // **The relaunch is a process too**, so its own line is given its moment.
    assert!(
        until(&mut app, 100, |_| launches(&root).len() == 2),
        "the relaunched game recorded no launch: {:?}",
        launches(&root)
    );

    // **The replacement's own group is ended as well**: the first game's guard covers only the pid it
    // was made from, and a relaunched game would otherwise outlive a failing test for its 60 seconds.
    let _replacement = bevy_cauldron::ops_play::running_pid(&root).map(KillGroupOnDrop);
    let after = launches(&root);
    assert_eq!(after.len(), 2, "a successful build did not relaunch the game: {after:?}");
    // **The map, the camera and the bytes the game read**: the first two are the same as the launch
    // before it, and the third is what the reload test below compares.
    assert_eq!(
        after[1].rsplit_once(' ').map(|(before, bytes)| (before, bytes.parse::<usize>().unwrap_or(0) > 0)),
        after[0].rsplit_once(' ').map(|(before, bytes)| (before, bytes.parse::<usize>().unwrap_or(0) > 0)),
        "the relaunch is not the same map at the same camera: {after:?}"
    );
    assert!(after[1].starts_with("m 4,5,6 "), "the relaunch is not the same map at the same camera: {after:?}");
    assert!(
        bevy_cauldron::ops_play::running_pid(&root).is_some(),
        "the relaunched game is not running"
    );
    let _ = dispatch(&app, "play_stop", json!({}));
    let _ = std::fs::remove_dir_all(&root);
}

/// **3. A build is over only once its output has been read**, even when the last lines arrive after
/// the process itself has exited: cargo's own compiler is a child that keeps the pipe open a moment
/// longer, so a poll that trusts `try_wait` alone loses the errors and `build-finished` — and reports
/// a failed build as a success.
#[test]
fn a_build_waits_for_the_last_line_before_it_says_what_happened() {
    let root = project(&format!("build_late_{}", std::process::id()));
    // **The last words come after the process has exited**, and they say it failed.
    std::fs::write(root.join("build_late"), "").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    dispatch(&app, "play", json!({"map": "m"})).unwrap_or_else(|e| panic!("play must start the fixture game: {e}"));
    let first = bevy_cauldron::ops_play::running_pid(&root).expect("the game the test started");
    let _game = KillGroupOnDrop(first);
    assert!(
        until(&mut app, 100, |_| !launches(&root).is_empty()),
        "the game the test started recorded no launch: {:?}",
        launches(&root)
    );

    dispatch(&app, "game_build", json!({"camera": [4.0, 5.0, 6.0]})).unwrap_or_else(|e| panic!("a build must start: {e}"));
    assert!(build_ends(&mut app, &root), "the build never finished");

    let said = problems(&app);
    let error = said
        .iter()
        .find(|p| p.contains("Wander"))
        .unwrap_or_else(|| panic!("the error that arrived after the process exited is lost: {said:?}"));
    assert!(error.contains("src/people.rs") && error.contains("42"), "{error:?}");
    assert_eq!(
        bevy_cauldron::ops_play::running_pid(&root),
        Some(first),
        "a build read before its last line reported a failure as a success and replaced the running game"
    );
    assert_eq!(launches(&root).len(), 1, "a build that failed relaunched the game: {:?}", launches(&root));
    let _ = dispatch(&app, "play_stop", json!({}));
    let _ = std::fs::remove_dir_all(&root);
}

/// **The relaunch's own acceptance: the game comes up on what the editor held, not on what the disk had
/// before.** A person changes the map and rebuilds without pressing Cmd+S; the restart has to read the
/// change, which is what "the game's state reloads as it would on a save" means.
#[test]
fn a_relaunch_reads_the_map_the_editor_holds_even_unsaved() {
    let root = project(&format!("build_unsaved_{}", std::process::id()));
    let mut app = editor(&root);
    dispatch(&app, "play", json!({"map": "m"})).unwrap_or_else(|e| panic!("play must start the fixture game: {e}"));
    let first = bevy_cauldron::ops_play::running_pid(&root).expect("the game the test started");
    let _game = KillGroupOnDrop(first);
    assert!(
        until(&mut app, 100, |_| !launches(&root).is_empty()),
        "the game the test started recorded no launch"
    );

    // **A change the person made and has not saved.** An operation writes through as it commits, so the
    // unsaved case is the one the keys make — the map in memory moved and `Cmd+S` never pressed, which
    // is what `live`'s write-through exists for and what a build without `--live` has to survive.
    {
        let mut open = app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>();
        for placement in &mut open.map.placements {
            if placement.id == "wall@0" {
                placement.yaw = 1.25;
            }
        }
    }
    assert!(
        app.world().resource::<bevy_cauldron::project::OpenMap>().dirty(),
        "the test's change did not leave the map unsaved, so the reload is not being tested"
    );
    let held = bevy_cauldron::project::OpenMap::open(&app.world().resource::<bevy_cauldron::project::Project>().clone(), "m")
        .map(|on_disk| on_disk.map.placements.iter().find(|p| p.id == "wall@0").map(|p| p.yaw))
        .ok()
        .flatten();
    assert_ne!(held, Some(1.25), "the change reached disk before the build, so nothing is under test");

    dispatch(&app, "game_build", json!({"camera": [4.0, 5.0, 6.0]})).unwrap_or_else(|e| panic!("a build must start: {e}"));
    assert!(build_ends(&mut app, &root), "the build never finished");
    // **The save and the relaunch are queued together**, so they happen within a frame or two.
    assert!(
        until(&mut app, 200, |_| launches(&root).len() == 2),
        "the build that succeeded did not relaunch the game: {:?}",
        launches(&root)
    );

    // **The game read the map the editor held**, which is the yaw the person set and never saved.
    let on_disk = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron")).unwrap_or_default();
    assert!(
        on_disk.contains("1.25"),
        "the unsaved yaw was not written before the game restarted, so it came up on the old map"
    );
    let _replacement = bevy_cauldron::ops_play::running_pid(&root).map(KillGroupOnDrop);
    assert!(
        !app.world().resource::<bevy_cauldron::project::OpenMap>().dirty(),
        "the save did not mark the map as what the disk holds, so the editor would ask for a save it made"
    );
    let _ = dispatch(&app, "play_stop", json!({}));
    let _ = std::fs::remove_dir_all(&root);
}

/// **4. A project whose `play.ron` names no build command says so**, rather than building nothing and
/// reporting success.
#[test]
fn a_project_with_no_build_command_says_how_to_write_one() {
    let root = fixtures::Fixture::new(&format!("build_none_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    // **A project that runs its game and names no build**: the case a version 1 file is.
    std::fs::write(
        root.join("play.ron"),
        format!(r#"(version: 1, command: ["{py}", "game.py"])"#, py = fixtures_python()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let app = editor(&root);
    let refused = dispatch(&app, "game_build", json!({})).expect_err("a project with no build command has nothing to build");
    assert!(refused.contains("build"), "the refusal does not name what is missing: {refused:?}");
    assert!(refused.contains("play.ron"), "the refusal does not name the file: {refused:?}");
    let _ = std::fs::remove_dir_all(&root);
}
