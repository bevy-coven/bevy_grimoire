//! **The history panel is a virtualized list: 5,000 log lines scroll, and only the rows on screen exist**
//! (UI-53, #239).
//!
//! The panel used to be a fixed seven-row window with no scroll: seven `HistorySlot`s around the current
//! position, and everything older or newer than three steps either way was unreachable except by pressing
//! Cmd+Z. UI-53's first acceptance line is "5,000 log lines scroll within the UI frame budget", and this
//! file is that line:
//!
//! - a session of 5,000 entries stands in gooey's virtualized `tree`, whose nodes are one per entry and
//!   whose rows are the pool: **the rows in the world are the ones on screen**, not one per entry;
//! - a wheel over the list moves it, and the pool follows the scroll to the end of the session — the
//!   row for position 5,000 is drawn once the person gets there;
//! - an agent's entry reads "the Familiar", and a person's reads "you", which is the second acceptance
//!   line the window already met and the list has to keep.
//!
//! Nothing here calls the panel's own state: the wheel is a pointer message at the list's box and the
//! rows are read off the widget's own `TableRow`, so a list that stopped scrolling would fail here.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy_cauldron::editor::{self, EditorState, HistoryTree};
use bevy_cauldron::project::OpenMap;
use bevy_gooey::{Table, TableRow, TableRows, Tree};

use crate::fixtures;

/// The session the issue names: 5,000 log lines.
const HISTORY_LINES: usize = 5_000;

/// How many of a session's entries may be entities at once. gooey's pool is the rows in view and one
/// more, and the list stands seven rows tall, so anything near the session's length is a list that built
/// one widget per entry.
const MOST_ROWS_IN_THE_WORLD: usize = 12;

/// A whole wheel's travel, in pixels: enough to reach the end of a five-thousand-line session in a
/// handful of notches whatever the skin's row pitch is.
const WHEEL_TRAVEL_PX: f32 = 100_000.0;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// The editor on a one-piece map, with a session of `HISTORY_LINES` entries on its history and the agent's
/// own change newest.
fn rig(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.0, 1.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 6);
    {
        let mut state = app.world_mut().resource_mut::<EditorState>();
        for _ in 0..HISTORY_LINES - 1 {
            editor::plant_history_entry_for_test(&mut state);
        }
        // **The agent's change is the newest**, so the row for the current position says whose it is.
        editor::record_agent_history_entry_for_test(&mut state);
    }
    updates(&mut app, 4);
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), HISTORY_LINES, "the session is the issue's depth");
    app
}

fn history_tree(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<HistoryTree>>().single(world).expect("the INSPECTOR draws one history list")
}

/// **What a row of the list draws**, walked to the text: a tree's row holds cells and a cell holds the text.
fn row_label(world: &World, entity: Entity) -> Option<String> {
    if let Some(text) = world.get::<Text>(entity)
        && !text.0.is_empty()
    {
        return Some(text.0.clone());
    }
    world.get::<Children>(entity)?.iter().find_map(|kid| row_label(world, kid))
}

/// Every row of the history list standing in the world now: `(position, label)`.
fn rows_in_the_world(app: &mut App) -> Vec<(u64, String)> {
    let tree = history_tree(app);
    let world = app.world_mut();
    let mut q = world.query::<(&TableRow, Entity)>();
    let mut found: Vec<(u64, Entity)> = q.iter(world).filter(|(row, _)| row.table == tree).map(|(row, e)| (row.key, e)).collect();
    let mut out: Vec<(u64, String)> = found
        .drain(..)
        .map(|(key, entity)| (key, row_label(world, entity).unwrap_or_default()))
        .collect();
    out.sort_by_key(|(key, _)| *key);
    out
}

/// The list's viewport: the scrolled box gooey makes under the table.
fn viewport(app: &mut App) -> (Entity, Vec2, f32) {
    let tree = history_tree(app);
    let world = app.world_mut();
    let node = *world.get::<ComputedNode>(tree).expect("the list is laid out");
    let at = *world.get::<UiGlobalTransform>(tree).expect("the list is placed");
    let scale = node.inverse_scale_factor();
    (tree, at.translation * scale, node.size().y * scale)
}

/// **One wheel notch over the history list**, as a mouse sends it: aimed at the list's box first, then the
/// scroll itself. `travel` is in pixels, downward.
fn wheel(app: &mut App, travel: f32) {
    let (_, centre, _) = viewport(app);
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: centre,
    };
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        location.clone(),
        PointerAction::Move { delta: Vec2::ZERO },
    ));
    app.update();
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        PointerAction::Scroll { unit: MouseScrollUnit::Pixel, x: 0.0, y: -travel, phase: TouchPhase::Moved },
    ));
    app.update();
    updates(app, 2);
}

/// **5,000 entries are 5,000 nodes and a screenful of rows.** The session is whole in the list's data, and
/// the entities are the pool: gooey draws the rows in view and no others.
#[test]
fn five_thousand_lines_are_five_thousand_nodes_and_a_screenful_of_rows() {
    let mut app = rig("history_virtual");
    let tree = history_tree(&mut app);
    let nodes = {
        let world = app.world_mut();
        let table = world.get::<Table>(tree).expect("the history is a gooey table");
        table.source::<Tree>().map_or(0, Tree::node_count)
    };
    assert_eq!(nodes, HISTORY_LINES + 1, "the list holds every line of the session, and the one before the first change");

    let rows = rows_in_the_world(&mut app);
    assert!(!rows.is_empty(), "the list draws the rows on screen");
    assert!(
        rows.len() <= MOST_ROWS_IN_THE_WORLD,
        "{} of {HISTORY_LINES} entries are entities: the list built a widget per line instead of drawing \
         the rows in view (gooey's pool is the rows on screen and one more): {rows:?}",
        rows.len()
    );
    let standing = app.world().get::<TableRows>(tree).map(|r| r.0).unwrap_or(0.0);
    assert!(standing > 0.0, "the list stands a screenful tall, not one row");
}

/// **A wheel scrolls the whole session, from the line the editor stands on back to the first line.** The list
/// opens where the person is — the line they would read — and the wheel takes them anywhere in the 5,000, which
/// is what the fixed seven-row window could not do.
#[test]
fn a_wheel_scrolls_to_the_end_of_the_session() {
    let mut app = rig("history_scroll");
    let opened = rows_in_the_world(&mut app);
    assert!(
        opened.iter().any(|(key, _)| *key as usize == HISTORY_LINES),
        "the list opens on the line the editor stands on: {opened:?}"
    );

    // **Up to the top of the session**, which is 5,000 lines away: the wheel's own travel, as many notches as
    // it takes.
    for _ in 0..8 {
        wheel(&mut app, -WHEEL_TRAVEL_PX);
    }
    let rows = rows_in_the_world(&mut app);
    assert!(
        rows.iter().any(|(key, _)| *key == 0),
        "the first line of the session — before the first change — is drawn once the list is scrolled to it: {rows:?}"
    );
    assert!(
        rows.len() <= MOST_ROWS_IN_THE_WORLD,
        "the pool at the top of the session is still a screenful: {rows:?}"
    );

    // **And back down to the end**, so the scroll is a scroll and not a jump to one place.
    for _ in 0..8 {
        wheel(&mut app, WHEEL_TRAVEL_PX);
    }
    let rows = rows_in_the_world(&mut app);
    assert!(
        rows.iter().any(|(key, _)| *key as usize == HISTORY_LINES),
        "the line the editor stands on is drawn again at the bottom: {rows:?}"
    );
}

/// **An agent's line reads "the Familiar" and a person's reads "you"**, which is UI-53's second acceptance
/// line and the words the window already used.
#[test]
fn an_agent_line_says_the_familiar_and_a_persons_says_you() {
    let mut app = rig("history_who");
    let tree = history_tree(&mut app);
    let depth = app.world().resource::<EditorState>().undo_depth();
    assert_eq!(depth, HISTORY_LINES);

    // **Every line is labelled, not only the ones on screen**: the label is the list's own answer for a
    // position, which is what a row shows when the pool reaches it.
    let labels: Vec<(usize, String)> = {
        let world = app.world_mut();
        let table = world.get::<Table>(tree).expect("the history is a gooey table");
        let rows = table.source::<Tree>().expect("the list is a tree");
        (1..=depth).filter_map(|position| rows.label(position as u64).map(|l| (position, l.to_owned()))).collect()
    };
    let newest = &labels.last().expect("the session has lines").1;
    assert!(newest.contains("the Familiar"), "the agent's newest change is the Familiar's: {newest:?}");
    let oldest = &labels.first().expect("the session has lines").1;
    assert!(oldest.contains("you"), "a person's change is the person's: {oldest:?}");

    // And on screen, the row the person would read now says it too.
    let on_screen = rows_in_the_world(&mut app);
    let some = on_screen
        .iter()
        .find(|(key, _)| *key as usize == depth)
        .map(|(_, label)| label.clone())
        .or_else(|| {
            // The newest line may be one row below the fold; scroll to it and read it there.
            wheel(&mut app, WHEEL_TRAVEL_PX);
            rows_in_the_world(&mut app).into_iter().find(|(key, _)| *key as usize == depth).map(|(_, label)| label)
        });
    let some = some.unwrap_or_else(|| panic!("the newest line is not a row of the list: {on_screen:?}"));
    assert!(some.contains("the Familiar"), "the row for the agent's change says whose it is: {some:?}");
}

/// **A click on a line steps the history back to it, and a click on the line the editor stands on does
/// nothing.** The window's own rule, on the list: the current position is not a jump target.
///
/// **Real placements, not planted entries.** A jump applies each entry it passes, so the lines here are the three
/// placements an author made — the ones a click can actually take back.
#[test]
fn a_click_on_a_line_steps_back_to_it() {
    let root = fixtures::Fixture::new(&format!("history_click_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.5, 0.5))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 6);
    for x in [1.5, 2.5, 3.5] {
        editor::place_for_test(app.world_mut(), "site/wall", (x, 0.5));
        updates(&mut app, 2);
    }
    let depth = app.world().resource::<EditorState>().undo_depth();
    assert_eq!(depth, 3, "three placements are three lines");

    // The line the editor stands on: a click on it costs nothing.
    editor::click_history_line_for_test(app.world_mut(), depth);
    updates(&mut app, 3);
    assert_eq!(
        app.world().resource::<EditorState>().undo_depth(),
        depth,
        "a click on the line the editor stands on is not a jump"
    );

    editor::click_history_line_for_test(app.world_mut(), 1);
    updates(&mut app, 3);
    let after = app.world().resource::<EditorState>();
    assert_eq!(after.undo_depth(), 1, "the click stepped the history back to the line it was on");
    assert_eq!(after.redo_depth(), 1, "and the jump is one group, so one redo puts it back");
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        2,
        "the map is as it stood at line 1: the fixture's piece and the first placed"
    );

    // **The line before the first change** is a jump target too, which the seven-row window could reach only
    // three presses back.
    editor::click_history_line_for_test(app.world_mut(), 0);
    updates(&mut app, 3);
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 0, "a click on the top line takes every change back");
    assert_eq!(app.world().resource::<OpenMap>().map.placements.len(), 1, "and the map is the fixture's own piece");
}
