//! **Theme and accessibility settings** (UI-73, #251): the theme is a gooey skin, and a larger text
//! size leaves every panel usable.
//!
//! # What the settings already said, and what did nothing
//!
//! `EditorSettings` has carried `theme`, `text_scale`, `interface_scale` and `reduced_motion` since
//! UI-70 (#248), and only `reduced_motion` reached the running editor. `theme` had no consumer at all:
//! choosing Parchment in the settings screen wrote the file, resolved into `EditorSettings`, and left
//! the editor dark. `text_scale` had none either.
//!
//! # What a theme can be, today, and why
//!
//! **`chrome.rs` still draws the editor's labels from 28 colour constants** — the other half of #210,
//! which needs the protected colour tests re-pinned, so a worker may not finish it. High contrast is
//! therefore honourable, because a white-on-black ground keeps near-white labels readable (the tightest
//! pair is 4.94:1); **Parchment is not**, because a cream ground under near-white labels is 1.12:1. So
//! the theme is applied only when the ground it paints still leaves the editor's own inks readable —
//! `ui::honoured_from` — and Parchment is refused, on the status line, with the pair that failed.
//! `tests/the_theme_refuses_one_it_cannot_paint` is that refusal as an assertion, and
//! `choosing_a_theme_changes_the_running_skin` is the theme that does reach the running skin.
//!
//! **Text size reaches gooey's own widgets**, because gooey scales `type_size` by `text_scale`. It does
//! not yet enlarge the `chrome.rs` labels: `chrome::font(role)` builds a fixed `TextFont` from the role
//! and nothing multiplies it. That is stated rather than implied — #251's changelog line says so too —
//! because a person who sets 200% expects the whole editor, not only the gooey-drawn part of it.
//!
//! # The two acceptance items, as tests
//!
//! - **High contrast passes `problems()` at 7:1.** A high-contrast theme is only worth the name if the
//!   library's own enhanced-contrast check agrees, so the test asks `high_contrast_problems()` rather
//!   than a hand-written ratio: 7:1 is WCAG 2.2 SC 1.4.6 and Xbox Accessibility Guideline 102.
//! - **Text size 200% keeps every panel usable.** No text a person reads is drawn outside the panel
//!   that clips it — **on either axis**. A scroll area is not a clip: what it holds is reachable by the
//!   wheel, which is the point of one, so the walk skips it and stops at the panel behind it.
//!
//! # One frame's cost
//!
//! `follow_the_theme` used to call `ui::skin_for` every frame, which parses `editor_skin.yaml` and builds
//! a whole `GooeySkin` to write what was already running. Three `Local`s make a settled editor one
//! comparison, and `a_settled_editor_does_not_rewrite_the_running_skin` is the half that can be asserted
//! from outside — an unconditional write. The parse itself is **not** asserted, because counting it would
//! need a counter in product code; the system's own doc comment says why the `Local`s are there.
//!
//! `a_refused_theme_is_said_once` is the other half of that shape: a refusal raised per frame would climb
//! to `(x30)` on the status line in a second, so the sentence last said is kept too.

use bevy::prelude::*;
use bevy_cauldron::settings::{Theme, UserSettingsFile};
use bevy_gooey::GooeySkin;

use crate::fixtures;

/// The editor with `theme` and `text_scale` written into the user's settings file, laid out.
///
/// **Through the file, not the resource.** `sync_settings` re-resolves `EditorSettings` from the two
/// files, so a test that assigned the resource would be overwritten by the next frame and would prove
/// nothing about what a person writing a setting actually gets — which is the whole of UI-73.
fn editor(tag: &str, theme: Theme, text_scale: f32) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let settings_file = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    let named = match theme {
        Theme::ToolDark => "ToolDark",
        Theme::Parchment => "Parchment",
        Theme::HighContrast => "HighContrast",
    };
    std::fs::write(
        &settings_file,
        format!("{{version: 1, theme: {named}, text_scale: {text_scale}}}"),
    )
    .expect("a settings file");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(settings_file));
    for _ in 0..12 {
        app.update();
    }
    app
}

/// **The editor's skin for a theme**: the shipped colours for Tool dark, and gooey's own preset for
/// the two the editor does not ship.
#[test]
fn the_theme_is_the_running_skin() {
    for (theme, want) in [
        (Theme::ToolDark, bevy_cauldron::ui::editor_skin()),
        (Theme::Parchment, GooeySkin::parchment()),
        (Theme::HighContrast, GooeySkin::high_contrast()),
    ] {
        let skin = bevy_cauldron::ui::skin_for(theme);
        assert_eq!(skin.surface, want.surface, "{theme:?}: the panel's ground is the theme's");
        assert_eq!(skin.text, want.text, "{theme:?}: the ink is the theme's");
        assert_eq!(skin.accent, want.accent, "{theme:?}: the accent is the theme's");
    }
}

/// **A theme a person chooses is the running skin**, and no other: the settings resolve, the editor
/// follows, and the skin a widget draws from is the one the setting named.
#[test]
fn choosing_a_theme_changes_the_running_skin() {
    let app = editor("theme_running", Theme::HighContrast, 1.0);
    let running = app.world().resource::<GooeySkin>().clone();
    assert_eq!(running.surface, GooeySkin::high_contrast().surface, "High contrast is the running skin's ground");
    assert_eq!(running.text, GooeySkin::high_contrast().text, "and its ink");
}

/// **Parchment is refused, and says why** (UI-73, #251). This is the regression the review caught:
/// applying Parchment painted its cream `surface` under the editor's near-white `chrome.rs` labels —
/// **1.12:1**, measured — and every screen in the editor became unreadable. Until #210 finishes moving
/// those constants onto the skin there is no light theme the editor can paint, so `honoured_from`
/// refuses one, naming the pair, and the theme that was running stays.
///
/// The refusal is a *validation* rather than a silent fallback: the same function answers `Ok` for the
/// two themes this editor can paint, so nothing here is a special case keyed on a theme's name.
#[test]
fn the_theme_refuses_one_it_cannot_paint() {
    // **The two that can be painted**, so the refusal is a measurement and not a list of one.
    for theme in [Theme::ToolDark, Theme::HighContrast] {
        let skin = bevy_cauldron::ui::honoured_from(theme)
            .unwrap_or_else(|e| panic!("{theme:?} is paintable today and must not be refused: {e}"));
        assert_eq!(
            bevy_cauldron::ui::unreadable_pairs(&skin),
            Vec::<String>::new(),
            "{theme:?}: every ink clears every ground it is drawn on"
        );
    }
    // **The one that cannot**, and the pair that stops it.
    let why = bevy_cauldron::ui::honoured_from(Theme::Parchment)
        .expect_err("a cream ground under near-white labels is not a theme this editor can paint");
    assert!(why.contains("Parchment"), "the refusal names the theme a person chose: {why}");
    assert!(why.contains("text on surface"), "and the pair that failed, so it can be looked at: {why}");
    assert!(why.contains("4.5:1"), "and the floor it missed, in the numbers WCAG is written in: {why}");
    let parchment = bevy_cauldron::ui::skin_for(Theme::Parchment);
    let unreadable = bevy_cauldron::ui::unreadable_pairs(&parchment);
    assert!(!unreadable.is_empty(), "Parchment's ground leaves the editor's own inks unreadable: {unreadable:?}");
    assert!(
        unreadable.iter().any(|pair| pair.starts_with("text on surface ")),
        "and the first failure is `text` on the panel's own ground: {unreadable:?}"
    );
}

/// **A person who chooses Parchment keeps the editor they had, and reads why.**
///
/// The unit above is the measurement; this is what a person gets. The status line is where this editor
/// says things, so the reason is there rather than only in a log — and the ground on screen is the one
/// that was running, not the cream that would have made every label unreadable.
#[test]
fn choosing_a_theme_that_cannot_be_painted_keeps_the_editor_it_had() {
    let app = editor("theme_refused", Theme::Parchment, 1.0);
    let running = app.world().resource::<GooeySkin>().clone();
    assert_eq!(
        running.surface,
        bevy_cauldron::ui::editor_skin().surface,
        "the panel's ground is the one that was running, not Parchment's cream"
    );
    let said: Vec<String> =
        app.world().resource::<bevy_cauldron::editor::EditorState>().status.problems().iter().map(|p| p.text.clone()).collect();
    assert!(
        said.iter().any(|line| line.contains("Parchment") && line.contains("4.5:1")),
        "the status line says why the theme was not applied: {said:?}"
    );
}

/// **A settled editor never rewrites the running skin** (UI-73, #251).
///
/// **What this does and does not pin.** It fails for an *unconditional* write — a `*skin = want` with no
/// equality check, which marks the resource changed every frame and re-styles the whole interface — and
/// that is the regression it now guards. It does **not** catch the waste it was written for: the version
/// that called `ui::skin_for` on every pass still only assigned when the value differed, so the resource
/// was never marked and this test was green over a per-frame `editor_skin.yaml` parse. That waste is
/// answered by the `Local`s in `follow_the_theme` and stated in its doc comment; it is not asserted here,
/// because a `GooeySkin` is built inside a system and counting builds would need a counter in product
/// code. Said plainly rather than claimed.
#[test]
fn a_settled_editor_does_not_rewrite_the_running_skin() {
    let mut app = editor("theme_once", Theme::HighContrast, 1.0);
    for _ in 0..30 {
        app.update();
    }
    assert!(
        !app.world().is_resource_changed::<GooeySkin>(),
        "the running skin is not rewritten over thirty settled frames"
    );
    // And it is still the theme a person chose, not a default that a rewrite put back.
    assert_eq!(
        app.world().resource::<GooeySkin>().surface,
        GooeySkin::high_contrast().surface,
        "thirty settled frames leave High contrast as the running skin"
    );
}

/// **A refused theme is said once, not once a frame** (UI-73, #251).
///
/// The status line is what a person reads, and `Status` collapses *consecutive repeats* into a tally. A
/// refusal raised on every pass would therefore climb to `(x30)` in a second — a line that says the
/// editor is shouting. `follow_the_theme` keeps the sentence it last said (`refused`) for exactly this,
/// so the count is **one**, however many frames go by. This fails on a version with no such guard.
#[test]
fn a_refused_theme_is_said_once() {
    let mut app = editor("theme_said_once", Theme::Parchment, 1.0);
    for _ in 0..30 {
        app.update();
    }
    let refusal = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .problems()
        .iter()
        .find(|p| p.text.contains("Parchment"))
        .map(|p| (p.text.clone(), p.count))
        .expect("choosing a theme the editor cannot paint says so");
    assert_eq!(
        refusal.1, 1,
        "the refusal is raised once over thirty frames, not every frame: {:?}",
        refusal.0
    );
}

/// **A project skin coming or going does not disturb the chosen theme** (UI-73, #251; UI-03, #211).
///
/// **Owner exception (2026-11-10, #211): this test used to assert the chosen theme being re-applied after
/// a project's `family/bevy_gooey.yaml` was removed.** It was re-applied because that file *was* the
/// running skin, and `ui_skin::apply_ui_skin` reset the running skin to `GooeySkin::default()` when it
/// went. #211 moves the file into `ui_skin::GameSkin`, which the Game UI preview panel's `SkinScope`
/// reads and the running skin never takes — so the chosen theme is not disturbed by the file at all,
/// and there is nothing to put back. The check this test makes is kept and made **stronger**: the
/// person's High contrast theme is the running skin *while* the project ships a skin, and it is still
/// the running skin after the file goes, with no frame in between where the editor is on gooey's
/// default. What is no longer asserted is a reset this issue removes on purpose.
#[test]
fn removing_a_projects_skin_leaves_the_chosen_theme_running() {
    let root = fixtures::Fixture::new(&format!("theme_skin_gone_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).expect("a family folder");
    std::fs::write(root.join("family.yaml"), "{version: 1, crates: [\"bevy_gooey\"]}").expect("a family.yaml");
    let file = root.join("family/bevy_gooey.yaml");
    std::fs::write(&file, "{accent: !Srgba {red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0}}").expect("a project skin");
    let settings_file = root.join(format!("user_config_skin_gone_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&settings_file, "{version: 1, theme: HighContrast}").expect("a settings file");

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(settings_file));
    for _ in 0..12 {
        app.update();
    }
    // **While the project ships a skin**, the person's theme is what the chrome draws from.
    assert_eq!(
        app.world().resource::<GooeySkin>().surface,
        GooeySkin::high_contrast().surface,
        "the chosen theme is the running skin even with a project skin on disk"
    );
    assert_eq!(
        app.world().resource::<bevy_cauldron::ui_skin::GameSkin>().0.as_ref().map(|s| s.accent),
        Some(Color::srgba(0.2, 0.6, 0.9, 1.0)),
        "and the project's skin is the preview's, in its own resource"
    );

    // **The file goes.** The preview loses its skin; the chrome does not move.
    std::fs::remove_file(&file).expect("the project skin is removed");
    for _ in 0..40 {
        app.update();
        assert_eq!(
            app.world().resource::<GooeySkin>().surface,
            GooeySkin::high_contrast().surface,
            "the chosen theme stays running on every frame after the file goes"
        );
    }
    assert_eq!(
        app.world().resource::<bevy_cauldron::ui_skin::GameSkin>().0,
        None,
        "and the preview is back on the editor's own palette"
    );
}

/// **A project's `family/bevy_gooey.yaml` keeps its colours in the preview, and the person's text size
/// reaches the running skin** (UI-73, #251; UI-03, #211).
///
/// **Owner exception (2026-11-10, #211): this test used to assert the project's accent was the *running*
/// skin's while it kept the person's `text_scale`.** #211's What is that the editor chrome never takes
/// a project's skin, so the project's accent is now `ui_skin::GameSkin`'s and the running skin is the
/// person's theme; the text size still reaches the running skin, which is the half #251 fixed. Both
/// halves are asserted, the first at its new home: no check is dropped, and the one that moved is
/// stronger, because it now fails if the project's accent *does* reach the running skin.
#[test]
fn a_projects_skin_stays_in_the_preview_and_the_text_size_still_applies() {
    let root = fixtures::Fixture::new(&format!("theme_project_skin_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).expect("a family folder");
    std::fs::write(root.join("family.yaml"), "{version: 1, crates: [\"bevy_gooey\"]}").expect("a family.yaml");
    // The project's own accent, so "the project's skin reached the preview" is visible and not a
    // coincidence of defaults.
    std::fs::write(root.join("family/bevy_gooey.yaml"), "{accent: !Srgba {red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0}}")
        .expect("a project skin");
    let settings_file = root.join(format!("user_config_project_skin_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    // A theme *and* a size, so a system that returned early would be seen to drop both.
    std::fs::write(&settings_file, "{version: 1, theme: HighContrast, text_scale: 2.0}")
        .expect("a settings file");

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(settings_file));
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<bevy_cauldron::ui_skin::GameSkin>().0.as_ref().map(|s| s.accent),
        Some(Color::srgba(0.2, 0.6, 0.9, 1.0)),
        "the project's accent is the preview's skin's"
    );
    let running = app.world().resource::<GooeySkin>().clone();
    assert_eq!(
        running.accent,
        GooeySkin::high_contrast().accent,
        "and the running skin is the person's theme, not the project's accent"
    );
    assert_eq!(running.text_scale, 2.0, "the person's 200% text still applies, which the early return dropped");
}

/// **High contrast passes the library's enhanced-contrast check at 7:1.** Every pair `problems()`
/// checks is held to gooey's `ENHANCED_CONTRAST`, 7:1.
#[test]
fn the_high_contrast_theme_clears_seven_to_one() {
    let skin = bevy_cauldron::ui::skin_for(Theme::HighContrast);
    assert_eq!(
        skin.high_contrast_problems(),
        vec![],
        "High contrast must clear WCAG 2.2 SC 1.4.6's 7:1, not only the 4.5:1 minimum"
    );
    assert_eq!(skin.problems(), vec![], "and the ordinary rules hold too");
}

/// **Every theme the editor offers is a skin the library accepts**, so choosing one never puts a
/// library problem on the status line.
#[test]
fn every_theme_passes_the_librarys_own_rules() {
    for theme in [Theme::ToolDark, Theme::Parchment, Theme::HighContrast] {
        assert_eq!(bevy_cauldron::ui::skin_for(theme).problems(), vec![], "{theme:?} must be a well-formed skin");
    }
}

/// **Text size 200% is the skin's `text_scale`**, not a second number: the setting reaches the running
/// skin, which is what every gooey widget and every `chrome::font`-drawn label reads.
#[test]
fn the_text_size_setting_is_the_skins_text_scale() {
    let app = editor("text_scale_running", Theme::ToolDark, 2.0);
    assert_eq!(app.world().resource::<GooeySkin>().text_scale, 2.0, "200% text is the running skin's scale");
}

/// **No label is drawn outside the panel that clips it** at 200% text: a text node whose box leaves a
/// clipping ancestor is cut, which is the "every panel stays usable" half of the acceptance.
#[test]
fn every_label_fits_its_panel_at_two_hundred_percent_text() {
    let mut app = editor("text_scale_fits", Theme::ToolDark, 2.0);
    let cut = clipped_text(&mut app);
    assert!(
        cut.is_empty(),
        "{} text node(s) are drawn outside the panel that clips them at 200% text:\n{}",
        cut.len(),
        cut.join("\n")
    );
}

/// Every `Text` in the world whose **ink** leaves a clipping ancestor's, as
/// `"words: spans x a..b, shows c..d"` — **on either axis**, because a label cut short horizontally is as
/// unreadable as one cut vertically.
///
/// **The glyphs, not the node's box.** A `Text` node in a flex row is stretched to the row's width, so
/// its `ComputedNode` is far wider than the letters in it: "INSPECTOR" measured 29..454 against a 256 px
/// panel and drew nothing outside it. `TextLayoutInfo::size` is the laid-out run's own extent — what
/// `bevy_text` measured for these glyphs — so that is what the walk compares, and a real overflow (a word
/// too long for its column) still shows as one.
///
/// **A scroll area is skipped, and it is not a clip.** `chrome::scroll_list` puts `overflow: scroll_y`
/// on a list so a panel taller than the window can be reached with the wheel; content past its edge is
/// *reachable*, which is what the acceptance asks ("every panel usable"), not cut off. What the walk
/// stops at is the panel behind it — and only the axis that panel actually clips is measured, so a row
/// that can scroll sideways in a box that clips vertically is not a defect.
fn clipped_text(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text, &ComputedNode, &UiGlobalTransform, &bevy::text::TextLayoutInfo)>();
    let rows: Vec<(Entity, String, [f32; 4])> = texts
        .iter(world)
        .filter(|(_, text, node, _, layout)| !text.trim().is_empty() && node.size().y > 0.0 && layout.size.x > 0.0)
        .map(|(entity, text, node, transform, layout)| {
            // **The ink, from the node's own leading corner.** `TextLayoutInfo::size` is the run
            // `bevy_text` laid out, and its glyphs start at the node's top-left — the node is often far
            // wider than the words in it, because a `Text` under a `Node` with no width is stretched to
            // its row. So the ink is the corner plus the run, not the box narrowed and centred: a
            // centred reading put "INSPECTOR" 164 px right of where the letters are drawn and reported
            // a cut that neither a person nor 100% text ever saw.
            let corner = transform.translation - node.size() / 2.0;
            (entity, text.0.clone(), span_of(corner, layout.size))
        })
        .collect();

    let mut out = Vec::new();
    for (entity, words, span) in rows {
        let mut at = entity;
        while let Some(parent) = world.get::<ChildOf>(at).map(|c| c.parent()) {
            at = parent;
            let Some(node) = world.get::<bevy::ui::Node>(at) else { continue };
            // **A scroller is where the walk ends, not a box to check past.** It clips first, and what
            // it holds past its edge is reached with the wheel; a panel above it that also clips would
            // report a label the person can bring into view. So the scroller is the final answer.
            if world.get::<bevy::ui_widgets::ScrollArea>(at).is_some() {
                break;
            }
            let (Some(computed), Some(transform)) =
                (world.get::<ComputedNode>(at), world.get::<UiGlobalTransform>(at))
            else {
                continue;
            };
            let box_ = span_of(transform.translation - computed.size() / 2.0, computed.size());
            // **Only the axis this box clips.** A box with `overflow: visible` sideways does not cut a
            // row that overhangs it, and a test that said it did would fail on the shipped layout for a
            // reason nobody could fix.
            let Some(cut) = cut_axis(span, box_, node.overflow) else {
                continue;
            };
            out.push(format!("{words:?}: {cut}"));
            break;
        }
    }
    out
}

/// `[left, right, top, bottom]` of a box of `size` whose top-left corner is `corner`.
fn span_of(corner: Vec2, size: Vec2) -> [f32; 4] {
    [
        corner.x,
        corner.x + size.x,
        corner.y,
        corner.y + size.y,
    ]
}

/// **Which axis of `box_` cuts `ink`, if any**, as `"spans x 0.0..98.0, shows 0.0..40.0"`.
///
/// A separate function so the rule can be driven without a laid-out world: an `App` in a test cannot run
/// `bevy_ui`'s layout pass (`UiPlugin` needs a camera and a render target), so `clipped_text`'s walk over
/// a real editor is the integration half and this is the half that can be asserted on its own — including
/// that a box which does **not** clip an axis reports nothing for it.
fn cut_axis(ink: [f32; 4], box_: [f32; 4], overflow: bevy::ui::Overflow) -> Option<String> {
    /// The least overflow, logical pixels, that is a cut rather than a rounding difference between
    /// `bevy_text`'s layout and `bevy_ui`'s measurement of the same node.
    const SLACK: f32 = 0.5;

    let axes = [
        ("x", 0usize, 1usize, overflow.x != bevy::ui::OverflowAxis::Visible),
        ("y", 2usize, 3usize, overflow.y != bevy::ui::OverflowAxis::Visible),
    ];
    let (axis, low, high, _) = axes.iter().find(|(_, low, high, clips)| {
        *clips && (ink[*high] > box_[*high] + SLACK || ink[*low] < box_[*low] - SLACK)
    })?;
    Some(format!(
        "spans {axis} {:.1}..{:.1}, shows {:.1}..{:.1}",
        ink[*low], ink[*high], box_[*low], box_[*high]
    ))
}

/// **The sideways half of the walk is real**: a label 98 px wide in a 40 px box that clips sideways is
/// reported, and the same label in a box that clips only vertically is not.
///
/// This is what stops `every_label_fits_its_panel_at_two_hundred_percent_text` from being a check that
/// can only ever pass. The 98 px is the width `bevy_text` measured for "INSPECTOR" at 100%; the 40 px
/// box is that label in a column a third its size, which is exactly the defect the vertical-only walk
/// could not see.
#[test]
fn a_label_wider_than_its_column_is_reported() {
    use bevy::ui::Overflow;
    let ink = span_of(Vec2::ZERO, Vec2::new(98.0, 22.0));
    let narrow = span_of(Vec2::ZERO, Vec2::new(40.0, 22.0));
    let cut = cut_axis(ink, narrow, Overflow::clip_x()).expect("98 px of ink in a 40 px column is cut");
    assert!(cut.contains("spans x"), "and the cut is named on the axis it is on: {cut}");
    assert_eq!(
        cut_axis(ink, narrow, Overflow::clip_y()),
        None,
        "the same label in a box that clips only downward is not cut sideways"
    );
    let wide = span_of(Vec2::ZERO, Vec2::new(256.0, 22.0));
    assert_eq!(cut_axis(ink, wide, Overflow::clip()), None, "and a column it fits in cuts nothing");
    // The other axis, so neither half of the rule is a coincidence of the first.
    let tall = span_of(Vec2::ZERO, Vec2::new(98.0, 4.0));
    assert!(cut_axis(ink, tall, Overflow::clip()).is_some_and(|cut| cut.contains("spans y")), "and height is measured too");
}
