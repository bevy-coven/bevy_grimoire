//! **The command palette: every command the editor has, found by a few letters** (UI-32, issue #226).
//!
//! Ctrl+Shift+P (Cmd+Shift+P on a mac) and Ctrl+K (Cmd+K) open `bevy_gooey`'s `command_palette`, one
//! modal field over a searchable list of [`crate::commands::all`], each row with its chord and, when it
//! cannot run, the registry's own reason ([`crate::commands::Command::enabled`]). Typing narrows the
//! list, recents come first, Enter or a click runs the lit row, Escape leaves.
//!
//! # Running is pressing the chord
//!
//! A chosen command runs the way `grimoire/run_command` runs one ([`crate::command_run`]): its chord is
//! queued and the editor's one key dispatcher answers. So "Undo" in the palette is Cmd+Z with the same
//! receipt, the same undo stack and the same refusals, and the palette is a third front end onto a verb
//! rather than a second implementation of it.
//!
//! # The list is judged when the palette opens, not while it is open
//!
//! While the palette's field has focus the editor's [`Live`] context is `Typing` and every binding is
//! refused, so a list refreshed under it would grey every row with "A text field is taking the keys".
//! The list is written once, in the frame the palette opens, from the context the author was in.
//!
//! # Two chords, outside the key census
//!
//! The two chords are not in [`crate::keys::BINDINGS`]: the census is the table of what a *tab* does,
//! the palette is a modal over every tab (as `confirm.rs` is), and both chords carry the primary
//! modifier, which no census row without `needs_mod` can answer to.

use bevy::prelude::*;
use bevy_gooey::{
    CommandList, CommandPalette, Modifiers, PaletteCommand, RunCommand, Shortcut, command_palette,
};

use crate::command_run::{Chords, CommandRunPlugin};
use crate::commands::{self, Command};
use crate::keys::{Context, Live};

/// The chords that open (and, while it is open, close) the palette.
pub fn chords() -> [Shortcut; 2] {
    [
        Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyP),
        Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyK),
    ]
}

/// One command as a palette row: `Category: Title`, its chord, and the reason it cannot run now.
pub fn row(command: &Command, live: Live) -> PaletteCommand {
    let row = PaletteCommand::new(command.id, command.title)
        .category(capitalised(command.category()))
        .shortcut(command.shortcut);
    match command.enabled(live) {
        Ok(()) => row,
        Err(why) => row.disabled(why),
    }
}

/// Every command as the palette lists it, judged by the rules the key dispatcher uses in `live`.
pub fn list(live: Live) -> Vec<PaletteCommand> {
    commands::all().iter().map(|c| row(c, live)).collect()
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// **Open the palette, or close it when it is open.** Not while a text field has the keys: Ctrl+K in a
/// field is the field's.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    live: Res<Live>,
    open: Query<Entity, With<CommandPalette>>,
    mut commands_list: ResMut<CommandList>,
    mut commands: Commands,
) {
    if !chords().iter().any(|chord| chord.just_pressed(&keys)) {
        return;
    }
    if let Some(palette) = open.iter().next() {
        commands.entity(palette).despawn();
        return;
    }
    if live.0 == Context::Typing {
        return;
    }
    commands_list.0 = list(*live);
    commands.spawn(command_palette());
}

/// **A chosen command runs through its chord**, the way an agent's `run_command` does. The palette is
/// already gone and focus is back, so the dispatcher sees the tab's own context when the chord lands.
fn run(
    chosen: On<RunCommand>,
    mut chords: ResMut<Chords>,
    state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(command) = commands::by_id(&chosen.id) else {
        return;
    };
    chords.queue(&command);
    if let Some(mut state) = state {
        state.status.note(format!("ran {} from the palette", command.title));
    }
}

/// The palette: its chords, its list and its dispatch.
pub struct CommandPalettePlugin;

impl Plugin for CommandPalettePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CommandRunPlugin>() {
            app.add_plugins(CommandRunPlugin);
        }
        app.add_systems(
            Update,
            toggle.run_if(in_state(crate::screen::Screen::Editor)),
        )
        .add_observer(run);
    }
}
