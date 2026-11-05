//! **The project page's first frame has its sidebar** (`feedback-1793901499397`, filed against 0.2.5 on
//! `mac_air`).
//!
//! The note: *"When I press this button, there is a garbled flash, before the UI paints correctly."*
//! The pick is the `bevy_grimoire` project card on the Projects home. Reproduced on `mac_air` against
//! the installed 0.2.6 in 2 of 8 runs: one frame between the pressed card and the finished page shows
//! the page's body laid out **across the whole window, with no sidebar at all** — no splash, no
//! Projects/Learn/What's new, no Settings, no version line — and the next frame is correct
//! (`/tmp/flash/evidence/wrong_c1_f008.png`, 158 KB against the correct frame's 208 KB).
//!
//! **The relation is the shell's own invariant, held on every frame it exists**: the page is a split of
//! a sidebar and a body, so on no frame may either pane be absent or the body take the window. That is
//! the metamorphic relation the note describes — "before the UI paints correctly" means there is a
//! frame where it does not — and it is chosen from the note rather than from the code (Segura, Fraser,
//! Sánchez & Ruiz-Cortés, *A Survey on Metamorphic Testing*, IEEE TSE 2016, `10.1109/TSE.2016.2532875`).
//!
//! Measured on the base commit, one `app.update()` at a time from the screen change: **frame 0 has the
//! body at 1066 px and the sidebar at 0**; frame 1 has 826 and 240. `gooey::update_splits` runs in
//! `GooeySystems::Style`, which is after layout, so the frame that *spawns* the split is laid out with
//! the pane defaults — `flex_grow: 0.5` each and no minimum — and the fraction is applied one frame
//! later.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::launcher::{LauncherPlugin, SidebarRoot};
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

/// The capture's window: `physical` 2560 x 1406 at `scale_factor` 2.0 is 1280 x 703 logical, which at
/// the harness's `UiScale` of 1.2 is this many texels.
const CAPTURE_SURFACE: (u32, u32) = (1280, 703);

/// The sidebar's own width, in logical pixels: `launcher::SIDEBAR_PX`, restated here so this test can
/// say what it expects without reading a private constant. The narrowest it may be drawn is what the
/// split's `SplitMin` holds it to.
const SIDEBAR_PX: f32 = 240.0;

/// One logical pixel of slack: layout rounds to whole physical pixels.
const SLACK_PX: f32 = 1.0;

/// **The page, entered from the home the way a click enters it**, with every frame's split measured.
///
/// The screen is set directly rather than by clicking a card: what is under test is the shell the page
/// spawns, and a click would add the home's own teardown to the frames being read. The crossing itself
/// is covered by `the_project_page::the_first_frame_of_the_page_is_whole`.
fn the_page_and_its_frames(tag: &str) -> Vec<(f32, f32)> {
    let root = Fixture::new(&format!("first_frame_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    bevy_cauldron::harness::resize_surface(&mut app, CAPTURE_SURFACE.0, CAPTURE_SURFACE.1)
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Project);

    let mut frames = Vec::new();
    for _ in 0..5 {
        app.update();
        let sidebar = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<&ComputedNode, With<SidebarRoot>>();
            q.iter(world).map(|node| node.size().x * node.inverse_scale_factor).next()
        };
        let body = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<&ComputedNode, With<bevy_cauldron::launcher::HomeContent>>();
            q.iter(world).map(|node| node.size().x * node.inverse_scale_factor).next()
        };
        frames.push((sidebar.unwrap_or(0.0), body.unwrap_or(0.0)));
    }
    frames
}

#[test]
fn the_pages_first_frame_has_a_sidebar() {
    let frames = the_page_and_its_frames("sidebar");
    // The shell exists from the frame the page is entered, so every frame measured is a frame it draws.
    let (sidebar, body) = frames[0];
    assert!(
        sidebar >= SIDEBAR_PX - SLACK_PX,
        "the page's first frame draws its sidebar {sidebar:.1} px wide where the shell holds it at \
         {SIDEBAR_PX:.0} px, and its body {body:.1} px across the whole {} px window: that is the \
         capture's \"garbled flash\" — the body painted over the application before the sidebar \
         arrives (frames: {frames:?})",
        CAPTURE_SURFACE.0,
    );
}

#[test]
fn no_frame_of_the_page_draws_the_body_without_the_sidebar() {
    // The same claim as a relation over the whole entrance: the split divides one window, so at no
    // point may one pane hold the window and the other hold nothing.
    let frames = the_page_and_its_frames("every_frame");
    for (index, (sidebar, body)) in frames.iter().enumerate() {
        assert!(
            *sidebar >= SIDEBAR_PX - SLACK_PX,
            "frame {index} draws the sidebar {sidebar:.1} px wide (body {body:.1} px): the page's \
             shell is a split of two panes and one of them cannot be nothing (frames: {frames:?})"
        );
        assert!(
            *body > 0.0,
            "frame {index} draws the body {body:.1} px wide beside a {sidebar:.1} px sidebar, so the \
             page has no content (frames: {frames:?})"
        );
    }
}
