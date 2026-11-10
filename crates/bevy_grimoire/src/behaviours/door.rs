//! **`Switch` and `Door`: a latch and the door that follows it, linked by name.**

use bevy::prelude::*;

/// **A named latch a [`Signal`](super::Signal) turns on and off.**
///
/// Its `latch` is the signal name it answers, so a button, a trigger volume or another game system presses
/// it by writing one [`Signal`](super::Signal) — no entity wiring, which is what lets a room be a file with
/// names in it. A [`Door`] with the same name follows it.
///
/// ```yaml
/// latch: { Switch: { latch: north, on: false } }
/// ```
#[derive(Component, Reflect, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
pub struct Switch {
    /// The signal name that presses this switch.
    pub latch: String,
    /// Whether it is on. A file may set it, and a press flips it.
    pub on: bool,
}


/// **A door that opens and closes with the switch of its name.**
///
/// `link` is a [`Switch`]'s `latch`, so a door is opened by the switch a person presses without the two
/// naming each other's entities: a save can move either one and the link still holds. `open` is the door's
/// state, and a file may set it — a door saved open comes up open.
///
/// **Two doors with one link share it**: two leaves of one doorway answer the same latch, which is why the
/// link is a name and not an entity reference.
///
/// ```yaml
/// gate: { Door: { link: north, open: false } }
/// ```
#[derive(Component, Reflect, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
pub struct Door {
    /// The `latch` of the [`Switch`] this door follows.
    pub link: String,
    /// Whether it is open.
    pub open: bool,
}

/// **Turn every switch a signal names, then set every door to its switch's state.**
///
/// One pass, in this order, so a door set beside the switch it follows opens in the frame that switch is
/// pressed rather than a frame later. A door whose `link` names no switch is closed: a door linked to
/// nothing is not open, and leaving it as it was would make a broken link look like a working one.
pub(crate) fn switched(
    mut signals: MessageReader<super::Signal>,
    mut switches: Query<&mut Switch>,
    mut doors: Query<&mut Door>,
) {
    let pressed: Vec<String> = signals.read().map(|signal| signal.name.clone()).collect();

    for mut switch in &mut switches {
        // **A switch with no name answers nothing**, so a file that forgot `latch` cannot be pressed by
        // every signal ever written.
        if switch.latch.is_empty() || !pressed.contains(&switch.latch) {
            continue;
        }
        switch.on = !switch.on;
    }

    // **The switches are read after the presses have landed**, which is why this is a second loop rather
    // than a lookup inside one: a door must follow this frame's state, not last frame's.
    let latches: Vec<(String, bool)> = switches.iter().map(|switch| (switch.latch.clone(), switch.on)).collect();
    for mut door in &mut doors {
        let open = latches.iter().find(|(latch, _)| *latch == door.link).is_some_and(|(_, on)| *on);
        if door.open != open {
            door.open = open;
        }
    }
}
