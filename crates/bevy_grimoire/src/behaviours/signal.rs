//! **`Signal`: the one message every named thing in a room listens for.**

use bevy::prelude::*;

/// **A named signal**: what a button press, a volume crossing or a countdown says, so a room's parts can
/// name each other by one vocabulary rather than by wiring entities together.
///
/// A [`Switch`](super::Switch)'s `latch` and a [`StateMachine`](super::StateMachine)'s `on` are both signal
/// names, so a game's input writes one message and every part that names it answers.
///
/// **Written by a game, never by a file**: a signal is something that happens, and a save that could write
/// one would fire it on every reload.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct Signal {
    /// The name; a switch whose `latch` is this answers it.
    pub name: String,
}
