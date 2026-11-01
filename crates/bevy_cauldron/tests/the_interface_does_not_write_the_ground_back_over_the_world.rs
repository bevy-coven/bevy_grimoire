//! **The interface camera must not copy the ground colour back over a dressed world** (#193, #194).
#![cfg(feature = "bevy_elements")]
//!
//! #118's composition (a transparent clear for the interface camera, premultiplied writes) was not
//! enough: the interface camera shares the ground camera's texture, and with MSAA on, Bevy's
//! `MsaaWriteback::Auto` copies what the lower camera wrote (the opaque ground colour) into the
//! interface camera's sampled texture *instead of* honouring its `Custom(Color::NONE)` clear. The
//! interface then wrote that opaque ground over the HDR world, so the viewport was one flat colour
//! with `family/bevy_elements.ron` present. Found live on 2026-10-31: with the interface camera's
//! `msaa_writeback` set to `Off` over BRP the viewport went from 1 colour to 456, the sky and the map's
//! box and grid. No GPU is available to a test, so this asserts the setting the frame depends on.

use bevy::camera::{Camera, MsaaWriteback};
use bevy::prelude::*;
use bevy_cauldron::environment::dress_camera;
use bevy_cauldron::family_host::FamilySettings;
use bevy_cauldron::surface::SurfaceCamera;
use bevy_cauldron::view::MainCamera;

#[test]
fn a_dressed_world_is_not_overwritten_by_a_copy_of_the_ground() {
    let mut app = App::new();
    app.init_resource::<FamilySettings>().add_systems(Update, dress_camera);
    app.world_mut().spawn((MainCamera, Camera::default()));
    let ui = app.world_mut().spawn((SurfaceCamera, Camera::default())).id();

    app.update();
    assert_eq!(
        app.world().get::<Camera>(ui).unwrap().msaa_writeback,
        MsaaWriteback::Auto,
        "no family file: the editor's own composition, untouched"
    );

    app.world_mut()
        .resource_mut::<FamilySettings>()
        .text
        .insert("bevy_elements".to_owned(), String::new());
    app.update();
    assert_eq!(
        app.world().get::<Camera>(ui).unwrap().msaa_writeback,
        MsaaWriteback::Off,
        "dressed: the interface starts from its transparent clear, not from a copy of the ground"
    );

    app.world_mut().resource_mut::<FamilySettings>().text.clear();
    app.update();
    assert_eq!(
        app.world().get::<Camera>(ui).unwrap().msaa_writeback,
        MsaaWriteback::Auto,
        "the file is gone: the editor's own composition returns"
    );
}
