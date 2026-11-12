//! **The Game workspace: the home for the running game** (NV-09, #386; series getting around, #377).
//!
//! The game keeps its own window; the Game workspace is where a person plays it, watches it and stops it from
//! the editor. This file drives the real editor with a **stub game**: a python process that prints log lines,
//! answers BRP the way `bevy_fairy` and `bevy_remote` do (one named entity, the resources, the frame time and the
//! entity's components), and — for the crash — exits with a code.
//!
//! What is proved, each by what a person would see:
//!
//! - **Play, Stop and Build are here and only here**: the PLAY and BUILD sections are not in the Map's INSPECTOR any
//!   more, and are in the Game workspace (there is no second launcher);
//! - **a game that runs** shows its log lines and an entity in the state view, its components when the entity is
//!   chosen, its frame time and memory; after Stop, the Game icon on the activity bar carries no badge;
//! - **a game that crashes** keeps its exit code and its last output on screen, and the Game icon is badged until a
//!   person plays again.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ComputedNode;

use bevy_cauldron::activity_bar::ActivityBar;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::game_view::GameView;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::ops_play::{self, PlayButton};
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::testing::click;

use crate::fixtures;

/// One frame of the test clock, in seconds: ten frames are a second, which is what the editor's polls count in.
const FRAME_S: f32 = 0.1;
/// How long a test waits for something the stub process does at its own pace.
const WAIT: Duration = Duration::from_secs(60);
/// The wall-clock pause between frames while waiting, so the stub gets the processor.
const PAUSE: Duration = Duration::from_millis(15);

/// **The stub game**: logs, and answers the BRP methods the Game workspace reads.
const GAME: &str = r#"
import http.server, json, sys, threading, time
print("stub game booted", flush=True)
def ticks():
    n = 0
    while True:
        n += 1
        print("stub tick %d" % n, flush=True)
        time.sleep(0.2)
threading.Thread(target=ticks, daemon=True).start()
NAME = "bevy_ecs::name::Name"
TRANSFORM = "bevy_transform::components::transform::Transform"
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))))
        method = request.get("method")
        if method == "bevy_fairy/status":
            result = {"ready": True}
        elif method == "world.query":
            result = [{"entity": 4294967297, "components": {NAME: "stub_player"}}]
        elif method == "world.list_resources":
            result = ["bevy_time::time::Time<bevy_time::real::Real>", "bevy_window::window::Window"]
        elif method == "world.get_resources":
            result = {"value": {"delta_secs": 0.0167}}
        elif method == "world.list_components":
            result = [NAME, TRANSFORM]
        elif method == "world.get_components":
            result = {"components": {NAME: "stub_player", TRANSFORM: {"translation": [1.0, 2.0, 3.0]}}, "errors": {}}
        else:
            result = None
        body = json.dumps({"jsonrpc": "2.0", "id": request.get("id", 1), "result": result}).encode()
        self.send_response(200); self.send_header("content-length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
"#;

/// **The stub that crashes**: says what it was doing, complains on stderr and exits with code 3.
const CRASH: &str = r#"
import sys
print("about to load the map", flush=True)
print("thread 'main' panicked: no such entity", file=sys.stderr, flush=True)
sys.exit(3)
"#;

fn python() -> &'static str {
    if Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}")).local_addr().unwrap_or_else(|e| panic!("{e}")).port()
}

/// **A project whose `play.ron` starts `script`**, and whose BRP port is `port`.
fn project(tag: &str, script: &str, port: u16) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("game_ws_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("game.py"), script).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        root.join("play.ron"),
        format!(r#"(version: 1, command: ["{py}", "game.py", "{port}"], port: {port})"#, py = python()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    root
}

/// **The editor, on the Map workspace**, with a test clock and the project's trust store, so Play is allowed.
fn editor(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME_S)));
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(fixtures::trusted_store(root));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **Switch the way the palette does**: the workspace's command, its chord queued and pressed.
fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "`{id}` did not open the {} workspace", workspace.title());
}

/// **Frames, paced by the wall clock, until `done`** — for what a child process does in its own time.
fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed() < WAIT {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(PAUSE);
    }
    panic!("timed out after {WAIT:?} waiting for {what}; on screen: {:#?}", words(app));
}

/// The words a person can see: every drawn `Text`.
fn words(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
    let mut out: Vec<String> = q
        .iter(world)
        .filter(|(t, shown, node)| shown.get() && !t.0.trim().is_empty() && node.size().x > 0.0 && node.size().y > 0.0)
        .map(|(t, _, _)| t.0.trim().to_owned())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Is there a drawn text that is exactly `label`?
fn shows(app: &mut App, label: &str) -> bool {
    words(app).iter().any(|w| w == label)
}

/// Is there a drawn text that contains `part`?
fn shows_part(app: &mut App, part: &str) -> bool {
    words(app).iter().any(|w| w.contains(part))
}

/// **Press a Play-section button**, by pointer, as a person does. A switch of workspace resets the editor's working
/// state, which holds the person's trust file, so the project is allowed again before each press.
fn press(app: &mut App, which: PlayButton) {
    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(fixtures::trusted_store(&root));
    let button = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &PlayButton)>();
        q.iter(world).find(|(_, b)| **b == which).map(|(e, _)| e)
    };
    let button = button.unwrap_or_else(|| panic!("no {which:?} button on screen: {:#?}", words(app)));
    click(app, button);
    for _ in 0..3 {
        app.update();
    }
}

/// **The Game icon's badge**, as the activity bar was drawn with it (`0` is none).
fn game_badge(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut q = world.query::<&ActivityBar>();
    let bar = q.iter(world).next().unwrap_or_else(|| panic!("the activity bar is not drawn"));
    bar.shown
        .iter()
        .find(|(w, _)| *w == Workspace::Game)
        .map(|(_, n)| *n)
        .unwrap_or_else(|| panic!("the Game workspace is not on the activity bar: {:?}", bar.shown))
}

/// **Play, Stop and Build are in the Game workspace and not in the Map's INSPECTOR.**
#[test]
fn play_stop_and_build_moved_to_the_game_workspace() {
    let root = project("moved", GAME, free_port());
    let mut app = editor(&root);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map);
    for label in ["PLAY", "BUILD", "Play", "Stop", "Build"] {
        assert!(!shows(&mut app, label), "`{label}` is still on the Map, so the game is launched two ways");
    }
    switch_to(&mut app, Workspace::Game);
    assert!(*app.world().resource::<Mode>() == Mode::Game, "the Game workspace opens on the Game panel");
    for label in ["PLAY", "BUILD", "LOG", "STATE", "Play", "Stop", "Build", "Show window"] {
        assert!(shows(&mut app, label), "the Game workspace does not show `{label}`: {:#?}", words(&mut app));
    }
    assert!(shows_part(&mut app, "Not running"), "a game that never ran says so");
}

/// **A stub game is played, its log lines and an entity are seen, and after Stop the badge is clear.**
#[test]
fn a_game_is_played_watched_and_stopped_from_the_game_workspace() {
    let port = free_port();
    let root = project("run", GAME, port);
    let mut app = editor(&root);
    switch_to(&mut app, Workspace::Game);
    assert_eq!(game_badge(&mut app), 0, "no game has run, so the Game icon says nothing");

    press(&mut app, PlayButton::Play);
    let pid = ops_play::running_pid(&root).unwrap_or_else(|| {
        let problems: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect();
        panic!("Play did not start the game; problems: {problems:?}")
    });
    let _cleanup = crate::play_starts_and_stops_the_game::KillGroupOnDrop(u64::from(pid));

    // **Its log lines**, from the file `play` writes the game's output to.
    until(&mut app, "the game's log lines", |app| shows_part(app, "stub tick"));
    assert!(app.world().resource::<GameView>().log.iter().any(|l| l == "stub game booted"), "the game's first line is in the log");

    // **An entity in the state view**, read over BRP.
    until(&mut app, "the entity in the state view", |app| shows(app, "stub_player"));
    assert!(shows_part(&mut app, "answering on port"), "the status says the game answers over BRP: {:#?}", words(&mut app));
    assert!(shows_part(&mut app, "Resources: "), "the resources are listed: {:#?}", words(&mut app));
    until(&mut app, "the frame time", |app| shows_part(app, "Frame time 16.7 ms"));

    // **Its components, when the entity is chosen** — read-only, as the game's own reflection says them.
    let row = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_cauldron::game_view::EntityButton)>();
        q.iter(world).next().map(|(e, _)| e).unwrap_or_else(|| panic!("the entity has no row"))
    };
    // The state list is far below the fold of the panel's scroll, so a pointer click would land on the clip; the row is
    // activated as the keyboard's Enter does.
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: row });
    app.update();
    until(&mut app, "the entity's components", |app| shows_part(app, "transform::Transform"));

    // **Stop** ends it, and the Game icon is clear.
    press(&mut app, PlayButton::Stop);
    assert!(ops_play::running_pid(&root).is_none(), "Stop ended the game");
    until(&mut app, "the status to say Stopped", |app| shows(app, "Stopped"));
    assert_eq!(game_badge(&mut app), 0, "a game stopped on purpose is not a crash, so the Game icon has no badge");
}

/// **A game that crashes keeps its exit code and its last output on screen, and the Game icon is badged** — in the
/// Map as in the Game workspace, so a person who is elsewhere is told.
#[test]
fn a_crash_stays_on_screen_with_its_exit_code_and_a_badge() {
    let root = project("crash", CRASH, free_port());
    let mut app = editor(&root);
    switch_to(&mut app, Workspace::Game);
    press(&mut app, PlayButton::Play);

    until(&mut app, "the crash to be shown", |app| shows_part(app, "exit code 3"));
    assert!(shows_part(&mut app, "no such entity"), "the last output stays on screen: {:#?}", words(&mut app));
    assert_eq!(game_badge(&mut app), 1, "a crashed game badges the Game icon");

    // **Elsewhere, the badge is still there**: the bar says so on the Map too.
    switch_to(&mut app, Workspace::Map);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(game_badge(&mut app), 1, "the crash is still badged on another workspace");

    // **Playing again clears it**: a new run is not the crash.
    switch_to(&mut app, Workspace::Game);
    std::fs::write(root.join("game.py"), GAME).unwrap_or_else(|e| panic!("{e}"));
    press(&mut app, PlayButton::Play);
    let pid = ops_play::running_pid(&root).unwrap_or_else(|| panic!("the second Play did not start"));
    let _cleanup = crate::play_starts_and_stops_the_game::KillGroupOnDrop(u64::from(pid));
    until(&mut app, "the badge to clear on a new run", |app| game_badge(app) == 0);
}
