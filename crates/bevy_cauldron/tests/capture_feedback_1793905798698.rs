//! **The sidebar's splash has the same margin on every side** (`feedback-1793905798698`, filed against
//! 0.2.6 on `mac_air`).
//!
//! The note: *"This has no margin on the right side, but the rest of the sides do. MEant to be?"* The
//! pick is the splash on the Projects home, `rect_px [29, 29, 605, 209]` at `scale_factor` 2.0 — so
//! `[14.5, 14.5, 302.5, 104.5]` logical — inside a sidebar that pads every side by `chrome::PAD`.
//!
//! **The relation is invariance under the box's padding**, chosen from what the person said they saw
//! rather than from the code (metamorphic testing; Segura, Fraser, Sánchez & Ruiz-Cortés, *A Survey on
//! Metamorphic Testing*, IEEE TSE 2016, `10.1109/TSE.2016.2532875`: a relation "strongly inspired by
//! the semantics of the program under test" is the effective kind). "The rest of the sides have a
//! margin and the right does not" is a claim about one inset differing from the others, so the
//! assertion is:
//!
//! > the splash's left, top and right insets are all the box's own padding.
//!
//! Three, not four: the splash is the first child of the sidebar's full-height column, so below it is
//! the rest of the column rather than a margin, and asking for a bottom inset would be asking the
//! sidebar to be as short as its picture.
//!
//! Measured on the base commit at the capture's own window: the splash is `[14, 302]` inside a sidebar
//! of `[0, 288]` — 14 px of margin on the left, **−14 on the right** — because `SIDEBAR_SPLASH` was
//! sized to the sidebar's *outer* width and drawn inside its padded content box.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::chrome::PAD;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

/// **The capture's window, as the layout the person saw**: `physical` 2560 x 1406 at `scale_factor`
/// 2.0 is 1280 x 703 logical, which at the harness's `UiScale` of 1.2 is this many texels.
const CAPTURE_SURFACE: (u32, u32) = (1280, 703);

/// One logical pixel of slack: layout rounds to whole physical pixels, and a margin one pixel off is
/// not "no margin".
const SLACK_PX: f32 = 1.0;

/// The home, on the Projects page, at the capture's window.
fn the_home(tag: &str) -> App {
    let root = Fixture::new(&format!("splash_margin_{tag}_{}", std::process::id()))
        .descriptor("bench", "props")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    bevy_cauldron::harness::resize_surface(&mut app, CAPTURE_SURFACE.0, CAPTURE_SURFACE.1)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The splash's four insets inside the box that holds it**, in logical pixels, and the box's own
/// padding.
///
/// The splash is the `Name`ed node `fill_sidebar` spawns; its box is the nearest ancestor whose `Node`
/// states padding — the sidebar's surface. Found by name and by that padding rather than by index,
/// because the sidebar's children are a list the sidebar itself decides.
fn splash_insets(app: &mut App) -> ((f32, f32, f32, f32), f32) {
    let named = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world).map(|(entity, name)| (entity, name.as_str().to_owned())).collect::<Vec<_>>()
    };
    let splash = named
        .iter()
        .find(|(_, name)| name == "sidebar splash")
        .map(|(entity, _)| *entity)
        .expect("the sidebar draws its splash");

    let world = app.world();
    let node = world.get::<ComputedNode>(splash).expect("the splash is laid out");
    let at = world.get::<UiGlobalTransform>(splash).expect("the splash is laid out");
    let splash_rect = Rect::from_center_size(at.translation, node.size());
    let logical = |physical: f32| physical * node.inverse_scale_factor;

    let mut up = world.get::<ChildOf>(splash).map(ChildOf::parent);
    while let Some(candidate) = up {
        let padded = world.get::<Node>(candidate).filter(|node| node.padding != UiRect::ZERO);
        if let (Some(padded), Some(node), Some(at)) = (
            padded,
            world.get::<ComputedNode>(candidate),
            world.get::<UiGlobalTransform>(candidate),
        ) {
            let box_rect = Rect::from_center_size(at.translation, node.size());
            let insets = (
                logical(splash_rect.min.x - box_rect.min.x),
                logical(box_rect.max.y - splash_rect.max.y),
                logical(box_rect.max.x - splash_rect.max.x),
                logical(splash_rect.min.y - box_rect.min.y),
            );
            let padding = match padded.padding.left {
                Val::Px(px) => px,
                _ => PAD,
            };
            return (insets, padding);
        }
        up = world.get::<ChildOf>(candidate).map(ChildOf::parent);
    }
    panic!("the splash sits in no padded box, so the note is about a box this build does not draw");
}

#[test]
fn the_splash_has_the_same_margin_on_every_side() {
    let mut app = the_home("every_side");
    let ((left, bottom, right, top), padding) = splash_insets(&mut app);

    assert!(
        (left - right).abs() <= SLACK_PX,
        "the splash's margins differ: {left:.1} px on the left and {right:.1} px on the right, which \
         is the capture's note — \"This has no margin on the right side, but the rest of the sides do\""
    );
    assert!(
        (left - top).abs() <= SLACK_PX,
        "the splash's margins are not one number: left {left:.1}, right {right:.1}, top {top:.1} \
         (bottom {bottom:.1} is the rest of the sidebar, not a margin)"
    );
    // **And it is the box's own padding**, not a number the splash chose: every other panel's content
    // keeps `chrome::PAD`, and a splash with a margin of its own is a second convention.
    assert!(
        (left - padding).abs() <= SLACK_PX * 2.0,
        "the splash keeps {left:.1} px from its box's left edge, where the box pads {padding:.1} px: \
         the splash has a margin of its own rather than the sidebar's"
    );
}

#[test]
fn the_splash_shrinks_with_the_sidebar_rather_than_hanging_out_of_it() {
    // The same defect stated the way it was caused: the splash was sized to the sidebar's OUTER width,
    // so any width the sidebar takes leaves it `2 * padding` too wide. A narrower window must not make
    // it worse — the invariance half of the relation.
    let mut app = the_home("narrow");
    bevy_cauldron::harness::resize_surface(&mut app, 1100, 703).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let ((left, bottom, right, top), _) = splash_insets(&mut app);
    assert!(
        (left - right).abs() <= SLACK_PX,
        "at a narrower window the splash keeps {left:.1} px on the left and {right:.1} px on the right: \
         the margin depends on the window, which is what let it go negative before"
    );
    assert!(
        (left - top).abs() <= SLACK_PX,
        "at a narrower window the margins differ: left {left:.1}, right {right:.1}, top {top:.1} \
         (bottom {bottom:.1} is the rest of the sidebar, not a margin)"
    );
}
