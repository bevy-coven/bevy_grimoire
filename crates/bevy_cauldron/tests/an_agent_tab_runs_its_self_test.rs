//! **An agent tab types its harness at the shell's prompt, and its self-test card shows each check as it
//! passes** (`terminal_agent`).
//!
//! The project's settings choose a custom command, so the harness is a `printf` the shell really runs; the
//! MCP server's two calls are made the way its BRP methods make them (`terminal_agent::agent_hello`,
//! `terminal_agent::self_test_result`), with the token the tab minted.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use serde_json::json;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::{self, AgentSetup, CheckStatus};
use bevy_cauldron::terminal_pane::{TerminalAgentCard, TerminalInvokeButton, TerminalNewTab};
use bevy_gooey::testing::find_by_label;

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits.
const TRIES: usize = 100;
/// What the harness prints. Its command line spells it apart (`AGENT-%s`), so the echo of the typed line
/// is not mistaken for its output.
const SAID: &str = "AGENT-UP";
const TOOLS: usize = 22;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project whose settings choose a custom agent, with an agent setup whose server is a
/// file that exists.
fn editor_with_an_agent() -> App {
    let tag = format!("agent_tab_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    let settings = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#;
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
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

fn screen_text(app: &App) -> String {
    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        return String::new();
    };
    tab.screen()
        .built_lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The card's rows, top to bottom; none while no card shows.
fn card_rows(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let children = world
        .query_filtered::<&Children, With<TerminalAgentCard>>()
        .iter(world)
        .next()
        .map(|children| children.to_vec())
        .unwrap_or_default();
    children.iter().filter_map(|child| world.get::<Text>(*child).map(|text| text.0.clone())).collect()
}

fn token(app: &App) -> String {
    let terminal = app.world().resource::<Terminal>();
    terminal.tab().and_then(|tab| tab.agent()).map(|post| post.token.clone()).expect("the first tab is the agent's")
}

fn now(app: &App) -> f64 {
    app.world().resource::<Time<Real>>().elapsed_secs_f64()
}

/// One pointer action at `at`, in the physical pixels of the image the editor draws its interface into.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// **The terminal starts no agent by itself** (it costs tokens to warm up), so the test presses `Invoke
/// Agent` the way a person does, through the pointer chain, before it looks at the agent's tab.
fn invoke_the_agent(app: &mut App) {
    let button = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().map(|at| at.translation)
    };
    until(app, "the Invoke Agent button was never drawn", |app| button(app).is_some());
    let at = button(app).expect("the button stands");
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        point(app, at, action);
    }
}

#[test]
fn an_agent_tab_types_its_command_then_shows_the_connection_and_the_test() {
    let mut app = editor_with_an_agent();
    invoke_the_agent(&mut app);
    until(&mut app, "the custom command was never typed into the first tab's shell", |app| {
        screen_text(app).lines().any(|line| line.trim() == SAID)
    });
    {
        let terminal = app.world().resource::<Terminal>();
        let post = terminal.tab().and_then(|tab| tab.agent()).expect("the first tab is an agent tab");
        assert_eq!(post.checks[0], CheckStatus::Passed("bevy_cauldron_mcp on port 15799".to_owned()));
        assert_eq!(post.checks[1], CheckStatus::Passed("typed printf into the shell".to_owned()));
        assert_eq!(post.checks[2], CheckStatus::Pending);
    }
    until(&mut app, "the card never said the agent started", |app| {
        card_rows(app).iter().any(|row| row.starts_with("OK Agent started"))
    });

    let token = token(&app);
    let at = now(&app);
    let known = terminal_agent::agent_hello(
        &mut app.world_mut().resource_mut::<Terminal>(),
        &json!({"token": token, "client": {"name": "test-client", "version": "1.0"}, "tools": TOOLS}),
        at,
    );
    assert_eq!(known, Ok(json!({"known": true})));
    let stranger = terminal_agent::agent_hello(
        &mut app.world_mut().resource_mut::<Terminal>(),
        &json!({"token": "not-a-tab", "client": null, "tools": TOOLS}),
        at,
    );
    assert_eq!(stranger, Ok(json!({"known": false})), "another server's hello changes no tab");
    let connected = format!("OK Agent connected: test-client 1.0 · {TOOLS} tools");
    until(&mut app, "the card never said the agent connected", |app| card_rows(app).contains(&connected));

    let checks = json!([
        {"name": "state", "ok": true, "detail": "the editor is on editor"},
        {"name": "operations", "ok": true, "detail": "90 operations"},
        {"name": "picture", "ok": true, "detail": "a 1280x720 picture"},
    ]);
    let known = terminal_agent::self_test_result(&mut app.world_mut().resource_mut::<Terminal>(), &json!({"token": token, "checks": checks}));
    assert_eq!(known, Ok(json!({"known": true})));
    let chip = format!("agent connected · {TOOLS} tools");
    until(&mut app, "the bar never showed the connected agent", |app| find_by_label(app, &chip).is_ok());
    assert!(
        card_rows(&mut app).contains(&"OK Agent self test: the editor is on editor · 90 operations · a 1280x720 picture".to_owned()),
        "the card lists the self test's three findings: {:?}",
        card_rows(&mut app)
    );

    // **`+` is clicked through `bevy_picking`'s own input** on the image the editor draws into, as
    // `a_click_puts_the_keyboard_in_the_terminal` does, so the click crosses the whole pointer chain.
    let plus = {
        let world = app.world_mut();
        world
            .query_filtered::<&UiGlobalTransform, With<TerminalNewTab>>()
            .iter(world)
            .next()
            .map(|at| at.translation)
            .expect("the bar has a +")
    };
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        point(&mut app, plus, action);
    }
    until(&mut app, "+ never opened a second tab", |app| app.world().resource::<Terminal>().len() == 2);
    let terminal = app.world().resource::<Terminal>();
    assert!(terminal.tab().is_some_and(|tab| tab.agent().is_none()), "+ opens a plain shell");
}
