//! **The terminal's bar reads the same to a first-time reader as to the person who built it** (#423; the owner: *"Agent Off,
//! lavender_hills, agent, Agent, wtf are all of these?"*).
//!
//! The bar read `Agent off`, a chip named for the project folder, a chip called `agent`, a button called `Agent`, `+` and `x`: three
//! things that said "agent" and meant three things. What it holds now:
//!
//! - a tab is named for the program running in it (`sh`, or the agent's own word), never for the folder;
//! - the agent's state is one LED with one word, and a failure's reason is the LED's tooltip, not a chip;
//! - the button that starts an agent says `Start agent`;
//! - `x` carries its chord in its tooltip;
//! - no label of the bar is repeated, in any agent state;
//! - a narrow bar gives items up in a stated order and never clips one.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use serde_json::json;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::{self, AgentSetup};
use bevy_cauldron::terminal_pane::{
    BAR_GIVES_UP, BarPlan, BarWants, TerminalAgentChip, TerminalAgentStatusChip, TerminalBar, TerminalCloseTab, TerminalInvokeButton, TerminalNewTab, TerminalTabChip, bar_plan,
    chord_words, close_tab_chord, tab_names,
};
use bevy_gooey::{Led, LedWord, Tooltip};

use crate::fixtures;

const TRIES: usize = 200;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project whose settings are `settings`, with the pane raised (it starts collapsed, #424) and an agent setup whose
/// server is a file that exists. Returns the project's folder name too.
fn editor(tag: &str, settings: &str) -> (App, String) {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    let folder = root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    (app, folder)
}

const AGENT_CUSTOM: &str = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#;
const NO_AGENT: &str = "{version: 1, terminal_agent: Off}";

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

fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

fn press_the_button(app: &mut App) {
    let button = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().map(|at| at.translation)
    };
    until(app, "the Start agent button was never drawn", |app| button(app).is_some());
    let at = button(app).expect("the button stands");
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        point(app, at, action);
    }
}

/// Every word the bar draws, left to right, as the entities carry them; empty strings are drawn as nothing and are left out.
fn labels(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let bar = world.query_filtered::<Entity, With<TerminalBar>>().iter(world).next().expect("the terminal's bar");
    let mut out = Vec::new();
    let mut stack = vec![bar];
    while let Some(at) = stack.pop() {
        // A tooltip is a box that stands beside its control, not a label of the bar: it is read separately.
        if world.get::<bevy_gooey::Role>(at) == Some(&bevy_gooey::Role::Tooltip) {
            continue;
        }
        if let Some(text) = world.get::<Text>(at)
            && !text.0.is_empty()
            && world.get::<Node>(at).is_none_or(|node| node.display != Display::None)
        {
            out.push((at, text.0.clone()));
        }
        if let Some(kids) = world.get::<Children>(at) {
            stack.extend(kids.iter().rev());
        }
    }
    out.into_iter().map(|(_, words)| words).collect()
}

/// What the bar says, and the project's folder name, as a failure message.
fn the_bar(app: &mut App) -> String {
    format!("{:?}", labels(app))
}

fn assert_no_label_repeats(app: &mut App, folder: &str, state: &str) {
    let all = labels(app);
    for (at, word) in all.iter().enumerate() {
        assert!(!all[..at].contains(word), "{state}: the bar repeats {word:?}: {all:?}");
        assert!(word != folder, "{state}: a tab is named for its project's folder {folder:?}: {all:?}");
        assert!(word != "agent", "{state}: a bare `agent` label, which says nothing about what it is: {all:?}");
    }
}

fn tab_words(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let chips: Vec<(usize, Entity)> = world.query::<(Entity, &TerminalTabChip)>().iter(world).map(|(entity, chip)| (chip.0, entity)).collect();
    let mut chips = chips;
    chips.sort();
    chips
        .into_iter()
        .map(|(_, entity)| world.get::<Children>(entity).and_then(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|text| text.0.clone()))).unwrap_or_default())
        .collect()
}

fn led_word(app: &mut App) -> Option<String> {
    let world = app.world_mut();
    let bar = world.query_filtered::<Entity, With<TerminalBar>>().iter(world).next()?;
    let mut stack = vec![bar];
    while let Some(at) = stack.pop() {
        if world.get::<Led>(at).is_some() {
            let kids = world.get::<Children>(at)?.to_vec();
            return kids.iter().find_map(|kid| world.get::<LedWord>(*kid).and_then(|_| world.get::<Text>(*kid)).map(|text| text.0.clone()));
        }
        if let Some(kids) = world.get::<Children>(at) {
            stack.extend(kids.iter());
        }
    }
    None
}

fn token(app: &App) -> String {
    app.world().resource::<Terminal>().tab().and_then(|tab| tab.agent()).map(|post| post.token.clone()).expect("the first tab is the agent's")
}

fn hello_and_self_test(app: &mut App, passing: bool) {
    let token = token(app);
    let at = app.world().resource::<Time<Real>>().elapsed_secs_f64();
    let known = terminal_agent::agent_hello(
        &mut app.world_mut().resource_mut::<Terminal>(),
        &json!({"token": token, "client": {"name": "omp-coding-agent", "version": "1.0.0"}, "tools": 22}),
        at,
    );
    assert_eq!(known, Ok(json!({"known": true})));
    let checks = json!([
        {"name": "state", "ok": true, "detail": "the editor is on launcher"},
        {"name": "picture", "ok": passing, "detail": if passing { "a 2560x1406 picture" } else { "the picture came back empty" }},
    ]);
    let known = terminal_agent::self_test_result(&mut app.world_mut().resource_mut::<Terminal>(), &json!({"token": token, "checks": checks}));
    assert_eq!(known, Ok(json!({"known": true})));
}

/// **A headless test reads the bar's labels in each agent state (none, off, busy, live, failed) and finds no label repeated and each
/// tab named for its program.**
#[test]
fn no_label_of_the_bar_repeats_in_any_agent_state_and_each_tab_is_named_for_its_program() {
    // None: no agent is chosen. The shell is a tab named `sh`, and there is no LED and no Start agent button.
    let (mut none, folder) = editor("bar_none", NO_AGENT);
    until(&mut none, "a shell to open", |app| app.world().resource::<Terminal>().len() == 1);
    updates(&mut none, 6);
    assert_eq!(tab_words(&mut none), ["sh"], "a shell tab is named for its program: {}", the_bar(&mut none));
    assert!(led_word(&mut none).is_none(), "no agent is chosen, so there is no LED: {}", the_bar(&mut none));
    assert_no_label_repeats(&mut none, &folder, "none");
    drop(none);

    // Off: an agent is chosen and nothing is running. One LED, one word, `off`; no tabs, so no tab chip says `agent`.
    let (mut off, folder) = editor("bar_off", AGENT_CUSTOM);
    until(&mut off, "the invitation", |app| led_word(app).is_some());
    updates(&mut off, 6);
    assert_eq!(led_word(&mut off).as_deref(), Some("off"), "{}", the_bar(&mut off));
    assert_no_label_repeats(&mut off, &folder, "off");

    // Busy: the agent's tab is named for the command's program, and the LED says busy.
    press_the_button(&mut off);
    until(&mut off, "the agent tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(&mut off, "the LED to be busy", |app| led_word(app).as_deref() == Some("busy"));
    assert_eq!(tab_words(&mut off), ["printf"], "an agent tab is named for the program it runs: {}", the_bar(&mut off));
    assert_no_label_repeats(&mut off, &folder, "busy");
    let starts: Vec<String> = {
        let world = off.world_mut();
        world
            .query_filtered::<&Children, With<TerminalAgentChip>>()
            .iter(world)
            .filter_map(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|text| text.0.clone())))
            .collect()
    };
    assert_eq!(starts, ["Start agent"], "the button that starts an agent says what it does");

    // Live.
    hello_and_self_test(&mut off, true);
    until(&mut off, "the LED to be live", |app| led_word(app).as_deref() == Some("live"));
    assert_no_label_repeats(&mut off, &folder, "live");

    // Failed: the reason is the LED's tooltip and not a label of the bar.
    {
        let token = token(&off);
        let known = terminal_agent::self_test_result(
            &mut off.world_mut().resource_mut::<Terminal>(),
            &json!({"token": token, "checks": [{"name": "picture", "ok": false, "detail": "the picture came back empty"}]}),
        );
        assert_eq!(known, Ok(json!({"known": true})));
    }
    until(&mut off, "the LED to be failed", |app| led_word(app).as_deref() == Some("failed"));
    assert_no_label_repeats(&mut off, &folder, "failed");
    let all = labels(&mut off);
    assert!(!all.iter().any(|word| word.contains("empty") || word.contains("self test")), "a failure's reason is not a chip of the bar: {all:?}");
    let tip = {
        let world = off.world_mut();
        world.query_filtered::<&Tooltip, With<TerminalAgentStatusChip>>().iter(world).next().map(|tip| tip.text.clone()).expect("the LED's control has a tooltip")
    };
    assert!(tip.contains("the picture came back empty"), "the reason is on the LED's tooltip: {tip:?}");
}

/// **Several shell tabs are numbered, so none reads like another.**
#[test]
fn two_shells_of_one_program_are_told_apart() {
    assert_eq!(tab_names(&["sh", "sh", "zsh", "sh"]), ["sh", "sh 2", "zsh", "sh 3"]);
    assert_eq!(tab_names(&["omp"]), ["omp"]);
    let (mut app, folder) = editor("bar_two", NO_AGENT);
    until(&mut app, "a shell to open", |app| app.world().resource::<Terminal>().len() == 1);
    let plus = {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalNewTab>>().iter(world).next().map(|at| at.translation).expect("the bar has a +")
    };
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        point(&mut app, plus, action);
    }
    until(&mut app, "a second shell", |app| app.world().resource::<Terminal>().len() == 2);
    updates(&mut app, 6);
    assert_eq!(tab_words(&mut app), ["sh", "sh 2"], "{}", the_bar(&mut app));
    assert_no_label_repeats(&mut app, &folder, "two shells");
}

/// **`x` has its chord in its tooltip**, and `+` says what it does.
#[test]
fn the_x_chip_carries_its_chord_and_the_plus_says_where_the_shell_starts() {
    let (mut app, _) = editor("bar_tips", NO_AGENT);
    until(&mut app, "a shell to open", |app| app.world().resource::<Terminal>().len() == 1);
    updates(&mut app, 6);
    let world = app.world_mut();
    let close = world.query_filtered::<&Tooltip, With<TerminalCloseTab>>().iter(world).next().map(|tip| tip.text.clone()).expect("x has a tooltip");
    let chord = chord_words(&close_tab_chord());
    assert!(close.contains(&chord), "x names its chord {chord:?}: {close:?}");
    let plus = world.query_filtered::<&Tooltip, With<TerminalNewTab>>().iter(world).next().map(|tip| tip.text.clone()).expect("+ has a tooltip");
    assert!(plus.to_lowercase().contains("new shell"), "{plus:?}");
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn wants(tabs: &[&str], go_to: bool, led: bool, start_agent: bool) -> BarWants {
    BarWants { tabs: tabs.iter().map(|tab| (*tab).to_owned()).collect(), active: 0, go_to: go_to.then(|| "Go to the_project".to_owned()), led, start_agent }
}

/// **A narrow bar gives items up in a stated order rather than clipping them.**
#[test]
fn a_narrow_bar_gives_items_up_in_the_stated_order() {
    assert_eq!(BAR_GIVES_UP[0], "the Go to chip");
    let full = wants(&["sh", "sh 2", "sh 3"], true, true, true);

    // Room to spare, or a bar not yet laid out: everything stays.
    let everything = BarPlan { tabs: vec![0, 1, 2], go_to: true, start_agent: true, led_name: true };
    assert_eq!(bar_plan(None, &full), everything);
    assert_eq!(bar_plan(Some(2000.0), &full), everything);

    // Walk the room down: what is held only ever shrinks, and it shrinks in the stated order.
    let mut last = everything.clone();
    let mut order = Vec::new();
    let mut room = 2000.0;
    while room > 0.0 {
        let plan = bar_plan(Some(room), &full);
        if plan != last {
            if last.go_to && !plan.go_to {
                order.push(BAR_GIVES_UP[0]);
            }
            if last.start_agent && !plan.start_agent {
                order.push(BAR_GIVES_UP[1]);
            }
            if last.led_name && !plan.led_name {
                order.push(BAR_GIVES_UP[2]);
            }
            if plan.tabs.len() < last.tabs.len() {
                order.push(BAR_GIVES_UP[3]);
            }
            // Nothing that was dropped comes back as the room shrinks.
            assert!(plan.tabs.iter().all(|at| last.tabs.contains(at)), "{plan:?} after {last:?}");
            assert!(!(plan.go_to && !last.go_to) && !(plan.start_agent && !last.start_agent) && !(plan.led_name && !last.led_name));
            last = plan;
        }
        room -= 4.0;
    }
    let mut seen: Vec<&str> = Vec::new();
    for step in order {
        if !seen.contains(&step) {
            seen.push(step);
        }
    }
    assert_eq!(seen, BAR_GIVES_UP, "a bar that shrinks gives items up in the stated order");
    assert!(last.tabs.contains(&0), "the tab that is drawn is the last to go: {last:?}");
    assert!(!last.go_to && !last.start_agent && !last.led_name);
}
