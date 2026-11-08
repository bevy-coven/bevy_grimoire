//! **The launcher's project list fades where rows are hidden, and only there** (UI-R05, #335).
//!
//! `chrome::scroll_list` gives its viewport gooey's `ScrollEdgeFade`, so a list taller than its box draws a fade over the
//! bottom edge while rows are hidden below and none at its end, and one over the top once it has scrolled. A list that fits
//! draws neither. The overlays are gooey's own (`ScrollFade`), read here as drawn: `Display::None` is "nothing hidden".

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::launcher::{LauncherPlugin, ProjectCard};
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::{FadeEdge, ScrollFade};

use crate::fixtures;

/// Projects enough that their cards cannot all stand in the list's box.
const MANY: usize = 40;
/// Frames the launcher is given to lay itself out and for the fade to be drawn.
const SETTLE_FRAMES: usize = 8;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_fade_{tag}_{}", std::process::id()));
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

/// The editor on the launcher with `count` recent projects.
fn on_the_launcher(tag: &str, scratch: &Scratch, count: usize) -> App {
    let root = fixtures::Fixture::new(&format!("fade_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let file = scratch.0.join("recent.ron");
    for n in 0..count {
        let path = project(&scratch.0, &format!("project_{n:02}"));
        recent::record(&file, &path).unwrap_or_else(|e| panic!("{e}"));
    }
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The viewport the cards stand in: the one with a gooey edge fade over it.
fn the_project_viewport(app: &mut App) -> Entity {
    let world = app.world_mut();
    let cards: Vec<Entity> = world.query_filtered::<Entity, With<ProjectCard>>().iter(world).collect();
    let fades: Vec<(Entity, Entity)> = world.query::<(Entity, &ScrollFade)>().iter(world).map(|(overlay, fade)| (overlay, fade.view)).collect();
    let mut viewports: Vec<Entity> = fades.iter().map(|(_, view)| *view).filter(|view| holds_a_card(world, *view, &cards)).collect();
    viewports.sort();
    viewports.dedup();
    assert_eq!(viewports.len(), 1, "exactly one scroll viewport holds the project cards and has a fade: {viewports:?}");
    viewports[0]
}

fn holds_a_card(world: &World, viewport: Entity, cards: &[Entity]) -> bool {
    cards.iter().any(|card| {
        let mut at = *card;
        loop {
            if at == viewport {
                return true;
            }
            match world.get::<ChildOf>(at) {
                Some(parent) => at = parent.parent(),
                None => return false,
            }
        }
    })
}

/// Is the fade over `edge` of `viewport` drawn? `Display::None` is an edge with nothing hidden under it.
fn drawn(app: &mut App, viewport: Entity, edge: FadeEdge) -> bool {
    let world = app.world_mut();
    let found: Vec<bool> = world
        .query::<(&ScrollFade, &Node)>()
        .iter(world)
        .filter(|(fade, _)| fade.view == viewport && fade.edge == edge)
        .map(|(_, node)| node.display != Display::None)
        .collect();
    assert_eq!(found.len(), 1, "one overlay over each edge of the viewport: {found:?}");
    found[0]
}

/// Scrolls `viewport` to the end of its content.
fn scroll_to_the_end(app: &mut App, viewport: Entity) {
    let (content, shown, scale) = {
        let node = app.world().get::<ComputedNode>(viewport).expect("the viewport is laid out");
        (node.content_size().y, node.size().y, node.inverse_scale_factor())
    };
    let end = ((content - shown) * scale).max(0.0);
    assert!(end > 0.0, "the list has to be taller than its box for this test to scroll it");
    app.world_mut().get_mut::<ScrollPosition>(viewport).expect("the viewport scrolls").y = end;
    settle(app);
}

#[test]
fn a_list_taller_than_its_box_fades_at_the_bottom_and_after_scrolling_at_the_top_instead() {
    let scratch = Scratch::new("tall");
    let mut app = on_the_launcher("tall", &scratch, MANY);
    let viewport = the_project_viewport(&mut app);

    assert!(drawn(&mut app, viewport, FadeEdge::Bottom), "rows are hidden below, so the bottom edge fades");
    assert!(!drawn(&mut app, viewport, FadeEdge::Top), "nothing is hidden above at the start, so the top edge does not");

    scroll_to_the_end(&mut app, viewport);
    assert!(!drawn(&mut app, viewport, FadeEdge::Bottom), "at the end of the list nothing is hidden below, so the bottom fade is gone");
    assert!(drawn(&mut app, viewport, FadeEdge::Top), "and rows are hidden above, so the top edge fades");
}

#[test]
fn a_list_that_fits_fades_at_neither_edge() {
    let scratch = Scratch::new("short");
    let mut app = on_the_launcher("short", &scratch, 2);
    let viewport = the_project_viewport(&mut app);
    assert!(!drawn(&mut app, viewport, FadeEdge::Bottom), "two cards fit, so nothing is hidden below");
    assert!(!drawn(&mut app, viewport, FadeEdge::Top), "or above");
}
