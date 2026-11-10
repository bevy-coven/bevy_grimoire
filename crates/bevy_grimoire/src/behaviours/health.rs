//! **`Health` and `Damage`: hit points, and the one-shot blow that takes them off.**

use bevy::prelude::*;

/// **The least health an entity may have**, which is a floor rather than a sign: damage past zero is still
/// zero, and the dead do not go negative and die again.
pub const NO_HEALTH: i32 = 0;

/// **What a file that writes `Health` with no fields gets**: one point of one.
///
/// One rather than zero because an entity is alive when its `Health` arrives; zero would make the first thing
/// a game spawns already dead, which no file means.
pub const DEFAULT_HEALTH: Health = Health { current: 1, max: 1 };

/// **What a file that writes `Damage: {}` gets**: one point off.
///
/// One rather than zero because the file asked for a blow; a zero-point blow would take nothing and report
/// nothing, which is a `Damage` that does not do what its name says.
pub const DEFAULT_DAMAGE: i32 = 1;

/// **Hit points an entity has left, out of its maximum.**
///
/// `current` is what a blow comes off and `max` is what a game shows a bar against. **`current` is clamped
/// to `[0, max]`** when it is written from a file, so a save cannot make an entity stronger than its own
/// maximum or bring it back below zero.
///
/// ```yaml
/// bandit: { Health: { current: 10, max: 10 } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Health {
    /// Hit points left. Never below [`NO_HEALTH`] nor above `max`.
    pub current: i32,
    /// The most this entity may have.
    pub max: i32,
}

impl Default for Health {
    fn default() -> Self {
        DEFAULT_HEALTH
    }
}

impl Health {
    /// **The value a file's `current` means, given its `max`** — the one place the clamp lives, so a save
    /// and the first frame a game runs agree about how hurt an entity is.
    pub fn clamped(&self) -> Self {
        let max = self.max.max(NO_HEALTH);
        Self { current: self.current.min(max).max(NO_HEALTH), max }
    }
}

/// **A blow waiting to land**: `amount` comes off [`Health`] on the next frame, and the component goes with
/// it.
///
/// **On an entity with no `Health` it is taken off and nothing is reported**: there are no hit points for it
/// to come off, and a component that stayed would land the day a `Health` was added — a blow struck in the
/// past arriving on a later frame is worse than one that did nothing.
///
/// A file writes it to give an entity a wound at load; a game writes it to hit one. **It is one-shot on
/// purpose**: a damage component that stayed would be a wound taken every frame, which is the shape that
/// makes an entity die of a single arrow while nobody is looking.
///
/// ```yaml
/// bandit: { Health: { current: 10, max: 10 }, Damage: { amount: 4 } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Damage {
    /// How many hit points it takes off, which may be more than are left.
    pub amount: i32,
}

impl Default for Damage {
    fn default() -> Self {
        Self { amount: DEFAULT_DAMAGE }
    }
}

/// **Damage was taken**, with how much actually came off — which is `Damage.amount` unless the entity had
/// less than that left.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct Damaged {
    /// The entity hit.
    pub entity: Entity,
    /// How many hit points came off.
    pub amount: i32,
    /// How many are left.
    pub remaining: i32,
}

/// **The last point went**: one message per death, and none for a blow on something already dead.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct Died {
    /// The entity that died.
    pub entity: Entity,
}

/// **Settle every entity's [`Health`], then take any waiting [`Damage`] off it**, and say what happened.
///
/// The clamp is here rather than in the reader because the reader is GD-05's and knows nothing about hit
/// points: a file says `current: 99, max: 10`, and the first frame it is live the value is 10. **One pass
/// over one query**, so a wound lands in the same frame the value it lands on is made legal and there is no
/// order between them to get wrong.
pub(crate) fn take_damage(
    mut commands: Commands,
    mut healths: Query<(Entity, Option<&mut Health>, Option<&Damage>)>,
    mut damaged: MessageWriter<Damaged>,
    mut died: MessageWriter<Died>,
) {
    for (entity, health, damage) in &mut healths {
        // **A blow on an entity with no hit points is still taken off it**: there is nothing for it to come
        // off, and a `Damage` left where it fell would land the day a `Health` was added. Nothing is
        // reported, because nothing was damaged.
        let Some(mut health) = health else {
            commands.entity(entity).remove::<Damage>();
            continue;
        };
        // Written only when it differs, so a settled entity is not marked changed every frame.
        let settled = health.clamped();
        if health.current != settled.current || health.max != settled.max {
            health.current = settled.current;
            health.max = settled.max;
        }
        let Some(damage) = damage else { continue };
        // **A negative amount is a heal nobody wrote**, so it is refused by doing nothing rather than
        // adding health through a path that says "damage" — a file that wants to heal writes `Health`.
        // `clamp` would panic on a minimum above a maximum, and a library never panics: the two-sided
        // `min`/`max` is the same answer for every input.
        let came_off = damage.amount.min(health.current).max(0);
        let was_alive = health.current > NO_HEALTH;
        health.current = (health.current - came_off).max(NO_HEALTH);
        commands.entity(entity).remove::<Damage>();
        if came_off > 0 {
            damaged.write(Damaged { entity, amount: came_off, remaining: health.current });
        }
        // **One death, one message**: a blow on a corpse reports nothing.
        if was_alive && health.current == NO_HEALTH {
            died.write(Died { entity });
        }
    }
}
