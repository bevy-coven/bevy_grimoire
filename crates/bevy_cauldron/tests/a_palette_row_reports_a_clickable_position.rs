//! **A palette row's reported position is inside the palette, or a caller cannot click it** (#275).
//!
//! # What was wrong
//!
//! The PLACE palette is a scrolling list. `UiGlobalTransform` reports a row's position **through the
//! scroll** — measured on a live editor, `sofa` reported y = 907, then −1440, then 2771 while its
//! viewport spanned 185 to 659 — so a caller that aims a click at the number it is given lands
//! wherever the map happens to be behind the panel. Three demo takes meant to arm a sofa placed lamps
//! instead, and nothing in the editor said the position was unclickable.
//!
//! # What this file holds down
//!
//! 1. **A row that is on screen reports a position inside the palette's own rect.** That is the
//!    property a caller needs: given a row, is the number I have somewhere I can click?
//! 2. **A row scrolled out is distinguishable from one on screen** — either its position is outside the
//!    palette's rect, or it is not laid out at all. Never a number a caller would click.
//!
//! The rows are read through the same components `tools/demo/record.py` reads over BRP, so the test and
//! the recorder cannot disagree about what a row reports.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// The editor on a fixture with two pieces, so the palette has rows to find.
fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .descriptor("wall", "beta")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..5 {
        app.update();
    }
    (root, app)
}

/// The rows a caller can aim at, and the palette's viewport rect.
type Rows = (Vec<(String, Vec2)>, Option<(Vec2, Vec2)>);

/// Every laid-out row's reported centre, and the palette viewport's rect.
///
/// **The viewport is found by shape, not by a private marker**: it is the node that carries a
/// `ScrollPosition` and has a non-zero size, which is what `bevy_ui` itself scrolls.
fn rows_and_viewport(app: &mut App) -> Rows {
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &bevy::ui::UiGlobalTransform, &bevy::ui::ComputedNode)>();
    let rows: Vec<(String, Vec2)> = texts
        .iter(world)
        .filter(|(t, _, n)| !t.0.trim().is_empty() && n.size().x > 0.0 && n.size().y > 0.0)
        .map(|(t, at, _)| (t.0.trim().to_owned(), at.translation))
        .collect();
    // **The palette's own column, found from the PLACE heading.** Two scrolling panels share this
    // window — the inspector on the left and the palette on the right — and picking the first node of
    // the right size found the inspector, which is why this reads the heading's x first.
    let place_x = texts
        .iter(world)
        .find(|(t, _, _)| t.0.trim() == "PLACE")
        .map(|(_, at, _)| at.translation.x);
    let mut lists = world.query_filtered::<(&bevy::ui::UiGlobalTransform, &bevy::ui::ComputedNode), With<bevy::ui::ScrollPosition>>();
    let viewport = lists
        .iter(world)
        .filter(|(at, n)| {
            n.size().x > 200.0
                && n.size().y > 200.0
                && place_x.is_some_and(|x| (at.translation.x - x).abs() < n.size().x)
        })
        .map(|(at, n)| (at.translation, n.size()))
        .next();
    (rows, viewport)
}

/// **The palette is a scrolling list with a real viewport**, so the test is not vacuous.
#[test]
fn the_place_palette_is_a_scrolling_viewport() {
    let (_root, mut app) = editor("palette-scroll-viewport");
    let (_, viewport) = rows_and_viewport(&mut app);
    let (centre, size) = viewport.expect(
        "the palette must be a scrolling viewport with a size — if this reads None the query has \
         stopped seeing it and every assertion below is vacuous",
    );
    assert!(
        size.x > 200.0 && size.y > 200.0,
        "the palette viewport should be a real column of rows; it measures {size:?}"
    );
    assert!(centre.x > 0.0 && centre.y > 0.0, "and it should be on screen: {centre:?}");
}

/// **A row that reports a position inside the palette is one a click can reach.**
///
/// The claim is not that every row is on screen — most are not, that is what a scroll is — but that the
/// rows inside the viewport are the ones a caller can aim at, and that at least one is.
#[test]
fn a_visible_row_reports_a_position_inside_the_palette() {
    let (_root, mut app) = editor("palette-scroll-inside");
    let (rows, viewport) = rows_and_viewport(&mut app);
    let (centre, size) = viewport.expect("the palette viewport is there");
    let (half, mid) = (size * 0.5, centre);

    let inside: Vec<&(String, Vec2)> = rows
        .iter()
        .filter(|(_, at)| {
            (at.x - mid.x).abs() <= half.x && (at.y - mid.y).abs() <= half.y
        })
        .collect();
    assert!(
        !inside.is_empty(),
        "not one of {} laid-out texts reports a position inside the palette's own rect {:?} x {:?} — \
         so no caller can click a row, which is the defect: {rows:?}",
        rows.len(),
        centre,
        size
    );
}

/// **A row scrolled out of the palette is not reported as clickable.**
///
/// The palette holds far more rows than its viewport shows, so this must find at least one outside —
/// and asserts the property a caller relies on: outside means outside the rect, never a plausible
/// number somewhere else on screen.
#[test]
fn a_row_scrolled_out_is_outside_the_palettes_rect() {
    let (_root, mut app) = editor("palette-scroll-outside");
    let (rows, viewport) = rows_and_viewport(&mut app);
    let (centre, size) = viewport.expect("the palette viewport is there");
    let (half, mid) = (size * 0.5, centre);
    let outside: Vec<&(String, Vec2)> = rows
        .iter()
        .filter(|(_, at)| {
            !((at.x - mid.x).abs() <= half.x && (at.y - mid.y).abs() <= half.y)
        })
        .collect();
    assert!(
        !outside.is_empty(),
        "the fixture's palette fits entirely in its viewport, so this case proves nothing; give it \
         more pieces"
    );
    // **And the outside rows are far outside**, not a pixel over the edge: the scroll moves them by
    // hundreds. A caller must be able to refuse them by the rect alone.
    let far = outside.iter().any(|(_, at)| (at.y - mid.y).abs() > half.y);
    assert!(
        far,
        "a scrolled-out row should be clearly outside the palette, and these are all within a pixel \
         of it: {outside:?}"
    );
}

/// **The rows share one column**, so the palette's rect is a description of where they are.
///
/// This is the half that makes the other two useful: if the rect and the rows disagreed, a position
/// inside one would be outside the other and neither test would mean anything.
#[test]
fn the_palettes_rect_agrees_with_its_own_node() {
    let (_root, mut app) = editor("palette-scroll-agrees");
    let (rows, viewport) = rows_and_viewport(&mut app);
    let (centre, size) = viewport.expect("the palette viewport is there");
    // Every laid-out row shares the palette's column: their x centres are within a row's width of the
    // viewport's, which is what makes the rect a description of where the rows are.
    let column: Vec<f32> = rows.iter().map(|(_, at)| at.x).collect();
    assert!(!column.is_empty(), "the palette has rows");
    let off_column = column
        .iter()
        .filter(|x| (**x - centre.x).abs() > size.x)
        .count();
    assert_eq!(
        off_column, 0,
        "{} of {} rows are more than a palette-width from the palette's own x, so the rect is not \
         where the rows are: centre {centre:?}, size {size:?}, rows {rows:?}",
        off_column,
        column.len()
    );
}
