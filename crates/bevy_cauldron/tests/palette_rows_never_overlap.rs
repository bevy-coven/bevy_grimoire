//! **A palette row's name never draws over its triangle count, and the list opens at its top** (#289).
//!
//! Audit, 2026-11-04, the bedroom example at 1280x720: "toilet_roll_holder" drew over "312" and
//! "desk_lamp_modern" over "216", because an id with no spaces is one word the row could not wrap,
//! and the list opened scrolled half-way down because the editor armed library row 0, which the
//! grouped palette shows mid-list.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy_cauldron::editor::{EditorState, PaletteList, PaletteRow};
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

/// Forty characters with no space in them: the case that cannot wrap at a word.
const LONG_ID: &str = "a_very_long_piece_name_that_runs_on_and";

/// Frames for the palette to be built and laid out.
const SETTLE_FRAMES: usize = 6;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The left and right edges of a laid-out node, in physical pixels.
fn span(node: &ComputedNode, at: &UiGlobalTransform) -> (f32, f32) {
    let half = node.size().x * 0.5;
    (at.translation.x - half, at.translation.x + half)
}

/// The row's texts, in child order, with their spans.
fn row_texts(app: &mut App, ix: usize) -> Vec<(String, (f32, f32))> {
    let world = app.world_mut();
    let mut rows = world.query::<(&PaletteRow, &Children)>();
    let kids: Vec<Entity> = rows
        .iter(world)
        .find(|(r, _)| r.0 == ix)
        .map(|(_, c)| c.iter().collect())
        .unwrap_or_default();
    let mut texts = world.query::<(&Text, &ComputedNode, &UiGlobalTransform)>();
    kids.into_iter()
        .filter_map(|e| texts.get(world, e).ok())
        .map(|(t, n, at)| (t.0.clone(), span(n, at)))
        .collect()
}

#[test]
fn a_long_name_stops_before_the_count() {
    let root = fixtures::Fixture::new("palette-long-name")
        .descriptor(LONG_ID, "alpha")
        .descriptor("wall", "beta")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    settle(&mut app);
    let ix = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == LONG_ID)
        .unwrap_or_else(|| panic!("{LONG_ID} is in the fixture's library"));
    let texts = row_texts(&mut app, ix);
    let [(name, (_, name_right)), (count, (count_left, _))] = texts.as_slice() else {
        panic!("a palette row draws a name and a count, and this one drew {texts:?}");
    };
    assert_eq!(
        name,
        &bevy_grimoire_core::naming::display_name(LONG_ID),
        "the row reads as words, not as the id"
    );
    assert!(!count.is_empty(), "the fixture's mesh has triangles, so the count is drawn");
    assert!(
        name_right <= count_left,
        "the name {name:?} ends at x = {name_right} and the count {count:?} starts at x = {count_left}: they overlap"
    );
}

/// **The editor arms the first row it shows, so the list opens at its top.**
///
/// Library row 0 is given a kind the vocabulary lists after `prop`, so the grouped palette draws it
/// below the other pieces: arming row 0 is what scrolled the bedroom's list half-way down.
#[test]
fn the_list_opens_at_its_top_with_its_first_row_armed() {
    let mut fixture = fixtures::Fixture::new("palette-opens-at-top")
        .kind_tokens(&["wall"])
        .descriptor("zz_wall", "alpha");
    for n in 0..40 {
        fixture = fixture.descriptor(&format!("prop_{n:02}"), "beta");
    }
    let root = fixture.build("m");
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.yaml");
    let text = std::fs::read_to_string(&library).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    let first_kind = r#"kind: ["prop"]"#;
    assert!(text.contains(first_kind), "the fixture's first row carries the shipped kind");
    std::fs::write(&library, text.replacen(first_kind, r#"kind: ["wall"]"#, 1))
        .unwrap_or_else(|e| panic!("{library:?}: {e}"));

    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    settle(&mut app);
    let world = app.world_mut();
    let first_shown = {
        let mut lists = world.query_filtered::<&Children, With<PaletteList>>();
        let kids: Vec<Entity> = lists.iter(world).flat_map(|c| c.iter()).collect();
        let mut rows = world.query::<&PaletteRow>();
        kids.into_iter().find_map(|e| rows.get(world, e).ok().map(|r| r.0))
    };
    let brush = world.resource::<EditorState>().brush;
    assert_eq!(brush, first_shown, "the editor arms the row the palette shows first");
    let mut scrolls = world.query_filtered::<&ScrollPosition, With<PaletteList>>();
    for scroll in scrolls.iter(world) {
        assert_eq!(scroll.0.y, 0.0, "the palette opens at its top");
    }
}
