//! **A launcher test that finds a button by its words and clicks it where it is drawn** (UI-06, #214).
//!
//! `the_launcher_opens_a_project_by_its_name` presses a button by triggering `Activate` on its entity,
//! which passes while the button is hidden, covered or unfocusable. gooey's `testing` helpers go through
//! the real input path: `find_by_label` is the one visible widget with those words (two are an error that
//! names both) and `click` moves the pointer to its laid-out centre and presses it on the camera that draws
//! it (the editor's is an image, so no window is needed), so a covered button is not clicked.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::launcher::{Handoff, LauncherPlugin};
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::testing::{click, find_by_label, focused_label};

use crate::fixtures;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_words_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
    std::fs::create_dir_all(root.join("assets/grimoire")).unwrap();
    root.canonicalize().unwrap()
}

fn on_the_launcher(tag: &str, scratch: &Scratch, projects: &[&Path]) -> App {
    let root = fixtures::Fixture::new(&format!("words_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let file = scratch.0.join("recent.ron");
    for p in projects {
        recent::record(&file, p).unwrap();
    }
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    app
}

/// **A person clicks the second project by its name**, and the next process is handed its folder.
#[test]
fn clicking_a_recent_project_by_its_name_hands_over_its_folder() {
    let s = Scratch::new("click");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("click", &s, &[&alpha, &beta]);
    assert!(app.world().get_resource::<Handoff>().is_none(), "nothing opens until a row is chosen");

    let row = find_by_label(&mut app, "alpha").unwrap_or_else(|e| panic!("the launcher shows a button that says `alpha`: {e:?}"));
    click(&mut app, row);

    assert_eq!(focused_label(&app).as_deref(), Some("alpha"), "the click that opened it also focused it, as a person's does");
    assert_eq!(app.world().get_resource::<Handoff>(), Some(&Handoff(vec![alpha.to_string_lossy().into_owned()])));
}

/// **A word that is nowhere on the screen is not found**, so a test cannot pass on a button that is not there.
#[test]
fn a_word_the_launcher_does_not_show_is_not_found() {
    let s = Scratch::new("absent");
    let alpha = project(&s.0, "alpha");
    let mut app = on_the_launcher("absent", &s, &[&alpha]);
    assert!(find_by_label(&mut app, "gamma").is_err());
}
