//! **The What's new page's bottom is reachable** (`feedback-1793905912313`, filed against 0.2.6 on
//! `mac_air`).
//!
//! The note: *"This has no scroll to it, so the bottom gets cut off and is not visible."* The frame
//! shows the What's new page on a 1280 x 703 logical window (2560 x 1406 physical at `scale_factor`
//! 2.0), its third entry ending at the window's edge, with no scrollbar: the entries after it and the
//! "See every change" button are simply gone.
//!
//! **The relation is containment**, chosen from what the person said they saw rather than from the
//! code (metamorphic testing; Segura, Fraser, Sánchez & Ruiz-Cortés, *A Survey on Metamorphic
//! Testing*, IEEE TSE 2016, `10.1109/TSE.2016.2532875`: a relation "strongly inspired by the
//! semantics of the program under test" is the effective kind). "The bottom gets cut off" is rows that
//! the box does not show, and "there is no scroll to it" is that nothing brings them in. So the
//! assertion is:
//!
//! > the page draws every entry it lists, **and** something can scroll the rows its box cuts off,
//! > **and** at that scroll's end the last entry is inside the box.
//!
//! Measured on the base commit with the fix reverted, at the capture's own window: *"nothing above the
//! What's new page scrolls: 10 entries are drawn into a window 703 px tall and the rest of them cannot
//! be brought into view"*. With the fix both tests pass, and the wheel over the page moves it 300 px.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::mouse::MouseScrollUnit;
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, OverflowAxis, ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::ScrollArea;
use bevy_cauldron::launcher::{ChangelogText, HomeTab, Launcher, LauncherPlugin, NewsEntry, NEWS_SHOWN};
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

/// **The capture's window, as the layout the person saw.**
///
/// The record says `physical` 2560 x 1406 at `scale_factor` 2.0, so the interface laid out over
/// 1280 x 703 and the surface image was that many *logical* pixels doubled for a 2x display. The
/// harness has no display factor, so 1280 x 703 texels at its own `UiScale` of 1.2 is the same
/// design, and the capture's own pick agrees: its page begins at 576 physical = 288 logical, which is
/// the 240 px sidebar plus the surface's padding.
const CAPTURE_SURFACE: (u32, u32) = (1280, 703);

/// Two pixels of slack. Everything below is measured in the surface's own **physical** pixels — the
/// `ComputedNode::size` unit and the `UiGlobalTransform::translation` unit alike — so the only
/// rounding is layout's own, and a rect a pixel past the window is not "cut off".
const SLACK_PX: f32 = 2.0;

/// The still every fixture entry names: one the shipped changelog already carries, so the page decodes
/// it exactly as it decodes a real entry's and the rows are as tall as the capture's were.
const A_STILL: &str = "docs/changelog/media/2026-11-04-307-window-not-black.png";

/// **`NEWS_SHOWN` entries, each with a sentence as long as a real one's** — what the page lists.
///
/// The prose is what makes a row tall, and a tall row is what the capture's frame shows: at the
/// capture's window the page runs past its box after the third entry. The still markup is written the
/// way the changelog writes it too, so the page takes the same path it takes on a real entry.
fn entries(count: usize) -> String {
    let mut text = String::from("# Changelog\n\n");
    for i in 0..count {
        text.push_str(&format!(
            "- **an-entry-{i}** `{i:07}` Something that changed *For you: entry {i} says what changed, \
             in the three or four lines the page gives each of these sentences, because every entry \
             here carries a sentence of that length and ten of them are taller than the box they are \
             drawn in — which is the whole of the capture's complaint: the page ran out of room, and \
             the rows after the third were left below the window with nothing to scroll them in.*<br>\
             <a href=\"{A_STILL}\"><img src=\"{A_STILL}\"></a>\n"
        ));
    }
    text
}

/// **The home, on the What's new page, at the window the capture was taken at.**
fn whats_new_at_the_captures_window(tag: &str) -> App {
    let root = Fixture::new(&format!("whats_new_{tag}_{}", std::process::id()))
        .descriptor("bench", "props")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(ChangelogText(entries(NEWS_SHOWN)));
    app.add_plugins(LauncherPlugin);
    // **The wheel machinery is asserted, not added**, so this test cannot pass by bringing the thing
    // it is meant to check: the harness installs `ScrollAreaPlugin` through `crate::ui::ScrollWheelPlugin`.
    assert!(
        app.is_plugin_added::<bevy::ui_widgets::ScrollAreaPlugin>(),
        "the harness installs the wheel handler, so the wheel test below measures the page and not the plumbing"
    );
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    bevy_cauldron::harness::resize_surface(&mut app, CAPTURE_SURFACE.0, CAPTURE_SURFACE.1)
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<Launcher>().tab = HomeTab::WhatsNew;
    for _ in 0..8 {
        app.update();
    }
    app
}

/// The What's new page's own node.
fn whats_new_page(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &bevy_cauldron::launcher::HomePage)>();
    q.iter(world)
        .find(|(_, page)| page.0 == HomeTab::WhatsNew)
        .map(|(entity, _)| entity)
        .expect("the home draws a node for the What's new page")
}

/// The surface's height in physical pixels: the window the capture was taken in.
fn surface_height(app: &App) -> f32 {
    let image = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    app.world()
        .resource::<Assets<Image>>()
        .get(&image)
        .map(|image| image.height() as f32)
        .unwrap_or(CAPTURE_SURFACE.1 as f32)
}

/// **How far the last entry's bottom sits below `area`'s top**, in logical pixels: the row the capture
/// says cannot be seen. Found by walking down from the page's own nodes, so no wrapper's size is read.
fn last_entry_bottom(app: &mut App, area: Entity) -> f32 {
    let top = app
        .world()
        .get::<UiGlobalTransform>(area)
        .expect("the area is laid out")
        .translation
        .y;
    let world = app.world_mut();
    let mut q = world.query::<(&NewsEntry, &ComputedNode, &UiGlobalTransform)>();
    let mut rows = q
        .iter(world)
        .filter(|(_, node, _)| node.size() != Vec2::ZERO)
        .map(|(entry, node, at)| {
            let bottom = at.translation.y + node.size().y / 2.0 * node.inverse_scale_factor;
            (entry.0.clone(), (bottom - top) * node.inverse_scale_factor)
        })
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "the page drew no entries, so there is no bottom to reach");
    rows.sort_by(|a, b| a.1.total_cmp(&b.1));
    rows.pop().expect("a last entry").1
}

/// **The box that can scroll the page's rows into view**, wherever a build puts it.
///
/// Either inside the page — `chrome::scroll_list` makes the page own its viewport, which is what this
/// editor now does — or above it. Both are the same answer to the note, so the test asks for the
/// behaviour rather than for one tree shape.
fn scrolling_box(app: &mut App, page: Entity) -> Option<Entity> {
    let scrolls = |app: &App, entity: Entity| {
        let world = app.world();
        world.get::<Node>(entity).is_some_and(|node| node.overflow.y == OverflowAxis::Scroll)
            && world.get::<ScrollArea>(entity).is_some()
            && world.get::<ScrollPosition>(entity).is_some()
    };
    // **Down from the page**, which is where `chrome::scroll_list` puts the viewport the page owns.
    let inside = {
        let mut stack = vec![page];
        let mut found = None;
        while let Some(entity) = stack.pop() {
            if scrolls(app, entity) {
                found = Some(entity);
                break;
            }
            if let Some(children) = app.world().get::<Children>(entity) {
                stack.extend(children.iter());
            }
        }
        found
    };
    if inside.is_some() {
        return inside;
    }
    // **Or above it**, which is where a build that wrapped the page would put it.
    let mut up = app.world().get::<ChildOf>(page).map(ChildOf::parent);
    while let Some(candidate) = up {
        if scrolls(app, candidate) {
            return Some(candidate);
        }
        up = app.world().get::<ChildOf>(candidate).map(ChildOf::parent);
    }
    None
}

#[test]
fn the_whats_new_page_can_be_reached_to_its_bottom() {
    let mut app = whats_new_at_the_captures_window("reachable");
    let page = whats_new_page(&mut app);

    // The page drew every entry it lists: there is a bottom to reach before there is any question of
    // reaching it. A page that drew nothing would "fit" for the wrong reason.
    let drawn = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::launcher::NewsEntry>>();
        q.iter(world).count()
    };
    let listed = app.world().resource::<Launcher>().news.iter().take(NEWS_SHOWN).filter(|n| n.user.is_some()).count();
    assert_eq!(drawn, listed, "the page draws one row per entry it lists");

    // **The defect**: nothing above the page scrolls, so whatever the box cuts off cannot be reached.
    let Some(area) = scrolling_box(&mut app, page) else {
        let window_h = surface_height(&app);
        panic!(
            "nothing above the What's new page scrolls: {drawn} entries are drawn into a window \
             {window_h} px tall and the rest of them cannot be brought into view, which is the \
             capture's note (\"This has no scroll to it, so the bottom gets cut off and is not visible\")"
        );
    };

    // **And the thing it cut off is real**: the area holds more than it shows, and the last entry is
    // past its fold — measured from the entry's own rect, which is what a person would look for.
    let (travel, fold, last) = {
        let node = app.world().get::<ComputedNode>(area).expect("the area is laid out");
        let travel = (node.content_size().y - node.size().y) * node.inverse_scale_factor;
        let fold = node.size().y * node.inverse_scale_factor;
        (travel, fold, last_entry_bottom(&mut app, area))
    };
    assert!(
        travel > SLACK_PX,
        "the area has {travel} px of travel, so nothing was cut off and this test proves nothing"
    );
    assert!(
        last > fold + SLACK_PX,
        "the last entry ends at {last} px inside a {fold} px box, so nothing needed scrolling"
    );

    // Scrolled to the area's end, the last entry is inside it: the bottom is reachable, which is the claim.
    app.world_mut().get_mut::<ScrollPosition>(area).expect("the area holds a position").y = travel;
    for _ in 0..2 {
        app.update();
    }
    let reached = last_entry_bottom(&mut app, area);
    assert!(
        reached <= fold + SLACK_PX,
        "scrolled to the end ({travel} px) the last entry still ends at {reached} px in a {fold} px \
         box, so the bottom stays cut off"
    );
}

#[test]
fn the_whats_new_page_scrolls_with_the_wheel_over_it() {
    // The same claim, driven the way a person's wheel arrives: a `ScrollPosition` a test wrote itself
    // proves the node exists, not that the page answers input.
    let mut app = whats_new_at_the_captures_window("wheel");
    let page = whats_new_page(&mut app);
    let area = scrolling_box(&mut app, page)
        .unwrap_or_else(|| panic!("nothing above the What's new page scrolls, so its bottom is unreachable"));
    let (size, at) = {
        let world = app.world();
        (
            world.get::<ComputedNode>(area).expect("laid out").size(),
            world.get::<UiGlobalTransform>(area).expect("laid out").translation,
        )
    };
    assert!(size != Vec2::ZERO);

    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: at,
    };
    // **The pointer arrives first.** `bevy_ui_widgets`' handler is an observer on the area and Bevy
    // routes a scroll through the hover map; measured here, without this move three notches left the
    // page at 0 and with it the page moved 300 px.
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        location.clone(),
        PointerAction::Move { delta: Vec2::ZERO },
    ));
    app.update();
    for _ in 0..3 {
        app.world_mut().write_message(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Scroll {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: -1.0,
                phase: bevy::input::touch::TouchPhase::Moved,
            },
        ));
        app.update();
    }
    let after = app.world().get::<ScrollPosition>(area).map(|p| p.y).unwrap_or(0.0);
    assert!(
        after > 1.0,
        "three wheel notches over the What's new page left it at {after}: the page does not scroll, \
         so its bottom is unreachable"
    );
}
