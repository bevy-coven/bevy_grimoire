//! **The editor's small panels are laid out with bevy_gooey's `row` and `column`, not raw `Node` literals** (#260, UI-99, slice 1).
//!
//! `the_editor_keeps_its_raw_nodes_counted.rs` holds the count down. This file measures the elements that stopped being raw: each
//! is asked what it is on screen (its gooey `Role`) and what the skin laid it out at (its resolved gap), so a swap that left a
//! hand-built node standing in a gooey widget's place, or a gap that moved by the swap, fails here and not in a screenshot.

use bevy::prelude::*;
use bevy_cauldron::chrome::GAP_TIGHT;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::Role;

use crate::fixtures;

/// A headless editor opened on `mode`, settled.
fn editor(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("small_gooey_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// The one entity carrying marker `M`.
fn the<M: Component>(app: &mut App) -> Entity {
    let mut found = app.world_mut().query_filtered::<Entity, With<M>>();
    found.iter(app.world()).next().unwrap_or_else(|| panic!("an entity carrying {} is on screen", std::any::type_name::<M>()))
}

/// The parent of `entity`.
fn parent(app: &App, entity: Entity) -> Entity {
    app.world().get::<ChildOf>(entity).map(ChildOf::parent).unwrap_or_else(|| panic!("{entity:?} has a parent"))
}

/// Every gooey widget below is asked the same three things: its role, its direction and the gap the skin wrote.
fn assert_column(app: &App, entity: Entity, what: &str) {
    assert_eq!(app.world().get::<Role>(entity), Some(&Role::Column), "{what} is a gooey `column`, not a raw `Node`");
    let node = app.world().get::<Node>(entity).unwrap_or_else(|| panic!("{what} has a Node"));
    assert_eq!(node.flex_direction, FlexDirection::Column, "{what} stacks its children");
    let Val::Px(gap) = node.row_gap else { panic!("{what}: the skin writes the gap in px, found {:?}", node.row_gap) };
    assert!((gap - GAP_TIGHT).abs() < 0.01, "{what} keeps the tight gap between its rows: {gap} px, wanted {GAP_TIGHT}");
}

/// A gooey `row` whose cells stand the tight gap apart.
fn assert_row(app: &App, entity: Entity, what: &str) {
    assert_eq!(app.world().get::<Role>(entity), Some(&Role::Row), "{what} is a gooey `row`, not a raw `Node`");
    let node = app.world().get::<Node>(entity).unwrap_or_else(|| panic!("{what} has a Node"));
    assert_eq!(node.flex_direction, FlexDirection::Row, "{what} lays its cells in a line");
    let Val::Px(gap) = node.column_gap else { panic!("{what}: the skin writes the gap in px, found {:?}", node.column_gap) };
    assert!((gap - GAP_TIGHT).abs() < 0.01, "{what} keeps the tight gap between its cells: {gap} px, wanted {GAP_TIGHT}");
}

/// **The World workspace**: each section's block and each list in it is a gooey column.
#[test]
fn the_world_workspaces_blocks_and_lists_are_gooey_columns() {
    use bevy_cauldron::world_lists::{ForestList, LightsList, WaterList};
    let mut app = editor("world", Mode::World);
    let water = the::<WaterList>(&mut app);
    assert_column(&app, water, "the water list");
    let found = the::<ForestList>(&mut app);
    assert_column(&app, found, "the forest list");
    let found = the::<LightsList>(&mut app);
    assert_column(&app, found, "the lights list");
    assert_column(&app, parent(&app, water), "the water section's block");
}

/// **The Sound workspace**: its cue lists are gooey columns too.
#[test]
fn the_sound_workspaces_lists_are_gooey_columns() {
    use bevy_cauldron::world_lists::{CueList, CueUses};
    let mut app = editor("sound", Mode::Sound);
    let cues = the::<CueList>(&mut app);
    assert_column(&app, cues, "the cue list");
    let found = the::<CueUses>(&mut app);
    assert_column(&app, found, "the cue uses list");
    assert_column(&app, parent(&app, cues), "the audio section's block");
}

/// **The Game workspace**: the PLAY and BUILD sections' button lines are gooey rows, and every block stacks as a gooey column.
#[test]
fn the_game_workspaces_sections_are_gooey_rows_and_columns() {
    use bevy_cauldron::build_view::BuildButton;
    use bevy_cauldron::game_view::{GameEntities, GameFieldFiles};
    use bevy_cauldron::ops_play::PlayButton;
    let mut app = editor("game", Mode::Game);
    let play = {
        let mut q = app.world_mut().query::<(Entity, &PlayButton)>();
        q.iter(app.world()).find(|(_, which)| **which == PlayButton::Play).map(|(e, _)| e).expect("the Play button is on screen")
    };
    let line = parent(&app, play);
    assert_row(&app, line, "the PLAY line");
    assert_column(&app, parent(&app, line), "the PLAY section's block");
    let build = the::<BuildButton>(&mut app);
    let line = parent(&app, build);
    assert_row(&app, line, "the BUILD line");
    let node = app.world().get::<Node>(line).expect("the BUILD line has a Node");
    assert_eq!(node.flex_shrink, 0.0, "the BUILD line keeps its height when the panel is short");
    assert_eq!(node.align_items, AlignItems::Center, "the Build button and its line of words stand level");
    assert_column(&app, parent(&app, line), "the BUILD section's block");
    let found = the::<GameEntities>(&mut app);
    assert_column(&app, found, "the entity list");
    let found = the::<GameFieldFiles>(&mut app);
    assert_column(&app, found, "the field files list");
}

/// **The Map's outliner** is a gooey column. It is spawned on demand (View > Show outliner), so the row is asked first.
#[test]
fn the_outliner_is_a_gooey_column() {
    use bevy_cauldron::outliner::{Outliner, OutlinerItem};
    let mut app = editor("outliner", Mode::Map);
    let row = the::<OutlinerItem>(&mut app);
    app.world_mut().commands().trigger(bevy_gooey::ValueChange { source: row, value: true, is_final: true });
    for _ in 0..6 {
        app.update();
    }
    let found = the::<Outliner>(&mut app);
    assert_column(&app, found, "the outliner");
}

/// **The Agent workspace's history** is a gooey column, in a block that is a gooey column and keeps its height.
#[test]
fn the_agent_workspaces_history_is_a_gooey_column() {
    use bevy_cauldron::agent_view::AgentHistory;
    let mut app = editor("agent", Mode::Agent);
    let history = the::<AgentHistory>(&mut app);
    assert_column(&app, history, "the agent's history list");
    let block = parent(&app, history);
    assert_column(&app, block, "the history block");
    let node = app.world().get::<Node>(block).expect("the block has a Node");
    assert_eq!(node.flex_shrink, 0.0, "the history block keeps its height when the pane is short");
}

/// **The library browser's section** is a gooey column, its tag toggles stand in a wrapping gooey row, and the grid's place is a gooey row.
#[test]
fn the_library_browser_is_a_gooey_column_with_a_wrapping_row_of_tags() {
    use bevy_cauldron::library_browser::{LibraryBrowser, LibraryItem, LibraryTags};
    let mut app = editor("library", Mode::Meshes);
    let row = the::<LibraryItem>(&mut app);
    app.world_mut().commands().trigger(bevy_gooey::ValueChange { source: row, value: true, is_final: true });
    for _ in 0..6 {
        app.update();
    }
    let browser = the::<LibraryBrowser>(&mut app);
    assert_column(&app, browser, "the library section");
    let tags = the::<LibraryTags>(&mut app);
    assert_row(&app, tags, "the tag toggles' line");
    let node = app.world().get::<Node>(tags).expect("the tags line has a Node");
    assert_eq!(node.flex_wrap, FlexWrap::Wrap, "the tag toggles wrap onto a second line instead of running off the panel");
}

/// **Every panel heading's row** (the title and its "?") is a gooey row whose cells stand level.
#[test]
fn every_panel_headings_row_is_a_gooey_row_level_with_its_words() {
    use bevy_cauldron::help::PanelHeading;
    let mut app = editor("heading", Mode::Map);
    let rows: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<PanelHeading>>().iter(app.world()).collect();
    assert!(!rows.is_empty(), "at least one panel has a heading row");
    for row in rows {
        assert_row(&app, row, "a heading row");
        let node = app.world().get::<Node>(row).expect("the heading row has a Node");
        assert_eq!(node.align_items, AlignItems::Center, "the \"?\" stands level with the heading's words");
    }
}

/// **The Familiar's readout lines** (HEARD, NEWEST, HAND-BACK, PLAN) are gooey rows that keep their height and start their cells at the top.
#[test]
fn the_familiars_readout_lines_are_gooey_rows_that_are_never_squeezed() {
    use bevy_cauldron::familiar::FamiliarLine;
    let mut app = editor("familiar", Mode::Agent);
    let lines: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<FamiliarLine>>().iter(app.world()).collect();
    assert_eq!(lines.len(), 3, "the HEARD, NEWEST and HAND-BACK values are on screen");
    for value in lines {
        // The value's text sits in a growing column, and that column in the line's row.
        let line = parent(&app, parent(&app, value));
        assert_eq!(app.world().get::<Role>(line), Some(&Role::Row), "a readout line is a gooey `row`, not a raw `Node`");
        let node = app.world().get::<Node>(line).expect("the readout line has a Node");
        assert_eq!(node.flex_shrink, 0.0, "a readout line keeps its height when the panel is short");
        assert_eq!(node.align_items, AlignItems::FlexStart, "the label and the wrapped value start at the line's top");
    }
}

/// **The Map inspector's sections** (the problems list, the composition picker) are gooey columns inside gooey section blocks.
#[test]
fn the_map_inspectors_lists_are_gooey_columns_in_gooey_blocks() {
    use bevy_cauldron::compose::picker::PickerBody;
    use bevy_cauldron::problems_view::ProblemsList;
    let mut app = editor("inspector", Mode::Map);
    let problems = the::<ProblemsList>(&mut app);
    assert_column(&app, problems, "the problems list");
    assert_column(&app, parent(&app, problems), "the problems section's block");
    let picker = the::<PickerBody>(&mut app);
    assert_column(&app, picker, "the composition picker's body");
    assert_column(&app, parent(&app, picker), "the compositions section's block");
}
