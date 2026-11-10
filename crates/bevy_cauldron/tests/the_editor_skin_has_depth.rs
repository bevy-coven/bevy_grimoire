//! **The editor's skin carries depth, and what stands above the screen casts it** (UI-R02, #332).
//!
//! `editor_skin.ron` keeps the editor's measured colours and sizes (the tests beside it pin them) and
//! adds bevy_gooey's elevation stacks, so a panel, a popover and a dialog stand at three heights.

use bevy::prelude::*;
use bevy_cauldron::ui::editor_skin;
use bevy_gooey::panel;

use crate::fixtures;

/// A stack's largest blur, logical px.
fn blur(layers: &[bevy_gooey::ShadowLayer]) -> f32 {
    layers.iter().map(|layer| layer.blur).fold(0.0, f32::max)
}

#[test]
fn the_skin_has_three_heights_each_softer_and_longer_than_the_last_and_is_still_a_ladder() {
    let skin = editor_skin();
    assert_eq!(skin.problems(), vec![], "depth did not break the library's own rules");
    let e = &skin.elevation;
    assert!(!e.raised.is_empty() && !e.floating.is_empty() && !e.overlay.is_empty(), "every height casts a shadow");
    assert!(blur(&e.raised) < blur(&e.floating) && blur(&e.floating) < blur(&e.overlay), "higher is softer");
    let drop = |layers: &[bevy_gooey::ShadowLayer]| layers.iter().map(|layer| layer.y).fold(0.0, f32::max);
    assert!(drop(&e.raised) < drop(&e.floating) && drop(&e.floating) < drop(&e.overlay), "and drops further");
    let lightness = |c: Color| Oklaba::from(c).lightness;
    for step in 1..skin.surfaces.len() {
        assert!(
            lightness(skin.surface_at(step as u8)) > lightness(skin.surface_at(step as u8 - 1)),
            "surface {step} is lighter than the one below"
        );
    }
    assert_ne!(skin.elevation, bevy_gooey::Elevation::none(), "not the empty default");
}

#[test]
fn the_file_states_the_stacks_itself() {
    // Not the library's default filling a gap: the file names `elevation`, so the depth is the editor's data.
    let text = include_str!("../src/editor_skin.yaml");
    let value = bevy_grimoire_core::yaml::to_json(text, "editor_skin.yaml").expect("the editor skin reads as YAML");
    let serde_json::Value::Object(fields) = value else { panic!("a skin is a struct") };
    assert!(fields.contains_key("elevation"), "src/editor_skin.yaml states its own `elevation`");
}

#[test]
fn a_panel_in_the_running_editor_casts_the_skins_raised_shadow() {
    let root = fixtures::Fixture::new(&format!("skin_depth_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let card = app.world_mut().spawn(panel()).id();
    for _ in 0..4 {
        app.update();
    }
    let shadow = app.world().get::<BoxShadow>(card).expect("a panel is given a shadow");
    assert_eq!(shadow.0.len(), editor_skin().elevation.raised.len(), "one shadow per layer of the raised stack");
}
