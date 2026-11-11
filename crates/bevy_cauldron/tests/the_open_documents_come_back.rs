//! **Document tabs** (NV-04, #381; series getting around, #377): inside a workspace the things you open
//! are tabs, and they come back when the project is opened again.
//!
//! - two maps and three files are open, the editor restarts, and the same tabs are there with the same
//!   one active — the session is the person's (`<data_dir>/Grimoire/projects/<hash of root>/session.yaml`),
//!   never the project's;
//! - the last tab closed is the one `Cmd+Shift+T` reopens;
//! - `Cmd+W` closes the active tab, `Ctrl+Tab` cycles, and choosing another map's tab opens that map;
//! - a map with unsaved edits asks before it is left, with the question the door switch already asks;
//! - a tab whose thing is gone closes, and the notice names it.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::session::{self, Doc, DocumentName, DirtyDot, Kind, Session, SessionStore};
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

const SETTLE: usize = 6;

/// A project with two maps and three files, and the directory its sessions are kept in.
struct Rig {
    root: std::path::PathBuf,
    store: std::path::PathBuf,
}

fn world(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(&format!("docs_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .build("m");
    fixtures::Fixture::write_map(&root, "n", 8.0, &[]);
    for file in ["a.md", "b.md", "c.md"] {
        std::fs::write(root.join(file), format!("# {file}\n")).unwrap_or_else(|e| panic!("{file}: {e}"));
    }
    let store = root.join(format!("data_{}", std::process::id()));
    Rig { root, store }
}

fn editor(w: &Rig, map: &str) -> App {
    let mut app = harness::build_headless(&w.root, map, None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(SessionStore(w.store.clone()));
    for _ in 0..SETTLE {
        app.update();
    }
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn tabs(app: &App, workspace: Workspace) -> (Vec<String>, Option<String>) {
    let t = app.world().resource::<Session>().tabs(workspace);
    (t.open.iter().map(|d| d.name.clone()).collect(), t.active_doc().map(|d| d.name.clone()))
}

fn open_map(app: &App) -> String {
    app.world().resource::<OpenMap>().map.name.clone()
}

/// **A chord held for two frames and let go**, as a keyboard does it.
fn chord(app: &mut App, keys: &[KeyCode]) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    settle(app, SETTLE);
}

fn primary() -> KeyCode {
    bevy_cauldron::keys::MOD_KEYS[0]
}

/// **Two maps and three files, a restart, and the same tabs with the same one active.**
#[test]
fn two_maps_and_three_files_come_back_with_the_same_one_active() {
    let w = world("restart");
    let mut app = editor(&w, "m");
    assert_eq!(tabs(&app, Workspace::Map), (vec!["m".to_owned()], Some("m".to_owned())), "the open map has a tab");

    // The second map, opened the way a tab opens it.
    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    assert_eq!(open_map(&app), "n", "choosing a map's tab opens that map");
    assert_eq!(tabs(&app, Workspace::Map), (vec!["m".to_owned(), "n".to_owned()], Some("n".to_owned())));

    // Three files; the middle one is the one the person was on.
    {
        let mut s = app.world_mut().resource_mut::<Session>();
        for file in ["a.md", "b.md", "c.md", "b.md"] {
            s.open(Workspace::Files, Doc::new(Kind::File, file)).unwrap_or_else(|e| panic!("{e}"));
        }
    }
    settle(&mut app, SETTLE);
    let file = session::path_for(&w.store, &w.root);
    assert!(file.is_file(), "the session is written under the person's data directory: {}", file.display());
    assert!(
        !w.root.join(session::FILE).exists() && !w.root.join("assets/grimoire").join(session::FILE).exists(),
        "and the project itself holds no session, so committing it never commits somebody's open tabs"
    );
    drop(app);

    // **Restart**: the editor opens the map it was on, and the tabs are read back.
    let app = editor(&w, "n");
    assert_eq!(tabs(&app, Workspace::Map), (vec!["m".to_owned(), "n".to_owned()], Some("n".to_owned())), "the two maps are back, on the same one");
    assert_eq!(
        tabs(&app, Workspace::Files),
        (vec!["a.md".to_owned(), "b.md".to_owned(), "c.md".to_owned()], Some("b.md".to_owned())),
        "the three files are back, on the same one"
    );
}

/// **Reopen restores the last tab that was closed.**
#[test]
fn reopen_closed_restores_the_last_tab_closed() {
    let w = world("reopen");
    let mut app = editor(&w, "m");
    {
        let mut s = app.world_mut().resource_mut::<Session>();
        for file in ["a.md", "b.md", "c.md"] {
            s.open(Workspace::Files, Doc::new(Kind::File, file)).unwrap_or_else(|e| panic!("{e}"));
        }
    }
    session::close_tab(app.world_mut(), Workspace::Files, &Doc::new(Kind::File, "a.md"));
    session::close_tab(app.world_mut(), Workspace::Files, &Doc::new(Kind::File, "b.md"));
    assert_eq!(tabs(&app, Workspace::Files).0, ["c.md"]);

    session::reopen_tab(app.world_mut(), Workspace::Files);
    assert_eq!(
        tabs(&app, Workspace::Files),
        (vec!["c.md".to_owned(), "b.md".to_owned()], Some("b.md".to_owned())),
        "b was the last tab closed, so it is the one that comes back, active"
    );
    session::reopen_tab(app.world_mut(), Workspace::Files);
    assert_eq!(tabs(&app, Workspace::Files).1.as_deref(), Some("a.md"), "and then the one closed before it");
}

/// **The keys**: `Cmd+W` closes the active map's tab and the map beside it opens, `Cmd+Shift+T` brings
/// it back, and `Ctrl+Tab` cycles.
#[test]
fn the_tab_keys_close_reopen_and_cycle() {
    let w = world("keys");
    let mut app = editor(&w, "m");
    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    assert_eq!(open_map(&app), "n");

    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Tab]);
    assert_eq!(open_map(&app), "m", "Ctrl+Tab goes to the next tab, which wraps, and a map's tab opens that map");
    assert_eq!(tabs(&app, Workspace::Map).1.as_deref(), Some("m"));
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::ShiftLeft, KeyCode::Tab]);
    assert_eq!(open_map(&app), "n", "and with Shift it goes the other way");

    chord(&mut app, &[primary(), KeyCode::KeyW]);
    assert_eq!(tabs(&app, Workspace::Map), (vec!["m".to_owned()], Some("m".to_owned())), "Cmd+W closed n's tab");
    assert_eq!(open_map(&app), "m", "and the map beside it is the one open now");

    chord(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyT]);
    assert_eq!(open_map(&app), "n", "Cmd+Shift+T reopens the tab that was closed, and with it the map");
    assert_eq!(tabs(&app, Workspace::Map), (vec!["m".to_owned(), "n".to_owned()], Some("n".to_owned())));

    // **The only map open cannot be closed under the person**: there would be none.
    chord(&mut app, &[primary(), KeyCode::KeyW]);
    chord(&mut app, &[primary(), KeyCode::KeyW]);
    assert_eq!(tabs(&app, Workspace::Map).0, ["m"], "the last map's tab stays");
    assert!(
        app.world().resource::<EditorState>().status.problems().iter().any(|p| p.line().contains("only map open")),
        "and the refusal says why"
    );
}

/// **A map with unsaved edits asks before another map's tab replaces it.**
#[test]
fn a_dirty_map_is_asked_about_before_its_tab_is_left() {
    let w = world("dirty");
    let mut app = editor(&w, "m");
    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    session::choose_map(app.world_mut(), "m");
    settle(&mut app, SETTLE);
    assert_eq!(open_map(&app), "m");

    bevy_cauldron::editor::place_for_test(app.world_mut(), "alpha/floor", (1.5, 0.5));
    settle(&mut app, SETTLE);
    assert!(app.world().resource::<OpenMap>().dirty(), "the test's own edit leaves the map unsaved");

    // The strip draws a dot on the dirty map's tab.
    let dots = {
        let world = app.world_mut();
        world.query::<&DirtyDot>().iter(world).count()
    };
    assert_eq!(dots, 1, "the dirty map's tab wears the dot");

    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    assert_eq!(open_map(&app), "m", "the map was not replaced under the person");
    let state = app.world().resource::<EditorState>();
    assert!(state.leaving, "the leaving question is up");
    assert_eq!(state.map_request.as_deref(), Some("n"), "and it remembers which map was asked for");

    // **Agreeing opens the other map**: the question is a refusal to lose work, not a refusal to go.
    chord(&mut app, &[KeyCode::KeyY]);
    assert_eq!(open_map(&app), "n", "agreeing opens the map that was asked for");
    assert!(!app.world().resource::<EditorState>().leaving, "and the question is down");
}

/// **The strip names every tab when there are two or more**, and none when there is one.
#[test]
fn the_strip_names_the_tabs() {
    let w = world("strip");
    let mut app = editor(&w, "m");
    let names = |app: &mut App| {
        let world = app.world_mut();
        let mut q = world.query::<(&DocumentName, &Text)>();
        let mut v: Vec<String> = q.iter(world).map(|(_, t)| t.0.clone()).collect();
        v.sort();
        v
    };
    assert!(names(&mut app).is_empty(), "one document is the title, not a strip");
    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    assert_eq!(names(&mut app), ["m", "n"], "two maps, two tabs, each named");
}

/// **A tab whose thing is gone closes, and the notice names it.**
#[test]
fn a_tab_whose_file_is_gone_closes_with_a_notice() {
    let w = world("gone");
    let mut app = editor(&w, "m");
    {
        let mut s = app.world_mut().resource_mut::<Session>();
        for file in ["a.md", "c.md"] {
            s.open(Workspace::Files, Doc::new(Kind::File, file)).unwrap_or_else(|e| panic!("{e}"));
        }
    }
    std::fs::remove_file(w.root.join("c.md")).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, 70);
    assert_eq!(tabs(&app, Workspace::Files).0, ["a.md"], "the deleted file's tab is closed");
    assert!(
        app.world().resource::<EditorState>().status.note_text().contains("c.md"),
        "the notice names it: {:?}",
        app.world().resource::<EditorState>().status.note_text()
    );
}

/// **What a door change does to the editor**, as far as the session goes: the project and its open map
/// are replaced, and the session must follow the new project (`restore` sees the new root).
fn move_to(app: &mut App, root: &std::path::Path, map: &str) {
    let project = Project::open(root, None).unwrap_or_else(|e| panic!("{e}"));
    let open = OpenMap::open(&project, map).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().insert_resource(project);
    app.world_mut().insert_resource(open);
    settle(app, 70);
}

/// A second project with one map, `q`, and one file, `b.md`.
fn project_b(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("docs_{tag}_b_{}", std::process::id())).descriptor("alpha/floor", "alpha").build("q");
    std::fs::write(root.join("b.md"), "# b\n").unwrap_or_else(|e| panic!("{e}"));
    root
}

/// **Another project's tabs never carry into a project that has no session file**, and the file is not
/// written until the person changes something — the editor following its own open map is not a change.
#[test]
fn a_project_with_no_session_starts_without_the_last_projects_tabs() {
    let a = world("carry");
    let b = project_b("carry");
    let mut app = editor(&a, "m");
    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    app.world_mut().resource_mut::<Session>().open(Workspace::Files, Doc::new(Kind::File, "a.md")).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, SETTLE);
    assert_eq!(tabs(&app, Workspace::Map).0, ["m", "n"], "A has two map tabs");
    assert_eq!(tabs(&app, Workspace::Files).0, ["a.md"], "and one file tab");
    let b_file = session::path_for(&a.store, &b);
    assert!(!b_file.exists(), "B has never had a session");

    move_to(&mut app, &b, "q");
    assert_eq!(tabs(&app, Workspace::Map), (vec!["q".to_owned()], Some("q".to_owned())), "only B's map is found");
    assert!(tabs(&app, Workspace::Files).0.is_empty(), "and none of A's files carried over");
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(!note.contains("closed the tab"), "no tab of A's is closed for being gone from B: {note:?}");
    assert!(
        !b_file.exists(),
        "B is given no session file by the editor opening its own map: {}",
        b_file.display()
    );

    // **A real change is what writes it**, and what it holds is B's alone.
    app.world_mut().resource_mut::<Session>().open(Workspace::Files, Doc::new(Kind::File, "b.md")).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, SETTLE);
    let written = Session::read(&b_file).unwrap_or_else(|e| panic!("B's session is written once the person opens a tab: {e}"));
    assert_eq!(written.tabs(Workspace::Map).open.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["q"], "and holds only B's map");
    assert_eq!(written.tabs(Workspace::Files).open.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["b.md"], "and only B's own file");
}

/// **A session file that will not read is left byte for byte as it is**, and A's tabs do not carry over.
#[test]
fn an_unreadable_session_file_is_never_overwritten() {
    let a = world("unreadable");
    let b = project_b("unreadable");
    let mut app = editor(&a, "m");
    session::choose_map(app.world_mut(), "n");
    settle(&mut app, SETTLE);
    assert_eq!(tabs(&app, Workspace::Map).0, ["m", "n"], "A has two map tabs");

    let b_file = session::path_for(&a.store, &b);
    std::fs::create_dir_all(b_file.parent().unwrap_or_else(|| panic!("a parent"))).unwrap_or_else(|e| panic!("{e}"));
    // A newer editor's file: a version this one refuses by name.
    let newer = "version: 99\nworkspaces: {}\n";
    std::fs::write(&b_file, newer).unwrap_or_else(|e| panic!("{e}"));

    move_to(&mut app, &b, "q");
    assert_eq!(tabs(&app, Workspace::Map).0, ["q"], "A's tabs did not carry into B");
    assert!(
        app.world().resource::<EditorState>().status.problems().iter().any(|p| p.line().contains("NOT RESTORED")),
        "and the person is told it was not read"
    );

    // **Even a real change does not write over it**: the tabs are the person's, the file is not this
    // editor's to replace.
    app.world_mut().resource_mut::<Session>().open(Workspace::Files, Doc::new(Kind::File, "b.md")).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, SETTLE);
    assert_eq!(
        std::fs::read_to_string(&b_file).unwrap_or_else(|e| panic!("{e}")),
        newer,
        "the file a newer editor wrote is byte-identical, even after the tabs change"
    );
}
