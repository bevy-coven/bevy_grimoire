//! **Every key the editor answers is a command** (UI-04, #212): a stable id, a title, a category and a
//! default shortcut, and no two commands take one chord in a state where both are live.

use std::collections::HashSet;

use bevy_gooey::{Modifiers, Shortcut};
use bevy::prelude::KeyCode;
use bevy_grimoire_mapper::commands::{self, Command};
use bevy_grimoire_mapper::keys::{Action, Context, Holder, Live, Stance};

#[test]
fn every_action_has_a_unique_id_and_a_unique_title() {
    let all = commands::all();
    assert_eq!(all.len(), Action::ALL.len(), "one command per action");

    let mut ids = HashSet::new();
    let mut titles = HashSet::new();
    for c in &all {
        assert!(ids.insert(c.id), "{:?} reuses the id `{}`", c.action, c.id);
        assert!(titles.insert(c.title), "{:?} reuses the title `{}`", c.action, c.title);
        let (category, verb) = c.id.split_once('.').unwrap_or_else(|| panic!("`{}` is not `<category>.<verb>`", c.id));
        let slug = |s: &str| !s.is_empty() && s.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
        assert!(slug(category) && slug(verb), "`{}` is not lower snake case", c.id);
        assert_eq!(c.category(), category);
        assert!(!c.title.trim().is_empty() && c.title.chars().next().is_some_and(char::is_uppercase), "`{}`'s title `{}` is not sentence case", c.id, c.title);
    }
}

#[test]
fn a_command_is_found_by_its_id() {
    for c in commands::all() {
        assert_eq!(commands::by_id(c.id), Some(c));
    }
    assert_eq!(commands::by_id("no.such_command"), None);
}

#[test]
fn no_two_default_shortcuts_conflict_where_both_are_live() {
    let clashes = commands::conflicts();
    assert!(
        clashes.is_empty(),
        "{} shortcut conflict(s):\n  {}",
        clashes.len(),
        clashes.iter().map(|c| format!("`{}` and `{}` on {:?} while {:?}", c.first, c.second, c.tab, c.stance)).collect::<Vec<_>>().join("\n  ")
    );
}

#[test]
fn undo_and_redo_share_a_key_and_differ_by_shift() {
    let undo = commands::by_id("edit.undo").expect("edit.undo");
    let redo = commands::by_id("edit.redo").expect("edit.redo");
    assert_eq!(undo.shortcut, Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyZ));
    assert_eq!(redo.shortcut, Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyZ));
    assert_eq!(commands::by_id("file.save").map(|c| c.shortcut), Some(Shortcut::primary(KeyCode::KeyS)));
}

#[test]
fn a_command_says_why_it_cannot_run_here() {
    let flush = commands::by_id("tiles.align_forward").expect("tiles.align_forward");
    let on = |tab, stance| flush.enabled(Live(tab, stance, Holder::Tab));
    assert_eq!(on(Context::Tiles, Stance::Holding), Ok(()));
    assert_eq!(on(Context::Tiles, Stance::Idle), Err("Only while holding a piece"));
    assert_eq!(on(Context::Map, Stance::Holding), Err("Not on this tab"));
    assert_eq!(on(Context::Typing, Stance::Holding), Err("A text field is taking the keys"));

    let save: Command = commands::by_id("file.save").expect("file.save");
    assert_eq!(save.enabled(Live(Context::Anim, Stance::Idle, Holder::Tab)), Ok(()), "a global command runs on every tab");
    assert_eq!(save.enabled(Live(Context::Map, Stance::Idle, Holder::Filter)), Err("A filter box is taking the keys"));
}
