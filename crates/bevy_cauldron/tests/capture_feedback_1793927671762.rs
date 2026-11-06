//! **A prompt's keycap centres its letter** — `feedback-1793927671762`, relation: *alignment*.
//!
//! The note, filed on the editor at 2560x1604: *"The X is not actually centered here, it looks like
//! it's right aligned?"* — the `X` keycap in the toolbar's Remove-mode tooltip.
//!
//! # The relation, and why this one
//!
//! A picture has no expected output, so the oracle is a relation between two things that both exist in
//! one run: **the glyph's ink and the box that holds it**. Segura, Fraser, Sánchez & Ruiz-Cortés, *A
//! Survey on Metamorphic Testing* (`10.1109/TSE.2016.2532875`), find that the effective relations are
//! those "strongly inspired by the semantics of the program under test"; the semantics here are a
//! *centred* label, which the code declares itself — every keycap is spawned with
//! `TextLayout::new(Justify::Center, LineBreak::NoWrap)`. A keycap whose ink sits off-centre contradicts
//! its own declaration, whatever the skin's numbers happen to be.
//!
//! The survey's other finding is why this is measured rather than asserted from a constant. A relation
//! close to the implementation strategy — "left padding equals right padding" — *passes on this defect*,
//! because the padding is symmetric and the asymmetry arrives from somewhere else. So the assertion is
//! on where the glyphs landed, which is what a person can see.
//!
//! # What the capture shows
//!
//! Measured from the recorded frame at `scale_factor` 2: the keycap's border box spans x `397..440`
//! (centre `418.5`), the glyph's ink spans x `408..420` (centre `414.0`) — the letter sits **4.5
//! physical px (2.25 logical) left of centre**. The same frame measures y dead centre (`274.0` on both
//! sides), which is this test's control: nothing skews the box as a whole, only the ink inside it.

use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_gooey::Role;

use crate::fixtures;

/// How far a glyph may sit from its box's centre before it reads as "pushed to one side", logical px.
///
/// A monospace glyph's ink is itself a little asymmetric around its own advance — `X` has a wider left
/// sidebearing than right in many faces — so demanding zero would fail on a correct layout. Three
/// logical px is under half a character and well inside "looks centred"; the defect measured 2.25
/// logical px *on top of* that ink asymmetry, which is why the assertion is on the **advance box**
/// rather than on the ink: the advance is symmetric by construction for a monospace face, so its centre
/// is exactly where a centred label must put it.
const CENTRE_TOLERANCE_PX: f32 = 0.5;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **The `X` keycap in the Remove-mode tooltip centres its letter.**
///
/// The capture's own screen: the toolbar's Remove-mode button with its tooltip open, which is what puts
/// the `X` cap on screen at all — every tooltip's cap exists from startup and has no box until its box is
/// shown, which the first version of this test learned the hard way (twelve caps, all `0.0 x 0.0`).
///
/// The tooltip is opened the way `a_toolbar_tooltip_sits_on_its_button` opens one: focus on the widget,
/// with focus made visible. That is the editor's own path, not a test-only lever.
#[test]
// **Ignored until bevy-coven/bevy_gooey#44 lands**, and the reason is in the doc comment above: the cause
// is in the crate that lays the cap out, and this repo must not patch, wrap or vendor it. The test is the
// receipt — it fails on the pin this repo holds, and it is what proves the pin bump fixes the letter. It
// is `ignore`d rather than deleted because a deleted test is a forgotten defect, and rather than left red
// because a permanently red suite teaches a loop to ignore red.
#[ignore = "blocked on bevy-coven/bevy_gooey#44: the keycap arm does not centre its content"]
fn the_remove_mode_keycap_centres_its_letter() {
    let mut app = editor("capture_1793927671762");

    // **The Remove-mode button**, found by the words its tooltip carries — the same identification the
    // capture's own path uses (`Tooltip "Remove mode"`).
    let button = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_gooey::Tooltip)>();
        q.iter(world)
            .find(|(_, tip)| tip.text == "Remove mode")
            .map(|(entity, _)| entity)
            .expect("the toolbar has a Remove-mode button with a tooltip")
    };
    app.insert_resource(InputFocusVisible(true));
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(button, bevy::input_focus::FocusCause::Navigated);
    for _ in 0..6 {
        app.update();
    }

    // **The cap the capture pointed at**: the `PromptKey` inside that tooltip's box.
    let cap = {
        let world = app.world_mut();
        // **This tooltip's own cap**, not any `X` in the application: the editor draws several caps, and a
        // query for the text alone would answer with whichever spawned first — the mistake the terminal's
        // context-menu test already made once.
        let tip = world
            .get::<Children>(button)
            .and_then(|kids| kids.iter().find(|k| world.get::<bevy_gooey::TooltipBox>(*k).is_some()))
            .expect("a button with a Tooltip owns a TooltipBox");
        let mut caps = world.query::<(Entity, &Role, &Text)>();
        let found: Vec<Entity> = caps
            .iter(world)
            .filter(|(_, role, text)| **role == Role::PromptKey && text.0 == "X")
            .map(|(entity, _, _)| entity)
            .collect();
        let under_the_box: Vec<Entity> = world.get::<Children>(tip).map(|c| c.iter().collect()).unwrap_or_default();
        found
            .into_iter()
            .find(|entity| under_the_box.contains(entity))
            .expect("the Remove-mode tooltip draws its shortcut on a PromptKey cap reading X")
    };

    let (node_w, node_h, block_left, block_width, text) = {
        let world = app.world();
        let node = world.get::<ComputedNode>(cap).expect("the cap is laid out");
        let scale = node.inverse_scale_factor();
        let layout = world
            .get::<bevy::text::TextLayoutInfo>(cap)
            .expect("the cap's text is laid out");
        (
            node.size().x * scale,
            node.size().y * scale,
            layout.glyphs.first().map_or(0.0, |g| g.position.x * scale),
            layout.size.x * scale,
            world.get::<Text>(cap).map(|t| t.0.clone()).unwrap_or_default(),
        )
    };

    assert_eq!(text, "X", "the capture's pick names the X keycap");
    assert!(
        node_w > 0.0 && node_h > 0.0,
        "the cap has no box ({node_w} x {node_h}), so the tooltip did not open and this test measures \
         nothing"
    );

    // **A centred block leaves equal slack on both sides.** `TextLayoutInfo`'s block starts at its first
    // glyph's position, so slack-left is that and slack-right is what is left.
    let slack_left = block_left;
    let slack_right = node_w - block_left - block_width;
    let error = slack_left - slack_right;
    assert!(
        error.abs() <= CENTRE_TOLERANCE_PX,
        "the `X` keycap's letter is not centred in its box — the note says \"The X is not actually \
         centered here, it looks like it's right aligned?\"\n\n\
         box {node_w:.1} x {node_h:.1}, text block {block_width:.1} wide, sitting {slack_left:.1} from \
         the left edge and leaving {slack_right:.1} on the right: **{error:+.1} logical px off centre** \
         (tolerance {CENTRE_TOLERANCE_PX})."
    );
}
