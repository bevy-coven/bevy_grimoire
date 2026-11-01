//! **The editor remembers the projects it opened, most recent first** (UI-11, #217).
//!
//! `<data_dir>/Grimoire/recent.ron` lists them. Opening one moves it to the top, and a folder that
//! has since gone is listed as missing rather than dropped, so a moved project is one the author can
//! find and point the list at again.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire_mapper::recent::{self, Recent, RecentProjects};
use bevy_grimoire_mapper::screen::Screen;

use crate::fixtures;

/// A scratch directory unique to its test, removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_recent_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn folder(&self, name: &str) -> PathBuf {
        let dir = self.0.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file(&self) -> PathBuf {
        self.0.join("data").join("Grimoire").join("recent.ron")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn paths(list: &[recent::Listed]) -> Vec<&Path> {
    list.iter().map(|l| l.project.path.as_path()).collect()
}

#[test]
fn opening_a_project_moves_it_to_the_top_and_a_deleted_folder_is_listed_as_missing_not_dropped() {
    let s = Scratch::new("top");
    let (a, b, c) = (s.folder("a"), s.folder("b"), s.folder("c"));
    let file = s.file();

    for p in [&a, &b, &c] {
        recent::record(&file, p).unwrap();
    }
    let list = Recent::load(&file).unwrap().listing();
    let (ca, cb, cc) = (a.canonicalize().unwrap(), b.canonicalize().unwrap(), c.canonicalize().unwrap());
    assert_eq!(paths(&list), [cc.as_path(), cb.as_path(), ca.as_path()], "the last opened is first");

    // The oldest is opened again, spelled another way (a `.` segment), and is still one entry.
    recent::record(&file, &a.join(".")).unwrap();
    let list = Recent::load(&file).unwrap().listing();
    assert_eq!(paths(&list), [ca.as_path(), cc.as_path(), cb.as_path()], "opening it again moves it to the top, once");

    // A folder deleted from disk stays listed, marked missing; the others are not.
    std::fs::remove_dir_all(&b).unwrap();
    let list = Recent::load(&file).unwrap().listing();
    assert_eq!(paths(&list), [ca.as_path(), cc.as_path(), cb.as_path()], "a deleted folder is not dropped");
    assert_eq!(list.iter().map(|l| l.missing).collect::<Vec<_>>(), [false, false, true]);
}

#[test]
fn a_pinned_project_stays_above_the_rest_and_a_removed_one_leaves_only_the_list() {
    let s = Scratch::new("pin");
    let (a, b, c) = (s.folder("a"), s.folder("b"), s.folder("c"));
    let mut r = Recent::default();
    for p in [&a, &b, &c] {
        r.opened(p, 1);
    }
    assert!(r.pin(&a, true), "the oldest is listed, so it pins");
    r.opened(&c, 2);
    let list = r.listing();
    let (ca, cb, cc) = (a.canonicalize().unwrap(), b.canonicalize().unwrap(), c.canonicalize().unwrap());
    assert_eq!(paths(&list), [ca.as_path(), cc.as_path(), cb.as_path()], "pinned first, then most recent");

    r.opened(&a, 3);
    assert!(r.listing()[0].project.pinned, "opening a pinned project keeps it pinned");

    assert!(r.remove(&b));
    assert!(!r.remove(&b), "a second removal finds nothing");
    assert!(b.is_dir(), "removing an entry never touches the folder");
    assert!(!r.pin(&b, true), "an unlisted path does not pin");
}

#[test]
fn locating_a_missing_project_keeps_its_place_and_refuses_what_is_not_a_folder() {
    let s = Scratch::new("locate");
    let (a, b) = (s.folder("a"), s.folder("b"));
    let mut r = Recent::default();
    r.opened(&a, 1);
    r.opened(&b, 2);
    r.pin(&a, true);
    let moved = s.0.join("moved");
    std::fs::rename(&a, &moved).unwrap();
    assert!(r.listing()[0].missing, "the moved folder is listed as missing");

    let gone = s.0.join("nowhere");
    // The entry was recorded under its canonical path, which no longer exists; the caller names it as it was shown.
    let shown = r.listing()[0].project.path.clone();
    assert!(r.locate(&shown, &gone).unwrap_err().contains("not a folder"));
    assert!(r.locate(&shown, &b).unwrap_err().contains("already in the recent projects"));
    assert!(r.locate(&s.0.join("never"), &moved).unwrap_err().contains("not in the recent projects"));

    r.locate(&shown, &moved).unwrap();
    let list = r.listing();
    assert_eq!(list[0].project.path, moved.canonicalize().unwrap(), "it now points at the new place");
    assert!(!list[0].missing && list[0].project.pinned && list[0].project.last_opened == 1, "and kept its pin and its time");
}

#[test]
fn a_list_of_another_version_is_refused_by_name_and_left_as_it_is() {
    let s = Scratch::new("version");
    let a = s.folder("a");
    let file = s.file();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let newer = "(version: 2, projects: [])";
    std::fs::write(&file, newer).unwrap();

    let err = recent::record(&file, &a).unwrap_err();
    assert!(err.contains("version 2") && err.contains("recent.ron"), "the refusal names the version and the file: {err}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), newer, "a list a newer build wrote is not overwritten");
}

#[test]
fn a_list_survives_a_round_trip_through_its_file() {
    let s = Scratch::new("round");
    let a = s.folder("a");
    let mut r = Recent::default();
    r.opened(&a, 7);
    r.pin(&a, true);
    r.save(&s.file()).unwrap();
    assert_eq!(Recent::load(&s.file()).unwrap(), r);
    assert_eq!(Recent::load(&s.0.join("absent.ron")).unwrap(), Recent::default(), "no file is an empty list");
}

/// **Every way into the editor records the opening**, because `open_the_door` is the one way in: an
/// agent's `map_open` from the menu goes through it as a person's Enter does.
#[test]
fn entering_the_editor_from_the_menu_records_the_project() {
    let s = Scratch::new("enter");
    let root = fixtures::Fixture::new("recent_enter").descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    app.add_plugins(bevy_grimoire_mapper::chooser::ChooserPlugin { root: root.clone(), preselect: None });
    app.insert_resource(RecentProjects(s.file()));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Menu);
    for _ in 0..5 {
        app.update();
    }
    assert!(!s.file().exists(), "being on the menu opens nothing");

    let id = match app.world().resource::<bevy::remote::RemoteMethods>().get("grimoire/operation_apply") {
        Some(bevy::remote::RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("operation_apply is registered"),
    };
    let params = serde_json::json!({"operation": "map_open", "parameters": {"name": "m"}, "dry_run": false});
    let answer = app.world_mut().run_system_with(id, Some(params)).unwrap().unwrap();
    assert_eq!(answer["outcome"], "applied", "{answer}");
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Editor);

    let list = Recent::load(&s.file()).unwrap().listing();
    assert_eq!(paths(&list), [root.canonicalize().unwrap().as_path()], "the opened project is the one entry");
    assert!(!list[0].missing);
}
