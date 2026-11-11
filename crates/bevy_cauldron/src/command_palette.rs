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
use crate::goto::{Finder, Source};
use crate::keys::{Context, Live};

/// The chords that open (and, while it is open, close) the palette.
///
/// **`Cmd`/`Ctrl`+`P` is the third, and it opens the finder over everything the project names**
/// (NV-05, #382): `goto.rs` is the same widget with a second source, and the `>` in front of a query
/// swaps between them. One chord for "go somewhere", one for "run something", which is what
/// `Cmd`+`P` means in every editor a person already uses.
pub fn chords() -> [Shortcut; 3] {
    [
        Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyP),
        Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyP),
        Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyK),
    ]
}

/// One command as a palette row: `Category: Title`, its chord, and the reason it cannot run now.
pub fn row(command: &Command, live: Live) -> PaletteCommand {
    let row = PaletteCommand::new(command.id, command.title)
        .category(capitalised(command.category()))
        .shortcut(command.shortcut_in(live));
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
    confirm: Res<crate::confirm::Confirm>,
    open: Query<(Entity, Option<&bevy_gooey::Leaving>), With<CommandPalette>>,
    mut commands_list: ResMut<CommandList>,
    mut finder: ResMut<Finder>,
    index: Res<crate::goto::GotoIndex>,
    history: Res<bevy_gooey::PaletteHistory>,
    mut commands: Commands,
) {
    let pressed = chords().iter().position(|chord| chord.just_pressed(&keys));
    let Some(which) = pressed else { return };
    // **Which source the chord asks for**: `Cmd`+`P` is everything, the other two are the command
    // palette (NV-05, #382). One widget, one spawn, two lists.
    let ask = if which == 1 { Source::Everything } else { Source::Commands };
    if let Some((palette, leaving)) = open.iter().next() {
        // It leaves on gooey's presence (a fade, then despawned), not in one frame; asked again while it
        // is leaving, it comes back from where it is drawn.
        match leaving {
            Some(leaving) if !leaving.returning() => commands.entity(palette).insert(bevy_gooey::Stay),
            _ => commands.entity(palette).insert(bevy_gooey::Leave),
        };
        return;
    }
    // Not over a text field, a filter box or the journal (each takes the keys on its own terms), and
    // not over a question on screen: `confirm` reads raw Y, N and Esc, so the first `y` typed into
    // the palette's field would answer it.
    if live.0 == Context::Typing || live.2 != crate::keys::Holder::Tab || confirm.is_open() {
        return;
    }
    commands_list.0 = match ask {
        Source::Commands => list(*live),
        Source::Everything => {
            // **The index is rebuilt by its own system**, on `Update`; this frame's list is the one
            // the person sees first, so it is read from whatever the index holds now and the source
            // swap takes over on the next change of the query.
            crate::goto::rows(&index, &history.0)
        }
    };
    finder.opened(ask);
    commands.spawn(command_palette());
}

/// **A chosen command runs through its chord**, the way an agent's `run_command` does. The palette is
/// leaving (it has given the keys back, see `editor::palette_is_open`) and focus is back, so the dispatcher sees the tab's own context when the chord lands.
fn run(
    chosen: On<RunCommand>,
    mut chords: ResMut<Chords>,
    live: Res<Live>,
    state: Option<ResMut<crate::editor::EditorState>>,
) {
    // **A goto row is `goto.rs`'s choice, not a command** (NV-05, #382): its observer opens what the
    // row names. Both observers are registered, and each ignores the other's ids.
    let Some(command) = commands::by_id(&chosen.id) else {
        return;
    };
    chords.queue(&command, *live);
    if let Some(mut state) = state {
        // "Asked for", not "ran": the chord goes down a frame or two from now and the dispatcher
        // judges it then.
        state.status.note(format!("asked for {} from the palette", command.title));
    }
}

/// **The finder is open only while its palette is.** The palette leaves over a fade
/// (`bevy_gooey::Leaving`) and is despawned some frames later, so the finder is told on the frame it is
/// gone rather than on the frame Enter was pressed — a source swap running into a departing palette
/// would rewrite a list nobody can see, and the next `Cmd`+`P` would open on the wrong one.
fn note_the_finder_closing(
    open: Query<(), With<CommandPalette>>,
    mut finder: ResMut<crate::goto::Finder>,
) {
    if finder.is_open() && open.is_empty() {
        finder.closed();
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
            (toggle, note_the_finder_closing)
                .chain()
                .run_if(in_state(crate::screen::Screen::Editor)),
        )
        .add_observer(run);
    }
}
