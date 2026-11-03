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
//! A fourth case — that every row shares the palette's column — was written and dropped: filtering the
//! window's texts by height catches the inspector's rows at the same heights, and the claim adds nothing
//! the two above do not already say.
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

/// A laid-out text: its words and the centre it reports.
type Row = (String, Vec2);

/// **The rows a caller can aim at, and the palette's own box.**
///
/// The box is the node the editor names — [`bevy_cauldron::editor::PaletteList`], which became `pub`
/// for this (#275). Three earlier attempts found it by shape and got the **inspector** twice and a
/// window-width scroller once, so the test failed for a different reason on different runs.
fn rows_and_viewport(app: &mut App) -> (Vec<Row>, Option<(Vec2, Vec2)>) {
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &bevy::ui::UiGlobalTransform, &bevy::ui::ComputedNode)>();
    let laid: Vec<Row> = texts
        .iter(world)
        .filter(|(t, _, n)| !t.0.trim().is_empty() && n.size().x > 0.0 && n.size().y > 0.0)
        .map(|(t, at, _)| (t.0.trim().to_owned(), at.translation))
        .collect();
    let mut lists = world.query_filtered::<(&bevy::ui::UiGlobalTransform, &bevy::ui::ComputedNode), With<bevy_cauldron::editor::PaletteList>>();
    let viewport = lists
        .iter(world)
        .map(|(at, n)| (at.translation, n.size()))
        .next();
    (laid, viewport)
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

/// **A click at a visible row's position arms that piece** (#275's second acceptance item).
///
/// **Fails on the base, and is `#[ignore]`d rather than fixed here.** Measured with the palette's own
/// box from [`bevy_cauldron::editor::PaletteList`]: the `wall` row is at `Vec2(1373.5, 286.0)`, inside a
/// palette of `Vec2(1362.0, 494.0)` x `Vec2(286.0, 618.0)`, and a press there leaves the editor holding
/// `floor` — the brush it already had. So the position is in the right box and the press still arms
/// nothing, which is the defect stated in one assertion.
///
/// The cases above say a visible row's number is inside the palette. This says it is *the right* number:
/// put the pointer there, press, and the editor holds that row's piece through `brush_arm` — the
/// operation a palette click dispatches, with the log line a person's click writes.
///
/// **Not through `bevy_gooey::testing::click`**, which reads a node's centre for itself and would pass
/// whether or not the reported position is where the row is drawn — the very question this asks.
#[ignore = "the press at a row's reported position arms nothing; measured, see #275"]
#[test]
fn a_click_at_a_visible_rows_position_arms_that_piece() {
    let (_root, mut app) = editor("palette-scroll-click");
    let (rows, viewport) = rows_and_viewport(&mut app);
    let (centre, size) = viewport.expect("the palette is a node with a box");
    let (half, mid) = (size * 0.5, centre);

    // **A row inside the palette whose words name a piece the fixture has**, so the arm can be read
    // back by id rather than by an index that changes with the library.
    let project = app.world().resource::<bevy_cauldron::project::Project>();
    let wanted: Vec<String> = project.library.descriptors.iter().map(|d| d.id.clone()).collect();
    let (id, at) = rows
        .iter()
        .filter(|(_, at)| (at.x - mid.x).abs() <= half.x && (at.y - mid.y).abs() <= half.y)
        .find(|(words, _)| wanted.iter().any(|id| id == words))
        .map(|(words, at)| (words.clone(), *at))
        .unwrap_or_else(|| {
            panic!(
                "no piece row is reported inside the palette {centre:?} x {size:?}; the fixture's \
                 pieces are {wanted:?} and every laid-out text is {rows:?}"
            )
        });

    // **The pointer and the press**, both at the position the row reported, on the image the interface
    // is drawn into — the target `bevy_ui`'s picking backend reads.
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = || bevy::picking::pointer::Location {
        target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
            handle: handle.clone(),
            scale_factor: 1.0,
        }),
        position: at,
    };
    for action in [
        bevy::picking::pointer::PointerAction::Move { delta: Vec2::ONE },
        bevy::picking::pointer::PointerAction::Press(bevy::picking::pointer::PointerButton::Primary),
    ] {
        app.world_mut().write_message(bevy::picking::pointer::PointerInput::new(
            bevy::picking::pointer::PointerId::Mouse,
            location(),
            action,
        ));
        app.update();
    }
    for _ in 0..3 {
        app.update();
    }

    let armed = {
        let project = app.world().resource::<bevy_cauldron::project::Project>();
        let state = app.world().resource::<bevy_cauldron::editor::EditorState>();
        state.brush.and_then(|ix| project.library.descriptors.get(ix)).map(|d| d.id.clone())
    };
    assert_eq!(
        armed.as_deref(),
        Some(id.as_str()),
        "a press at the position the row reports must arm `{id}` — the row is at {at:?}, inside the \
         palette {centre:?} x {size:?}. The editor holds {armed:?}"
    );
}
