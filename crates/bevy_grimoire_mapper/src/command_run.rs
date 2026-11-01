//! **Run a registered command by id, through the key that runs it** (UI-90, issue #256).
//!
//! `grimoire/run_command { id }` is the agent's door onto [`crate::commands`]. The id names an
//! [`crate::keys::Action`]; this module presses that action's chord, one frame, as the keyboard
//! would, and lets the dispatcher that every key goes through answer. Nothing here knows what
//! `edit.undo` does, so there is no second dispatcher to drift from the first: a command that the
//! key would refuse in this stance is refused here with the registry's own reason
//! ([`crate::commands::Command::enabled`]), and one that fires fires through the same system, with the
//! same side effects (the undo stack, the receipt, the status line), as the key.
//!
//! **Why a key and not a function call.** The verbs live in systems that take a dozen resources each
//! (`editor.rs`), because they are written against a frame's input. Rebuilding that context in a
//! second place is how two front ends come to disagree; pressing the chord reuses the one place it is
//! built. The cost is two frames: the chord goes down in one frame's `PreUpdate` and up in the next, so
//! the answer says `ran` when the press was queued, and a caller reads `grimoire/state` for the result.
//!
//! **A chord is held one frame, then let go.** The modifier and the key go down together and come up
//! together, so the key census's `needs_mod` and `needs_shift` see exactly the keys a person holds.

use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use crate::commands::Command;
use crate::keys::{self, Binding};

/// The chords waiting to go down, and the keys that are down for this frame's input.
#[derive(Resource, Default, Debug)]
pub struct Chords {
    /// Chords asked for and not yet pressed, oldest first. One goes down per two frames.
    waiting: Vec<Vec<KeyCode>>,
    /// The keys pressed last frame, released at the start of this one.
    down: Vec<KeyCode>,
}

impl Chords {
    /// Ask for a command's chord to be pressed.
    pub fn queue(&mut self, command: &Command) {
        self.waiting.push(keys_of(keys::binding(command.action)));
    }

    /// How many chords have not gone down yet.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }
}

/// **The keys a binding asks of the keyboard**, modifiers first so the key lands with them held.
/// A row that does not ask for Shift is pressed without it: `Cmd+Z` undoes, `Shift+Cmd+Z` redoes.
fn keys_of(binding: &Binding) -> Vec<KeyCode> {
    let mut chord = Vec::with_capacity(3);
    if binding.needs_mod {
        chord.push(keys::MOD_KEYS[0]);
    }
    if binding.needs_shift == Some(true) {
        chord.push(KeyCode::ShiftLeft);
    }
    chord.push(binding.key);
    chord
}

fn message(key: KeyCode, state: ButtonState) -> KeyboardInput {
    KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

/// **Let go of last frame's chord, then press the next waiting one.** Runs before the keyboard system
/// reads its messages, so a press written here is `just_pressed` in this frame's `Update`, and the
/// release written next frame is read before that frame's `Update`.
///
/// A chord is never pressed in the frame the last one came up: the key census must see the released
/// state once, or two chords on one key would read as one held press.
pub fn press_waiting(mut chords: ResMut<Chords>, mut keyboard: MessageWriter<KeyboardInput>) {
    if !chords.down.is_empty() {
        for key in std::mem::take(&mut chords.down).into_iter().rev() {
            keyboard.write(message(key, ButtonState::Released));
        }
        return;
    }
    if chords.waiting.is_empty() {
        return;
    }
    let chord = chords.waiting.remove(0);
    for key in &chord {
        keyboard.write(message(*key, ButtonState::Pressed));
    }
    chords.down = chord;
}

/// The plugin: the queue and the system that presses it.
pub struct CommandRunPlugin;

impl Plugin for CommandRunPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chords>()
            .add_systems(PreUpdate, press_waiting.before(InputSystems));
    }
}
