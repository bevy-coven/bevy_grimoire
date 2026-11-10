//! **Undressing gives the editor's own camera back** (#193 step 3, #194).
//!
//! Deleting a project's `family/bevy_elements.yaml` removes `ElementsCamera`, and the crate now undoes
//! its own dressing (`bevy_coven/bevy_elements@41407d6`, this repo's #55). One thing it deliberately
//! cannot take back is `Hdr`: `ElementsCamera` arrives with `#[require(Hdr)]`, so the crate cannot know
//! whether the host had it first. #55's conclusion says plainly that this is *"left to the caller
//! (bevy_grimoire removes it itself)"* — and **this editor never did**. `grep -n 'Hdr' src/*.rs` has no
//! non-comment hit: nothing here adds it and nothing removes it.
//!
//! **Measured on big, 2026-11-03**, editor built `--features bevy_elements,bevy_flora` under Xvfb `:99`,
//! with `bevy_elements@41407d6` already pinned. Mean RGB and standard deviation of a 120×320 patch
//! inside the viewport:
//!
//! | state | mean | std |
//! |---|---|---|
//! | no settings file | `(132,127,120)` | `99.4` |
//! | file present (dressed) | `(149,157,172)` | `43.6` |
//! | file deleted, +5 s | `(19,18,15)` | **`1.7`** |
//! | file deleted, +10 s | `(19,18,15)` | `1.7` |
//!
//! A deviation of 1.7 across that patch is one flat colour: the world was gone while the status line
//! read *"bevy_elements settings removed; its defaults apply"*. That is this issue's step 3, still
//! broken in the editor after the crate fix.
//!
//! This file pins the part a test can prove without a GPU: the camera the editor keeps after the
//! undress is **not** HDR, and the composition is the editor's own again. Together they are the state
//! in which the world is drawn.
#![cfg(feature = "bevy_elements")]

use bevy::camera::{CameraOutputMode, ClearColorConfig};
use bevy::prelude::*;
use bevy_cauldron::environment::dress_camera;
use bevy_cauldron::family_host::FamilySettings;
use bevy_cauldron::surface::SurfaceCamera;
use bevy_cauldron::view::MainCamera;

/// The editor's map camera and its interface camera, with a `FamilySettings` a test can edit.
fn app() -> App {
    let mut app = App::new();
    app.init_resource::<FamilySettings>().add_systems(Update, dress_camera);
    let world = app
        .world_mut()
        .spawn((MainCamera, Camera::default()))
        .id();
    let ui = app
        .world_mut()
        .spawn((
            SurfaceCamera,
            Camera { clear_color: ClearColorConfig::None, ..default() },
        ))
        .id();
    app.insert_resource(Ids { world, ui });
    app.update();
    app
}

#[derive(Resource)]
struct Ids {
    world: Entity,
    ui: Entity,
}

/// **After the settings file goes, the map camera is not HDR.** This is the assertion that would have
/// caught #193 step 3, and the one `a_dressed_camera_leaves_the_world_on_the_surface` does not make:
/// that test pins the composition, which comes back, and never asks what mode the camera is in.
#[test]
fn the_undressed_camera_is_not_hdr() {
    let mut app = app();
    let ids = app.world().resource::<Ids>();
    let (world, ui) = (ids.world, ids.ui);

    // Dressed.
    app.world_mut()
        .resource_mut::<FamilySettings>()
        .text
        .insert("bevy_elements".to_owned(), String::new());
    app.update();

    // Undressed, by deleting the file, which is what a person does.
    app.world_mut().resource_mut::<FamilySettings>().text.clear();
    app.update();
    // One more frame: an observer that removes a component lands on the next flush.
    app.update();

    assert!(
        app.world().get::<bevy::camera::Hdr>(world).is_none(),
        "the editor must remove the Hdr that ElementsCamera required; leaving it is what drew one \
         flat colour for the rest of the session (#193 step 3)"
    );
    // **The interface composes the way the editor has always composed it.** `default()` is a `Write`
    // with `blend_state: None` — "write as-is" — so the thing that must come back is the *blend*, not
    // the variant: a dressed interface blends premultiplied over the world, an undressed one replaces.
    let ui_camera = app.world().get::<Camera>(ui).unwrap();
    match ui_camera.output_mode {
        CameraOutputMode::Write { blend_state, .. } => assert!(
            blend_state.is_none(),
            "an undressed interface writes as-is; a blend state left on is the dressed composition"
        ),
        other => panic!("the interface must write its target when undressed, got {other:?}"),
    }
}
