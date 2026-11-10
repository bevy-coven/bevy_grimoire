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
///
/// **A pair this map has never seen is not a crossing.** A body standing in a doorway is a *state*, not something
/// that happened: a save put it there, nobody walked in. The pair is recorded and nothing is reported, and only a
/// change afterwards is a crossing.
///
/// **One path decides that, and it is the `insert` in the loop below.** It covers a body that was there when the
/// volume was first looked at and a body that appears inside it later — what a YAML reload does when it writes a
/// walker into a yard that already exists — because a new volume has no pairs and every pair of an old one is
/// either recorded or unseen. A second "first frame" set beside it changed no result and covered only half the
/// cases; it is gone, and `a_body_a_save_adds_inside_an_existing_volume_does_not_press_its_switch` is the test that
/// tells them apart.
#[derive(Resource, Default)]
pub(crate) struct Inside {
    /// The pairs this resource has seen, and whether the volume held the body. A pair absent here — either the
    /// volume or the body is new to it — is being looked at for the first time.
    seen: std::collections::BTreeMap<(Entity, Entity), bool>,
}

/// **Send a message for every body that entered or left a volume this frame**, and one signal for every volume
/// a body entered.
///
/// Two messages with two readings: `Triggered` reports a crossing in either direction, once per (volume, body)
/// pair, because a listener may want to know a guard left; [`Signal`](super::Signal) is the **way in**, once
/// per volume per frame, because it is what a `Switch` and a `StateMachine` read and the two must agree about
/// one event.
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
    inside.seen.retain(|(volume, body), _| here.contains(volume) && here.contains(body));

    for (volume, trigger, at) in &volumes {
        // **`signalled` is per volume, not per body**: a signal is a name something happened, and a
        // `StateMachine` takes one step per signal it reads while a `Switch` flips once however many it reads
        // — so two bodies entering one volume in one frame through two signals would step a machine twice
        // and flip a switch once. That is the two consumers disagreeing about one event. One signal per
        // volume per frame is what makes them agree.
        let mut signalled = false;
        for (body, _, where_) in &bodies {
            let holds = super::within(trigger.radius_m, at.translation(), where_.translation());
            // **A pair with no record yet is a body or a volume this resource has never looked at**, so its first
            // look records where it is and reports nothing: a save that puts someone in a doorway is a state, not
            // something that happened. Every other case is the crossing.
            let Some(before) = inside.seen.insert((volume, body), holds) else { continue };
            if holds == before {
                continue;
            }
            out.write(Triggered { trigger: volume, body, entered: holds, signal: trigger.signal.clone() });
            // **The signal is the way IN.** A volume writes it on entry and not on exit: a `Switch` toggles,
            // so signalling the way out would close the door the person just walked through — the guide's
            // "crossing the tripwire opens the gate" has to stay true after they have crossed. `Triggered`
            // still reports both, because a listener may want the exit; the signal is the one-way reading of
            // the same crossing. A volume with no `signal` writes none, rather than an empty name that would
            // match every other nameless part.
            if holds && !signalled && !trigger.signal.is_empty() {
                signals.write(super::Signal { name: trigger.signal.clone() });
                signalled = true;
            }
        }
    }
}
