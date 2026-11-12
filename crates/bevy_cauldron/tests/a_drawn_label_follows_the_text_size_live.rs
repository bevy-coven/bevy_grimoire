//! **A label already on screen follows the text size when the setting changes** (UI-73, #251).
//!
//! `the_theme_reaches_the_editors_own_labels` proves the setting reaches the atomic `chrome::font`
//! reads, and so every label drawn *afterwards*. What it says it does not prove is a label that was
//! already drawn: a `TextFont` is written once, when a widget is spawned, so the editor built at 100%
//! kept 100% after a person chose 175%. This is that half: the editor is built first, **the settings
//! file is written afterwards**, and the labels the editor drew at build time are read again.
//!
//! **Through the file, not the resource**: `sync_settings` re-resolves `EditorSettings` from the two
//! files every look, so a test that assigned the resource would be overwritten and prove nothing about
//! what a person editing their settings gets.
//!
//! **Every label, not one.** A single label growing would pass a weaker test while a panel's heading
//! stayed at the old size, which is the editor half-enlarged. So the assertion is the rule — *no
//! label [`chrome::typed`] drew is left at its old size* — plus a count that says the labels read are
//! the editor's own (fifty-odd of them, not the three a harness happens to spawn).

use bevy::prelude::*;
use bevy::text::{FontSize, TextFont};
use bevy_cauldron::chrome::{self, text, TypeRole};

use crate::fixtures;

/// Every label the editor drew through [`chrome::typed`], as `(entity, the role it is drawn at, its
/// size in logical pixels)` — the whole set the live resize has to move.
fn drawn(app: &mut App) -> Vec<(Entity, text::Role, f32)> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &TypeRole, &TextFont)>();
    q.iter(world)
        .filter_map(|(e, role, font)| match font.font_size {
            FontSize::Px(px) => Some((e, role.role(), px)),
            _ => None,
        })
        .collect()
}

/// **The editor is built at 100%, the file then says 175%, and every label already drawn is 175% of
/// its role.**
#[test]
fn a_label_already_drawn_follows_the_text_size_when_it_changes() {
    let root = fixtures::Fixture::new(&format!("live_text_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let settings_file = root.join(format!("user_config_live_text_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&settings_file, "(version: 1)").expect("a settings file");

    let was = chrome::text_scale();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(settings_file.clone()));
    for _ in 0..12 {
        app.update();
    }
    // The labels the editor drew at 100%: each one exactly its role's designed size.
    let before = drawn(&mut app);
    assert!(
        before.len() > 40,
        "the editor drew its labels through `chrome::typed`: only {} carried a role",
        before.len()
    );
    for (entity, role, px) in &before {
        assert!(
            (px - role.px()).abs() < 1e-3,
            "at 100% {entity:?} ({role:?}) is drawn at its role's size: {px} vs {}",
            role.px()
        );
    }

    // **After the build**, the person asks for larger text.
    std::fs::write(&settings_file, "(version: 1, text_scale: Some(1.75))").expect("a settings file");
    for _ in 0..12 {
        app.update();
    }
    let after = drawn(&mut app);
    chrome::set_text_scale(was);
    assert!(
        after.len() * 10 >= before.len() * 8,
        "most of the labels the editor drew are still drawn ({} then, {} now), so the rule below \
         covers the same editor",
        before.len(),
        after.len()
    );
    let survivors = before.iter().filter(|(e, _, _)| after.iter().any(|(e2, _, _)| e2 == e)).count();
    assert!(
        survivors * 10 >= before.len() * 8,
        "{survivors} of the {} labels drawn at 100% are still on screen to be read at 175%",
        before.len()
    );
    let stale: Vec<String> = after
        .iter()
        .filter(|(_, role, px)| (px - role.px() * 1.75).abs() >= 1e-3)
        .map(|(e, role, px)| format!("{e:?} {role:?}: {px} px, not {}", role.px() * 1.75))
        .collect();
    assert!(
        stale.is_empty(),
        "{} of {} labels already on screen kept the size they were spawned at after the setting \
         became 175%:\n{}",
        stale.len(),
        after.len(),
        stale.join("\n")
    );
}
