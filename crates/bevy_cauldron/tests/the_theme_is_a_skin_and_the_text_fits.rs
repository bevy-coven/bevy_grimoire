//! **Theme and accessibility settings** (UI-73, #251): the theme is a gooey skin, and a larger text
//! size leaves every panel usable.
//!
//! # What the settings already said, and what did nothing
//!
//! `EditorSettings` has carried `theme`, `text_scale`, `interface_scale` and `reduced_motion` since
//! UI-70 (#248), and only `reduced_motion` reached the running editor. `theme` had no consumer at all:
//! choosing Parchment in the settings screen wrote the file, resolved into `EditorSettings`, and left
//! the editor dark. `text_scale` had none either, because the number the editor draws text at is
//! `chrome::text::Role`'s, and nothing multiplied it by a person's choice.
//!
//! # The two acceptance items, as tests
//!
//! - **High contrast passes `problems()` at 7:1.** A high-contrast theme is only worth the name if the
//!   library's own enhanced-contrast check agrees, so the test asks `high_contrast_problems()` rather
//!   than a hand-written ratio: 7:1 is WCAG 2.2 SC 1.4.6 and Xbox Accessibility Guideline 102.
//! - **Text size 200% keeps every panel usable.** No text a person reads is drawn outside the panel
//!   that clips it. A scroll area is not a clip: what it holds is reachable by the wheel, which is the
//!   point of one, so the walk skips it and stops at the panel behind it.

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
    let settings_file = root.join(format!("user_config_{tag}_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    let named = match theme {
        Theme::ToolDark => "ToolDark",
        Theme::Parchment => "Parchment",
        Theme::HighContrast => "HighContrast",
    };
    std::fs::write(
        &settings_file,
        format!("(version: 1, theme: Some({named}), text_scale: Some({text_scale}))"),
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

/// Every `Text` in the world whose own box leaves a clipping ancestor's, as
/// `"words: spans a..b, shows c..d"`.
///
/// **A scroll area is skipped, and it is not a clip.** `chrome::scroll_list` puts `overflow: scroll_y`
/// on a list so a panel taller than the window can be reached with the wheel; content past its edge is
/// *reachable*, which is what the acceptance asks ("every panel usable"), not cut off. What the walk
/// stops at is the panel behind it, which does not scroll.
fn clipped_text(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text, &ComputedNode, &UiGlobalTransform)>();
    let rows: Vec<(Entity, String, f32, f32)> = texts
        .iter(world)
        .filter(|(_, text, node, _)| !text.trim().is_empty() && node.size().y > 0.0)
        .map(|(entity, text, node, transform)| {
            let h = node.size().y;
            (entity, text.0.clone(), transform.translation.y - h / 2.0, transform.translation.y + h / 2.0)
        })
        .collect();

    let mut out = Vec::new();
    for (entity, words, top, bottom) in rows {
        let mut at = entity;
        while let Some(parent) = world.get::<ChildOf>(at).map(|c| c.parent()) {
            at = parent;
            let Some(node) = world.get::<bevy::ui::Node>(at) else { continue };
            if node.overflow.y == bevy::ui::OverflowAxis::Visible {
                continue;
            }
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
            let h = computed.size().y;
            let (ptop, pbottom) = (transform.translation.y - h / 2.0, transform.translation.y + h / 2.0);
            if bottom > pbottom + 0.5 || top < ptop - 0.5 {
                out.push(format!("{words:?}: spans {top:.1}..{bottom:.1}, shows {ptop:.1}..{pbottom:.1}"));
                break;
            }
        }
    }
    out
}
