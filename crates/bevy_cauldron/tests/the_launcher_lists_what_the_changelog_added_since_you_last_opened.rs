//! **The launcher lists the changelog entries added since the last time a project was opened from it**
//! (UI-14, #220).
//!
//! `recent.ron` remembers the id of the newest entry a person had been shown. The launcher lists every entry
//! above it, and nothing when it is the newest.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::launcher::{self, ChangelogText, ClipboardOff, Launcher, LauncherPlugin};
use bevy_cauldron::recent::{self, Recent, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::Role;

use crate::fixtures::Fixture;

const CHANGELOG: &str = "# Changelog\n\nWhat each change added.\n\n## 2026-11-02\n\n\
- **gamma** `c0ffee1` The third thing *No clip: a test*\n\
- **beta** `b0b0b0b` The second thing, with a clip<br><a href=\"x.mp4\"><img src=\"x.png\"></a>\n\
\n## 2026-11-01\n\n\
- **alpha** The first thing\n";

fn ids(news: &[launcher::News]) -> Vec<&str> {
    news.iter().map(|n| n.id.as_str()).collect()
}

#[test]
fn a_stored_older_entry_lists_exactly_the_newer_lines_and_the_newest_lists_nothing() {
    let all = launcher::news(CHANGELOG);
    assert_eq!(ids(&all), ["gamma", "beta", "alpha"], "newest first, as the file keeps them");
    assert_eq!(all[0].title, "The third thing", "the sha and the clip note are not part of the title");
    assert_eq!(all[1].title, "The second thing, with a clip", "the clip's markup is not part of the title");
    assert_eq!(all[2].title, "The first thing", "an entry with no sha reads the same");

    assert_eq!(ids(&launcher::whats_new(CHANGELOG, Some("alpha"))), ["gamma", "beta"]);
    assert_eq!(ids(&launcher::whats_new(CHANGELOG, Some("beta"))), ["gamma"]);
    assert!(launcher::whats_new(CHANGELOG, Some("gamma")).is_empty(), "the version already seen shows nothing");
    assert!(launcher::whats_new(CHANGELOG, None).is_empty(), "a first run has nothing to compare with");
    assert!(launcher::whats_new(CHANGELOG, Some("gone")).is_empty(), "an id the file no longer has is not a reason to list everything");
}

fn launcher_with(tag: &str, seen: Option<&str>) -> (App, PathBuf) {
    let root = Fixture::new(&format!("news_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let dir = std::env::temp_dir().join(format!("grimoire_news_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("recent.ron");
    recent::record(&file, &root).unwrap();
    if let Some(id) = seen {
        let mut r = Recent::load(&file).unwrap();
        r.changelog_seen = Some(id.to_owned());
        r.save(&file).unwrap();
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(file.clone())).insert_resource(ChangelogText(CHANGELOG.to_owned())).insert_resource(ClipboardOff).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    (app, dir)
}

fn texts(app: &mut App, role: Role) -> Vec<String> {
    let mut q = app.world_mut().query::<(&Role, &Text)>();
    q.iter(app.world()).filter(|(r, _)| **r == role).map(|(_, t)| t.0.clone()).collect()
}

#[test]
fn the_launcher_shows_the_newer_entries_and_remembers_the_newest_once_a_project_is_opened() {
    let (mut app, dir) = launcher_with("older", Some("alpha"));
    assert!(texts(&mut app, Role::Heading).contains(&"New since you last opened".to_owned()));
    let labels = texts(&mut app, Role::Label);
    assert!(labels.contains(&"gamma: The third thing".to_owned()) && labels.contains(&"beta: The second thing, with a clip".to_owned()), "{labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("alpha:")), "the entry already seen is not listed: {labels:?}");
    assert_eq!(app.world().resource::<Launcher>().newest.as_deref(), Some("gamma"));

    let name = app.world().resource::<Launcher>().rows[0].project.path.file_name().unwrap().to_string_lossy().into_owned();
    let button = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Children)>();
        let mut t = world.query::<(&Role, &Text)>();
        q.iter(world)
            .find_map(|(e, kids)| kids.iter().find_map(|k| t.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == name).map(|_| e)))
            .unwrap()
    };
    app.world_mut().commands().trigger(Activate { entity: button });
    app.update();
    app.update();
    let stored = Recent::load(&dir.join("recent.ron")).unwrap();
    assert_eq!(stored.changelog_seen.as_deref(), Some("gamma"), "opening a project is what marks the news as read");
    assert!(launcher::whats_new(CHANGELOG, stored.changelog_seen.as_deref()).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_listed_when_the_newest_entry_was_the_last_one_seen() {
    let (mut app, dir) = launcher_with("same", Some("gamma"));
    assert!(!texts(&mut app, Role::Heading).contains(&"New since you last opened".to_owned()));
    assert!(app.world().resource::<Launcher>().whats_new.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
