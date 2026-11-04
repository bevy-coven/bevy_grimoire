//! **A project's own font applies when the project is opened from the home** (F1 of #294).
//!
//! The text pipeline registers a font asset once per id, so a second face written over the registered one used to be ignored (or,
//! written naively, blanked every `Text`). Opening B after A must leave B's face registered under the editor's font id (it has an
//! alias, which the pipeline only gives a face it registered) and every label laid out.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::loading::LoadingPlugin;
use bevy_cauldron::project::Project;
use bevy_cauldron::project_assets::open_project;
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

const PATIENCE: Duration = Duration::from_secs(20);

/// Trailing bytes that make project B's font file differ from A's; a font's tables are found through its directory, so it is still the face.
const B_EXTRA_BYTES: usize = 8;

/// How many words must be on screen, laid out, for the editor to count as drawing text.
const WORDS_LAID_OUT: usize = 5;

/// A project whose own font is the shipped face plus `extra` trailing bytes.
fn project(tag: &str, extra: usize) -> PathBuf {
    let root = Fixture::new(&format!("own_font_{tag}_{}", std::process::id())).descriptor("bench", "props").place("bench", (2.0, 2.0)).build("m");
    let face = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/FiraMono-Regular.ttf")).unwrap_or_else(|e| panic!("{e}"));
    let mut bytes = face;
    bytes.extend(std::iter::repeat_n(0u8, extra));
    std::fs::create_dir_all(root.join("assets/fonts")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/fonts/FiraMono-Regular.ttf"), bytes).unwrap_or_else(|e| panic!("{e}"));
    root.canonicalize().unwrap_or_else(|e| panic!("{e}"))
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed() < PATIENCE {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("never got there: {what} (screen {:?})", screen_of(app));
}

/// The face under the editor's font id: its size in bytes, and whether the text pipeline registered it (gave it an alias).
fn face(app: &App) -> Option<(usize, bool)> {
    app.world().resource::<Assets<Font>>().get(AssetId::default()).map(|f| (f.data.len(), !f.alias.is_empty()))
}

/// Words on screen with a width: text the pipeline laid out.
fn laid_out(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<(&Text, &bevy::ui::ComputedNode)>().iter(world).filter(|(t, n)| !t.0.trim().is_empty() && n.size().x > 0.0).count()
}

#[test]
fn project_b_opened_after_project_a_draws_in_b_s_own_font() {
    let (a, b) = (project("a", 0), project("b", B_EXTRA_BYTES));
    let face_len = std::fs::metadata(a.join("assets/fonts/FiraMono-Regular.ttf")).map(|m| m.len() as usize).unwrap_or_else(|e| panic!("{e}"));

    let mut app = bevy_cauldron::harness::build_headless(&a, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins((LauncherPlugin, LoadingPlugin));
    app.finish();
    app.cleanup();
    until(&mut app, "A's editor draws words", |app| laid_out(app) >= WORDS_LAID_OUT);
    assert_eq!(face(&app), Some((face_len, true)), "A's face is the registered one");

    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    until(&mut app, "the window is home", |app| screen_of(app) == Screen::Launcher);
    app.world_mut().commands().queue({
        let b = b.clone();
        move |world: &mut World| open_project(world, &b, Some("m")).unwrap_or_else(|e| panic!("{e}"))
    });
    until(&mut app, "B opens", |app| screen_of(app) == Screen::Editor && app.world().resource::<Project>().root == b);
    until(&mut app, "B's editor draws words", |app| face(app) == Some((face_len + B_EXTRA_BYTES, true)) && laid_out(app) >= WORDS_LAID_OUT);

    assert_eq!(face(&app), Some((face_len + B_EXTRA_BYTES, true)), "B's own font is the registered face, with an alias the pipeline gave it");
    assert!(laid_out(&mut app) >= WORDS_LAID_OUT, "and the words are laid out in it");
}
