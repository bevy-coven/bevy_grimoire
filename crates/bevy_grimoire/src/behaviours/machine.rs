//! **`StateMachine`: states and the transitions between them, as data, stepped by signals.**

use bevy::prelude::*;

/// **One legal step: from a state, on a signal, to another state.**
///
/// It is a plain reflected value rather than a component — a machine carries a list of them and a file
/// writes the list — so it is registered as a type (`register_type`) and not as a component, which is why
/// `every_behaviour_is_a_registered_component_a_file_can_name` asks for one or the other.
#[derive(Reflect, Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[reflect(Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Transition {
    /// The state this step may be taken from.
    pub from: String,
    /// The signal name that takes it.
    pub on: String,
    /// The state it lands in.
    pub to: String,
}

/// **States, and the transitions between them, as data.**
///
/// `initial` is where a machine starts and `state` is where it is now: a file writes both, which is what lets
/// a save be resumed rather than reset. **A transition is only legal from the state it names**, so a signal
/// the current state does not carry moves nothing, and a machine with two transitions on one signal from one
/// state takes the first in the list — the list is the file's own order, so the file decides.
///
/// ```yaml
/// guard:
///   StateMachine:
///     initial: asleep
///     state: asleep
///     transitions:
///       - { from: asleep, on: noise, to: awake }
///       - { from: awake, on: quiet, to: asleep }
/// ```
#[derive(Component, Reflect, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct StateMachine {
    /// The state it starts in, kept so a game can reset to it.
    pub initial: String,
    /// The state it is in now.
    pub state: String,
    /// Every legal step, in the file's order.
    pub transitions: Vec<Transition>,
}

/// **A [`StateMachine`] stepped**, saying where it came from and where it went.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Transitioned {
    /// The machine that stepped.
    pub entity: Entity,
    /// The state it left.
    pub from: String,
    /// The state it is in.
    pub to: String,
    /// The signal that took it.
    pub on: String,
}

/// **Step every machine a signal moves**, once per signal, and only along a transition its own state carries.
pub(crate) fn steps(
    mut signals: MessageReader<super::Signal>,
    mut machines: Query<(Entity, &mut StateMachine)>,
    mut out: MessageWriter<Transitioned>,
) {
    let pressed: Vec<String> = signals.read().map(|signal| signal.name.clone()).collect();
    if pressed.is_empty() {
        return;
    }
    for (entity, mut machine) in &mut machines {
        // **Every signal in order**, so two signals in one frame are two steps rather than one arbitrary
        // one: the machine's state after the frame is what a game reading the last message would expect.
        for name in &pressed {
            let Some(step) = machine
                .transitions
                .iter()
                .find(|step| step.from == machine.state && step.on == *name)
                .cloned()
            else {
                continue;
            };
            machine.state = step.to.clone();
            out.write(Transitioned { entity, from: step.from, to: step.to, on: step.on });
        }
    }
}
