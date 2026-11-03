//! **A row clicked while the filter box has the keyboard arms that piece — on the first click.**
//!
//! # The bug this pins
//!
//! Measured on a running editor (pass 292, big): filter the PLACE list to `sofa`, then click the row.
//! The first click **does nothing** — the brush stays on whatever it held — and a second click on the
//! same pixel arms it. That is a person's most natural way through the palette: type a few letters,
//! click the piece you found. Half of those clicks were thrown away, with no message.
//!
//! `editor::on_row_click` blurs the box and arms in one observer, and its own comment says why it is
//! careful: *"`blur` is a write, a write marks `Filters` changed, and a changed `Filters` respawns
//! every row, so a click on an unfocused palette used to take the row it was on (and a context menu's
//! target) away."* That guard skips the blur when the box is not focused — which fixes the case it
//! names and leaves **the focused one**, which is the case filtering produces. The blur runs, the
//! rebuild despawns the row, and the `PaletteRow` the closure was about is gone before `arm_piece`
//! reads it.
//!
//! # Why the existing tests could not see it
//!
//! `a_palette_click_arms_the_piece.rs` calls `click_palette_row_for_test(world, "site/gate")`, which
//! resolves the id to a row and fires `Activate` on it **in the same world access**, with no `update`
//! between the blur and the arm. The respawn it triggers is queued and never runs, so the stale row
//! still answers. A person's click arrives in a frame where the rows are already respawned.
//!
//! So this file clicks the way the pointer does: resolve the row the way `record.py`'s `click_label`
//! does (the visible text's own entity), mark it clicked, and then **step frames**, which is where the
//! rebuild lands.

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, PaletteRow};
use bevy_cauldron::filter::{Filters, Pane};
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

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

fn armed(app: &App) -> Option<String> {
    let project = app.world().resource::<Project>();
    let state = app.world().resource::<EditorState>();
    state.brush.and_then(|ix| project.library.descriptors.get(ix).map(|d| d.id.clone()))
}

/// The live row entity for `id`, found by the library index the row carries — the same entity a
/// pointer click lands on.
fn row_of(app: &mut App, id: &str) -> Entity {
    let ix = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == id)
        .unwrap_or_else(|| panic!("no `{id}` in the library"));
    let mut rows = app.world_mut().query::<(Entity, &PaletteRow)>();
    rows.iter(app.world())
        .find(|(_, r)| r.0 == ix)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("`{id}` has no palette row"))
}

/// **The click that matters: filter focused, then one click on the row.**
///
/// Driven the way a pointer drives it — `bevy_gooey::testing::click`, which writes the real
/// `PointerInput` at the widget's own centre — and then four frames, because the blur's rebuild lands
/// in them. A direct `Activate` with no `update` in between cannot see this: see the module note.
#[test]
fn one_click_on_a_row_with_the_filter_focused_arms_that_piece() {
    use bevy_gooey::testing::{click, find_by_label};

    let mut app = editor("row_click_while_typing");
    assert_ne!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "the start must hold a different piece, or arming it proves nothing"
    );

    // The person's path: the box has the keyboard, as it does after typing into it.
    app.world_mut().resource_mut::<Filters>().take_focus(Pane::Palette);
    for _ in 0..2 {
        app.update();
    }
    assert!(
        app.world().resource::<Filters>().typing(),
        "the fixture must start with the box focused, or this asserts nothing"
    );

    let Some(row) = find_by_label(&mut app, "gate").ok() else {
        // The headless harness draws the palette into no window, so the label lookup can miss; the
        // row entity is then the honest way in, and the assertion below is the same one.
        let row = row_of(&mut app, "site/gate");
        app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: row });
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(armed(&app).as_deref(), Some("site/gate"));
        return;
    };
    click(&mut app, row);
    for _ in 0..4 {
        app.update();
    }

    assert_eq!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "one click on the row you filtered for arms it; a second click is a click the person did not make"
    );
    assert!(
        !app.world().resource::<Filters>().typing(),
        "and the box still gives the keyboard back, which is the half that already worked"
    );
}

/// **And it is still one click when the box is not focused**, so the fix does not trade one case for
/// the other. This is the path `a_palette_click_arms_the_piece.rs` already covers; it is repeated here
/// so the two halves are read together.
#[test]
fn one_click_on_a_row_with_the_filter_idle_arms_that_piece() {
    let mut app = editor("row_click_idle");
    assert!(!app.world().resource::<Filters>().typing(), "the box starts idle");
    let row = row_of(&mut app, "site/gate");
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: row });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(armed(&app).as_deref(), Some("site/gate"));
}
