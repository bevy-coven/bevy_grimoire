//! **Text size and Interface size are sliders, and the screen says who a change applies to** (#303).
//!
//! The first-run audit found the two sizes as numeric boxes ("1.0" typed into a field), the layer
//! chooser as "Edit: ‹ Your settings ›". A bounded value is a slider (Lightbown 2015: familiar controls,
//! recognition over recall), so each size is a slider from 75% to 200% in steps of 5%, drawn as a
//! percentage, and the chooser reads **Applies to: Just me / This project**.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui_widgets::{Slider, SliderRange, SliderStep, SliderValue, ValueChange};
use bevy_cauldron::project::Project;
use bevy_cauldron::settings::{SettingsLayer, UserSettingsFile};
use bevy_cauldron::settings_screen::OpenSettings;
use bevy_gooey::testing::find_by_label;

use crate::fixtures;

fn rig(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().get_resource::<Project>().is_some(), "the rig opens a project, so both scopes exist");
    app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        app.update();
    }
    (app, user)
}

/// The slider whose label says `label`: gooey names a widget by its words, and a slider is the widget.
fn slider_named(app: &mut App, label: &str) -> Entity {
    let slider = find_by_label(app, label).unwrap_or_else(|e| panic!("{e}"));
    assert!(app.world().get::<Slider>(slider).is_some(), "`{label}` is not a slider: the widget that says it has no `Slider`");
    slider
}

/// The percentage the slider's readout shows.
fn readout(app: &mut App, slider: Entity) -> String {
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &bevy_gooey::Role)>();
    let kids: Vec<Entity> = world.entity(slider).get::<Children>().map(|c| c.iter().collect()).unwrap_or_default();
    let mut found = Vec::new();
    for e in kids.iter().flat_map(|k| std::iter::once(*k).chain(world.get::<Children>(*k).map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default())) {
        if let Ok((t, role)) = texts.get(world, e)
            && *role == bevy_gooey::Role::SliderReadout
        {
            found.push(t.0.clone());
        }
    }
    found.into_iter().next().unwrap_or_else(|| panic!("the slider has no readout"))
}

#[test]
fn text_and_interface_size_are_sliders_from_75_to_200_percent_in_steps_of_5() {
    let (mut app, _) = rig("sizes_are_sliders");
    for label in ["Text size", "Interface size"] {
        let slider = slider_named(&mut app, label);
        let range = app.world().get::<SliderRange>(slider).expect("a slider has a range");
        assert_eq!((range.start(), range.end()), (0.75, 2.0), "{label} runs from 75% to 200%");
        assert_eq!(app.world().get::<SliderStep>(slider).map(|s| s.0), Some(0.05), "{label} steps by 5%");
        assert_eq!(readout(&mut app, slider), "100%", "{label} is drawn as a percentage, and its default is 100%");
    }
}

#[test]
fn moving_a_size_slider_writes_the_key_and_the_readout_follows() {
    let (mut app, user) = rig("size_slider_writes");
    let slider = slider_named(&mut app, "Text size");
    app.world_mut().trigger(ValueChange { source: slider, value: 1.25_f32, is_final: true });
    for _ in 0..4 {
        app.update();
    }
    let text = std::fs::read_to_string(&user).unwrap_or_else(|e| panic!("the slider wrote no file: {e}"));
    let layer = SettingsLayer::read("test", &text).unwrap_or_else(|r| panic!("{}", r.reason));
    assert_eq!(layer.text_scale, Some(1.25), "the slider's final value is the file's key");
    let slider = slider_named(&mut app, "Text size");
    assert_eq!(app.world().get::<SliderValue>(slider).map(|v| v.0), Some(1.25));
    assert_eq!(readout(&mut app, slider), "125%", "the readout says the new size as a percentage");
}

#[test]
fn a_drag_in_progress_writes_nothing_until_it_lets_go() {
    let (mut app, user) = rig("size_slider_drag");
    let slider = slider_named(&mut app, "Interface size");
    app.world_mut().trigger(ValueChange { source: slider, value: 1.5_f32, is_final: false });
    for _ in 0..4 {
        app.update();
    }
    assert!(!user.exists(), "a drag in progress must not write the file on every frame: the rows are rebuilt from the file and would throw the slider away under the pointer");
    let slider = slider_named(&mut app, "Interface size");
    assert_eq!(readout(&mut app, slider), "150%", "but the readout follows the pointer");
}

#[test]
fn the_layer_chooser_says_who_a_change_applies_to() {
    let (mut app, _) = rig("applies_to");
    assert!(find_by_label(&mut app, "Applies to").is_ok(), "the chooser is named for what it decides");
    // A choice shows the option it is on, which is the first: the user's own file.
    assert!(find_by_label(&mut app, "Just me").is_ok(), "the chooser opens on `Just me`");
    assert!(find_by_label(&mut app, "Your settings").is_err());
}
