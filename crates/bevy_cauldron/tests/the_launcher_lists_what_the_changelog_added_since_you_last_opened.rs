//! **The What's new page lists the changelog entries that have a sentence for a person, and counts the ones added since the last time
//! a project was opened from the home** (UI-14, #220; #300).
//!
//! An entry is a line of `CHANGELOG.md`; one carries ` *For you: <sentence>*` when it is worth telling a person who never reads the
//! file, and only those are listed, each with the still of its clip. `recent.ron` remembers the id of the newest entry a person had
//! been shown, and the sidebar's What's new counts the sentences above it.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::launcher::{self, ChangelogText, ClipboardOff, Launcher, LauncherPlugin};
use bevy_cauldron::recent::{self, Recent, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::Role;

use crate::fixtures::Fixture;

const CHANGELOG: &str = "# Changelog\n\nWhat each change added.\n\n## 2026-11-02\n\n\
- **delta** `d00d000` An internal change *No clip: a test*\n\
- **gamma** `c0ffee1` The third thing *For you: The third thing, said plainly.* *No clip: a test*\n\
- **beta** `b0b0b0b` The second thing, with a clip *For you: The second thing, said plainly.*<br><a href=\"docs/changelog/media/x.mp4\"><img src=\"docs/changelog/media/x.png\"></a>\n\
\n## 2026-11-01\n\n\
- **alpha** The first thing\n";

fn ids(news: &[launcher::News]) -> Vec<&str> {
    news.iter().map(|n| n.id.as_str()).collect()
}

#[test]
fn a_stored_older_entry_counts_exactly_the_newer_sentences_and_the_newest_counts_nothing() {
    let all = launcher::news(CHANGELOG);
    assert_eq!(ids(&all), ["delta", "gamma", "beta", "alpha"], "newest first, as the file keeps them");
    assert_eq!(all[1].title, "The third thing", "the sha, the sentence and the clip note are not part of the title");
    assert_eq!(all[2].title, "The second thing, with a clip", "the clip's markup is not part of the title");
    assert_eq!(all[3].title, "The first thing", "an entry with no sha reads the same");
    assert_eq!(all[1].user.as_deref(), Some("The third thing, said plainly."), "the sentence for a person is parsed");
    assert_eq!(all[2].user.as_deref(), Some("The second thing, said plainly."), "also when a clip follows it");
    assert_eq!((all[0].user.as_deref(), all[3].user.as_deref()), (None, None), "an entry with none has none");
    assert_eq!(all[2].still.as_deref(), Some("docs/changelog/media/x.png"), "the clip's still is the entry's picture");
    assert_eq!(all[1].still, None, "and an entry with no clip has none");

    assert_eq!(ids(&launcher::for_you(CHANGELOG)), ["gamma", "beta"], "only entries with a sentence are for a person");
    assert_eq!(ids(&launcher::whats_new(CHANGELOG, Some("alpha"))), ["gamma", "beta"], "an internal change newer than the sentences is not counted");
    assert_eq!(ids(&launcher::whats_new(CHANGELOG, Some("beta"))), ["gamma"]);
    assert!(launcher::whats_new(CHANGELOG, Some("gamma")).is_empty(), "delta is newer but says nothing to a person");
    assert!(launcher::whats_new(CHANGELOG, Some("delta")).is_empty(), "the version already seen shows nothing");
    assert!(launcher::whats_new(CHANGELOG, None).is_empty(), "a first run has nothing to compare with");
    assert!(launcher::whats_new(CHANGELOG, Some("gone")).is_empty(), "an id the file no longer has is not a reason to list everything");
}

/// **One entry with a sentence and one without lists exactly one.**
#[test]
fn a_changelog_with_one_for_you_line_and_one_without_lists_exactly_one() {
    let text = "# Changelog\n\n## 2026-11-02\n\n- **inside** Something only the repository cares about *No clip: a test*\n- **outside** Something a person sees *For you: You can now pick a project from its card.* *No clip: a test*\n";
    let (mut app, dir) = launcher_with_text("one_of_two", text, None);
    assert_eq!(entries(&mut app), ["outside"], "only the entry with a sentence is on the page");
    let said = texts(&mut app, Role::ButtonText);
    assert!(said.contains(&"You can now pick a project from its card.".to_owned()), "and it is said in the sentence: {said:?}");
    assert!(!said.iter().any(|t| t.contains("Something only")), "the title of the other is nowhere on screen: {said:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn launcher_with(tag: &str, seen: Option<&str>) -> (App, PathBuf) {
    launcher_with_text(tag, CHANGELOG, seen)
}

fn entries(app: &mut App) -> Vec<String> {
    app.world_mut().query::<&launcher::NewsEntry>().iter(app.world()).map(|e| e.0.clone()).collect()
}

fn launcher_with_text(tag: &str, changelog: &str, seen: Option<&str>) -> (App, PathBuf) {
    let root = Fixture::new(&format!("news_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let dir = std::env::temp_dir().join(format!("grimoire_news_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("recent.yaml");
    recent::record(&file, &root).unwrap();
    if let Some(id) = seen {
        let mut r = Recent::load(&file).unwrap();
        r.changelog_seen = Some(id.to_owned());
        r.save(&file).unwrap();
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(file.clone())).insert_resource(ChangelogText(changelog.to_owned())).insert_resource(ClipboardOff).add_plugins(LauncherPlugin);
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
fn the_page_lists_the_sentences_marks_the_new_ones_and_remembers_the_newest_once_a_project_is_opened() {
    let (mut app, dir) = launcher_with("older", Some("alpha"));
    assert!(texts(&mut app, Role::Heading).contains(&"What's new".to_owned()));
    let said = texts(&mut app, Role::ButtonText);
    assert!(said.contains(&"The third thing, said plainly.".to_owned()) && said.contains(&"The second thing, said plainly.".to_owned()), "{said:?}");
    assert_eq!(entries(&mut app), ["gamma", "beta"], "each entry that has a sentence is listed, and no other");
    assert!(texts(&mut app, Role::Label).iter().filter(|l| *l == "New").count() == 2, "both are newer than the one seen");
    let tabs = texts(&mut app, Role::ButtonText);
    assert!(tabs.contains(&"What's new (2)".to_owned()), "the sidebar counts the sentences newer than the one seen: {tabs:?}");
    assert_eq!(app.world().resource::<Launcher>().newest.as_deref(), Some("delta"));

    let name = app.world().resource::<Launcher>().rows[0].project.path.file_name().unwrap().to_string_lossy().into_owned();
    let button = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Children)>();
        let mut t = world.query::<(&Role, &Text)>();
        q.iter(world)
            .find_map(|(e, kids)| kids.iter().find_map(|k| t.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == name).map(|_| e)))
            .unwrap()
    };
    // The first press chooses the card, the second opens it.
    for _ in 0..2 {
        app.world_mut().commands().trigger(Activate { entity: button });
        app.update();
        app.update();
    }
    let stored = Recent::load(&dir.join("recent.yaml")).unwrap();
    assert_eq!(stored.changelog_seen.as_deref(), Some("delta"), "opening a project is what marks the news as read");
    assert!(launcher::whats_new(CHANGELOG, stored.changelog_seen.as_deref()).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_marked_new_when_the_newest_entry_was_the_last_one_seen() {
    let (mut app, dir) = launcher_with("same", Some("delta"));
    assert!(app.world().resource::<Launcher>().whats_new.is_empty());
    assert!(!texts(&mut app, Role::Label).contains(&"New".to_owned()), "nothing is marked new");
    assert!(texts(&mut app, Role::ButtonText).contains(&"What's new".to_owned()), "and the sidebar tab carries no count");
    assert_eq!(entries(&mut app), ["gamma", "beta"], "though the page still lists what is worth knowing");
    let _ = std::fs::remove_dir_all(&dir);
}
