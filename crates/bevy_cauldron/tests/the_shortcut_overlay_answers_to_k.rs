//! **`K` reveals the shortcuts through gooey's own overlay** (UI-41, #235).
//!
//! `ShortcutBadges` is ExposeHK's overlay, and until bevy-coven/bevy_gooey#36 (G-32) it could only be
//! shown while a *modifier* was held exactly — `Modifiers::held(keys) == hold` — so this editor's own
//! key, `K`, could never reveal it, and `badges.rs` polled `keys::pressed` by hand. G-32 added
//! `Reveal::Key` and `BadgesShown`.
//!
//! Two things here, and each can fail:
//!
//! 1. **The editor installs the reveal**, so a `K` held down means the overlay is up. This is a fact
//!    about the editor's resources, not about a picture: it holds even with no widget to draw on.
//! 2. **It is `K` and not a modifier**, which is the whole point of the crate change. A build that
//!    regressed to `Modifiers::ALT` would pass a test that only asked "does holding something show
//!    badges"; asking *what* is held is what catches it.
//!
//! The drawing itself is `badges.rs`'s, with its own 13 tests in `headless.rs`; this file is about
//! which key the overlay answers to, not where the chips land.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    (root, app)
}

/// **The editor's overlay reveal is `K`, a plain key.**
#[test]
fn the_shortcut_overlay_is_revealed_by_k_and_not_by_a_modifier() {
    let (_root, app) = editor("badge-reveal-is-k");
    let badges = app
        .world()
        .get_resource::<bevy_gooey::ShortcutBadges>()
        .copied()
        .expect(
            "the editor must install `ShortcutBadges`, or holding K reveals nothing and \
             `badges.rs` is the only thing drawing an overlay",
        );
    assert_eq!(
        badges.hold,
        bevy_gooey::Reveal::Key(KeyCode::KeyK),
        "the editor's overlay key is K (`keys::Action::Shortcuts`, and \
         `a_key_still_fires_while_k_is_held` pins that K+verb still fires). A modifier here would \
         mean a key that does nothing in this editor reveals the overlay, and K does not."
    );
    assert_eq!(
        badges.delay_seconds, 0.0,
        "K is the overlay's own key, not a chord prefix: the 0.3 s default exists to stop a \
         modifier flashing the screen on the way to Ctrl+S, and nothing types K to reach a chord"
    );
}

/// **Holding `K` tells the host the overlay is up.**
///
/// `BadgesShown` is what a host reads to react to the overlay — this editor dims the ground and
/// suppresses its ink census — so it has to be true on the frames `K` is down and false again when
/// it comes up. Written through gooey's own system, not a second flag of ours.
#[test]
fn holding_k_raises_the_overlay_flag_and_releasing_it_drops_it() {
    let (_root, mut app) = editor("badge-reveal-flag");
    let shown = |app: &App| {
        app.world()
            .get_resource::<bevy_gooey::BadgesShown>()
            .copied()
            .unwrap_or_default()
            .0
    };
    assert!(
        !shown(&app),
        "the overlay must not be up before the key is pressed"
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyK);
    for _ in 0..3 {
        app.update();
    }
    assert!(
        shown(&app),
        "holding K must raise `BadgesShown`, or a host cannot tell the overlay is up and the \
         ground is never dimmed"
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyK);
    for _ in 0..3 {
        app.update();
    }
    assert!(
        !shown(&app),
        "releasing K must drop `BadgesShown` in the same press"
    );
}
