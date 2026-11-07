//! **The editor's dialogs leave on gooey's springs, and Reduced motion is gooey's** (UI-R01, bevy-coven/bevy_grimoire#331).
//!
//! The headless editor runs with `MotionSettings::off()` so every other test reads a result in the frame
//! it is made. These put the motion back and step a clock the test sets, so a frame is a known length.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;
use bevy_gooey::{CommandPalette, GooeySkin, Leaving, MotionSettings};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn press(app: &mut App, keys: &[KeyCode]) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("a window");
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *key,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window,
            });
        }
        app.update();
    }
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn palettes(app: &mut App) -> Vec<Entity> {
    app.world_mut().query_filtered::<Entity, With<CommandPalette>>().iter(app.world()).collect()
}

#[test]
fn the_command_palette_leaves_on_a_spring_and_is_gone_after_the_default_spring_plus_a_frame() {
    let mut app = editor("moves_palette");
    app.insert_resource(MotionSettings::default());
    app.update();
    press(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyP]);
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(palettes(&mut app).len(), 1, "the palette is open");

    press(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyP]);
    assert_eq!(palettes(&mut app).len(), 1, "its card is still drawn in the frame it was asked to close");
    let motion = GooeySkin::default().motion;
    let halfway = (motion.fade_seconds / 2.0 / FRAME) as usize;
    for _ in 0..halfway {
        app.update();
    }
    let card = palettes(&mut app).first().copied().expect("and still drawn while it fades");
    let leaving = app.world().get::<Leaving>(card).expect("it is leaving, not open");
    assert!(!leaving.returning());
    assert!(leaving.opacity() > 0.0 && leaving.opacity() < 1.0, "mid-exit it is partly faded: {}", leaving.opacity());
    let by = (motion.default.duration / FRAME).ceil() as usize + 1;
    for _ in 0..by.saturating_sub(halfway) {
        app.update();
    }
    assert!(palettes(&mut app).is_empty(), "gone within the default spring plus a frame");
}

#[test]
fn a_command_chosen_from_the_palette_runs_while_the_palette_fades() {
    // The palette's marker outlives the close by the length of the exit; while it stood, the editor read
    // "a palette is open" as "typing" and refused the chord the chosen command is run through.
    let mut app = editor("moves_runs");
    app.insert_resource(MotionSettings::default());
    app.update();
    press(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyP]);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(palettes(&mut app).len(), 1, "the palette is open");
    press(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyP]);
    app.update();
    let card = palettes(&mut app).first().copied().expect("fading");
    assert!(app.world().get::<Leaving>(card).is_some(), "it is leaving");
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    assert_ne!(live.0, bevy_cauldron::keys::Context::Typing, "a palette on its way out holds no keys");

    // And asked again on its way out, it comes back instead of nothing happening.
    press(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyP]);
    for _ in 0..40 {
        app.update();
    }
    let cards = palettes(&mut app);
    assert_eq!(cards.len(), 1, "one palette, back from where it was drawn");
    assert!(app.world().get::<Leaving>(cards[0]).is_none());
}

#[test]
fn the_players_reduced_motion_is_gooeys_and_only_that_flag() {
    let mut app = editor("moves_reduced");
    let user = std::env::temp_dir().join(format!("moves_reduced_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a folder")).expect("a folder");
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(user.clone()));
    app.insert_resource(MotionSettings { duration_scale: 1.5, reduced: false });
    let until = |app: &mut App, want: bool| {
        let start = std::time::Instant::now();
        while start.elapsed().as_secs_f64() < 6.0 {
            app.update();
            if app.world().resource::<MotionSettings>().reduced == want {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    };
    std::fs::write(&user, "(version: 1, reduced_motion: Some(true))").expect("a settings file");
    assert!(until(&mut app, true), "Reduced motion writes gooey's MotionSettings::reduced");
    assert_eq!(app.world().resource::<MotionSettings>().duration_scale, 1.5, "and only that flag");
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&user, "(version: 1, reduced_motion: Some(false))").expect("a settings file");
    assert!(until(&mut app, false), "switching it off turns it off again");
}
