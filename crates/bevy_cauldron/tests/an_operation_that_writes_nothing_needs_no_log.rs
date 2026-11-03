//! **`Ctrl+\` arms the snip without painting a red refusal over the editor** (#224).
//!
//! `snip_key` dispatches `snip_arm` — the operation an agent calls — so that the key and an agent
//! change one thing (E-002). It built its `Ctx` with `..Ctx::default()`, which carries **no project
//! and no root**, and `ops::dispatch` appends a log line for the change it made. The append could
//! not find a place to write, `take_back` reported a refusal, and the status painted
//!
//! ```text
//! the operation could not be logged, so nothing was changed:
//! no project is open, so there is nowhere to write the log.
//! ```
//!
//! over the editor on every press — while the tool **was** armed, so the sentence was false in both
//! halves. Measured on the 2026-11-03 hero recording: the banner is on every frame after the key,
//! and `grimoire/oplog_read` holds no `snip_arm` row.
//!
//! Every other key path that dispatches an operation carries the project (`environment_key`,
//! `set_yaw`, `set_tip`); this pins the one that did not. The assertion is the status the person
//! reads, which is what the recording showed, not an internal flag.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::harness;

use crate::fixtures;

/// **`Ctrl+\` as a person presses it**: the modifier held, the key tapped, the modifier released.
fn ctrl_backslash(app: &mut App) {
    let m = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(m);
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Backslash,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(m);
    app.update();
}

#[test]
fn the_snip_key_arms_and_its_status_is_a_receipt_not_a_refusal() {
    let root = fixtures::Fixture::new("snip_key_receipt")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..6 {
        app.update();
    }

    ctrl_backslash(&mut app);

    let armed = app.world().resource::<bevy_cauldron::snip::SnipEdit>().armed;
    assert!(armed, "Ctrl+\\ arms the snip");
    let status = app.world().resource::<bevy_cauldron::editor::EditorState>().status.clone();
    let problems = status.problems();
    assert!(
        problems.is_empty(),
        "after Ctrl+\\ the editor says {problems:?}. The snip IS armed, so \"nothing was changed\" \
         is false, and the red line covers the editor on every press. The key's context carries no \
         project, so the operation's own log line cannot be written"
    );
    let line = status.line().to_owned();
    assert!(
        !line.contains("could not be logged"),
        "the status reads {line:?} after arming the snip"
    );
}
