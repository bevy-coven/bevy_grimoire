//! **The ENVIRONMENT panel is reachable at the window the editor ships in** (#272).
//!
//! The Map tab's left panel is `chrome::scroll_list(p, MapControlsPane)`, and its own comment says the
//! scroll exists *for height*: "the readout, the map-size fields and anything ever added below them
//! simply ran off the bottom of the window". Measured on big, 2026-11-02, the panel **does not scroll
//! at all** — its region changed 0.0 % of its pixels after a burst of wheel notches — so
//! `environment::spawn_section` (spawned at `editor.rs:3481`, before the Compositions section) is in the
//! flow but below an unscrollable fold. Every clock, weather, season and climate button is unreachable
//! by a person at 1280x720.
//!
//! **What this file asserts is the thing a person needs**: the ENVIRONMENT heading is inside the
//! Inspector's scroll *content*, and when that scroll is at its end the heading is on screen — its top
//! is at or above the viewport's bottom, not below it. A headless `App` draws the same nodes the window
//! does, so this is the rule, not a picture of it.

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use crate::fixtures;

/// The Inspector's scroll viewport: the node carrying `ScrollArea` inside `MapControlsPane`.
fn viewport(app: &mut App) -> (Entity, Rect) {
    use bevy::ui_widgets::ScrollArea;
    // **By its marker, not by position**: ten scroll areas exist in the editor and only this one is
    // the Map tab's Inspector.
    let mut q = app.world_mut().query_filtered::<
        (Entity, &ComputedNode, &bevy::ui::UiGlobalTransform),
        (With<ScrollArea>, With<bevy_cauldron::editor::MapControlsPane>),
    >();
    let (entity, size, transform) = q
        .iter(app.world())
        .map(|(e, n, t)| (e, n.size(), *t))
        .next()
        .expect("the Map tab's Inspector is a scroll area with `MapControlsPane` on it");
    let centre = transform.translation;
    (entity, Rect::from_center_size(centre, size))
}

#[test]
fn the_environment_section_is_inside_the_inspectors_scroll_and_on_screen_at_its_end() {
    let root = fixtures::Fixture::new("environment_reachable")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }

    let (viewport_entity, viewport_rect) = viewport(&mut app);
    // **The heading is a child of the scroll's content**, which is the "it is in the flow" half.
    let heading = app
        .world_mut()
        .query::<(&ChildOf, &Text)>()
        .iter(app.world())
        .filter(|(_, text)| text.0 == "ENVIRONMENT")
        .map(|(child, _)| child.parent())
        .next()
        .expect("an ENVIRONMENT heading exists (the feature is on and the section is spawned)");
    let mut ancestor = Some(heading);
    let mut inside = false;
    while let Some(e) = ancestor {
        if e == viewport_entity {
            inside = true;
            break;
        }
        ancestor = app.world().get::<ChildOf>(e).map(|c| c.parent());
    }
    if !inside {
        // Name the ancestry, so a failure says which panel the heading is under.
        let mut names = Vec::new();
        let mut at = Some(heading);
        while let Some(e) = at {
            if e == viewport_entity {
                names.push("VIEWPORT".to_owned());
            }
            at = app.world().get::<ChildOf>(e).map(|c| c.parent());
        }
        panic!(
            "the ENVIRONMENT heading is not inside the Inspector's scroll content (viewport {viewport_entity:?}, \
             chain reached {names:?}); scroll areas in the world: {}",
            app.world_mut().query_filtered::<Entity, With<bevy::ui_widgets::ScrollArea>>().iter(app.world()).count()
        );
    }

    // **And it can be reached.** Scroll to the end the way the wheel does, then read where the heading
    // landed: its top must be at or above the viewport's bottom (a row entirely below it is off screen).
    let content = app.world().get::<ComputedNode>(viewport_entity).map(|n| n.content_size().y).unwrap_or(0.0);
    let page = viewport_rect.height();
    if content > page {
        if let Some(mut scroll) = app.world_mut().get_mut::<bevy::ui::ScrollPosition>(viewport_entity) {
            scroll.0 = Vec2::new(0.0, content - page);
        }
        app.update();
    }
    let heading_rect = {
        let node = app.world().get::<ComputedNode>(heading).expect("the heading has a computed node");
        let t = app.world().get::<bevy::ui::UiGlobalTransform>(heading).expect("and a transform");
        Rect::from_center_size(t.translation, node.size())
    };
    assert!(
        heading_rect.min.y < viewport_rect.max.y,
        "at the scroll's end the ENVIRONMENT heading is still below the Inspector's viewport \
         (heading at {:.0}..{:.0}, viewport ends at {:.0}; content {:.0} over a {:.0} page) — the panel \
         cannot be scrolled to",
        heading_rect.min.y,
        heading_rect.max.y,
        viewport_rect.max.y,
        content,
        page
    );
}
