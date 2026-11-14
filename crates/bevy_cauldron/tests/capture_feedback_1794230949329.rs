//! **Every chip in the terminal's bar says what it does, and an agent is not offered a `cd`** (feedback-1794230949329; oracle
//! `other:chip-explains-itself`).
//!
//! The note, from the editor with `omp` drawn in the terminal and the project opened after it started: *"What are the different
//! chips on here for? It's unclear. And "cd lavender_hills" doesn't make a lot of sense."* The pick is the bar. It held
//! `omp connected · 22 tools`, `omp`, `cd lavender_hills`, `Agent`, `+` and `x`: each a name and none a sentence, and a shell
//! command on the agent's tab, where the keystroke it types goes into the agent's prompt as a message.
//!
//! Two relations, on a real editor through `app.update()`:
//!
//! - **Every chip carries a tooltip that says more than its label does**, for each of the six kinds the bar draws.
//! - **The move chip is a shell's, and says where it goes in words**: with an agent tab drawn it is not offered (the pane would
//!   offer it to a shell in the same state), and on a shell it reads `Go to <folder>`, not `cd <folder>`, with the command it
//!   types in its tooltip.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{
    TerminalAgentChip, TerminalAgentStatusChip, TerminalCdChip, TerminalCloseTab, TerminalInvokeButton, TerminalNewTab, TerminalTabChip,
};
use bevy_gooey::Tooltip;

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits for something to happen.
const TRIES: usize = 200;

/// A tooltip that is a sentence has at least this many words; a label echoed back as one has fewer.
const MIN_WORDS: usize = 2;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project that chooses a custom agent, whose server is a file that exists.
fn editor_with_an_agent(tag: &str) -> App {
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
    app.insert_resource(AgentSetup { port: Some(15801), server, files });
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
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        point(app, at, action);
    }
}

/// Where the entity marked `M` stands, once it is laid out.
fn where_is<M: Component>(app: &mut App) -> Option<Vec2> {
    let world = app.world_mut();
    world.query_filtered::<&UiGlobalTransform, With<M>>().iter(world).next().map(|at| at.translation)
}

/// Where the bar's chip for tab `index` stands.
fn where_is_tab(app: &mut App, index: usize) -> Option<Vec2> {
    let world = app.world_mut();
    world.query::<(&TerminalTabChip, &UiGlobalTransform)>().iter(world).find(|(chip, _)| chip.0 == index).map(|(_, at)| at.translation)
}

/// A chip's words: its first child that says something, which is its label (the tooltip's box is added after it).
fn label_of(world: &World, chip: Entity) -> String {
    world
        .get::<Children>(chip)
        .and_then(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|t| t.0.clone())))
        .unwrap_or_default()
}

/// Every chip marked `M`, as its label and its tooltip's line (empty when it has no tooltip).
fn chips_of<M: Component>(app: &mut App) -> Vec<(String, String)> {
    let world = app.world_mut();
    let chips: Vec<(Entity, Option<String>)> = world.query_filtered::<(Entity, Option<&Tooltip>), With<M>>().iter(world).map(|(e, t)| (e, t.map(|t| t.text.clone()))).collect();
    chips.into_iter().map(|(e, tip)| (label_of(world, e), tip.unwrap_or_default())).collect()
}

/// **A chip says what it does**: it stands, and its tooltip is a sentence that is not its own label.
fn assert_explained<M: Component>(app: &mut App, name: &str) {
    let chips = chips_of::<M>(app);
    assert!(!chips.is_empty(), "the bar has no {name} chip");
    for (label, tip) in chips {
        assert!(
            tip.split_whitespace().count() >= MIN_WORDS && tip != label,
            "the {name} chip {label:?} does not say what it does: its tooltip is {tip:?}"
        );
    }
}

/// A scratch directory standing for "the project the editor is pointed at now", away from where the tabs started.
fn elsewhere(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-capture-1794230949329-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **The capture's screen**: the agent started where the project was, and the editor is now pointed at another one. A shell is
/// opened there (the bar redraws on a tab opening) and the agent's tab is drawn again.
fn an_agent_tab_drawn_while_the_editor_points_elsewhere(app: &mut App, other: &std::path::Path) {
    until(app, "the Invoke Agent button was never drawn", |app| where_is::<TerminalInvokeButton>(app).is_some());
    let invoke = where_is::<TerminalInvokeButton>(app).expect("the button stands");
    click(app, invoke);
    until(app, "the press never opened the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(app, "the agent's status chip never stood", |app| where_is::<TerminalAgentStatusChip>(app).is_some());
    app.world_mut().resource_mut::<Terminal>().set_next_cwd(Some(other.to_path_buf()));
    let plus = where_is::<TerminalNewTab>(app).expect("the bar has a +");
    click(app, plus);
    until(app, "the + never opened a second tab", |app| app.world().resource::<Terminal>().len() == 2);
    until(app, "the first tab's chip never stood", |app| where_is_tab(app, 0).is_some());
    let first = where_is_tab(app, 0).expect("the agent's tab chip");
    click(app, first);
    until(app, "the agent's tab was never drawn again", |app| {
        let terminal = app.world().resource::<Terminal>();
        terminal.active() == 0 && terminal.tab().is_some_and(|tab| tab.agent().is_some())
    });
}

#[test]
fn every_chip_of_an_agent_tab_says_what_it_does_and_the_agent_is_offered_no_cd() {
    let mut app = editor_with_an_agent("chips_agent");
    let other = elsewhere("agent");
    an_agent_tab_drawn_while_the_editor_points_elsewhere(&mut app, &other);

    let terminal = app.world().resource::<Terminal>();
    let tab = terminal.tab().expect("a tab is drawn");
    assert!(tab.agent().is_some(), "the drawn tab is the agent's");
    assert!(tab.wants_cd_to(&other), "the pane would offer a shell in this state the move: otherwise the test below shows nothing");

    assert_explained::<TerminalAgentStatusChip>(&mut app, "status");
    assert_explained::<TerminalTabChip>(&mut app, "tab");
    assert_explained::<TerminalAgentChip>(&mut app, "Agent");
    assert_explained::<TerminalNewTab>(&mut app, "+");
    assert_explained::<TerminalCloseTab>(&mut app, "x");
    let moves = chips_of::<TerminalCdChip>(&mut app);
    assert!(moves.is_empty(), "an agent's tab is offered a shell command: {moves:?}");
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn the_move_chip_of_a_shell_reads_go_to_the_folder_and_shows_the_command_in_its_tooltip() {
    let mut app = editor_with_an_agent("chips_shell");
    let other = elsewhere("shell");
    until(&mut app, "the bar's + was never drawn", |app| where_is::<TerminalNewTab>(app).is_some());
    let plus = where_is::<TerminalNewTab>(&mut app).expect("the bar has a +");
    click(&mut app, plus);
    until(&mut app, "the + never opened a shell", |app| app.world().resource::<Terminal>().len() == 1);
    app.world_mut().resource_mut::<Terminal>().set_next_cwd(Some(other.clone()));
    let plus = where_is::<TerminalNewTab>(&mut app).expect("the bar has a +");
    click(&mut app, plus);
    until(&mut app, "the second + never opened a shell", |app| app.world().resource::<Terminal>().len() == 2);
    until(&mut app, "the first shell's chip never stood", |app| where_is_tab(app, 0).is_some());
    let first = where_is_tab(&mut app, 0).expect("the first shell's chip");
    click(&mut app, first);
    until(&mut app, "the first shell was never drawn again", |app| app.world().resource::<Terminal>().active() == 0);
    let tab = app.world().resource::<Terminal>().tab().expect("a tab is drawn");
    assert!(tab.agent().is_none() && tab.wants_cd_to(&other), "the drawn tab is a shell that started elsewhere");

    let folder = other.file_name().expect("a folder name").to_string_lossy().into_owned();
    let moves = chips_of::<TerminalCdChip>(&mut app);
    assert_eq!(moves.len(), 1, "a shell started elsewhere is offered the move: {moves:?}");
    let (label, tip) = &moves[0];
    assert_eq!(label, &format!("Go to {folder}"), "the chip says where it goes in words, not as a command");
    assert!(tip.contains("cd ") && tip.contains(&folder), "its tooltip shows the command it types and where it goes: {tip:?}");

    assert_explained::<TerminalTabChip>(&mut app, "tab");
    assert_explained::<TerminalCdChip>(&mut app, "Go to");
    assert_explained::<TerminalNewTab>(&mut app, "+");
    assert_explained::<TerminalCloseTab>(&mut app, "x");
    let _ = std::fs::remove_dir_all(&other);
}
