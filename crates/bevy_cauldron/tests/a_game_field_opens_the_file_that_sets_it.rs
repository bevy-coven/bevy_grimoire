//! **A field in the Game workspace's state view opens the file that sets it** (NV-09, #386; NV-07, #384; series getting around, #377).
//!
//! The state view is read-only: the game is changed through its YAML, so the file stays the truth (P3). What a person needs from a field they
//! are looking at is the way to the line that sets it. This file plays a **stub game** (a python process that answers BRP the way `bevy_fairy`
//! does, with one named entity), chooses the entity, and measures:
//!
//! 1. **Each component an entity file sets has an Open file**, and one the files do not set has none — a button that goes nowhere is the dead
//!    link the series forbids.
//! 2. **Open file lands in the Files workspace on that entity file, at the line of the component.**

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ComputedNode;

use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::files;
use bevy_cauldron::game_view::{EntityButton, FieldFileButton};
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::ops_play::{self, PlayButton};
use bevy_cauldron::session::Session;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::CodeEditor;
use bevy_gooey::testing::click;

use crate::fixtures;

/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 0.1;
/// How long a test waits for something the stub process does at its own pace.
const WAIT: Duration = Duration::from_secs(60);
/// The wall-clock pause between frames while waiting.
const PAUSE: Duration = Duration::from_millis(15);

/// The entity file the stub's one entity is defined in. `Transform` is set on its fifth line (index 4); `Name` is not set by any file.
const ENTITY_FILE: &str = "game/stub.entities.yaml";
const ENTITIES: &str = "entities:\n  stub_player:\n    Health:\n      current: 10\n    Transform:\n      translation: [1.0, 2.0, 3.0]\n";

/// **The stub game**: logs, and answers the BRP methods the Game workspace reads.
const GAME: &str = r#"
import http.server, json, sys, threading, time
print("stub game booted", flush=True)
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
            result = ["bevy_time::time::Time<bevy_time::real::Real>"]
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

fn python() -> &'static str {
    if Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}")).local_addr().unwrap_or_else(|e| panic!("{e}")).port()
}

/// Removes the project's game log when the test ends, however it ends.
struct LogGuard(PathBuf);

impl Drop for LogGuard {
    fn drop(&mut self) {
        ops_play::remove_log(&self.0);
    }
}

fn project(tag: &str, port: u16) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("game_field_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("game.py"), GAME).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("play.yaml"), format!("version: 1\ncommand: [\"{}\", game.py, {}]\nport: {}\n", python(), port, port))
        .unwrap_or_else(|e| panic!("{e}"));
    let path = root.join(ENTITY_FILE);
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("{path:?}"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&path, ENTITIES).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    root
}

fn editor(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME_S)));
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(fixtures::trusted_store(root));
    for _ in 0..10 {
        app.update();
    }
    app
}

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

fn words(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
    q.iter(world)
        .filter(|(t, shown, node)| shown.get() && !t.0.trim().is_empty() && node.size().x > 0.0 && node.size().y > 0.0)
        .map(|(t, _, _)| t.0.trim().to_owned())
        .collect()
}

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

fn field_buttons(app: &mut App) -> Vec<(Entity, FieldFileButton)> {
    let world = app.world_mut();
    world.query::<(Entity, &FieldFileButton)>().iter(world).map(|(e, b)| (e, b.clone())).collect()
}

/// **Play the stub, choose its entity**, and wait until its components are read.
fn playing_with_the_entity_chosen(tag: &str) -> (App, PathBuf, LogGuard, crate::play_starts_and_stops_the_game::KillGroupOnDrop) {
    let root = project(tag, free_port());
    let log = LogGuard(root.clone());
    let mut app = editor(&root);
    switch_to(&mut app, Workspace::Game);
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(fixtures::trusted_store(&root));
    let play = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &PlayButton)>();
        q.iter(world).find(|(_, b)| **b == PlayButton::Play).map(|(e, _)| e).unwrap_or_else(|| panic!("no Play button"))
    };
    click(&mut app, play);
    for _ in 0..3 {
        app.update();
    }
    let pid = ops_play::running_pid(&root).unwrap_or_else(|| panic!("Play did not start the stub game"));
    let kill = crate::play_starts_and_stops_the_game::KillGroupOnDrop(u64::from(pid));
    until(&mut app, "the entity in the state view", |app| words(app).iter().any(|w| w == "stub_player"));
    let row = {
        let world = app.world_mut();
        world.query::<(Entity, &EntityButton)>().iter(world).next().map(|(e, _)| e).unwrap_or_else(|| panic!("the entity has no row"))
    };
    // The state list is far below the fold of the panel's scroll, so the row is activated as the keyboard's Enter does.
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: row });
    app.update();
    until(&mut app, "the entity's components", |app| words(app).iter().any(|w| w.contains("Transform")));
    (app, root, log, kill)
}

/// **Each component an entity file sets has an Open file, and the others have none.**
#[test]
fn a_component_an_entity_file_sets_has_an_open_file_and_one_it_does_not_set_has_none() {
    let (mut app, _root, _log, _kill) = playing_with_the_entity_chosen("rows");
    until(&mut app, "the Open file rows", |app| !field_buttons(app).is_empty());
    let buttons: Vec<FieldFileButton> = field_buttons(&mut app).into_iter().map(|(_, b)| b).collect();
    assert_eq!(buttons.len(), 1, "the game's `Transform` is set by the file and its `Name` by nothing: {buttons:?}");
    assert_eq!(buttons[0].rel, ENTITY_FILE, "the file that defines the entity");
    assert_eq!(buttons[0].line, 4, "the line (0-based) `Transform:` stands on in that file");
}

/// **Open file lands on the entity file at the component's line.**
#[test]
fn open_file_on_a_component_lands_in_files_at_its_line() {
    let (mut app, _root, _log, _kill) = playing_with_the_entity_chosen("lands");
    until(&mut app, "the Open file rows", |app| !field_buttons(app).is_empty());
    let (button, _) = field_buttons(&mut app).into_iter().next().unwrap_or_else(|| panic!("no Open file"));
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: button });
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "Open file leaves the Game workspace for the Files workspace");
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some(ENTITY_FILE), "on the entity file");
    let line = {
        let world = app.world_mut();
        world.query::<&CodeEditor>().iter(world).next().map(|e| e.buffer().caret().line)
    };
    assert_eq!(line, Some(4), "the caret is on the component's line, not at the top");
}
