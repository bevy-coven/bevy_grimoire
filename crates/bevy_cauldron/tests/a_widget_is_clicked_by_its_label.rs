//! **A UI test finds a widget by the words on it and clicks it, with no window** (UI-06, #214).
//!
//! `bevy_gooey::testing` is how a player meets a widget: `find_by_label` reads what is written on it and
//! `click` presses it where it is drawn. The editor draws into an image and a test app has no
//! `PrimaryWindow`, so the click is a `PointerInput` on the image's target. It has to reach the button:
//! the editor's own pointer (`view::Pointer`) is `None` with no window, and `surface::retarget_pointer` used
//! to take that for "the pointer left" and clear `PointerLocation` every frame, which erased the click
//! before picking read it.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::launcher::{Handoff, LauncherPlugin};
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::testing::{click, find_by_label, focused_label};

use crate::fixtures;

fn project(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    std::fs::create_dir_all(root.join("assets/grimoire")).unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn a_recent_project_is_opened_by_clicking_its_name() {
    let dir = std::env::temp_dir().join(format!("grimoire_click_by_label_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (alpha, beta) = (project(&dir, "alpha"), project(&dir, "beta"));

    let root = fixtures::Fixture::new(&format!("click_by_label_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let file = dir.join("recent.ron");
    for p in [&alpha, &beta] {
        recent::record(&file, p).unwrap();
    }
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }

    let row = find_by_label(&mut app, "alpha").expect("the launcher has one button that says alpha");
    assert!(app.world().get_resource::<Handoff>().is_none(), "nothing opens until a row is chosen");
    click(&mut app, row);
    assert_eq!(
        app.world().get_resource::<Handoff>(),
        Some(&Handoff(vec![alpha.to_string_lossy().into_owned()])),
        "a click on the row named alpha hands the next process that folder (focus is on {:?})",
        focused_label(&app)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
