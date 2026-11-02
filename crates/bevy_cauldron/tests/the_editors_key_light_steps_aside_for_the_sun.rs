//! **The editor's own key light steps aside while the camera wears `bevy_elements`' sky** (#103).
//!
//! The crate meters the camera's exposure for the sky's physical light, so a night is a gain of about ten
//! stops; the editor's fixed 6,000 lux key light at that gain lit the whole viewport white at
//! `clock.hour = 22.0`. While the editor camera carries `ElementsCamera` the key light is hidden, and it
//! comes back with the file. Needs `--features bevy_elements`.
#![cfg(feature = "bevy_elements")]

use bevy::prelude::*;
use bevy_cauldron::environment::key_light_follows_dressing;
use bevy_cauldron::view::{EditorKeyLight, MainCamera};
use bevy_elements::ElementsCamera;

fn rig() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_systems(Update, key_light_follows_dressing);
    let camera = app.world_mut().spawn(MainCamera).id();
    let light = app.world_mut().spawn((EditorKeyLight, DirectionalLight::default(), Visibility::Inherited)).id();
    (app, camera, light)
}

#[test]
fn the_key_light_is_hidden_while_the_camera_is_dressed_and_back_when_it_is_not() {
    let (mut app, camera, light) = rig();
    app.update();
    assert_eq!(*app.world().get::<Visibility>(light).unwrap(), Visibility::Inherited, "a project with no environment file keeps the editor's own light");

    app.world_mut().entity_mut(camera).insert(ElementsCamera::default());
    app.update();
    assert_eq!(*app.world().get::<Visibility>(light).unwrap(), Visibility::Hidden, "the sun and the moon are the key light");

    app.world_mut().entity_mut(camera).remove::<ElementsCamera>();
    app.update();
    assert_eq!(*app.world().get::<Visibility>(light).unwrap(), Visibility::Inherited, "deleting the file gives the editor's light back");
}

#[test]
fn a_key_light_the_editor_draws_again_while_dressed_is_hidden_too() {
    // A door change despawns and respawns what the editor spawned at startup; the new light must not
    // light the night.
    let (mut app, camera, light) = rig();
    app.world_mut().entity_mut(camera).insert(ElementsCamera::default());
    app.update();
    app.world_mut().despawn(light);
    let again = app.world_mut().spawn((EditorKeyLight, DirectionalLight::default(), Visibility::Inherited)).id();
    app.update();
    assert_eq!(*app.world().get::<Visibility>(again).unwrap(), Visibility::Hidden);
}
