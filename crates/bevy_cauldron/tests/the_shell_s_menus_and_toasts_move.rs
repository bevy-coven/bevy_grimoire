//! **The shell's other motion: toasts enter from their edge and menus open from their anchor** (UI-R06, bevy-coven/bevy_grimoire#336).
//!
//! These are gooey's (#72 and #71) and the editor draws them through gooey's own widgets, so what is held here is that **the
//! editor's wiring lets them happen**: its toast stack hangs under the viewport in a strip, its menus are `menu_title`s of a bar. The headless editor runs with motion off so every other test
//! reads a result in the frame it is made; this puts the motion back and steps a clock the test sets, so a frame is a known length.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::report::ReportEdit;
use bevy_gooey::testing::press;
use bevy_gooey::{MotionSettings, PopoverPanel, PopoverSlide, TOAST_SLIDE_PX, Toast};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;
/// The scale a popover's panel starts at (gooey's `POPOVER_SCALE_FROM`), and the slack on reading it.
const POPOVER_SCALE_FROM: f32 = 0.96;
const SCALE_SLACK: f32 = 0.01;
/// Frames that settle any motion of this file on the clock above.
const SETTLE_FRAMES: usize = 90;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    // The motion back on, as a person has it.
    app.insert_resource(MotionSettings::default());
    frames(&mut app, SETTLE_FRAMES);
    app
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn toast_offset(app: &mut App) -> Option<f32> {
    let world = app.world_mut();
    let toast = world.query_filtered::<Entity, With<Toast>>().iter(world).next()?;
    match world.get::<UiTransform>(toast).map(|t| t.translation.y) {
        Some(Val::Px(y)) => Some(y),
        _ => Some(0.0),
    }
}

/// The toast's offset on each of the next `n` frames, from the first one it stands in.
fn follow_the_toast(app: &mut App, n: usize) -> Vec<f32> {
    let mut seen = Vec::new();
    for _ in 0..n {
        app.update();
        if let Some(y) = toast_offset(app) {
            seen.push(y);
        }
    }
    seen
}

/// **The first frame a toast is drawn it already stands toward the top edge, it only falls into place, and it ends exactly in it.**
fn enters_from_the_top(seen: &[f32], what: &str) {
    assert!(seen.len() >= 30, "{what} raised its toast: {seen:?}");
    assert!(seen[0] < -0.9 * TOAST_SLIDE_PX, "{what}: its first frame stands toward the top edge, {TOAST_SLIDE_PX} px above its place: {:?}", &seen[..seen.len().min(8)]);
    assert!(seen.windows(2).all(|pair| pair[1] >= pair[0] - 1e-3), "{what}: it only ever falls into place: {seen:?}");
    assert_eq!(*seen.last().expect("frames"), 0.0, "{what}: and settles exactly");
}

/// **A refusal's toast enters from the edge of the window its stack is anchored to**: the editor's stack is at the top, so it
/// starts above its place and settles on gooey's snappy spring.
#[test]
fn a_refusals_toast_enters_from_the_top_edge_and_settles() {
    let mut app = editor("shell_toast_enters");
    assert!(toast_offset(&mut app).is_none(), "a quiet editor raises nothing");
    app.world_mut().resource_mut::<EditorState>().status.problem("cannot do that: it is not allowed".to_owned());
    let seen = follow_the_toast(&mut app, 40);
    enters_from_the_top(&seen, "a refusal");
}

/// **The toast a report's Esc raises (report.rs) enters from the edge too**, not one frame late.
#[test]
fn a_cancelled_reports_toast_enters_from_the_top_edge_and_settles() {
    let mut app = editor("shell_report_toast");
    // Cmd+D opens a report, pointing; Esc closes it and says so in a toast.
    let chord = [bevy_cauldron::keys::MOD_KEYS[0], KeyCode::KeyD];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in chord {
            let logical_key = if key == KeyCode::KeyD { Key::Character("d".into()) } else { Key::Unidentified(NativeKey::Unidentified) };
            app.world_mut().write_message(KeyboardInput { key_code: key, logical_key, state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        frames(&mut app, 3);
    }
    assert!(app.world().resource::<ReportEdit>().phase().is_some(), "Cmd+D opened a report");
    assert!(toast_offset(&mut app).is_none(), "and it has said nothing yet");

    app.world_mut().write_message(KeyboardInput { key_code: KeyCode::Escape, logical_key: Key::Escape, state: ButtonState::Pressed, text: None, repeat: false, window: Entity::PLACEHOLDER });
    let seen = follow_the_toast(&mut app, 40);
    assert!(app.world().resource::<ReportEdit>().phase().is_none(), "Esc closed it");
    enters_from_the_top(&seen, "a cancelled report");
}

/// **A menu opens from its anchor**: the frame its panel is first drawn it is a touch small, and by the end of the spring it is whole.
#[test]
fn a_menu_opens_from_its_title_small_and_settles_whole() {
    let mut app = editor("shell_menu_opens");
    // The way a keyboard opens the bar: F10 lands on its first title and Down opens that menu.
    press(&mut app, KeyCode::F10);
    press(&mut app, KeyCode::ArrowDown);

    let mut scales = Vec::new();
    let mut seen_panels = Vec::new();
    for _ in 0..12 {
        let world = app.world_mut();
        let mut panels = world.query_filtered::<&UiTransform, With<PopoverSlide>>();
        scales.extend(panels.iter(world).map(|t| t.scale.x));
        let mut all = world.query_filtered::<(&UiTransform, &Visibility, Has<PopoverSlide>), With<PopoverPanel>>();
        seen_panels.push(all.iter(world).filter(|(_, v, _)| **v != Visibility::Hidden).map(|(t, _, slide)| (t.scale.x, slide)).collect::<Vec<_>>());
        app.update();
    }
    assert!(
        scales.iter().any(|&s| (POPOVER_SCALE_FROM - SCALE_SLACK..1.0 - 1e-3).contains(&s)),
        "the menu's panel was drawn on its way from {POPOVER_SCALE_FROM} to whole, not at once: {scales:?}; shown panels by frame (scale, sliding): {seen_panels:?}"
    );
    assert!(scales.iter().all(|&s| (POPOVER_SCALE_FROM - SCALE_SLACK..=1.0).contains(&s)), "and never outside that range: {scales:?}");

    frames(&mut app, SETTLE_FRAMES);
    let world = app.world_mut();
    assert_eq!(world.query_filtered::<(), With<PopoverSlide>>().iter(world).count(), 0, "the entrance is over and nothing of it is left on a node");
}
