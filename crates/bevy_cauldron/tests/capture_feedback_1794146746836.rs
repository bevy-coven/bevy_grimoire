//! **The empty-map prompt goes away while an agent is invoked** (feedback-1794146746836; oracle `spec`).
//!
//! The note, from the editor with `omp` running in the terminal below an empty map: *"This should go away if if I invoke an
//! agent."* The pick is a scene point just under the viewport's one overlay: the sentence *Place a piece from the library*
//! and its *Open library* button, which stand while the map has no placements. An agent that has been invoked is who is
//! placing (the map in the picture holds a forest it built and still says `0 placed`, because trees are not placements), so a
//! sentence that sends the person to the library is in the way. It comes back when the agent's tab is closed.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::empty_state::EmptyMapPrompt;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{TerminalCloseTab, TerminalInvokeButton};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits for something to happen.
const TRIES: usize = 120;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project with a piece in its library and **no placement on its map**, which chooses a custom agent.
fn editor_with_an_empty_map_and_an_agent(tag: &str) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(&file, r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#)
        .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15797), server, files });
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

/// How many prompts stand over the map.
fn standing(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<(), With<EmptyMapPrompt>>().iter(world).count()
}

#[test]
fn the_empty_map_prompt_goes_while_an_agent_is_invoked_and_comes_back_with_its_tab() {
    let mut app = editor_with_an_empty_map_and_an_agent("prompt_agent");
    until(&mut app, "the Invoke Agent button was never drawn", |app| where_is::<TerminalInvokeButton>(app).is_some());
    assert_eq!(standing(&mut app), 1, "an empty map with an agent chosen but not invoked says what to do next");

    let button = where_is::<TerminalInvokeButton>(&mut app).expect("the button stands");
    click(&mut app, button);
    until(&mut app, "the press never opened the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(&mut app, "an agent is invoked and the map still tells the person to place a piece from the library", |app| {
        standing(app) == 0
    });

    let close = where_is::<TerminalCloseTab>(&mut app).expect("the bar has a ×");
    click(&mut app, close);
    until(&mut app, "closing the agent's tab never emptied the terminal", |app| app.world().resource::<Terminal>().is_empty());
    until(&mut app, "the agent is gone and the empty map never says what to do next again", |app| standing(app) == 1);
}
