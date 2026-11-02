//! **The editor's state colours are gooey's own roles now, and `problems()` checks them** (UI-02, #210).
//!
//! G-33 (bevy-coven/bevy_gooey#37) added the interaction-state roles this editor had ten colours and no
//! field for: `hover`, `pressed`, `held`, `held_ground`, `unchecked`, `card`, `preview_edge` — plus
//! `held_fade`, one fade for both held colours. Before it, `chrome.rs` had to draw those states from its
//! own constants, and the theme could not be a skin.
//!
//! Three things this file holds down, and each can fail:
//!
//! 1. **Every state role is set on the editor's skin**, not left to gooey's defaults. A `None` answers
//!    from the nearest field, which is a sane default for a game and the wrong answer here: this editor
//!    has a *measured* colour for each state, and a skin that let them fall back would be a second,
//!    unmeasured palette.
//! 2. **Each is the colour `chrome.rs` draws that state with today**, so moving the theme onto the skin
//!    changes nothing to the eye — which is the second acceptance item's claim, made checkable before
//!    the clip exists.
//! 3. **`problems()` is still empty**, so the states a reader must tell apart — pressed from hovered —
//!    are far enough apart at the crate's own bar.
//!
//! What this does **not** do: delete `chrome.rs`'s constants. Twenty-eight of them still have readers
//! outside that module (SUGGEST and ACCENT 13 sites each, DIM and DANGER 10), so removing them is a wide
//! refactor and the rest of this issue. This file is the skin half, and it is what G-33 was for.

use bevy::prelude::*;
use bevy_cauldron::chrome;

fn skin() -> bevy_gooey::GooeySkin {
    bevy_cauldron::ui::editor_skin()
}

/// **Each state role carries the editor's own colour**, so the skin is a palette and not a default.
#[test]
fn every_interaction_state_role_carries_the_editors_own_colour() {
    let skin = skin();
    let got = [
        ("hover", skin.hover, chrome::ROW_HOVER),
        ("pressed", skin.pressed, chrome::ROW_PRESSED),
        // `GROUND_HELD` and `WORLD_HELD` are *opacities* (0.55 and 0.25), not colours: a held ground
        // fades its fill and a held gizmo fades further. gooey's `held`/`held_ground` are the colours
        // those opacities are applied to, and `held_fade` is the one number — so the pair is checked by
        // `one_fade_serves_both_held_colours` below, against the two constants, rather than as colours.
        ("held", skin.held, chrome::PANEL_BG),
        ("held_ground", skin.held_ground, chrome::ACCENT),
        ("unchecked", skin.unchecked, chrome::UNCHECKED),
        ("card", skin.card, chrome::OVERLAY_BG),
        ("preview_edge", skin.preview_edge, chrome::EDGE_STRONG),
    ];
    let mut missing = Vec::new();
    let mut wrong = Vec::new();
    for (name, role, want) in got {
        match role {
            None => missing.push(name),
            Some(colour) if colour != want => wrong.push(format!("{name}: {colour:?} is not {want:?}")),
            Some(_) => {}
        }
    }
    assert!(
        missing.is_empty(),
        "these state roles are unset on the editor's skin, so gooey answers them from the nearest field \
         and the editor's measured colour is lost: {missing:?}"
    );
    assert!(
        wrong.is_empty(),
        "these state roles carry a different colour from the one `chrome.rs` draws: {wrong:?}"
    );
}

/// **The skin still passes `problems()`**, so the states a reader must tell apart are.
#[test]
fn the_state_roles_keep_the_skin_passing_problems() {
    let skin = skin();
    let problems = skin.problems();
    assert_eq!(
        problems, vec![],
        "the editor's skin must pass `GooeySkin::problems()`; a state colour that landed too close to \
         its neighbour is exactly what this check exists to catch"
    );
    // **And pressed and hovered are the pair that matters.** A pressed row recessed *below* a hovered
    // one is the rule `chrome.rs` states; a skin where the two are one shade is a skin whose rows stop
    // answering the pointer.
    assert_ne!(
        skin.pressed, skin.hover,
        "a pressed row must not look like a hovered one"
    );
}

/// **One fade serves both held colours**, so the overlay dims the ground and its gizmos by one amount
/// rather than two that drift apart.
#[test]
fn one_fade_serves_both_held_colours() {
    let skin = skin();
    // **`GROUND_HELD` and `WORLD_HELD` are opacities**, so the skin's one `held_fade` stands for the
    // pair. It cannot be both numbers, so what is pinned is that it sits between them: a fade outside
    // that range would dim the ground more than the gizmos or less, and either way one of the two
    // constants would no longer describe what is on screen.
    let (ground, world) = (chrome::GROUND_HELD, chrome::WORLD_HELD);
    assert!(
        (world..=ground).contains(&skin.held_fade),
        "`held_fade` is {} — the editor fades a held ground to {ground} and a held gizmo to {world}, so \
         one number serving both has to sit between them",
        skin.held_fade
    );
}
