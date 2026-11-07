//! **The command palette leaves on the skin's motion, and a motion setting reaches gooey** (#331).
//!
//! Closing the palette with its chord used to despawn it in the frame, so a person who saw it arrive saw
//! it vanish. It now takes gooey's `Leave`: the card is still drawn, fading, during the exit, and is gone
//! once the skin's `motion.default` has run and a frame has passed. The editor's "Reduced motion" setting
//! is gooey's `MotionSettings::reduced`. The clock is stepped by the test (`TimeUpdateStrategy`), so what
//! is measured is the exit's length and not the machine's load.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;
use bevy_gooey::{CommandPalette, GooeySkin, Leaving, MotionSettings, MotionToken};

use crate::fixtures;

/// One frame of the stepped clock, 60 Hz.
const FRAME: Duration = Duration::from_micros(16_667);

fn editor(tag: &str, settings: Option<&str>) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.ron");
    if let Some(text) = settings {
        std::fs::write(&file, text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    // **The headless editor runs with motion off** (`harness::build_headless_at`); this file is about the motion.
    app.insert_resource(MotionSettings::default());
    app.world_mut().resource_mut::<Time<Virtual>>().set_max_delta(FRAME);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME));
    for _ in 0..8 {
        app.update();
    }
    (app, file)
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// Ctrl+K (Cmd+K on a mac), pressed and released in two frames.
fn ctrl_k(app: &mut App) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one window");
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [primary(), KeyCode::KeyK] {
            app.world_mut().write_message(KeyboardInput {
                key_code: key,
                logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window,
            });
        }
        app.update();
    }
}

fn palettes(app: &mut App) -> Vec<Entity> {
    app.world_mut().query_filtered::<Entity, With<CommandPalette>>().iter(app.world()).collect()
}

#[test]
fn the_palette_is_drawn_while_it_leaves_and_gone_after_the_skins_default_motion() {
    let (mut app, _) = editor("palette_exit", None);
    ctrl_k(&mut app);
    for _ in 0..40 {
        app.update();
    }
    assert_eq!(palettes(&mut app).len(), 1, "Ctrl+K opens the palette");

    ctrl_k(&mut app);
    let palette = *palettes(&mut app).first().expect("the palette is still there in the frames after the chord");
    let leaving = app.world().get::<Leaving>(palette).expect("the palette is leaving, not despawned").opacity();
    assert!(leaving > 0.0 && leaving < 1.0, "a card mid-exit is drawn partly faded, not {leaving}");

    // `motion.default`, and one frame on top.
    let length = app.world().resource::<GooeySkin>().motion.spring(MotionToken::Default).duration;
    let frames = (length / FRAME.as_secs_f32()).ceil() as usize + 1;
    for _ in 0..frames {
        app.update();
    }
    assert!(palettes(&mut app).is_empty(), "{frames} frames ({length} s of motion.default and one more) later the palette is gone");
}

#[test]
fn the_chord_during_the_exit_brings_the_palette_back() {
    let (mut app, _) = editor("palette_stay", None);
    ctrl_k(&mut app);
    for _ in 0..40 {
        app.update();
    }
    ctrl_k(&mut app);
    ctrl_k(&mut app);
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(palettes(&mut app).len(), 1, "a second chord in the exit is a change of mind: the palette stays, once");
}

#[test]
fn reduced_motion_is_gooeys_reduced_motion_and_taking_it_out_puts_it_back() {
    let (mut app, file) = editor("reduced_motion", Some("(version: 1, reduced_motion: Some(true))"));
    let step = |app: &mut App| {
        for _ in 0..90 {
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    step(&mut app);
    assert!(app.world().resource::<MotionSettings>().reduced, "`reduced_motion: true` is gooey's reduced motion");
    std::fs::write(&file, "(version: 1)").unwrap_or_else(|e| panic!("{e}"));
    step(&mut app);
    assert!(!app.world().resource::<MotionSettings>().reduced, "without the key, motion is full again");
}
