//! **The agent self-test card's rows each fit on one line** (feedback-1794089256357; oracle `containment`).
//!
//! The note, on the card an `omp` agent's self test draws over the terminal: *"Please think through the UX of this panel. Each
//! item should fit on one line, or be handled appropriately."* The card was a fixed 320 px wide, so `OK Agent connected:
//! omp-coding-agent 1.0.0 · 22 tools` and `OK Agent self test: the editor is on launcher · 88 operations · a 2560x1406 picture`
//! broke onto a second line whose last word (`tools`, `picture`) stood alone at the card's left edge, under the mark. The
//! card now takes the width its longest row needs, up to the terminal's own, so every row is one line while there is room for
//! it, and the card stays inside the terminal's body.
//!
//! (The note's second request, a glowing indicator for whether something is live, is an enhancement and not part of this fix.)

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use serde_json::json;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::{self, AgentSetup};
use bevy_cauldron::terminal_pane::{TerminalAgentCard, TerminalBody, TerminalInvokeButton};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits.
const TRIES: usize = 100;

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

/// The card's rows' words, top to bottom; none while no card shows.
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

/// The rows of the card as laid out: `(words, height)` in physical px, with the heading first.
fn laid_out_rows(app: &mut App) -> Vec<(String, f32)> {
    let world = app.world_mut();
    let children = world
        .query_filtered::<&Children, With<TerminalAgentCard>>()
        .iter(world)
        .next()
        .map(|children| children.to_vec())
        .unwrap_or_default();
    children
        .iter()
        .filter_map(|child| {
            let text = world.get::<Text>(*child)?;
            let node = world.get::<ComputedNode>(*child)?;
            Some((text.0.clone(), node.size().y))
        })
        .collect()
}

/// A rect in physical px: `(min, max)`.
fn rect_of(app: &mut App, entity: Entity) -> (Vec2, Vec2) {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("placed").translation;
    (at - node.size() / 2.0, at + node.size() / 2.0)
}

#[test]
fn every_row_of_the_self_test_card_is_one_line_and_the_card_stays_inside_the_terminal() {
    let mut app = editor_with_an_agent();
    invoke_the_agent(&mut app);
    until(&mut app, "the card never said the agent started", |app| card_rows(app).iter().any(|row| row.starts_with("OK Agent started")));

    // What omp's own self test said in the capture, word for word.
    let token = token(&app);
    let at = now(&app);
    let known = terminal_agent::agent_hello(
        &mut app.world_mut().resource_mut::<Terminal>(),
        &json!({"token": token, "client": {"name": "omp-coding-agent", "version": "1.0.0"}, "tools": 22}),
        at,
    );
    assert_eq!(known, Ok(json!({"known": true})));
    let checks = json!([
        {"name": "state", "ok": true, "detail": "the editor is on launcher"},
        {"name": "operations", "ok": true, "detail": "88 operations"},
        {"name": "picture", "ok": true, "detail": "a 2560x1406 picture"},
    ]);
    let known = terminal_agent::self_test_result(&mut app.world_mut().resource_mut::<Terminal>(), &json!({"token": token, "checks": checks}));
    assert_eq!(known, Ok(json!({"known": true})));
    let longest = "OK Agent self test: the editor is on launcher · 88 operations · a 2560x1406 picture";
    until(&mut app, "the card never listed the self test's findings", |app| card_rows(app).iter().any(|row| row == longest));
    for _ in 0..4 {
        app.update();
    }

    let rows = laid_out_rows(&mut app);
    assert!(rows.len() >= 5, "the heading and the four checks are drawn: {rows:?}");
    let line = rows[0].1;
    assert!(line > 0.0, "the heading has a height: {rows:?}");
    for (words, height) in &rows {
        assert!(*height <= line * 1.05, "`{words}` is {height} px tall where one line is {line} px: it wraps");
    }

    let (card, body) = {
        let world = app.world_mut();
        let card = world.query_filtered::<Entity, With<TerminalAgentCard>>().iter(world).next().expect("the card");
        let body = world.query_filtered::<Entity, With<TerminalBody>>().iter(world).next().expect("the terminal's body");
        (card, body)
    };
    let (card, body) = (rect_of(&mut app, card), rect_of(&mut app, body));
    assert!(
        card.0.x >= body.0.x - 0.5 && card.1.x <= body.1.x + 0.5 && card.0.y >= body.0.y - 0.5 && card.1.y <= body.1.y + 0.5,
        "the card {card:?} lies inside the terminal's body {body:?}"
    );
}
