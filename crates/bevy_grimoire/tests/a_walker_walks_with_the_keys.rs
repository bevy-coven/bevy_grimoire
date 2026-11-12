//! **`Walker` moves an entity with WASD and the arrows** (GD-07, #369; the sample game's hero, #372).

use std::time::Duration;

use bevy::prelude::*;
use bevy_grimoire::behaviours::{BehaviourPlugin, Walker};

/// **One second a frame**, so a walker at `4 m/s` moves four metres a frame.
fn frame(app: &mut App) {
    app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs(1));
    app.update();
}

fn walker(speed_m_s: f32) -> (App, Entity) {
    let mut app = App::new();
    app.init_resource::<Time>().init_resource::<ButtonInput<KeyCode>>();
    app.add_plugins(bevy::transform::TransformPlugin).add_plugins(BehaviourPlugin);
    let entity = app.world_mut().spawn((Walker { speed_m_s }, Transform::default())).id();
    (app, entity)
}

fn at(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<Transform>(entity).map(|t| t.translation).unwrap_or_default()
}

#[test]
fn a_held_key_walks_the_map_axes_at_the_speed() {
    let (mut app, hero) = walker(4.0);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyD);
    frame(&mut app);
    assert!((at(&app, hero) - Vec3::new(4.0, 0.0, 0.0)).length() < 1e-4, "D is +X: {:?}", at(&app, hero));
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyD);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ArrowUp);
    frame(&mut app);
    assert!((at(&app, hero) - Vec3::new(4.0, 0.0, -4.0)).length() < 1e-4, "Up is -Z: {:?}", at(&app, hero));
}

#[test]
fn a_diagonal_is_no_faster_than_a_straight_line() {
    let (mut app, hero) = walker(4.0);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyD);
        keys.press(KeyCode::KeyS);
    }
    frame(&mut app);
    assert!((at(&app, hero).length() - 4.0).abs() < 1e-4, "one step of the speed: {:?}", at(&app, hero));
}

#[test]
fn no_keys_no_walk_and_no_keyboard_no_panic() {
    let (mut app, hero) = walker(4.0);
    frame(&mut app);
    assert_eq!(at(&app, hero), Vec3::ZERO);
    app.world_mut().remove_resource::<ButtonInput<KeyCode>>();
    frame(&mut app);
    assert_eq!(at(&app, hero), Vec3::ZERO, "a game with no keyboard walks nowhere");
}
