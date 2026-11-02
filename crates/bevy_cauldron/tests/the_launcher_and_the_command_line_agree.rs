//! **`bevy_cauldron <root>` skips the launcher, a bare `bevy_cauldron` opens it, and the line the launcher
//! copies opens the same project and door** (UI-13, #219).
//!
//! The launcher hands the next process an argv and copies the same words as a command line, so a project
//! opened from a row and one opened by pasting the line cannot differ.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::args;
use bevy_cauldron::launcher::{self, Launcher, LauncherPlugin};
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::Role;

use crate::fixtures::Fixture;

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

/// A POSIX shell's reading of a line made of plain words and single-quoted ones, which is all
/// `launcher::command_line` writes.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut started) = (false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if !quoted && chars.peek().is_some() => {
                word.push(chars.next().unwrap());
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

#[test]
fn a_bare_start_opens_the_launcher_and_a_named_project_or_door_skips_it() {
    assert!(args::opens_the_launcher(&argv(&[])), "no argument: the launcher");
    assert!(args::opens_the_launcher(&argv(&["--live"])), "a flag that opens nothing names no project");
    for named in [&["."][..], &["/some/project"], &["--door", "kit"], &["--kit", "site"], &[".", "harbour"]] {
        assert!(!args::opens_the_launcher(&argv(named)), "{named:?} names what to open");
    }
}

#[test]
fn the_copied_line_run_headless_opens_the_same_project_and_door() {
    // A folder with a space and a quote in its name, so the line has to be quoted to mean the same thing.
    let root = Fixture::new(&format!("line_{}", std::process::id())).descriptor("bench", "props").kit("site", "ozea", &["site/wall"]).build("m");
    let odd = root.parent().unwrap().join(format!("it's here {}", std::process::id()));
    std::os::unix::fs::symlink(&root, &odd).unwrap();

    for (kit, door) in [(None, None), (Some("site"), Some("kit"))] {
        let line = launcher::command_line(&odd, kit, door);
        assert!(line.starts_with("bevy_cauldron '"), "the odd path is quoted: {line}");
        let words = shell_words(&line);
        assert_eq!(words[0], "bevy_cauldron");
        let from_line = args::open(&words[1..]).unwrap_or_else(|e| panic!("{e}"));
        let from_launcher = args::open(&launcher::argv_for(&odd, kit, door)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(from_line.project.root, from_launcher.project.root, "the same project");
        assert_eq!(from_line.project.root, odd.canonicalize().unwrap());
        assert_eq!(from_line.door, from_launcher.door, "the same door");
        assert_eq!(from_line.project.namespace, from_launcher.project.namespace, "the same kit");
        assert!(!args::opens_the_launcher(&words[1..]), "and it skips the launcher");
    }
    let _ = std::fs::remove_file(&odd);
}

#[test]
fn copy_command_line_shows_the_line_for_the_row_even_when_the_clipboard_cannot_take_it() {
    let root = Fixture::new(&format!("copy_{}", std::process::id())).descriptor("bench", "props").build("m");
    let scratch: PathBuf = std::env::temp_dir().join(format!("grimoire_copy_line_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let file = scratch.join("recent.ron");
    recent::record(&file, &root).unwrap();

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    let button = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Children)>();
        let mut texts = world.query::<(&Role, &Text)>();
        q.iter(world)
            .find_map(|(e, kids)| kids.iter().find_map(|k| texts.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == "Copy command line").map(|_| e)))
            .expect("a recent project's row has a Copy command line button")
    };
    app.world_mut().commands().trigger(Activate { entity: button });
    for _ in 0..3 {
        app.update();
    }

    let want = launcher::command_line(&root.canonicalize().unwrap(), None, None);
    assert_eq!(app.world().resource::<Launcher>().line.as_deref(), Some(want.as_str()));
    let mut q = app.world_mut().query::<(&Role, &Text)>();
    assert!(q.iter(app.world()).any(|(r, t)| *r == Role::Label && t.0 == want), "the line is on the screen, to copy by hand if the clipboard is out of reach");
    let _ = std::fs::remove_dir_all(&scratch);
}
