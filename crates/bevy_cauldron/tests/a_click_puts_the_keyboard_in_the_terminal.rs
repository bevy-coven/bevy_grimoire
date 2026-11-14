//! **A click on the terminal puts the keyboard in it, and a click elsewhere gives it back** (#317).
//!
//! Driven through `bevy_picking`'s own input — a pointer move, a press and a release on the image the
//! editor draws its interface into — exactly as a mouse reaches the editor, so the click has to survive
//! the whole pointer chain to the pane's observer. Found live (pass 423): a click on the terminal's bar
//! left `TerminalFocus` false, and the only way into the shell was the chord.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_cauldron::terminal_pane::{TerminalBar, TerminalBody, TerminalFocus};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    for _ in 0..10 {
        app.update();
    }
    app
}

fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: at,
    };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// The centre of the first node with `T`, in the physical pixels `PointerInput` speaks.
fn centre_of<T: Component>(app: &mut App) -> Vec2 {
    let world = app.world_mut();
    world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<T>>()
        .iter(world)
        .next()
        .map(|(node, at)| {
            assert!(node.size().y > 0.0, "the node is laid out");
            at.translation
        })
        .expect("the node exists")
}

/// A press and a release where a hand makes them: a move there first, and a frame between.
fn click(app: &mut App, at: Vec2) {
    point(app, at, PointerAction::Move { delta: Vec2::ZERO });
    point(app, at, PointerAction::Press(PointerButton::Primary));
    point(app, at, PointerAction::Release(PointerButton::Primary));
    for _ in 0..2 {
        app.update();
    }
}

fn holds(app: &App) -> bool {
    app.world().resource::<TerminalFocus>().holds()
}

#[test]
fn a_click_on_the_terminal_body_takes_the_keyboard() {
    let mut app = editor("click_terminal_body");
    assert!(!holds(&app), "the editor opens with its own keyboard");
    let body = centre_of::<TerminalBody>(&mut app);
    click(&mut app, body);
    assert!(holds(&app), "a click in the terminal's body did not put the keyboard in the shell");
}

#[test]
fn a_click_on_the_terminal_bar_takes_the_keyboard_and_one_elsewhere_gives_it_back() {
    let mut app = editor("click_terminal_bar");
    let bar = centre_of::<TerminalBar>(&mut app);
    click(&mut app, bar);
    assert!(holds(&app), "a click on the terminal's bar did not put the keyboard in the shell");

    let viewport = {
        let frame = app.world().resource::<bevy_cauldron::chrome::Frame>().viewport;
        let world = app.world_mut();
        world
            .query::<&UiGlobalTransform>()
            .get(world, frame)
            .map(|at| at.translation)
            .expect("the frame has a viewport")
    };
    click(&mut app, viewport);
    assert!(!holds(&app), "a click on the map did not give the editor its keyboard back");
}
