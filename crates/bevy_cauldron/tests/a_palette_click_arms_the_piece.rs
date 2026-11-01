//! **A click on a palette row arms that piece, through the operation** — E-041's gap.
//!
//! # Why this file exists
//!
//! `editor::on_row_click` is the primary way a person chooses what to place, and nothing in the
//! suite could reach it. The palette's *other* route was tested (`walk_palette`, the arrows), and
//! `brush_arm_is_an_operation` proved both front ends agree — but it proved that by calling
//! `arm_piece` directly, which skips everything the observer itself does:
//!
//! 1. it resolves the clicked entity to a `PaletteRow` index, so the row's own library index is
//!    what arms the brush;
//! 2. it **blurs the filter box** first, because `drive_place` is gated on `not_typing` — filtering
//!    the palette to find a piece and then clicking it used to leave the editor unable to place;
//! 3. it takes `Option<Res<..>>` for every resource, because a global `Activate` observer fires
//!    outside the Map door too, where `Res<Project>` panics rather than skipping.
//!
//! [`editor::click_palette_row_for_test`] fires the same `Activate` the button sends, so all three
//! are exercised. The assertions are on the editor's own state, the operation log, and the screen
//! state a person would notice (the filter losing the keyboard).

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::filter::{Filters, Pane};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;

use crate::fixtures;

/// A Map-door editor holding a wall and a gate, so a click can arm one while the other is armed.
fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

/// The piece the editor holds armed, by id: the brush is an index into the library.
fn armed(app: &App) -> Option<String> {
    let project = app.world().resource::<Project>();
    let state = app.world().resource::<EditorState>();
    let ix = state.brush?;
    project.library.descriptors.get(ix).map(|d| d.id.clone())
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

#[test]
fn a_click_on_a_palette_row_arms_that_piece_as_a_human() {
    let mut app = editor("palette_click_arms");
    assert_ne!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "the start must hold a different piece, or arming it proves nothing"
    );
    let before = log(&app).len();

    bevy_cauldron::editor::click_palette_row_for_test(app.world_mut(), "site/gate");
    app.update();

    assert_eq!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "the click arms the row it landed on"
    );
    let written = log(&app);
    let new = written.get(before).unwrap_or_else(|| {
        panic!(
            "the click must write one log line, and the log went from {before} to {}: {written:?}",
            written.len()
        )
    });
    // **The line is the operation's, and it is a person's.** A click that armed the brush itself
    // would leave this log short by one, which is exactly the second front end the parity rule
    // forbids.
    assert_eq!(new.op, "brush_arm", "the click dispatches `brush_arm`, got `{}`", new.op);
    assert_eq!(new.outcome, "applied", "{new:?}");
    assert_eq!(
        new.source,
        ops::Source::Human,
        "a click is the author's action, not an agent's"
    );
    assert_eq!(written.len(), before + 1, "one click, one line");
}

/// **The click puts the tool back to placing**, which is what makes a click useful after any other
/// tool — a tool is otherwise only escapable by a key, and a borderless fullscreen window can have
/// taken the key before Bevy sees it.
#[test]
fn a_palette_click_returns_the_tool_to_placing() {
    let mut app = editor("palette_click_tool");
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Remove;

    bevy_cauldron::editor::click_palette_row_for_test(app.world_mut(), "site/wall");
    app.update();

    assert_eq!(
        app.world().resource::<EditorState>().tool,
        Tool::Place,
        "clicking a row is a statement that you are done deleting"
    );
}

/// **The click takes the keyboard back from the filter box.**
///
/// Filtering the palette to find a piece and then clicking it left the filter focused, and
/// `drive_place` is gated on `not_typing` — so the most natural way to find a piece was also the
/// way to break placing it, with no message. The row's own id is not the point here: the focus is.
#[test]
fn a_palette_click_blurs_the_filter_box() {
    let mut app = editor("palette_click_blur");
    app.world_mut()
        .resource_mut::<Filters>()
        .take_focus(Pane::Palette);
    // The rebuild watches `Filters`, so the rows come back with the box still holding the keyboard.
    app.update();
    assert!(
        app.world().resource::<Filters>().typing(),
        "the fixture must start with the box focused, or this asserts nothing"
    );

    bevy_cauldron::editor::click_palette_row_for_test(app.world_mut(), "site/gate");
    app.update();

    assert!(
        !app.world().resource::<Filters>().typing(),
        "a click on the row you filtered for must give the keyboard back to the editor"
    );
    assert_eq!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "and the row clicked is still the one armed"
    );
}
