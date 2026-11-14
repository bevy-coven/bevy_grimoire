//! **The keymap screen: every command with its shortcut, and a rebinding that refuses a clash** (UI-72, #250).
//!
//! Opened from the settings screen's *Keymap* button (and by [`OpenKeymap`]). One row per command of
//! [`crate::commands::all`]: its title and a `bevy_gooey` `binding` row that takes the next chord the
//! person presses. A search narrows the rows by title, id or chord.
//!
//! # A rebinding that clashes saves nothing
//!
//! A chord is **taken** when another command already answers to it where both can fire: the same context,
//! or either one live everywhere ([`overlaps`]). Binding a second command to `Ctrl+S` while Save has it
//! shows *"Ctrl+S is already Save"* on the screen, the row goes back to the chord it had, and **no file is
//! written**. The way through is the one the message names: rebind the other command first, or pick
//! another chord. Binding a command to the chord it already has, or to its default again, takes the key
//! out of the file.
//!
//! # Stored as `keymap`, per command
//!
//! A rebinding is one entry of the `keymap` table in the file being edited (`settings.yaml` or `editor.yaml`,
//! chosen as on the settings screen), keyed by the command's stable id (`file.save`), so it survives a
//! title being reworded. Per command the project's chord wins over the user's, which wins over the default.
//!
//! **The editor's key dispatcher follows the table** (UI-72b, #265): `keys::Live` carries the rebound
//! chords, so a rebound command answers to its new chord at once and no longer to the old one.

use std::collections::BTreeMap;

use bevy::ecs::relationship::RelatedSpawner;
use bevy::ecs::spawn::{SpawnIter, SpawnWith};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_gooey::{
    Activate, Binding, BindingValue, Cancel, Shortcut, TextHint, ValueChange, binding, button, dialog, heading, label, list, menu, observe,
    text_input,
};

use crate::commands::{self, Command};
use crate::keys::Context;
use crate::settings_screen::{Files, Layer, Scope, scopes};

/// How many button-heights of commands the list shows before it scrolls.
const LIST_ROWS: f32 = 9.5;
/// How often the open screen looks at the two files, in seconds.
const LOOK_EVERY: f32 = 0.25;

/// Ask for the keymap screen.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct OpenKeymap;

/// The open screen's choices.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct KeymapDialog {
    pub scope: Scope,
    pub query: String,
    /// A rebinding the screen refused, said at the top until the next one works.
    pub problem: Option<String>,
    /// Writes and refusals so far, so the rows are built again at once (a refused row goes back to its chord).
    pub touched: u32,
}

#[derive(Component, Default)]
struct Built(Option<Snapshot>);

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    dialog: KeymapDialog,
    user: Layer,
    project: Layer,
}

/// The column the rows are built into.
#[derive(Component)]
struct Rows;

#[derive(Component)]
struct ProblemLine;

/// **Whether two commands can both fire on one chord**: they live in the same context, or either one is
/// live everywhere. (`Typing` is a suppressor, not a place a command lives.)
pub fn overlaps(a: Context, b: Context) -> bool {
    a == b || a == Context::Global || b == Context::Global
}

/// Every command with the chord it answers to now: its default, or the rebinding in `keymap`.
pub fn effective(keymap: &BTreeMap<String, Shortcut>) -> Vec<(Command, Shortcut)> {
    commands::all().into_iter().map(|c| (c, keymap.get(c.id).copied().unwrap_or(c.shortcut))).collect()
}

/// One command, or one of the editor's modal chords, that answers to a chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taker {
    pub id: &'static str,
    pub title: &'static str,
}

/// **The terminal's own chords** (#426): `terminal.collapse` (Cmd/Ctrl+J) and `terminal.maximise` (Shift+Cmd/Ctrl+J). They are read by the
/// pane before the shell hears anything, so they are not rows of the key census; they are listed under the commands, fixed, and a command
/// cannot be rebound onto either.
pub fn terminal_chords() -> Vec<(Taker, Shortcut)> {
    vec![
        (Taker { id: "terminal.collapse", title: "Collapse or raise the terminal" }, crate::terminal_pane::COLLAPSE_CHORD),
        (Taker { id: "terminal.maximise", title: "Maximise the terminal" }, crate::terminal_pane::maximise_chord()),
    ]
}

/// **The chords the editor answers to outside the command registry**: the settings screen's and the
/// command palette's (`command_palette.rs`), which are modals over every tab and so clash with a command
/// anywhere.
fn reserved() -> Vec<(Taker, Shortcut)> {
    let mut out = vec![(Taker { id: "settings.open", title: "Open the settings" }, Shortcut::primary(KeyCode::Comma))];
    out.extend(crate::command_palette::chords().into_iter().map(|(source, chord)| {
        let taker = match source {
            crate::goto::Source::Everything => Taker { id: "goto.open", title: "Go to anything" },
            crate::goto::Source::Commands => Taker { id: "palette.open", title: "Open the command palette" },
        };
        (taker, chord)
    }));
    out.extend(terminal_chords());
    out
}

/// **What already answers to `chord` where `id` would fire**, other than `id` itself.
pub fn taken_by(keymap: &BTreeMap<String, Shortcut>, id: &str, chord: Shortcut) -> Vec<Taker> {
    let Some(me) = commands::by_id(id) else { return Vec::new() };
    let mut taken: Vec<Taker> = effective(keymap)
        .into_iter()
        .filter(|(c, now)| c.id != id && *now == chord && overlaps(c.context, me.context))
        .map(|(c, _)| Taker { id: c.id, title: c.title })
        .collect();
    taken.extend(reserved().into_iter().filter(|(_, c)| *c == chord).map(|(t, _)| t));
    taken.dedup();
    taken
}

/// The rebindings of both files as the editor lays them: the user's, then the project's over them.
fn resolved(user: &Layer, project: &Layer) -> BTreeMap<String, Shortcut> {
    let mut out = BTreeMap::new();
    for layer in [user, project] {
        if let Layer::Read(l) = layer {
            out.extend(l.keymap.iter().map(|(k, v)| (k.clone(), *v)));
        }
    }
    out
}

fn own_keymap(layer: &Layer) -> BTreeMap<String, Shortcut> {
    match layer {
        Layer::Read(l) => l.keymap.clone(),
        _ => BTreeMap::new(),
    }
}

/// **Rebinds `id` to `chord` in the file at `path`**, or takes the rebinding out with `None`, so the command
/// answers to what the layer below gives it: `below`, the chord a lower layer rebinds it to, else its
/// default. A chord another command has refuses and writes nothing, and so does a Reset that would hand a
/// command back a chord something else has taken since.
pub fn rebind(
    path: &std::path::Path,
    id: &str,
    chord: Option<Shortcut>,
    others: &BTreeMap<String, Shortcut>,
    below: Option<Shortcut>,
) -> Result<(), String> {
    let Some(command) = commands::by_id(id) else { return Err(format!("`{id}` is not a command")) };
    let fallback = below.unwrap_or(command.shortcut);
    let target = chord.unwrap_or(fallback);
    let clash = taken_by(others, id, target);
    if !clash.is_empty() {
        let who: Vec<String> = clash.iter().map(|c| format!("{} ({})", c.title, c.id)).collect();
        return Err(format!(
            "{} is already {}. Nothing was saved: rebind {} first, or choose another chord for {}.",
            target.label(),
            who.join(" and "),
            if who.len() == 1 { "it" } else { "them" },
            command.title
        ));
    }
    crate::setting_catalogue::edit_layer(path, |layer| {
        // The chord the layer below already gives is no rebinding at all.
        if target == fallback {
            layer.keymap.remove(id);
        } else {
            layer.keymap.insert(id.to_owned(), target);
        }
        Ok(())
    })
}

#[derive(SystemParam)]
struct Writer<'w, 's> {
    dialog: Query<'w, 's, &'static mut KeymapDialog>,
    files: Files<'w>,
}

impl Writer<'_, '_> {
    fn set(&mut self, id: &'static str, chord: Option<Shortcut>) {
        let Ok(mut dialog) = self.dialog.single_mut() else { return };
        let (user, project) = (Layer::of(self.files.user().as_ref()), Layer::of(self.files.project().as_ref()));
        // What every other command answers to: both layers, with the one being edited minus this command.
        let mut others = resolved(&user, &project);
        others.remove(id);
        // What the layer below the one being edited gives this command: the user's rebinding, for the project's file.
        let below = match dialog.scope {
            Scope::User => None,
            Scope::Project => own_keymap(&user).get(id).copied(),
        };
        let outcome = match self.files.path(dialog.scope) {
            Some(path) => rebind(&path, id, chord, &others, below),
            None => Err("there is no file to write this to".to_owned()),
        };
        dialog.problem = outcome.err();
        dialog.touched += 1;
    }
}

pub struct KeymapScreenPlugin;

impl Plugin for KeymapScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenKeymap>().add_systems(Update, (open, refresh).chain());
    }
}

fn open(mut asks: MessageReader<OpenKeymap>, existing: Query<(), With<KeymapDialog>>, files: Files, mut commands: Commands) {
    if asks.read().count() == 0 || !existing.is_empty() {
        return;
    }
    let available = scopes(&files);
    let state = KeymapDialog { scope: available.first().copied().unwrap_or_default(), ..default() };
    commands
        .spawn((dialog(), Name::new("keymap"), state, Built::default()))
        .observe(|cancel: On<Cancel>, mut keyboard: ResMut<ButtonInput<KeyCode>>, mut commands: Commands| {
            // **The `Esc` that closes this is this dialog's alone** (#286): taken, so the editor's
            // own `Esc` (back out, ask to leave) does not also read it in the same frame.
            crate::confirm::take(&mut keyboard, KeyCode::Escape);
            commands.entity(cancel.entity).insert(bevy_gooey::Leave);
        })
        .with_children(|dialog| {
            dialog
                .spawn(bevy_gooey::sized_panel(bevy_gooey::CardWidth::Text(crate::launcher::DIALOG_CARD_TEXT_SIZES)))
                .with_children(|card| {
                    card.spawn(heading("Keymap"));
                    card.spawn(label(
                        "Press a row, then the chord. A clash is refused and nothing is saved. The rebound chord works at once.",
                    ));
                    if available.len() > 1 {
                        card.spawn((
                            bevy_gooey::choice("Edit", ["Your keymap", "This project's keymap"], 0),
                            observe(|c: On<ValueChange<usize>>, mut dialogs: Query<&mut KeymapDialog>| {
                                if let Ok(mut d) = dialogs.single_mut() {
                                    d.scope = if c.value == 0 { Scope::User } else { Scope::Project };
                                    d.problem = None;
                                }
                            }),
                        ));
                    }
                    card.spawn((
                        text_input("Search commands", ""),
                        TextHint("a command or a chord".into()),
                        observe(|c: On<ValueChange<String>>, mut dialogs: Query<&mut KeymapDialog>| {
                            if let Ok(mut d) = dialogs.single_mut()
                                && d.query != c.value
                            {
                                d.query = c.value.clone();
                            }
                        }),
                    ));
                    card.spawn((label(""), ProblemLine));
                    card.spawn((bevy_gooey::column(0.0), Rows));
                    card.spawn(menu()).with_children(|actions| {
                        actions.spawn((
                            button("Close"),
                            observe(|_: On<Activate>, dialogs: Query<Entity, With<KeymapDialog>>, mut commands: Commands| {
                                for dialog in &dialogs {
                                    commands.entity(dialog).insert(bevy_gooey::Leave);
                                }
                            }),
                        ));
                    });
                });
        });
}

fn refresh(
    time: Res<Time>,
    files: Files,
    mut dialogs: Query<(Ref<KeymapDialog>, &mut Built)>,
    rows: Query<Entity, With<Rows>>,
    mut problem_line: Query<&mut Text, With<ProblemLine>>,
    mut since: Local<f32>,
    mut commands: Commands,
) {
    let Ok((dialog, mut built)) = dialogs.single_mut() else { return };
    *since += time.delta_secs();
    if *since < LOOK_EVERY && !dialog.is_changed() && built.0.is_some() {
        return;
    }
    *since = 0.0;
    let snapshot = Snapshot { dialog: (*dialog).clone(), user: Layer::of(files.user().as_ref()), project: Layer::of(files.project().as_ref()) };
    if built.0.as_ref() == Some(&snapshot) {
        return;
    }
    let edited = match dialog.scope {
        Scope::User => &snapshot.user,
        Scope::Project => &snapshot.project,
    };
    let said = dialog
        .problem
        .clone()
        .or_else(|| match edited {
            Layer::Unreadable(why) => Some(format!("{why}; nothing is written until it reads")),
            Layer::Unavailable => Some("there is no file to edit here: no project is open".to_owned()),
            _ => None,
        })
        .unwrap_or_default();
    for mut text in &mut problem_line {
        if text.0 != said {
            text.0 = said.clone();
        }
    }

    let now = resolved(&snapshot.user, &snapshot.project);
    let own = own_keymap(edited);
    let query = dialog.query.to_lowercase();
    let found: Vec<(Command, Shortcut)> = effective(&now)
        .into_iter()
        .filter(|(c, chord)| {
            let hay = format!("{} {} {}", c.title, c.id, chord.label()).to_lowercase();
            query.split_whitespace().all(|w| hay.contains(w))
        })
        .collect();
    let Ok(column) = rows.single() else { return };
    commands.entity(column).despawn_children();
    if found.is_empty() {
        commands.entity(column).with_children(|c| {
            c.spawn(label(format!("No command matches \"{}\".", dialog.query.trim())));
        });
    } else {
        let below = |c: &Command| match dialog.scope {
            Scope::User => c.shortcut,
            Scope::Project => own_keymap(&snapshot.user).get(c.id).copied().unwrap_or(c.shortcut),
        };
        let items: Vec<_> = found.into_iter().map(|(command, chord)| command_row(command, chord, own.contains_key(command.id), below(&command))).collect();
        commands.entity(column).with_children(|c| {
            c.spawn(list(LIST_ROWS, Children::spawn(SpawnIter(items.into_iter()))));
        });
    }
    // **The terminal's own chords are listed under the commands** (#426), fixed, and found by the same words.
    let fixed: Vec<String> = terminal_chords()
        .into_iter()
        .map(|(taker, chord)| format!("{} ({}): {}, fixed", taker.title, taker.id, chord.label()))
        .filter(|words| query.split_whitespace().all(|w| words.to_lowercase().contains(w)))
        .collect();
    commands.entity(column).with_children(|c| {
        for words in fixed {
            c.spawn(label(words));
        }
    });
    built.0 = Some(snapshot);
}

/// One command: the rebinding row, and while the file being edited rebinds it, its default and a Reset.
fn command_row(command: Command, chord: Shortcut, modified: bool, default_chord: Shortcut) -> impl Bundle {
    let id = command.id;
    (
        bevy_gooey::column(0.0),
        Children::spawn(SpawnWith(move |row: &mut RelatedSpawner<ChildOf>| {
            row.spawn((
                binding(command.title, Binding::Chord(chord)),
                observe(move |c: On<ValueChange<Binding>>, mut w: Writer| {
                    if let Binding::Chord(chosen) = c.value {
                        w.set(id, Some(chosen));
                    }
                }),
            ));
            if modified {
                row.spawn(bevy_gooey::row_with(crate::chrome::GAP_GROUP_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }))
                    .with_children(|line| {
                        line.spawn(label(format!("Modified; without it the chord is {}", default_chord.label())));
                        line.spawn((button(format!("Reset {}", command.title)), observe(move |_: On<Activate>, mut w: Writer| w.set(id, None))));
                    });
            }
        })),
    )
}

/// Marker so a test can read the chord a row shows: the row's own state.
pub fn shown(world: &World, row: Entity) -> Option<Shortcut> {
    match world.get::<BindingValue>(row)?.0 {
        Binding::Chord(c) => Some(c),
        _ => None,
    }
}
