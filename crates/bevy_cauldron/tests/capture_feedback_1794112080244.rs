//! **A shell told to `exit` closes its terminal, and another can be made** (feedback-1794112080244; oracle `spec`).
//!
//! The note, from the launcher with `omp` as the chosen agent: *"If I exit omp, then type "exit" from the shell, it
//! freezes. We should close this terminal, then allow the user to create another one."* Leaving `omp` leaves the tab at
//! the shell it was typed into; `exit` then ended that shell, and the pane went on showing a dead one: the bar kept the
//! tab's chip, the caret sat under the word `exit`, a key reached no one. Nothing in the editor looked at the child.
//!
//! What the person asks is what every terminal does: the tab goes with its shell, and the pane is a pane with nothing in
//! it, which here (an agent chosen, nothing open) is the `Invoke Agent` button and the `+` chip that opens a plain shell.
//!
//! The editor is the real one with a real shell on a real PTY (`/bin/sh`), and the agent a command that prints and returns
//! to the prompt, as `omp` does when it is left.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalInvokeButton, TerminalNewTab};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits for something to happen.
const TRIES: usize = 120;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project that chooses an agent whose command prints `AGENT-UP` and returns to the shell's prompt.
fn editor_with_an_agent(tag: &str) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(
        &file,
        r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#,
    )
    .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15798), server, files });
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

/// Where the `Invoke Agent` button stands, once it is laid out; `None` while there is none.
fn the_button(app: &mut App) -> Option<Vec2> {
    let world = app.world_mut();
    world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().map(|at| at.translation)
}

/// What the active tab's emulator holds, as one string.
fn the_screen(app: &mut App) -> String {
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    let Some(tab) = terminal.tab_mut() else {
        return String::new();
    };
    tab.screen_mut().lines().iter().map(|line| line.cells.iter().map(|c| c.ch).collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// Every word drawn inside the pane's body: the rows of the shell on show, if any.
fn drawn_in_the_body(app: &mut App) -> String {
    let world = app.world_mut();
    let bodies: Vec<Entity> = world.query_filtered::<Entity, With<TerminalBody>>().iter(world).collect();
    let mut words = Vec::new();
    let mut texts = world.query::<(Entity, &Text, &ComputedNode)>();
    for (entity, text, node) in texts.iter(world) {
        if node.size().x <= 0.0 || text.0.trim().is_empty() {
            continue;
        }
        if world.entity(entity).get::<ChildOf>().is_some() && bodies.iter().any(|body| is_under(world, entity, *body)) {
            words.push(text.0.clone());
        }
    }
    words.join(" ")
}

fn is_under(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    while let Some(parent) = world.entity(entity).get::<ChildOf>() {
        entity = parent.parent();
        if entity == ancestor {
            return true;
        }
    }
    false
}

#[test]
fn a_shell_told_to_exit_closes_its_terminal_and_another_can_be_made() {
    let mut app = editor_with_an_agent("exit_closes");
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());
    let button = the_button(&mut app).expect("the button stands");
    click(&mut app, button);
    until(&mut app, "the press never opened the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
    // The agent has run and returned to the prompt, as omp does when it is left.
    until(&mut app, "the agent's command never ran", |app| the_screen(app).contains("AGENT-UP"));

    // The person types `exit` at that shell.
    app.world_mut().resource_mut::<Terminal>().write(b"exit\n");
    until(&mut app, "the shell was told `exit` and its terminal is still open", |app| app.world().resource::<Terminal>().is_empty());

    // The pane is a pane with nothing in it: the button that starts the agent, and none of the shell that left.
    until(&mut app, "the Invoke Agent button never came back", |app| the_button(app).is_some());
    let left_behind = drawn_in_the_body(&mut app);
    assert!(!left_behind.contains("AGENT-UP"), "the closed terminal's rows are still drawn in the body: {left_behind:?}");

    // And another can be made: the `+` chip opens a plain shell that is up.
    let plus = {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalNewTab>>().iter(world).next().map(|at| at.translation).expect("the bar has a +")
    };
    click(&mut app, plus);
    until(&mut app, "the + never opened another terminal", |app| app.world().resource::<Terminal>().len() == 1);
    let terminal = app.world().resource::<Terminal>();
    assert!(terminal.tab().is_some_and(|tab| tab.problem().is_none()), "the new terminal has a shell: {:?}", terminal.tab().and_then(|tab| tab.problem()));
}
