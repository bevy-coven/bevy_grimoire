//! **An agent that was running when Cauldron closed is started again, resuming its session** (`feedback-1794148425925`;
//! oracle `spec`).
//!
//! The note, from the editor reopened on a project with `omp` chosen and the terminal offering *Invoke Agent*: *"If there was
//! an agent in process, Cauldron should remember it and restart there. For example, capturing omp's resume ID on exit and
//! automaticaly resuming with that id on restart."*
//!
//! The omp extension Cauldron writes tells the editor which session its tab is in (`grimoire/agent_session`, here the body
//! of that method called with the tab's token); the editor keeps one record per project; the next time the project is open and
//! the terminal would offer *Invoke Agent*, the tab opens by itself and types `omp --resume <session>`. `omp` is a shell
//! function from the file `$ENV` names, so the line the editor typed is the line that ran, and the screen says what it was.
//! What ends an agent (its tab closed, omp leaving its session) takes the record with it; closing the editor does not.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use std::path::PathBuf;

use bevy_cauldron::agent_resume;
use bevy_cauldron::project_assets::ProjectRoot;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::{self, AgentSetup};
use bevy_cauldron::terminal_pane::{TerminalCloseTab, TerminalInvokeButton};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits for something to happen.
const TRIES: usize = 120;
/// A session as omp names one.
const SESSION: &str = "01a1bb82-fdc4-7343-83c6-b6dce13deffa";

/// **`SHELL` and `ENV` are process-wide**, so the editors here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// What one test shares between the editors it opens one after the other.
struct Machine {
    root: PathBuf,
    files: PathBuf,
    server: PathBuf,
    rc: PathBuf,
}

/// **A project with `omp` chosen, a folder for the agent's files that outlives each editor, and an `omp` that prints its
/// arguments.** The folder is the machine's, as `<data dir>/agents` is the person's.
fn machine(tag: &str) -> Machine {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(&file, "{version: 1, terminal_agent: OhMyPi}").unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let _ = std::fs::remove_dir_all(&files);
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let rc = std::env::temp_dir().join(format!("{tag}_rc.sh"));
    std::fs::write(&rc, "omp() { printf 'FAKE-OMP %s\\n' \"$*\"; }\n").unwrap_or_else(|e| panic!("{}: {e}", rc.display()));
    Machine { root, files, server, rc }
}

/// **The editor opened on the machine's project**: its terminal's shell is `/bin/sh`, which reads `$ENV` for `omp`.
fn open(machine: &Machine) -> App {
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe {
        std::env::set_var("SHELL", "/bin/sh");
        std::env::set_var("ENV", &machine.rc);
    }
    let mut app = bevy_cauldron::harness::build_headless(&machine.root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15797), server: machine.server.clone(), files: machine.files.clone() });
    app
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}");
}

/// One pointer action at `at`, in the physical pixels of the image the editor draws its interface into.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// A primary click at `at`.
fn click(app: &mut App, at: Vec2) {
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        point(app, at, action);
    }
}

/// Where the entity marked `M` stands, once it is laid out.
fn where_is<M: Component>(app: &mut App) -> Option<Vec2> {
    let world = app.world_mut();
    world.query_filtered::<&UiGlobalTransform, With<M>>().iter(world).next().map(|at| at.translation)
}

/// What the active tab's screen says.
fn screen_text(app: &App) -> String {
    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        return String::new();
    };
    tab.screen().built_lines().iter().map(|line| line.cells.iter().map(|c| c.ch).collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// The project as the editor holds it, which is the key of its record.
fn project(app: &App) -> PathBuf {
    app.world().resource::<ProjectRoot>().current().expect("the editor has a project open")
}

/// **Invoke the agent by its button, and wait until the line it typed has run.**
fn invoke(app: &mut App) {
    until(app, "the Invoke Agent button was never drawn", |app| where_is::<TerminalInvokeButton>(app).is_some());
    let button = where_is::<TerminalInvokeButton>(app).expect("the button stands");
    click(app, button);
    until(app, "the press never opened the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(app, "the agent's launch line never ran in its shell", |app| screen_text(app).contains("FAKE-OMP"));
}

/// **What the omp extension does**: tells the editor, with the tab's token, which session the agent is in (or that it left).
fn report(app: &mut App, session: Option<&str>) {
    let token = app.world().resource::<Terminal>().tab().and_then(|tab| tab.agent()).map(|post| post.token.clone()).expect("an agent tab");
    let said = terminal_agent::agent_session(
        &mut app.world_mut().resource_mut::<Terminal>(),
        &serde_json::json!({"token": token, "session": session}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(said["known"], true, "the tab's own token is known: {said}");
    for _ in 0..3 {
        app.update();
    }
}

/// The session the record holds for the editor's project.
fn remembered(machine: &Machine, app: &App) -> Option<String> {
    agent_resume::recall(&machine.files, &project(app)).unwrap_or_else(|e| panic!("{e}")).map(|entry| entry.session)
}

/// **A second editor on the same project offers `Invoke Agent` and starts nothing by itself.**
fn offers_the_button_and_starts_nothing(machine: &Machine) {
    let mut app = open(machine);
    until(&mut app, "the second editor never offered Invoke Agent", |app| where_is::<TerminalInvokeButton>(app).is_some());
    for _ in 0..30 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(app.world().resource::<Terminal>().is_empty(), "an agent nobody had running was started: {}", screen_text(&app));
}

#[test]
fn an_agent_running_when_the_editor_closed_is_resumed_in_its_session() {
    let machine = machine("resume");
    {
        let mut app = open(&machine);
        invoke(&mut app);
        assert_eq!(remembered(&machine, &app), None, "nothing is remembered before the agent says which session it is in");
        report(&mut app, Some(SESSION));
        assert_eq!(remembered(&machine, &app).as_deref(), Some(SESSION), "the session the agent reported is written down");
    }
    // The editor has closed: its shells were hung up, and nothing struck the record.
    let mut app = open(&machine);
    until(&mut app, "the project opened with an agent running when it closed, and no agent tab was started", |app| {
        app.world().resource::<Terminal>().len() == 1
    });
    let resuming = app.world().resource::<Terminal>().tab().and_then(|tab| tab.agent()).and_then(|post| post.resuming.clone());
    assert_eq!(resuming.as_deref(), Some(SESSION), "the tab was not started to resume the remembered session");
    let resumed = format!("--resume {SESSION}");
    until(&mut app, "the resumed agent's launch line never ran", |app| screen_text(app).contains("FAKE-OMP"));
    let screen = screen_text(&app);
    assert!(screen.contains(&resumed), "omp was started without resuming the session: {screen}");
    assert!(!screen.contains("power-on self test"), "a resumed session was asked its self test again: {screen}");
}

#[test]
fn an_agent_the_person_closed_is_not_resumed() {
    let machine = machine("closed");
    {
        let mut app = open(&machine);
        invoke(&mut app);
        report(&mut app, Some(SESSION));
        assert_eq!(remembered(&machine, &app).as_deref(), Some(SESSION));
        let close = where_is::<TerminalCloseTab>(&mut app).expect("the bar has a ×");
        click(&mut app, close);
        until(&mut app, "closing the agent's tab never emptied the terminal", |app| app.world().resource::<Terminal>().is_empty());
        assert_eq!(remembered(&machine, &app), None, "the person ended the agent, and its record is still there");
    }
    offers_the_button_and_starts_nothing(&machine);
}

#[test]
fn an_agent_that_left_its_session_is_not_resumed() {
    let machine = machine("left");
    {
        let mut app = open(&machine);
        invoke(&mut app);
        report(&mut app, Some(SESSION));
        report(&mut app, None);
        assert_eq!(remembered(&machine, &app), None, "omp left its session, and its record is still there");
    }
    offers_the_button_and_starts_nothing(&machine);
}

#[test]
fn an_agent_that_never_said_its_session_leaves_nothing_to_resume() {
    let machine = machine("silent");
    {
        let mut app = open(&machine);
        invoke(&mut app);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(remembered(&machine, &app), None);
    }
    offers_the_button_and_starts_nothing(&machine);
}
