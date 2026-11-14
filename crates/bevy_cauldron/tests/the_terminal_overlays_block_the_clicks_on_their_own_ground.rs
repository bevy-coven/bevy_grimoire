//! **The terminal's two overlays are drawn from gooey's `placed`/`cover`, which ignore the pointer, and take it back where they must** (#260 review).
//!
//! `bevy_gooey::placed` carries `Pickable::IGNORE`, so a tuple that also listed `Pickable::default()` held the component twice and bevy_ecs 0.20
//! panicked at spawn: opening the find box on Linux, where a shell exists, was a crash. Each overlay now takes `Pickable::default()` after it is
//! spawned, so a click on its padding or its edge is its own and does not fall to the shell beneath.
//!
//! - the find box opens (that is the spawn that panicked), and its frame blocks the pointer on its own ground;
//! - the invitation's frame blocks the pointer, a click beside its button takes the keyboard and starts nothing, and the button still takes a click.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{TerminalFindFrame, TerminalFocus, TerminalInvite, TerminalInvokeButton};

use crate::fixtures;
use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, settle};

/// **`SHELL` is process-wide**, so the tests that start an agent take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// One pointer action at `at`, in the physical pixels of the image the editor draws its interface into.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

fn click(app: &mut App, at: Vec2) {
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        point(app, at, action);
    }
}

/// Is the pointer blocked by this entity's own box (a `Pickable` that is not `IGNORE`)?
fn takes_the_pointer(app: &App, entity: Entity) -> bool {
    app.world().get::<Pickable>(entity).is_some_and(|pickable| pickable.should_block_lower && pickable.is_hoverable)
}

/// Ctrl+F, as the editor's Primary+F off a mac.
fn ctrl_f(app: &mut App) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for (key_code, logical_key) in [(KeyCode::ControlLeft, unidentified()), (KeyCode::KeyF, Key::Character("f".into()))] {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                text: match (&logical_key, state) {
                    (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
                    _ => None,
                },
                logical_key,
                state,
                repeat: false,
                window,
            });
        }
        app.update();
    }
    settle(app, 3);
}

/// **Opening the find box does not panic, and its frame blocks the pointer on its own ground.**
#[test]
fn the_find_box_opens_and_its_frame_takes_the_pointer() {
    let mut app = editor_with_terminal("overlay_find");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 6);
    ctrl_f(&mut app);
    let frame = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<TerminalFindFrame>>().iter(world).next().expect("Primary+F opened the find box")
    };
    assert!(takes_the_pointer(&app, frame), "the find frame blocks the pointer on its padding and edge: {:?}", app.world().get::<Pickable>(frame));
}

/// The editor on a project whose settings choose a custom agent: the empty pane invites it.
fn editor_inviting_the_agent() -> App {
    let tag = format!("overlay_invite_{}", std::process::id());
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
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    app
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..200 {
        settle(app, 2);
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}");
}

/// **A click beside the invitation's button is the pane's (it takes the keyboard and starts nothing); the button still takes a click.**
#[test]
fn a_click_beside_the_invitations_button_starts_nothing_and_the_button_still_does() {
    let mut app = editor_inviting_the_agent();
    until(&mut app, "the invitation was never drawn", |app| {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<TerminalInvite>>().iter(world).next().is_some()
    });
    settle(&mut app, 4);
    let (frame, button_at, corner) = {
        let world = app.world_mut();
        let frame = world.query_filtered::<Entity, With<TerminalInvite>>().iter(world).next().expect("the invitation");
        let button = world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().expect("the button").translation;
        let node = world.get::<ComputedNode>(frame).expect("laid out");
        let centre = world.get::<UiGlobalTransform>(frame).expect("placed").translation;
        // A point in the frame's top-left corner, far from the centred button.
        (frame, button, centre - node.size() * 0.4)
    };
    assert!(takes_the_pointer(&app, frame), "the invitation's frame blocks the pointer: {:?}", app.world().get::<Pickable>(frame));

    app.world_mut().resource_mut::<TerminalFocus>().release();
    click(&mut app, corner);
    settle(&mut app, 3);
    assert!(app.world().resource::<Terminal>().is_empty(), "a click beside the button starts no agent");
    assert!(app.world().resource::<TerminalFocus>().holds(), "it is the pane's click: the pane takes the keyboard");

    click(&mut app, button_at);
    until(&mut app, "the button never opened the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
}

/// **The projects home's split takes exactly what the docked terminal leaves** (#260 review: the split's `flex_basis` went from 0 to its content's height when it became a gooey `Sizing`).
///
/// With the pane raised, the split and the pane stand one under the other in the home's root, so their heights add up to the root's and the pane
/// ends at the window's foot. A basis of its content's height grows and shrinks into the same room (the pane never shrinks, so the split gives way alone), so the
/// real pages measure what they measured; this pins it.
#[test]
fn the_launchers_split_takes_exactly_what_the_terminal_leaves() {
    use bevy_cauldron::launcher::{LauncherPlugin, LauncherRoot};
    use bevy_cauldron::screen::Screen;
    use bevy_cauldron::terminal_pane::TerminalPaneRoot;

    let root = fixtures::Fixture::new(&format!("overlay_split_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.add_plugins(LauncherPlugin);
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    settle(&mut app, 6);
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app, 10);

    let world = app.world_mut();
    let home = world.query_filtered::<Entity, With<LauncherRoot>>().iter(world).next().expect("the projects home");
    let kids: Vec<Entity> = world.get::<Children>(home).expect("the home has children").to_vec();
    let height = |world: &World, entity: Entity| world.get::<ComputedNode>(entity).map(|n| n.size().y * n.inverse_scale_factor());
    let pane = kids.iter().copied().find(|kid| world.get::<TerminalPaneRoot>(*kid).is_some()).expect("the pane is docked in the home's root");
    let split = kids.iter().copied().find(|kid| world.get::<bevy_gooey::SplitFraction>(*kid).is_some()).expect("the home's split");
    let (home_h, pane_h, split_h) = (height(world, home).unwrap_or(0.0), height(world, pane).unwrap_or(0.0), height(world, split).unwrap_or(0.0));
    assert!(pane_h > 0.0 && split_h > 0.0, "both are laid out: pane {pane_h}, split {split_h}");
    assert!((pane_h + split_h - home_h).abs() <= 1.0, "the split ({split_h}) takes exactly what the pane ({pane_h}) leaves of the home ({home_h})");
}
