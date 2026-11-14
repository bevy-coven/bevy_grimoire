//! **A dock tab that is reordered glides to its place in the editor's own dock, and what does not glide, measured** (UI-R06, #336).
//!
//! gooey carries the motion (G-63, bevy-coven/bevy_gooey#70: a dock's rebuilt strip starts each tab of the same panel from where the old
//! one was drawn). What is the editor's is that its dock gets it. The claim was left unmeasured because "two tabs of a group land at one
//! point in the headless editor". That is true only of the **arrangement** dock, whose body is `Display::None` on purpose
//! (`chrome::spawn_frame`: the shell's own panels are still drawn by `chrome`, so that dock's tabs are never laid out). The **bottom dock**
//! (`chrome::BottomDock`) is a real gooey dock with a laid-out strip, so given a window with room for it and a clock the test steps, its
//! tabs stand at two points and a reorder can be measured frame by frame:
//!
//! - **the premise**: two tabs stand at two points, in the layout's order;
//! - **the tab whose corner stays put glides**: several frames, one direction, no frame more than half the way (the reordered strip's
//!   first tab);
//! - **the tab behind it lands in two frames, and the cause is measured**: a tab spawned by the rebuild is laid out unskinned and
//!   skinned a frame later (font 11 px to 11.7 px, border 1 px to 0.83 px at the editor's `UiScale` of 1.2), which widens the first tab
//!   by 3 px and moves the second's corner inside gooey's `LAYOUT_QUIET_FRAMES`, where a move lands instead of travelling. That is gooey's
//!   (the skin pass, and the quiet window reading it as settling; filed as `dark-incorporated/bevy_gooey#94`, G-83), so it is pinned here as measured: the day gooey skins a new tab on its
//!   first frame this test fails and says to tighten the glide test to both tabs;
//! - **the control**: with motion off the reorder lands in the frame it is made, so the measurements tell a glide from a jump.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::chrome::BottomDock;
use bevy_cauldron::harness;
use bevy_gooey::{Dock, DockLayout, MotionSettings, Role};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;
/// Frames that settle a glide (the default spring takes well under a second).
const SETTLE_FRAMES: usize = 120;
/// How close to its place a landed tab stands, and the most a spring may overshoot, logical px.
const SLACK_PX: f32 = 2.0;
/// The window the bottom dock has room in: the workbench floor and the dock's own height fit (`shell_view::give_the_bottom_dock_its_room`).
const TALL_SURFACE: (u32, u32) = (1536, 1500);
/// The shell's two registered panels the dock is given, in the order they start in (`ui::register_the_shell_panels`).
const START_ORDER: [&str; 2] = ["reload", "status"];
/// The tab titles those two panels draw.
const TITLES: (&str, &str) = ("RELOAD", "STATUS");
/// Frames a glide must be seen moving in, so a spring that is two frames long is not called a glide.
const LEAST_GLIDE_FRAMES: usize = 6;
/// The most frames a tab that lands may take to be where it is going (it lands on the frame after the skin is applied).
const LANDED_WITHIN_FRAMES: usize = 3;
/// The least distance, logical px, two tabs stand apart for them to be at two points and not one.
const LEAST_TAB_GAP_PX: f32 = 20.0;
/// The least distance, logical px, a reordered tab must travel for the test to be measuring anything.
const LEAST_TRAVEL_PX: f32 = 5.0;
/// A step smaller than this, logical px, is a tab at rest (a spring's last tail), not a frame it was moving in.
const MOVING_STEP_PX: f32 = 0.05;
/// The most a single frame may cover of the whole way, as a share of it: above this the tab jumped.
const MOST_OF_THE_WAY_IN_ONE_FRAME: f32 = 0.5;
/// Frames the editor is stepped before the dock is given its two tabs, so the shell is built and laid out.
const WARM_FRAMES: usize = 12;
/// Frames the dock is stepped after it is given its two tabs, so their strip is built and laid out before motion is turned on.
const DOCK_BUILD_FRAMES: usize = 30;
/// How many frames of a track a failure message prints.
const TRACK_SHOWN_FRAMES: usize = 8;
/// How much wider, logical px, a tab must be between its first frame and its last for "skinned a frame late" to be what was measured.
const LEAST_SKIN_WIDENING_PX: f32 = 1.0;
/// The gooey issue that owns the late skin: `dark-incorporated/bevy_gooey#94` (G-83).
const GOOEY_ISSUE: &str = "dark-incorporated/bevy_gooey#94";

fn editor(tag: &str, motion: MotionSettings) -> App {
    let root = fixtures::Fixture::new(&format!("dock_tab_glide_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, TALL_SURFACE.0, TALL_SURFACE.1).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..WARM_FRAMES {
        app.update();
    }
    // **Two tabs in the bottom dock's one group**, in the order they start in.
    set_the_order(&mut app, START_ORDER);
    for _ in 0..DOCK_BUILD_FRAMES {
        app.update();
    }
    app.insert_resource(motion);
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

fn set_the_order(app: &mut App, order: [&str; 2]) {
    let world = app.world_mut();
    let bottom = world.query_filtered::<Entity, With<BottomDock>>().single(world).expect("the frame has a bottom dock");
    let mut dock = world.get_mut::<Dock>(bottom).expect("a gooey dock");
    dock.0 = DockLayout::group(order);
}

/// **The tab drawn for a panel**: the parent of the `TabText` that says `title`.
fn tab(app: &mut App, title: &str) -> Entity {
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &Role, &ChildOf)>();
    texts
        .iter(world)
        .find(|(text, role, _)| **role == Role::TabText && text.0.trim() == title)
        .map(|(_, _, parent)| parent.parent())
        .unwrap_or_else(|| panic!("no tab titled `{title}` is drawn"))
}

/// Where a tab is drawn now, logical px of its centre: layout and travel included.
fn centre_x(app: &App, tab: Entity) -> f32 {
    let node = app.world().get::<ComputedNode>(tab).expect("a laid-out tab");
    let at = app.world().get::<UiGlobalTransform>(tab).expect("a placed tab").translation;
    at.x * node.inverse_scale_factor()
}

/// A tab's laid-out width, logical px.
fn width(app: &App, tab: Entity) -> f32 {
    let node = app.world().get::<ComputedNode>(tab).expect("a laid-out tab");
    node.size().x * node.inverse_scale_factor()
}

/// **What the reorder did, frame by frame**: where each tab started, and where it was drawn on every frame after.
struct Reorder {
    start: (f32, f32),
    first: Vec<f32>,
    second: Vec<f32>,
    /// The first recorded frame's width of each tab, and the last's.
    widths: ((f32, f32), (f32, f32)),
}

fn reorder(app: &mut App) -> Reorder {
    let (a, b) = (tab(app, TITLES.0), tab(app, TITLES.1));
    let start = (centre_x(app, a), centre_x(app, b));
    set_the_order(app, [START_ORDER[1], START_ORDER[0]]);
    let (mut first, mut second) = (Vec::new(), Vec::new());
    let mut widths = ((0.0, 0.0), (0.0, 0.0));
    for frame in 0..SETTLE_FRAMES {
        app.update();
        // The strip is rebuilt on the reorder, so the tabs are looked up again each frame.
        let (a, b) = (tab(app, TITLES.0), tab(app, TITLES.1));
        first.push(centre_x(app, a));
        second.push(centre_x(app, b));
        if frame == 0 {
            widths.0 = (width(app, a), width(app, b));
        }
        widths.1 = (width(app, a), width(app, b));
    }
    Reorder { start, first, second, widths }
}

/// **How many frames a tab was seen moving in, and its biggest single step along the way** (toward `to`, from `from`).
fn motion_of(track: &[f32], from: f32, to: f32) -> (usize, f32) {
    let sign = (to - from).signum();
    let (mut previous, mut moving, mut biggest) = (from, 0, 0.0_f32);
    for &x in track {
        let step = (x - previous) * sign;
        assert!(step >= -SLACK_PX, "a tab moved back by {} px mid-glide ({previous} to {x}): {track:?}", -step);
        if step.abs() > MOVING_STEP_PX {
            moving += 1;
        }
        biggest = biggest.max(step.abs());
        previous = x;
    }
    (moving, biggest)
}

/// **The premise the issue doubted**: in the editor's bottom dock the two tabs stand at two points, in the layout's order.
#[test]
fn the_bottom_docks_two_tabs_stand_at_two_points() {
    let mut app = editor("premise", MotionSettings::default());
    let (a, b) = (tab(&mut app, TITLES.0), tab(&mut app, TITLES.1));
    let (x_first, x_second) = (centre_x(&app, a), centre_x(&app, b));
    assert!(x_second - x_first > LEAST_TAB_GAP_PX, "the two tabs land at one point: `{}` at {x_first}, `{}` at {x_second}", TITLES.0, TITLES.1);
}

/// **A reordered tab glides to its place**: the tab that leads the new strip moves over many frames, one way, and no frame covers most of it.
#[test]
fn a_reordered_tab_glides_to_its_place_with_motion_on() {
    let mut app = editor("glide", MotionSettings::default());
    let r = reorder(&mut app);
    let end = (*r.first.last().expect("frames"), *r.second.last().expect("frames"));
    assert!(end.1 < end.0, "the strip is in the new order: `{}` at {}, `{}` at {}", TITLES.1, end.1, TITLES.0, end.0);
    assert!(r.start.0 < r.start.1, "and it started in the old one");

    // The reordered strip's first tab is `STATUS`, and it is the tab whose corner stays where the strip starts.
    let whole = (end.1 - r.start.1).abs();
    assert!(whole > LEAST_TRAVEL_PX, "`{}` did not move: {} to {}", TITLES.1, r.start.1, end.1);
    let (moving, biggest) = motion_of(&r.second, r.start.1, end.1);
    assert!(moving >= LEAST_GLIDE_FRAMES, "`{}` moved in only {moving} frame(s): that is a jump, not a glide: {:?}", TITLES.1, &r.second[..TRACK_SHOWN_FRAMES]);
    assert!(biggest <= whole * MOST_OF_THE_WAY_IN_ONE_FRAME + SLACK_PX, "`{}` covered {biggest} of {whole} px in one frame: that is a jump", TITLES.1);
}

/// **The tab behind it lands in two frames, and why** (measured; gooey's to fix). The strip rebuilt by the reorder is laid out before its
/// tabs are skinned and again after, at the editor's `UiScale`, so the leading tab is wider on the second frame; the tab behind has its
/// corner moved by that inside gooey's quiet window and lands. When gooey skins a new tab on its first frame this fails: then both tabs glide,
/// and `a_reordered_tab_glides_to_its_place_with_motion_on` should hold both.
#[test]
fn the_tab_behind_the_glide_lands_at_once_because_the_strip_is_skinned_a_frame_late() {
    let mut app = editor("landed", MotionSettings::default());
    let r = reorder(&mut app);
    let end = *r.first.last().expect("frames");
    assert!((r.first[LANDED_WITHIN_FRAMES] - end).abs() <= SLACK_PX, "`{}` was still travelling after {LANDED_WITHIN_FRAMES} frames: gooey now glides it too ({:?}) — {GOOEY_ISSUE} is fixed: hold both tabs to the glide test and delete this one", TITLES.0, &r.first[..TRACK_SHOWN_FRAMES]);
    let ((first_a, first_b), (last_a, last_b)) = r.widths;
    assert!(
        (last_a - first_a).abs() > LEAST_SKIN_WIDENING_PX || (last_b - first_b).abs() > LEAST_SKIN_WIDENING_PX,
        "the tabs were the same width on the first frame ({first_a}, {first_b}) and the last ({last_a}, {last_b}): the cause named here ({GOOEY_ISSUE}) is gone"
    );
}

/// **The control**: with motion off the reorder lands in the frame it is made, so the measurements above can tell a glide from a jump.
#[test]
fn with_motion_off_a_reordered_tab_lands_at_once() {
    let mut app = editor("jump", MotionSettings::off());
    let r = reorder(&mut app);
    let whole = (r.first.last().expect("frames") - r.start.0).abs().max((r.second.last().expect("frames") - r.start.1).abs());
    assert!(whole > LEAST_TRAVEL_PX, "the control moved nothing");
    for (title, track) in [(TITLES.0, &r.first), (TITLES.1, &r.second)] {
        let end = *track.last().expect("frames");
        assert!((track[LANDED_WITHIN_FRAMES] - end).abs() <= SLACK_PX, "`{title}` glided with motion off: {:?}", &track[..TRACK_SHOWN_FRAMES]);
    }
}
