//! **Arming a piece from the palette does not respawn the palette's rows.**
//!
//! `editor::on_row_click` blurred the filter box on every click, and the blur is a write: it marked
//! `Filters` changed even when the box was not focused, and `rebuild_palette` reruns on a changed
//! `Filters`. So every click on a row, and the right-click that opens a row's menu, respawned every
//! row. The menu's `ContextTarget` is the row it was opened on, and the next frame that row was
//! gone: choosing an item found no `PaletteRow` and did nothing, silently, in the live editor (the
//! headless menu tests chose before any rebuild ran). The rows now stay until the library, the folds,
//! the filter or the portraits change.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_cauldron::editor::PaletteRow;
use bevy_cauldron::filter::{Filters, Pane};
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// The row entity of each library index.
fn rows(app: &mut App) -> BTreeMap<usize, Entity> {
    let mut q = app.world_mut().query::<(Entity, &PaletteRow)>();
    q.iter(app.world()).map(|(e, r)| (r.0, e)).collect()
}

#[test]
fn a_click_that_arms_a_piece_leaves_every_row_standing() {
    let mut app = editor("palette_rows_stand");
    let before = rows(&mut app);
    assert_eq!(before.len(), 2, "the fixture's two pieces have a row each: {before:?}");

    bevy_cauldron::editor::click_palette_row_for_test(app.world_mut(), "site/gate");
    for _ in 0..3 {
        app.update();
    }

    assert_eq!(rows(&mut app), before, "the click armed a piece; it did not rebuild the palette");
}

#[test]
fn a_click_while_the_filter_has_the_keyboard_still_gives_it_back() {
    let mut app = editor("palette_rows_blur");
    app.world_mut().resource_mut::<Filters>().take_focus(Pane::Palette);

    bevy_cauldron::editor::click_palette_row_for_test(app.world_mut(), "site/gate");
    app.update();

    assert!(!app.world().resource::<Filters>().typing(), "the click takes the keyboard back from the box");
}
