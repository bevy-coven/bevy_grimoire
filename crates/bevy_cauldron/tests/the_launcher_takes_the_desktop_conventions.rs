//! **The editor takes gooey's desktop conventions** (UI-R04, #334): a tooltip waits 0.5 s, a card opens on a double press,
//! typing a project's first letters moves to it, and a card shrinks a little under the press.
//!
//! The pointer is gooey's own picking input on the image the editor draws into; the clock is set by the test, 16 ms a frame.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_cauldron::launcher::{LauncherPlugin, ProjectCard};
use bevy_cauldron::project_assets::PointedAt;
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_cauldron::toolbar::ToolbarCommand;
use bevy_gooey::testing::{focused, type_text};
use bevy_gooey::{PressScale, TooltipBox};

use crate::fixtures;

/// One frame of the test's clock.
const FRAME_MS: u64 = 16;
/// A hover is held this long to be sure a tooltip that waits 0.5 s has not come: 0.4 s.
const NOT_YET_FRAMES: usize = 25;
/// And this long to be sure it has: 0.64 s.
const BY_NOW_FRAMES: usize = 40;
/// What a card is drawn shrunk to while it is pressed.
const CARD_PRESS_SCALE: f32 = 0.97;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_conventions_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    std::fs::create_dir_all(root.join("assets/grimoire")).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    root.canonicalize().unwrap_or_else(|e| panic!("{}: {e}", root.display()))
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// The editor on the launcher, with `names` as its recent projects, on a clock the test sets.
fn on_the_launcher(tag: &str, scratch: &Scratch, names: &[&str]) -> (App, Vec<PathBuf>) {
    let root = fixtures::Fixture::new(&format!("conventions_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(FRAME_MS)));
    let file = scratch.0.join("recent.ron");
    let paths: Vec<PathBuf> = names.iter().map(|name| project(&scratch.0, name)).collect();
    for path in &paths {
        recent::record(&file, path).unwrap_or_else(|e| panic!("{e}"));
    }
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app, 6);
    (app, paths)
}

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

fn card_of(app: &mut App, path: &Path) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &ProjectCard)>()
        .iter(world)
        .find(|(_, card)| card.path == path)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no card for {}", path.display()))
}

fn centre_of(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<UiGlobalTransform>(entity).expect("laid out").translation
}

#[test]
fn a_toolbar_tooltip_is_not_shown_at_0_4_s_and_is_at_0_5_s() {
    let root = fixtures::Fixture::new(&format!("conventions_tooltip_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(FRAME_MS)));
    settle(&mut app, 8);
    let button = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<ToolbarCommand>>().iter(world).next().expect("the toolbar has a command button")
    };
    let at = centre_of(&app, button);
    point(&mut app, at, PointerAction::Move { delta: Vec2::ZERO });
    settle(&mut app, NOT_YET_FRAMES);
    assert!(!tooltip_shown(&mut app), "0.4 s into the hover nothing is shown");
    settle(&mut app, BY_NOW_FRAMES - NOT_YET_FRAMES);
    assert!(tooltip_shown(&mut app), "by 0.6 s the tooltip is up");
}

fn tooltip_shown(app: &mut App) -> bool {
    let world = app.world_mut();
    let boxes: Vec<Entity> = world.query_filtered::<Entity, With<TooltipBox>>().iter(world).collect();
    boxes.iter().any(|tip| world.get::<Node>(*tip).is_some_and(|node| node.display != Display::None))
}

#[test]
fn a_double_press_on_a_card_opens_the_project() {
    let scratch = Scratch::new("double");
    let (mut app, paths) = on_the_launcher("double", &scratch, &["alpha"]);
    let card = card_of(&mut app, &paths[0]);
    let at = centre_of(&app, card);
    let before = app.world().resource::<PointedAt>().root.clone();
    click(&mut app, at);
    settle(&mut app, 2);
    assert_eq!(app.world().resource::<PointedAt>().root, before, "one press opens nothing");
    assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Launcher, "one press chooses the card and opens nothing");
    click(&mut app, at);
    settle(&mut app, 4);
    assert_eq!(app.world().resource::<PointedAt>().root, paths[0], "the second press of a double press opens that folder");
}

#[test]
fn typing_a_projects_first_letters_moves_to_it() {
    let scratch = Scratch::new("typeselect");
    let (mut app, paths) = on_the_launcher("typeselect", &scratch, &["alpha", "beta", "bravo"]);
    // A card has the keyboard, as it does after Tab or an arrow; typing goes from there.
    let alpha = card_of(&mut app, &paths[0]);
    app.world_mut().resource_mut::<InputFocus>().set(alpha, FocusCause::Navigated);
    settle(&mut app, 4);

    type_text(&mut app, "br");
    let bravo = card_of(&mut app, &paths[2]);
    assert_eq!(focused(&app), Some(bravo), "`br` goes to the card named bravo");
}

#[test]
fn a_project_card_shrinks_under_the_press() {
    let scratch = Scratch::new("press");
    let (mut app, paths) = on_the_launcher("press", &scratch, &["alpha"]);
    let card = card_of(&mut app, &paths[0]);
    assert_eq!(app.world().get::<PressScale>(card), Some(&PressScale(CARD_PRESS_SCALE)), "a card is a PressScale widget at 0.97");
}
