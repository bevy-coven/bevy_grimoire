//! **`Pickup`, `Body` and `Collector`: something on the floor, and the one who reaches it.**

use bevy::prelude::*;

/// **How close a collector must come to take a pickup when a file does not say**, in metres.
pub const DEFAULT_PICKUP_RADIUS_M: f32 = 1.0;

/// **Marks an entity a [`Trigger`](super::Trigger) volume notices and a [`Pickup`] may be taken by.**
///
/// A body is what "someone is here" means: a trigger that fired on any entity would fire on the floor, the
/// walls and its own volume. It is a marker, so it carries no fields and a file writes it as `{}`.
///
/// ```yaml
/// hero: { Body: {} }
/// ```
#[derive(Component, Reflect, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[require(Transform)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Body {}

/// **Marks an entity that takes a [`Pickup`] it reaches.**
///
/// Separate from [`Body`] because they are different questions — a guard is something a tripwire notices
/// and does not take the coin lying beside it — and a game may put one on an entity without the other.
///
/// ```yaml
/// hero: { Collector: {} }
/// ```
#[derive(Component, Reflect, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[require(Transform)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Collector {}

/// **Something a [`Collector`] that reaches it takes, once.**
///
/// `item` is the name the game gives what was taken, and `radius_m` is how far a collector may stand from
/// the pickup and still take it. **A pickup despawns when it is taken**: a thing picked up is not on the
/// floor waiting to be picked up again.
///
/// ```yaml
/// purse: { Pickup: { item: gold_coin, radius_m: 0.75 } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[require(Transform)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Pickup {
    /// What it is, in the game's own vocabulary.
    pub item: String,
    /// How close a collector must come, in world metres.
    pub radius_m: f32,
}

impl Default for Pickup {
    fn default() -> Self {
        Self { item: String::new(), radius_m: DEFAULT_PICKUP_RADIUS_M }
    }
}

/// **A [`Collector`] took a [`Pickup`].**
#[derive(Message, Clone, Debug, PartialEq)]
pub struct PickedUp {
    /// The pickup, which is despawned by the time this is read.
    pub pickup: Entity,
    /// The collector that took it.
    pub by: Entity,
    /// The pickup's `item`.
    pub item: String,
}

/// **Let each collector take the nearest pickup within its own radius**, and despawn what it takes.
///
/// **Nearest, not first**: two coins a metre apart must not be taken in the order the world happens to
/// iterate them, and a collector standing between them should get the closer one. Ties go to the lower
/// `Entity`, so a run of the same save takes the same coin.
pub(crate) fn taken(
    mut commands: Commands,
    collectors: Query<(Entity, &GlobalTransform), With<Collector>>,
    pickups: Query<(Entity, &Pickup, &GlobalTransform), Without<Collector>>,
    mut out: MessageWriter<PickedUp>,
) {
    // **What is already taken this frame**, so two collectors standing on one coin do not both take it and
    // the second report a pickup that is no longer there.
    let mut gone: std::collections::BTreeSet<Entity> = std::collections::BTreeSet::new();
    for (by, at) in &collectors {
        let nearest = pickups
            .iter()
            .filter(|(entity, _, _)| !gone.contains(entity))
            .filter(|(_, one, where_)| super::within(one.radius_m, at.translation(), where_.translation()))
            .min_by_key(|(entity, _, where_)| {
                // Distance compared as squared metres: the square root would change no ordering and cost a
                // `sqrt` per candidate.
                let gap = at.translation().distance_squared(where_.translation());
                (bevy::math::FloatOrd(gap), *entity)
            });
        let Some((pickup, one, _)) = nearest else { continue };
        gone.insert(pickup);
        out.write(PickedUp { pickup, by, item: one.item.clone() });
        commands.entity(pickup).despawn();
    }
}
