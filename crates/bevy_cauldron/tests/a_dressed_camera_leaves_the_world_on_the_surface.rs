//! **Dressing the editor camera with `ElementsCamera` must not wipe the world off the surface** (#118).
#![cfg(feature = "bevy_elements")]
//!
//! `ElementsCamera` requires `Hdr`, which moves the world camera onto its own texture, and each
//! camera then writes its result into the surface image. With the default `blend_state: None` the
//! later write replaces everything below it, so the interface camera (ordered after the world,
//! holding only the ground colour on the texture it shares with the ground camera) erased the
//! world: a black viewport on every frame. No GPU is available to a test, so this asserts what
//! the live frame depends on: while dressed, the interface camera starts from transparent and
//! both it and the world are written *over* what is below them; undressed, the editor's own
//! settings come back.

use bevy::camera::{CameraOutputMode, ClearColorConfig};
use bevy::prelude::*;
use bevy_cauldron::environment::dress_camera;
use bevy_cauldron::family_host::FamilySettings;
use bevy_cauldron::surface::SurfaceCamera;
use bevy_cauldron::view::MainCamera;

fn blends(c: &Camera) -> bool {
    matches!(c.output_mode, CameraOutputMode::Write { blend_state: Some(_), .. })
}

#[test]
fn a_dressed_camera_is_composited_over_the_ground_and_under_the_interface() {
    let mut app = App::new();
    app.init_resource::<FamilySettings>().add_systems(Update, dress_camera);
    let world = app
        .world_mut()
        .spawn((MainCamera, Camera { clear_color: ClearColorConfig::None, ..default() }))
        .id();
    let ui = app
        .world_mut()
        .spawn((SurfaceCamera, Camera { clear_color: ClearColorConfig::None, ..default() }))
        .id();

    app.update();
    assert!(!blends(app.world().get::<Camera>(ui).unwrap()), "no family file: untouched");

    app.world_mut()
        .resource_mut::<FamilySettings>()
        .text
        .insert("bevy_elements".to_owned(), String::new());
    app.update();
    let w = app.world().get::<Camera>(world).unwrap();
    let u = app.world().get::<Camera>(ui).unwrap();
    assert!(blends(w), "the HDR world is written over the ground, not instead of it");
    assert!(blends(u), "the interface is written over the world, not instead of it");
    assert!(matches!(u.clear_color, ClearColorConfig::Custom(_)), "the interface starts from transparent");

    app.world_mut().resource_mut::<FamilySettings>().text.clear();
    app.update();
    let u = app.world().get::<Camera>(ui).unwrap();
    assert!(!blends(u), "the file is gone: the editor's own composition returns");
    assert!(matches!(u.clear_color, ClearColorConfig::None));
}
