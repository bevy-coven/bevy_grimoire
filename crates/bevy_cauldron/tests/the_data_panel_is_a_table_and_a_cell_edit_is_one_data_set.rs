//! **The DATA panel is a sortable table, and a cell edited in place is one `data_set`** (UI-56, #242).
//!
//! The registered files' leaves were one text row each; they are a gooey table now (file, path, value), so a column sorts and a row
//! opens. Opening a row puts a text field over its value; its final change writes through `data_set`, so the file shows the value,
//! the oplog holds one line for it and one undo takes it back. With no `data.yaml` there is no table, only the line that says so.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::editor::DataRows;
use bevy_cauldron::harness;
use bevy_cauldron::ops;
use bevy_cauldron::project::Project;
use bevy_gooey::testing;
use bevy_gooey::{RowActivated, RowSource, Table, TextInputOf, TextInputValue};

use crate::fixtures;

const TEXT: &str = "// tuning\n(mob: (speed: 1.50, name: \"orc\", hp: 7), fast: false)\n";

fn editor(tag: &str, registry: bool) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..4 {
        app.update();
    }
    let dir = app.world().resource::<Project>().grimoire_dir.clone();
    let file = dir.join("mob.yaml");
    std::fs::write(&file, TEXT).unwrap();
    if registry {
        std::fs::write(dir.join("data.yaml"), "{version: 1, files: [{id: \"tuning/mob\", path: \"mob.yaml\", bounds: {\"mob.speed\": {min: 0.0, max: 9.0}}}]}").unwrap();
    }
    settle(&mut app);
    (app, file)
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
}

fn tables(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<Table>>().iter(world).collect()
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect()
}

#[test]
fn the_registered_leaves_are_one_table_with_a_row_each() {
    let (mut app, _) = editor(&format!("data_table_rows_{}", std::process::id()), true);
    let table = tables(&mut app);
    assert_eq!(table.len(), 1, "one table");
    let dir = app.world().resource::<Project>().grimoire_dir.clone();
    let want = bevy_cauldron::ops_data::listing(&dir);
    assert!(!want.is_empty());
    let shown = app.world().get::<Table>(table[0]).unwrap().source::<DataRows>().map(RowSource::len);
    assert_eq!(shown, Some(want.len()), "a row per leaf");
}

#[test]
fn a_project_with_no_data_file_has_no_table_and_says_so() {
    let (mut app, _) = editor(&format!("data_table_none_{}", std::process::id()), false);
    assert!(tables(&mut app).is_empty());
    assert!(texts(&mut app).iter().any(|t| t == "no data files registered (data.yaml)"));
}

#[test]
fn a_cell_edited_in_place_is_one_data_set_in_the_log_and_the_file() {
    let (mut app, file) = editor(&format!("data_table_edit_{}", std::process::id()), true);
    let table = tables(&mut app)[0];
    let key = {
        let rows = app.world().get::<Table>(table).unwrap().source::<DataRows>().expect("the data rows").clone();
        let at = (0..rows.len()).find(|&i| rows.cell(i, 1) == "mob.speed").expect("the speed leaf is a row");
        rows.key(at)
    };
    app.world_mut().commands().trigger(RowActivated { entity: table, key });
    settle(&mut app);
    // The field that has the keys is the cell's: it stands in a text row that holds the leaf's value as the file has it.
    let field = testing::focused(&app).expect("opening the row gives a field the keys");
    let row = app.world().get::<TextInputOf>(field).expect("the focus is on a text field").0;
    assert_eq!(app.world().get::<TextInputValue>(row).map(|v| v.0.as_str()), Some("1.50"), "the field holds the leaf's value");
    for _ in 0.."1.50".len() {
        testing::press(&mut app, KeyCode::Backspace);
    }
    testing::type_text(&mut app, "2.5");
    testing::press(&mut app, KeyCode::Enter);
    settle(&mut app);

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "// tuning\n(mob: (speed: 2.5, name: \"orc\", hp: 7), fast: false)\n");
    let project = app.world().resource::<Project>().clone();
    let sets: Vec<_> = ops::read_entries(&ops::oplog_path(&project)).into_iter().filter(|e| e.op == "data_set").collect();
    assert_eq!(sets.len(), 1, "one data_set line: {sets:?}");
    assert_eq!(sets[0].params["path"], "mob.speed");
    assert!(app.world().get_entity(row).is_err(), "the field is gone once it has written");
}
