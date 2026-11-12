//! **`Walker`: the entity a person walks with the keyboard.**

use bevy::prelude::*;

/// **How fast a walker walks when a file does not say**, in metres a second.
pub const DEFAULT_WALK_SPEED_M_S: f32 = 4.0;

/// **An entity the arrow keys and WASD move across the ground**, at `speed_m_s`.
///
/// Up and W walk toward `-Z`, down and S toward `+Z`, left and A toward `-X`, right and D toward `+X`: the map's
/// own axes, which is what the ground a courtyard is laid out on reads. **Two keys at once are one step of
/// `speed_m_s`, not two**: a diagonal is no faster than a straight line.
///
/// **It writes the entity's local `Transform` and nothing else**, in `Update`, before propagation, so a
/// [`Trigger`](super::Trigger) volume and a [`Pickup`](super::Pickup) see this frame's position in the same frame.
/// It does not turn the entity: what faces which way is a game's own business.
///
/// **A game with no keyboard resource walks nowhere** rather than panicking, and so does one with no `Time`: the
/// rule every behaviour here follows.
///
/// ```yaml
/// hero: { Walker: { speed_m_s: 4.0 } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[require(Transform)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Walker {
    /// How fast it walks, in metres a second.
    pub speed_m_s: f32,
}

impl Default for Walker {
    fn default() -> Self {
        Self { speed_m_s: DEFAULT_WALK_SPEED_M_S }
    }
}

/// **The direction the held keys ask for**, on the ground, as a unit step or zero.
fn heading(keys: &ButtonInput<KeyCode>) -> Vec2 {
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let x = f32::from(u8::from(held(KeyCode::KeyD, KeyCode::ArrowRight))) - f32::from(u8::from(held(KeyCode::KeyA, KeyCode::ArrowLeft)));
    let z = f32::from(u8::from(held(KeyCode::KeyS, KeyCode::ArrowDown))) - f32::from(u8::from(held(KeyCode::KeyW, KeyCode::ArrowUp)));
    Vec2::new(x, z).normalize_or_zero()
}

/// **Move every walker one frame's step toward the held keys.**
pub(crate) fn walks(time: Option<Res<Time>>, keys: Option<Res<ButtonInput<KeyCode>>>, mut walkers: Query<(&Walker, &mut Transform)>) {
    let (Some(time), Some(keys)) = (time, keys) else { return };
    let step = heading(&keys);
    if step == Vec2::ZERO {
        return;
    }
    let delta_s = time.delta_secs();
    for (walker, mut transform) in &mut walkers {
        let metres = walker.speed_m_s.max(0.0) * delta_s;
        transform.translation += Vec3::new(step.x, 0.0, step.y) * metres;
    }
}
