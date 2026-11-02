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
//! A rebinding is one entry of the `keymap` table in the file being edited (`settings.ron` or `editor.ron`,
//! chosen as on the settings screen), keyed by the command's stable id (`file.save`), so it survives a
//! title being reworded. Per command the project's chord wins over the user's, which wins over the default.
//!
//! **The editor's key dispatcher does not read the table yet.** The keys still do what `keys::BINDINGS`
//! says; the screen says so under its heading, so nobody rebinds a key and waits for it. Making the
//! dispatcher follow `EditorSettings::keymap` is the next step of this issue.

use std::collections::BTreeMap;

use bevy::ecs::relationship::RelatedSpawner;
use bevy::ecs::spawn::{SpawnIter, SpawnWith};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_gooey::{
    Activate, Binding, BindingValue, Cancel, Shortcut, TextHint, ValueChange, binding, button, dialog, heading, label, list, menu, observe,
    panel, text_input,
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

/// **The commands that already answer to `chord` where `id` would fire**, other than `id` itself.
pub fn taken_by(keymap: &BTreeMap<String, Shortcut>, id: &str, chord: Shortcut) -> Vec<Command> {
    let Some(me) = commands::by_id(id) else { return Vec::new() };
    effective(keymap).into_iter().filter(|(c, now)| c.id != id && *now == chord && overlaps(c.context, me.context)).map(|(c, _)| c).collect()
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

/// **Rebinds `id` to `chord` in the file at `path`**, or takes the rebinding out with `None`; a chord another
/// command has refuses and writes nothing.
pub fn rebind(path: &std::path::Path, id: &str, chord: Option<Shortcut>, others: &BTreeMap<String, Shortcut>) -> Result<(), String> {
    let Some(command) = commands::by_id(id) else { return Err(format!("`{id}` is not a command")) };
    if let Some(chord) = chord {
        let clash = taken_by(others, id, chord);
        if !clash.is_empty() {
            let who: Vec<String> = clash.iter().map(|c| format!("{} ({})", c.title, c.id)).collect();
            return Err(format!(
                "{} is already {}. Nothing was saved: rebind {} first, or choose another chord for {}.",
                chord.label(),
                who.join(" and "),
                if who.len() == 1 { "it" } else { "them" },
                command.title
            ));
        }
    }
    crate::setting_catalogue::edit_layer(path, |layer| {
        match chord {
            // Back on its default is no rebinding at all.
            Some(chord) if chord != command.shortcut => {
                layer.keymap.insert(id.to_owned(), chord);
            }
            _ => {
                layer.keymap.remove(id);
            }
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
        let outcome = match self.files.path(dialog.scope) {
            Some(path) => rebind(&path, id, chord, &others),
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
        .observe(|cancel: On<Cancel>, mut commands: Commands| {
            commands.entity(cancel.entity).despawn();
        })
        .with_children(|dialog| {
            dialog
                .spawn(panel())
                .insert(Node { flex_direction: FlexDirection::Column, width: Val::Px(crate::launcher::CARD_WIDTH), max_width: Val::Percent(100.0), ..default() })
                .with_children(|card| {
                    card.spawn(heading("Keymap"));
                    card.spawn(label(
                        "Press a row, then the chord. A clash is refused and nothing is saved. The editor's keys follow the table built in until its key dispatcher reads this one.",
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
                    card.spawn((Node { flex_direction: FlexDirection::Column, ..default() }, Rows));
                    card.spawn(menu()).with_children(|actions| {
                        actions.spawn((
                            button("Close"),
                            observe(|_: On<Activate>, dialogs: Query<Entity, With<KeymapDialog>>, mut commands: Commands| {
                                for dialog in &dialogs {
                                    commands.entity(dialog).despawn();
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
        let items: Vec<_> = found.into_iter().map(|(command, chord)| command_row(command, chord, own.contains_key(command.id))).collect();
        commands.entity(column).with_children(|c| {
            c.spawn(list(LIST_ROWS, Children::spawn(SpawnIter(items.into_iter()))));
        });
    }
    built.0 = Some(snapshot);
}

/// One command: the rebinding row, and while the file being edited rebinds it, its default and a Reset.
fn command_row(command: Command, chord: Shortcut, modified: bool) -> impl Bundle {
    let id = command.id;
    (
        Node { flex_direction: FlexDirection::Column, ..default() },
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
                row.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(crate::chrome::GAP_GROUP), ..default() })
                    .with_children(|line| {
                        line.spawn(label(format!("Modified; the default is {}", command.shortcut.label())));
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
