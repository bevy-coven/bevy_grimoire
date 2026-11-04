//! **No screen says a word only the people who build the editor know** (#303, #302).
//!
//! An issue number (`UI-73`, `#249`), the name of the binary, a folder the app keeps its files in and
//! the word *door* (the code's name for a workspace) are internal. A person reading "The theme settings
//! (UI-73) apply it." has been told something about this project's tracker, not about their editor. The
//! rule is one test over every screen, so a screen that regains one fails here and not in a review: the
//! first-run audit of 2026-11-04 found them one screen at a time.
//!
//! The words are matched by hand (the pattern is `\b(UI-\d+|#\d{2,}|bevy_cauldron|assets/grimoire|door)\b`):
//! the crate has no regex dependency and a test should not add one for four needles.
//!
//! Each screen is one test, added by the slice that cleaned it: Settings (#303), the Map and Kit
//! workspaces (#302).

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;

use crate::fixtures;

/// Is `c` a regex word character (`\w`)? The boundary of `\b`.
fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The internal word `text` says, if it says one.
fn internal_word(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let before = |i: usize| i.checked_sub(1).and_then(|j| chars.get(j)).copied();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    // The literal needles, each bounded by non-word characters on both sides.
    for needle in ["bevy_cauldron", "assets/grimoire", "door"] {
        let n: Vec<char> = needle.chars().collect();
        for start in 0..lower.len().saturating_sub(n.len() - 1) {
            if lower[start..start + n.len()] == n[..]
                && !before(start).is_some_and(word)
                && !at(start + n.len()).is_some_and(word)
            {
                return Some(needle.to_owned());
            }
        }
    }
    for start in 0..chars.len() {
        // `UI-` and digits, bounded on the left.
        if chars[start..].starts_with(&['U', 'I', '-']) && !before(start).is_some_and(word) {
            let digits = chars[start + 3..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && !at(start + 3 + digits).is_some_and(word) {
                return Some(chars[start..start + 3 + digits].iter().collect());
            }
        }
        // `#` and at least two digits.
        if chars[start] == '#' {
            let digits = chars[start + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits >= 2 && !at(start + 1 + digits).is_some_and(word) {
                return Some(chars[start..start + 1 + digits].iter().collect());
            }
        }
    }
    None
}

/// Every `Text` a person can see: shown, and laid out with a size. With a `root`, only the texts inside it:
/// a dialog is drawn over the workspace behind it, and what that workspace says is its own screen's
/// business.
fn visible_texts(app: &mut App, root: Option<Entity>) -> Vec<String> {
    let mut q = app.world_mut().query::<(Entity, &Text, &ComputedNode, &InheritedVisibility)>();
    let world = app.world();
    q.iter(world)
        .filter(|(e, _, n, vis)| {
            vis.get()
                && n.size() != Vec2::ZERO
                && root.is_none_or(|r| std::iter::successors(Some(*e), |c| world.get::<ChildOf>(*c).map(|p| p.parent())).any(|a| a == r))
        })
        .filter(|(_, t, ..)| !t.0.trim().is_empty())
        .map(|(_, t, ..)| t.0.clone())
        .collect()
}

/// Panics, naming the screen and the words, when any visible text on it is internal.
fn assert_plain(app: &mut App, screen: &str, root: Option<Entity>) {
    let texts = visible_texts(app, root);
    assert!(texts.len() >= 5, "{screen} shows only {} text(s): the screen did not open, so the rule was asked of nothing", texts.len());
    let internal: Vec<String> = texts
        .iter()
        .filter_map(|t| internal_word(t).map(|w| format!("`{w}` in {t:?}")))
        .collect();
    assert!(internal.is_empty(), "{screen} says words only the editor's makers know:\n  {}", internal.join("\n  "));
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// The matcher agrees with the pattern it stands for, so the screen tests below mean something.
#[test]
fn the_matcher_finds_exactly_the_internal_words() {
    for yes in [
        "The theme settings (UI-73) apply it.",
        "see #249 for more",
        "(#12)",
        "Quit bevy_cauldron?",
        "every mesh under assets/grimoire",
        "the kit door",
        "The Map door",
        "door",
    ] {
        assert!(internal_word(yes).is_some(), "{yes:?} is internal and the matcher missed it");
    }
    for no in ["Text size", "a doorway", "outdoor", "UI-", "#1", "issue 249", "pieces: 12 placed", "Grid lines", "bevy_cauldron_extra"] {
        assert_eq!(internal_word(no), None, "{no:?} is plain and the matcher flagged it");
    }
}

/// **Settings** (#303): the descriptions cite no issue, and the layer chooser says who it applies to.
#[test]
fn the_settings_screen_says_nothing_internal() {
    let mut app = editor("plain_settings");
    app.world_mut().write_message(bevy_cauldron::settings_screen::OpenSettings);
    for _ in 0..6 {
        app.update();
    }
    let dialog = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_cauldron::settings_screen::SettingsDialog>>()
        .single(app.world())
        .expect("the Settings dialog is open");
    assert_plain(&mut app, "the Settings screen", Some(dialog));
}

/// A stepped editor opened on `mode`'s panel, so a workspace is read as a person finds it.
fn editor_on(tag: &str, mode: bevy_cauldron::tiles::Mode) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// **The Map workspace** (#302): the strip, the toolbar, the inspector and the status band.
#[test]
fn the_map_workspace_says_nothing_internal() {
    let mut app = editor_on("plain_map", bevy_cauldron::tiles::Mode::Map);
    assert_plain(&mut app, "the Map workspace", None);
}

/// **The Kit workspace** (#302): every panel of it, because the Kit's one workspace holds several.
#[test]
fn the_kit_workspace_says_nothing_internal() {
    use bevy_cauldron::tiles::Mode;
    for (mode, name) in [(Mode::Meshes, "Meshes"), (Mode::Tiles, "Tiles"), (Mode::Compose, "Compose"), (Mode::Anim, "Anim")] {
        let mut app = editor_on(&format!("plain_kit_{name}"), mode);
        assert_plain(&mut app, &format!("the Kit workspace on its {name} panel"), None);
    }
}

/// A scratch directory under the system temp dir, removed when it goes out of scope.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_plain_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The Projects home over a recent list holding `projects`, the last one first.
fn the_home(tag: &str, scratch: &Scratch, projects: &[&str]) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let file = scratch.0.join("recent.ron");
    for name in projects {
        let folder = scratch.0.join(name);
        std::fs::create_dir_all(folder.join("assets/grimoire")).expect("a project folder");
        bevy_cauldron::recent::record(&file, &folder).expect("a recent entry");
    }
    app.insert_resource(bevy_cauldron::recent::RecentProjects(file)).add_plugins(bevy_cauldron::launcher::LauncherPlugin);
    app.world_mut().resource_mut::<NextState<bevy_cauldron::screen::Screen>>().set(bevy_cauldron::screen::Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    app
}

/// **The Projects home on a first run**: the welcome.
#[test]
fn the_first_run_welcome_says_nothing_internal() {
    let scratch = Scratch::new("welcome");
    let mut app = the_home("plain_welcome", &scratch, &[]);
    assert_plain(&mut app, "the first-run welcome", None);
}

/// **The Projects home with projects listed**: the sidebar, the header and a card per project.
#[test]
fn the_projects_home_with_projects_says_nothing_internal() {
    let scratch = Scratch::new("cards");
    let mut app = the_home("plain_cards", &scratch, &["alpha", "beta"]);
    assert_plain(&mut app, "the Projects home", None);
}

/// **The project page**: the breadcrumb, a card per map, a row per kit.
#[test]
fn the_project_page_says_nothing_internal() {
    let root = fixtures::Fixture::new("plain_page").descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_cauldron::loading::LoadingPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    assert_plain(&mut app, "the project page", None);
}
