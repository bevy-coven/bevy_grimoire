//! **The outline of a page can be read, shown and clicked** (SG-04, #403): what a person and an agent reach, and not the component.
//!
//! - An agent reads `grimoire_state.file_outline`: the open file's path and its headings (`level`, `title`, `line` from 1), from the text
//!   the editor holds, so a heading typed and not yet saved is in it.
//! - A person chooses View ▸ Show outline: a gooey tree of the headings is spawned into the Files panel, and taken away again when the row
//!   is chosen again; nothing is there before it is asked for.
//! - A click on a heading puts the caret on its line.
//! - A link that climbs out of the project is no finding and says nothing on hover, in the editor as in the reader.

use std::time::Duration;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::file_outline::{OutlineItem, OutlineTree};
use bevy_cauldron::files::{self, DIAGNOSE_IDLE_S};
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodePos, CodeHover, CodeHoverRequested, Diagnostics, Table, TableSelection, Tree, ValueChange};
use serde_json::{Value, json};

use crate::fixtures;

const SETTLE: usize = 12;

const PAGE: &str = "design/levels/m/style.md";
const PAGE_TEXT: &str = "# Yard\n\nGrey concrete.\n\n## Light\n\n### Dusk\n\n## Fog\n\ntext\n";

fn files_editor(tag: &str, text: &str) -> App {
    let root = fixtures::Fixture::new(&format!("outline_{tag}_{}", std::process::id())).descriptor("alpha/floor", "alpha").place_as("floor@0", "alpha/floor", (0.0, 0.0)).build("m");
    let page = root.join(PAGE);
    std::fs::create_dir_all(page.parent().unwrap_or(&root)).unwrap();
    std::fs::write(&page, text).unwrap();
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files);
    files::open_file(app.world_mut(), PAGE).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let editor = editor_of(&mut app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn editor_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
}

fn grimoire_state(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    app.world_mut().run_system_with(id, Some(json!({}))).unwrap_or_else(|e| panic!("{e:?}")).unwrap_or_else(|e| panic!("{e:?}"))
}

/// The person chose View ▸ Show outline (or chose it again).
fn show_the_outline(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlineItem>>().single(world).expect("the View menu carries one Show outline row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    settle(app);
}

fn trees(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<OutlineTree>>().iter(world).collect()
}

fn ticked(app: &mut App) -> bool {
    let world = app.world_mut();
    world.query_filtered::<Has<bevy_gooey::Checked>, With<OutlineItem>>().single(world).expect("one row")
}

#[test]
fn an_agent_reads_the_open_pages_headings_from_the_state_as_the_editor_holds_them() {
    let mut app = files_editor("state", PAGE_TEXT);
    let state = grimoire_state(&mut app);
    assert_eq!(
        state["file_outline"],
        json!({"file": PAGE, "headings": [
            {"level": 1, "title": "Yard", "line": 1},
            {"level": 2, "title": "Light", "line": 5},
            {"level": 3, "title": "Dusk", "line": 7},
            {"level": 2, "title": "Fog", "line": 9},
        ]}),
        "{state}"
    );

    // A heading typed and not saved is in the next answer.
    let editor = editor_of(&mut app);
    let end = CodePos::new(PAGE_TEXT.matches('\n').count(), 0);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(end, end)).unwrap_or_else(|| panic!("an editor"));
    settle(&mut app);
    bevy_gooey::testing::type_text(&mut app, "# Typed");
    let state = grimoire_state(&mut app);
    let titles: Vec<&str> = state["file_outline"]["headings"].as_array().unwrap().iter().map(|h| h["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Yard", "Light", "Dusk", "Fog", "Typed"], "{state}");
}

#[test]
fn view_show_outline_draws_the_headings_as_rows_and_takes_them_away_again() {
    let mut app = files_editor("rows", PAGE_TEXT);
    assert!(trees(&mut app).is_empty(), "nothing is drawn until it is asked for");
    assert!(!ticked(&mut app));

    show_the_outline(&mut app, true);
    let [tree] = trees(&mut app).try_into().expect("one outline tree");
    assert!(ticked(&mut app), "the tick says it is there");
    let table = app.world().get::<Table>(tree).expect("a gooey table");
    let source = table.source::<Tree>().expect("a tree");
    let labels: Vec<String> = (0..4u64).map(|k| source.label(k).unwrap_or_default().to_owned()).collect();
    assert_eq!(labels, ["Yard", "Light", "Dusk", "Fog"], "one row per heading, in order");
    assert!(!source.contains(4), "and no row more");

    show_the_outline(&mut app, false);
    assert!(trees(&mut app).is_empty(), "it is taken away, not hidden");
    assert!(!ticked(&mut app));
}

#[test]
fn a_click_on_a_heading_puts_the_caret_on_its_line() {
    let mut app = files_editor("click", PAGE_TEXT);
    show_the_outline(&mut app, true);
    let [tree] = trees(&mut app).try_into().expect("one outline tree");
    let editor = editor_of(&mut app);
    for (key, line) in [(3u64, 8usize), (1, 4), (2, 6)] {
        app.world_mut().commands().trigger(ValueChange { source: tree, value: TableSelection::of(key), is_final: true });
        settle(&mut app);
        let caret = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().caret()).unwrap_or_else(|| panic!("an editor"));
        assert_eq!(caret.line, line, "heading {key} is on line {line}");
    }
}

#[test]
fn a_link_that_climbs_out_of_the_project_is_no_finding_and_says_nothing_on_hover() {
    let text = "# Yard\n\n[x](../../../../etc/passwd)\n";
    let mut app = files_editor("passwd", text);
    std::thread::sleep(Duration::from_secs_f64(DIAGNOSE_IDLE_S + 0.05));
    settle(&mut app);
    let editor = editor_of(&mut app);
    let drawn = app.world().get::<Diagnostics>(editor).map(|d| d.items().len()).unwrap_or(0);
    assert_eq!(drawn, 0, "it names no file of the project, so it is not one that is missing");
    let at = text.find("passwd").map_or(0, |at| at + 1);
    let revision = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    app.world_mut().trigger(CodeHoverRequested { entity: editor, offset: at, revision });
    app.update();
    assert!(app.world().get::<CodeHover>(editor).is_none(), "and nothing is said of it");
}
