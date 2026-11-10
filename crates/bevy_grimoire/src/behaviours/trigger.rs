//! **`Trigger`: a volume that says when a body enters it and when it leaves.**

use bevy::prelude::*;

/// **How wide a trigger is when a file does not say**, in metres.
pub const DEFAULT_TRIGGER_RADIUS_M: f32 = 1.0;

/// **A volume that sends a [`Triggered`] message when a [`Body`](super::Body) enters it and again when it
/// leaves.**
///
/// The volume is a sphere of `radius_m` world metres around the entity's own position, so a doorway is a
/// trigger on the door's threshold. Its `signal` is copied into every message it sends, which is what a game
/// compares against — the same name a [`Switch`](super::Switch) latch or a [`Countdown`](super::Countdown)
/// uses, so a room's parts name each other by one vocabulary.
///
/// **An event is a crossing, not a state.** Standing still inside a volume sends nothing; the message
/// carries `entered` so a listener can tell the way in from the way out.
///
/// ```yaml
/// tripwire: { Trigger: { radius_m: 1.5, signal: gate_crossed } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[require(Transform)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Trigger {
    /// How far from the volume's own position it still holds a body, in world metres.
    pub radius_m: f32,
    /// The name every message this volume sends carries.
    pub signal: String,
}

impl Default for Trigger {
    fn default() -> Self {
        Self { radius_m: DEFAULT_TRIGGER_RADIUS_M, signal: String::new() }
    }
}

/// **A body entered or left a [`Trigger`]'s volume.**
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Triggered {
    /// The volume that was crossed.
    pub trigger: Entity,
    /// The body that crossed it.
    pub body: Entity,
    /// `true` for the way in, `false` for the way out.
    pub entered: bool,
    /// The volume's own `signal`, copied so a listener does not have to look the volume up.
    pub signal: String,
}

/// **Which bodies each volume held last frame**, keyed by (volume, body), so a crossing is told from a
/// body that was already there.
///
/// A map rather than a component on the volume, because it is the volume's memory of *other* entities and
/// it must be pruned when either one despawns: two components holding each other's entities is the shape
/// that leaks.
#[derive(Resource, Default)]
pub(crate) struct Inside(std::collections::BTreeMap<(Entity, Entity), bool>);

/// **Send a message for every body that entered or left a volume this frame.**
pub(crate) fn crossings(
    mut inside: ResMut<Inside>,
    volumes: Query<(Entity, &Trigger, &GlobalTransform)>,
    bodies: Query<(Entity, &super::Body, &GlobalTransform)>,
    mut out: MessageWriter<Triggered>,
    mut signals: MessageWriter<super::Signal>,
) {
    // **Forget pairs whose volume or body is gone**, so a despawned body is not remembered forever and a
    // body respawned into the same index does not look like a body that was already inside.
    let here: std::collections::BTreeSet<Entity> =
        volumes.iter().map(|(volume, _, _)| volume).chain(bodies.iter().map(|(body, _, _)| body)).collect();
    inside.0.retain(|(volume, body), _| here.contains(volume) && here.contains(body));

    for (volume, trigger, at) in &volumes {
        for (body, _, where_) in &bodies {
            let holds = super::within(trigger.radius_m, at.translation(), where_.translation());
            let before = inside.0.insert((volume, body), holds).unwrap_or(false);
            if holds == before {
                continue;
            }
            out.write(Triggered { trigger: volume, body, entered: holds, signal: trigger.signal.clone() });
            // **The volume writes the signal as well as reporting the crossing**, so a `Switch` or a
            // `StateMachine` that names it answers without a game's code in between: one vocabulary across
            // every part of a room. A volume with no `signal` writes none, rather than an empty name that
            // would match every other nameless part.
            if !trigger.signal.is_empty() {
                signals.write(super::Signal { name: trigger.signal.clone() });
            }
        }
    }
}
