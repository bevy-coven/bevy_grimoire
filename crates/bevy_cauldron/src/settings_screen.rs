//! **The settings screen: every setting, searchable, with its default and a Reset** (UI-71, #249).
//!
//! Cmd/Ctrl+, opens it over whatever is on screen (the launcher, the loader, a door) and the launcher's
//! Settings button opens it too; Escape or Close takes it down. It is a modal like the command palette
//! (`command_palette.rs`), so its chord is outside the key census for the same reason: it is not a thing
//! a tab does.
//!
//! # What it shows
//!
//! - **A search field and the categories as tabs** ([`setting_catalogue::CATEGORIES`], with *All* first).
//!   A query is the words a setting is found by (its label, key, category and description); the tab the
//!   person is on keeps only its own category's matches, so *All* is where a query looks everywhere.
//! - **One row per setting**: its control (a switch, a number, a choice), what it does, its default, a
//!   *Modified* marker while the file being edited sets it, and a *Reset* that takes the key out of that
//!   file, so the layer below decides it again.
//! - **A User and Project switch** when both can be edited: the layer a change is written to. A row's
//!   control shows the value the layer being edited gives, or the one it inherits when it sets none.
//!
//! # Settings are files, and the screen only writes them
//!
//! A change is one key written into `settings.yaml` or `editor.yaml` ([`setting_catalogue::write_key`],
//! atomic, and a file that does not read is refused and left as it is). The editor's watcher
//! ([`crate::settings::sync_settings`]) reads the file back as it reads an edit made by hand, so the screen
//! has no second path to the running settings. The screen reads the two files itself, a few times a
//! second while it is open, so an edit made elsewhere shows here too.

use std::path::PathBuf;

use bevy::ecs::spawn::{SpawnIter, SpawnWith};
use bevy::ecs::relationship::RelatedSpawner;
use bevy::ecs::system::SystemParam;
use bevy::input_focus::AutoFocus;
use bevy::prelude::*;
use bevy::picking::events::PointerRelease;
use bevy::ui_widgets::{SliderPrecision, SliderRange, SliderStep, SliderValue};
use bevy_gooey::{
    Activate, Cancel, TextHint, ValueChange, button, choice, dialog, heading, label, list, menu, observe, slider, tab_page, tabs, text_input,
    toggle,
};

use crate::project::Project;
use crate::setting_catalogue::{self, CATEGORIES, Kind, SCALE_STEP, Setting, Value};
use crate::settings::{AgentFile, PROJECT_FILE, SettingsLayer, TerminalAgent, UserSettingsFile};

/// How often the open screen looks at the two files, in seconds.
const LOOK_EVERY: f32 = 0.25;

/// How many button-heights of settings the list shows before it scrolls.
const LIST_ROWS: f32 = 9.5;

/// Which file a change is written to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    /// `<config_dir>/Grimoire/settings.yaml`: the person's, for every project.
    #[default]
    User,
    /// `<project>/assets/grimoire/editor.yaml`: this project's, for everyone who opens it.
    Project,
}

/// Ask for the settings screen. Written by the chord and by the launcher's Settings button.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct OpenSettings;

/// The open screen's choices: which file, which tab, what was searched, and the last refusal.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct SettingsDialog {
    pub scope: Scope,
    /// The tab: `0` is *All*, `n` is `CATEGORIES[n - 1]`.
    pub tab: usize,
    pub query: String,
    /// A write the screen could not make, said at the top until the next one works.
    pub problem: Option<String>,
    /// How many writes the screen has made, so the rows are built again at once and not at the next look at the files.
    pub writes: u32,
    /// **The Terminal agent row is asking for a command** (#425): `Custom command` is not a choice until one is set, so the row spawns
    /// a field for it on demand and a final value saves the command and the choice together.
    pub command_editing: bool,
}

/// What the rows were last built from, so they are built again only when it changes.
#[derive(Component, Default)]
struct Built(Option<Snapshot>);

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    dialog: SettingsDialog,
    user: Layer,
    project: Layer,
}

/// One file as the screen read it: absent, a layer, or the reason it does not read.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Layer {
    /// This build cannot name the file (no user settings folder, or no project open).
    Unavailable,
    Missing,
    Read(SettingsLayer),
    Unreadable(String),
}

impl Layer {
    pub(crate) fn of(path: Option<&PathBuf>) -> Layer {
        let Some(path) = path else { return Layer::Unavailable };
        let file = path.to_string_lossy().replace('\\', "/");
        match std::fs::read_to_string(path) {
            Ok(text) => match SettingsLayer::read(&file, &text) {
                Ok(layer) => Layer::Read(layer),
                Err(r) => Layer::Unreadable(format!("{file} does not read ({}): {}", r.field.as_deref().unwrap_or("file"), r.reason)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Layer::Missing,
            Err(e) => Layer::Unreadable(format!("{file}: {e}")),
        }
    }

    /// The value this file sets for `key`.
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match self {
            Layer::Read(layer) => layer.get(key),
            _ => None,
        }
    }

    /// The shell line this file gives `TerminalAgent::Custom`, when it gives one.
    pub(crate) fn command(&self) -> Option<&str> {
        match self {
            Layer::Read(layer) => layer.terminal_agent_command.as_deref(),
            _ => None,
        }
    }

    /// The layer this file read as, when it read.
    pub(crate) fn layer(&self) -> Option<&SettingsLayer> {
        match self {
            Layer::Read(layer) => Some(layer),
            _ => None,
        }
    }
}

/// A slider whose readout is a percentage: `1.25` reads `125%`. gooey draws a percentage only for a range
/// of 0 to 1, and a size runs from 75% to 200%, so the readout is written here, after gooey has written
/// the number ([`say_the_percentage`]).
#[derive(Component)]
struct PercentReadout;

/// How many decimals a dragged slider keeps: hundredths, so the readout never shows a fraction of a percent.
const SLIDER_DECIMALS: i32 = 2;

/// A value to the nearest [`SCALE_STEP`], as a number a file reads back exactly (`1.25`, not
/// `1.2500001`): counted in hundredths, which a step of 5% divides.
fn on_the_step(value: f32) -> f32 {
    let hundredths = (value / SCALE_STEP).round() * (SCALE_STEP * 100.0);
    hundredths.round() / 100.0
}

/// A scale as a percentage: `1.25` is `125%`.
fn percent(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

/// The page of the tabs a [`Rows`] column lists: `0` is *All*.
#[derive(Component)]
struct Rows(usize);

/// The line that says why the screen cannot write, or what is wrong with a file.
#[derive(Component)]
struct ProblemLine;

/// The dialog's two files, as this app names them — and, beside them, **what this machine has** (#341).
#[derive(SystemParam)]
pub(crate) struct Files<'w> {
    pub(crate) user: Option<Res<'w, UserSettingsFile>>,
    pub(crate) project: Option<Res<'w, Project>>,
    /// What the binary asked this machine's login shell at start-up. Absent in a test, and absent is
    /// `Off` — the same answer the terminal's own resolution gives for an app that never asked.
    pub(crate) machine: Option<Res<'w, crate::settings::MachineAgent>>,
}

impl Files<'_> {
    /// **The agent this machine's terminal is using when no file states one** (#341). The row a file
    /// does not set shows this, so the screen and the pane cannot say two things about one setting.
    pub(crate) fn machine_agent(&self) -> TerminalAgent {
        crate::settings::MachineAgent::resolved(self.machine.as_deref())
    }

    pub(crate) fn user(&self) -> Option<PathBuf> {
        self.user.as_ref().map(|f| f.0.clone())
    }

    pub(crate) fn project(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.project_dir.join(PROJECT_FILE))
    }

    pub(crate) fn path(&self, scope: Scope) -> Option<PathBuf> {
        match scope {
            Scope::User => self.user(),
            Scope::Project => self.project(),
        }
    }
}

/// **The one way a control writes**: the key into the file of the scope being edited.
#[derive(SystemParam)]
struct Writer<'w, 's> {
    dialog: Query<'w, 's, &'static mut SettingsDialog>,
    files: Files<'w>,
}

impl Writer<'_, '_> {
    /// Sets `key` in the edited file, or takes it out with `None`; a refusal is kept for the screen to say.
    fn set(&mut self, key: &'static str, value: Option<Value>) {
        let Ok(mut dialog) = self.dialog.single_mut() else { return };
        let outcome = match self.files.path(dialog.scope) {
            Some(path) => setting_catalogue::write_key(&path, key, value),
            None => Err("there is no file to write this to".to_owned()),
        };
        dialog.problem = outcome.err();
        dialog.writes += 1;
    }

    /// **Sets `key` in the file of `scope`**, whichever one the screen is editing (#425): the fix for a file that chose Custom with
    /// no command is written to that file, not to the one on screen.
    fn set_in(&mut self, scope: Scope, key: &'static str, value: Option<Value>) {
        let Ok(mut dialog) = self.dialog.single_mut() else { return };
        let outcome = match self.files.path(scope) {
            Some(path) => setting_catalogue::write_key(&path, key, value),
            None => Err("there is no file to write this to".to_owned()),
        };
        dialog.problem = outcome.err();
        dialog.writes += 1;
    }

    /// **Opens or closes the row's command field** (#425).
    fn edit_command(&mut self, editing: bool) {
        let Ok(mut dialog) = self.dialog.single_mut() else { return };
        dialog.command_editing = editing;
        dialog.writes += 1;
    }

    /// **A command typed into the Terminal agent row**: saved with `terminal_agent: Custom` in one write, so a file never holds the
    /// choice without it. A blank command saves nothing and closes the field.
    fn finish_command(&mut self, typed: &str) {
        let command = typed.trim().to_owned();
        let Ok(mut dialog) = self.dialog.single_mut() else { return };
        dialog.command_editing = false;
        dialog.writes += 1;
        if command.is_empty() {
            return;
        }
        let outcome = match self.files.path(dialog.scope) {
            Some(path) => setting_catalogue::edit_layer(&path, |layer| {
                layer.terminal_agent_command = Some(command);
                layer.terminal_agent = Some(TerminalAgent::Custom);
                Ok(())
            }),
            None => Err("there is no file to write this to".to_owned()),
        };
        dialog.problem = outcome.err();
    }
}

pub struct SettingsScreenPlugin;

impl Plugin for SettingsScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenSettings>().add_systems(Update, (chord, open, refresh).chain()).add_systems(
            Update,
            // After gooey has written its own readout (the number), so the percentage is the last word.
            say_the_percentage.after(bevy_gooey::GooeySystems::Style),
        );
    }
}

/// Cmd/Ctrl+, : open the screen, or take it down when it is open. Not from a text field, where the
/// comma is a comma.
fn chord(
    keys: Res<ButtonInput<KeyCode>>,
    open_now: Query<Entity, With<SettingsDialog>>,
    editing: Query<(), With<bevy_gooey::TextEditing>>,
    mut asks: MessageWriter<OpenSettings>,
    mut commands: Commands,
) {
    if !bevy_gooey::Shortcut::primary(KeyCode::Comma).just_pressed(&keys) {
        return;
    }
    if let Some(dialog) = open_now.iter().next() {
        commands.entity(dialog).insert(bevy_gooey::Leave);
    } else if editing.is_empty() {
        asks.write(OpenSettings);
    }
}

/// The scopes this app can write: the user's file when it has one, the project's when one is open.
pub(crate) fn scopes(files: &Files) -> Vec<Scope> {
    let mut out = Vec::new();
    if files.user().is_some() {
        out.push(Scope::User);
    }
    if files.project().is_some() {
        out.push(Scope::Project);
    }
    out
}

fn open(mut asks: MessageReader<OpenSettings>, existing: Query<(), With<SettingsDialog>>, files: Files, mut commands: Commands) {
    if asks.read().count() == 0 || !existing.is_empty() {
        return;
    }
    let available = scopes(&files);
    let has_project = files.project().is_some();
    let state = SettingsDialog { scope: available.first().copied().unwrap_or_default(), ..default() };
    let tab_names: Vec<String> = std::iter::once("All".to_owned()).chain(CATEGORIES.iter().map(|c| (*c).to_owned())).collect();
    let pages: Vec<usize> = (0..tab_names.len()).collect();

    commands
        .spawn((dialog(), Name::new("settings"), state, Built::default()))
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
                    card.spawn(heading("Settings"));
                    if available.len() > 1 {
                        card.spawn((
                            choice("Applies to", ["Just me", "This project"], 0),
                            observe(|c: On<ValueChange<usize>>, mut dialogs: Query<&mut SettingsDialog>| {
                                if let Ok(mut d) = dialogs.single_mut() {
                                    d.scope = if c.value == 0 { Scope::User } else { Scope::Project };
                                    d.problem = None;
                                }
                            }),
                        ));
                    }
                    card.spawn((
                        text_input("Search settings", ""),
                        TextHint("a word, like grid".into()),
                        observe(|c: On<ValueChange<String>>, mut dialogs: Query<&mut SettingsDialog>| {
                            if let Ok(mut d) = dialogs.single_mut()
                                && d.query != c.value
                            {
                                d.query = c.value.clone();
                            }
                        }),
                    ));
                    card.spawn((label(""), ProblemLine));
                    card.spawn((
                        tabs(
                            tab_names,
                            0,
                            Children::spawn(SpawnIter(pages.into_iter().map(|page| (tab_page(()), Rows(page))))),
                        ),
                        observe(|c: On<ValueChange<usize>>, mut dialogs: Query<&mut SettingsDialog>| {
                            if let Ok(mut d) = dialogs.single_mut() {
                                d.tab = c.value;
                            }
                        }),
                    ));
                    card.spawn(menu()).with_children(|actions| {
                        if has_project {
                            actions.spawn((
                                button("Project configuration"),
                                observe(
                                    |_: On<Activate>,
                                     dialogs: Query<Entity, With<SettingsDialog>>,
                                     mut asks: MessageWriter<crate::project_config::OpenProjectConfig>,
                                     mut commands: Commands| {
                                        for dialog in &dialogs {
                                            commands.entity(dialog).despawn();
                                        }
                                        asks.write(crate::project_config::OpenProjectConfig);
                                    },
                                ),
                            ));
                        }
                        actions.spawn((
                            button("Keymap"),
                            observe(
                                |_: On<Activate>,
                                 dialogs: Query<Entity, With<SettingsDialog>>,
                                 mut asks: MessageWriter<crate::keymap_screen::OpenKeymap>,
                                 mut commands: Commands| {
                                    for dialog in &dialogs {
                                        commands.entity(dialog).despawn();
                                    }
                                    asks.write(crate::keymap_screen::OpenKeymap);
                                },
                            ),
                        ));
                        actions.spawn((
                            button("Close"),
                            observe(|_: On<Activate>, dialogs: Query<Entity, With<SettingsDialog>>, mut commands: Commands| {
                                for dialog in &dialogs {
                                    commands.entity(dialog).insert(bevy_gooey::Leave);
                                }
                            }),
                        ));
                    });
                });
        });
}

/// Build the rows again when what they are built from changed: a choice, a search, or either file.
fn refresh(
    time: Res<Time>,
    files: Files,
    mut dialogs: Query<(Ref<SettingsDialog>, &mut Built)>,
    pages: Query<(Entity, &Rows)>,
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
    // The tab is not part of what the rows are built from (every tab's page is built), so turning one rebuilds nothing.
    let snapshot = Snapshot { dialog: SettingsDialog { tab: 0, ..(*dialog).clone() }, user: Layer::of(files.user().as_ref()), project: Layer::of(files.project().as_ref()) };
    if built.0.as_ref() == Some(&snapshot) {
        return;
    }

    // What is wrong, said once at the top: a refused write, or the file being edited not reading.
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

    // What each row shows: the value the edited layer sets, else the one it inherits.
    //
    // **The inherited value is this machine's default for the terminal's agent** (#341), because that
    // is the value the terminal is using: a machine with `omp` installed starts with it chosen, and a
    // screen saying `Off` beside a pane offering `Invoke Agent` would be two answers to one question.
    // Every other key's default is the built-in one, which no machine answers.
    let default_of = |key: &str| match key {
        "terminal_agent" => Some(Value::Option(crate::setting_catalogue::agent_index(files.machine_agent()))),
        _ => setting_catalogue::default_value(key),
    };
    // **A file that chose Custom and gave it no command** (#425), named with the scope its fix is written to. Read from the two files
    // as the screen has them, so the row says what is wrong the moment the file is, not only at start-up.
    let agent_problem = crate::settings::custom_agent_without_command(snapshot.user.layer(), snapshot.project.layer()).map(|file| match file {
        AgentFile::User => (Scope::User, "your settings file".to_owned()),
        AgentFile::Project => (Scope::Project, "this project's settings file".to_owned()),
    });
    for (page, Rows(tab)) in &pages {
        commands.entity(page).despawn_children();
        let category = tab.checked_sub(1).and_then(|i| CATEGORIES.get(i)).copied();
        let found = setting_catalogue::search(&dialog.query, category);
        if found.is_empty() {
            commands.entity(page).with_children(|rows| {
                rows.spawn(label(format!("No setting matches \"{}\".", dialog.query.trim())));
            });
            continue;
        }
        let rows: Vec<_> = found
            .into_iter()
            .map(|setting| {
                let own = edited.get(setting.key);
                let below = match dialog.scope {
                    Scope::User => default_of(setting.key),
                    Scope::Project => snapshot.user.get(setting.key).or_else(|| default_of(setting.key)),
                };
                let overridden = match dialog.scope {
                    Scope::User => snapshot.project.get(setting.key),
                    Scope::Project => None,
                };
                let agent = (setting.key == "terminal_agent").then(|| AgentRow {
                    has_command: edited.command().is_some(),
                    command: edited.command().map(str::to_owned),
                    editing: dialog.command_editing,
                    problem: agent_problem.clone(),
                    omp_installed: files.machine_agent() == TerminalAgent::OhMyPi,
                });
                setting_row(setting, own, below.unwrap_or(Value::On(false)), overridden, default_of(setting.key), agent)
            })
            .collect();
        // A list, so a tab with more settings than the window is tall scrolls instead of running off it.
        commands.entity(page).with_children(|p| {
            p.spawn(list(LIST_ROWS, Children::spawn(SpawnIter(rows.into_iter()))));
        });
    }
    built.0 = Some(snapshot);
}

/// A scale slider that was just built or whose value moved.
type PercentSlider = (With<PercentReadout>, Or<(Added<PercentReadout>, Changed<SliderValue>)>);

/// **Write each scale slider's readout as a percentage**, once it is built and whenever its value moves.
fn say_the_percentage(
    sliders: Query<(Entity, &SliderValue), PercentSlider>,
    children: Query<&Children>,
    mut readouts: Query<(&bevy_gooey::Role, &mut Text)>,
) {
    for (slider, value) in &sliders {
        let said = percent(value.0);
        for part in children.iter_descendants(slider) {
            if let Ok((role, mut text)) = readouts.get_mut(part)
                && *role == bevy_gooey::Role::SliderReadout
                && text.0 != said
            {
                text.0 = said.clone();
            }
        }
    }
}

/// **What the Terminal agent row shows beyond its choice** (#425).
#[derive(Clone, Debug)]
struct AgentRow {
    /// The file being edited sets a command, so `Custom command` is a choice.
    has_command: bool,
    /// That command, shown so it can be read and changed.
    command: Option<String>,
    /// The row's command field is open.
    editing: bool,
    /// The scope and the words for a file that chose Custom with no command.
    problem: Option<(Scope, String)>,
    /// The agent is installed on this machine, so the problem has a one-press fix.
    omp_installed: bool,
}

/// **The Terminal agent row's extras**: the problem with its one-press fix, the command and the way to set it.
fn spawn_agent_extras(row: &mut RelatedSpawner<ChildOf>, agent: &AgentRow) {
    if let Some((scope, file)) = &agent.problem {
        row.spawn(label(format!("Problem: {file} chooses a Custom agent but sets no terminal_agent_command, so no agent can start.")));
        if agent.omp_installed {
            let scope = *scope;
            row.spawn((
                button("Use Oh My Pi, which is installed"),
                observe(move |_: On<Activate>, mut w: Writer| {
                    w.set_in(scope, "terminal_agent", Some(Value::Option(setting_catalogue::agent_index(TerminalAgent::OhMyPi))));
                }),
            ));
        }
    }
    if agent.editing {
        row.spawn((
            text_input("Custom command", agent.command.clone().unwrap_or_default()),
            TextHint("a program, like claude --mcp-config $CAULDRON_MCP_CONFIG".into()),
            AutoFocus,
            observe(|c: On<ValueChange<String>>, mut w: Writer| {
                if c.is_final {
                    w.finish_command(&c.value);
                }
            }),
        ));
    } else {
        if let Some(command) = &agent.command {
            row.spawn(label(format!("Command: {command}")));
        }
        let words = if agent.has_command { "Change command" } else { "Custom command…" };
        row.spawn((button(words), observe(|_: On<Activate>, mut w: Writer| w.edit_command(true))));
    }
}

/// One setting: its control, what it does, its default, whether it is modified and the Reset.
fn setting_row(
    setting: &'static Setting,
    own: Option<Value>,
    below: Value,
    overridden: Option<Value>,
    default_value: Option<Value>,
    agent: Option<AgentRow>,
) -> impl Bundle {
    let shown = own.unwrap_or(below);
    let key = setting.key;
    (
        Node { flex_direction: FlexDirection::Column, margin: UiRect::bottom(Val::Px(crate::chrome::GAP_GROUP)), ..default() },
        Children::spawn(SpawnWith(move |row: &mut RelatedSpawner<ChildOf>| {
            match (setting.kind, shown) {
                (Kind::Switch, Value::On(on)) => {
                    row.spawn((
                        toggle(setting.label, on),
                        observe(move |c: On<ValueChange<bool>>, mut w: Writer| w.set(key, Some(Value::On(c.value)))),
                    ));
                }
                (Kind::Scale { min, max }, Value::Number(n)) => {
                    let mut control = row.spawn((
                        slider(setting.label, n, min..=max),
                        PercentReadout,
                        // **A write is the end of a move, never the middle of one.** The rows are built
                        // again from the file, so a write on every frame of a drag would rebuild the
                        // slider under the pointer. A key press and the end of a drag are final.
                        observe(move |c: On<ValueChange<f32>>, mut w: Writer| {
                            if c.is_final {
                                w.set(key, Some(Value::Number(on_the_step(c.value))));
                            }
                        }),
                        // **And a click on the rail is final too**, though gooey calls it a change in
                        // progress: it sends no end, so the release is where the click is done.
                        observe(move |release: On<PointerRelease>, sliders: Query<(&SliderValue, &SliderRange)>, mut w: Writer| {
                            if let Ok((value, _)) = sliders.get(release.entity) {
                                let landed = on_the_step(value.0);
                                if (landed - n).abs() > f32::EPSILON {
                                    w.set(key, Some(Value::Number(landed)));
                                }
                            }
                        }),
                    ));
                    // Inserted over the slider's own step (a twentieth of its range) rather than named in the
                    // bundle, which would hold the component twice.
                    control.insert((SliderStep(SCALE_STEP), SliderPrecision(SLIDER_DECIMALS)));
                }
                (Kind::Options(names), Value::Option(i)) => {
                    // **`Custom command` is a choice only once a command is set** (#425), so no step of the row can save it without one.
                    let offered: Vec<&str> = match &agent {
                        Some(agent) => {
                            let current = if i == setting_catalogue::agent_index(TerminalAgent::Custom) { TerminalAgent::Custom } else { TerminalAgent::Off };
                            setting_catalogue::agent_choices(agent.has_command, current).to_vec()
                        }
                        None => names.to_vec(),
                    };
                    row.spawn((
                        choice(setting.label, offered, i),
                        observe(move |c: On<ValueChange<usize>>, mut w: Writer| w.set(key, Some(Value::Option(c.value)))),
                    ));
                }
                _ => {
                    row.spawn(label(format!("{}: unreadable value", setting.label)));
                }
            }
            if let Some(agent) = &agent {
                spawn_agent_extras(row, agent);
            }
            row.spawn(label(setting.description));
            let default_text = default_value.map_or_else(String::new, |d| format!("Default: {}", setting_catalogue::say(setting, d)));
            row.spawn(bevy_gooey::row_with(crate::chrome::GAP_GROUP_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::Center), wrap: Some(FlexWrap::Wrap), ..default() }))
                .with_children(|line| {
                    line.spawn(label(default_text));
                    if own.is_some() {
                        line.spawn(label("Modified"));
                        line.spawn((
                            button(format!("Reset {}", setting.label)),
                            observe(move |_: On<Activate>, mut w: Writer| w.set(key, None)),
                        ));
                    }
                    if let Some(over) = overridden {
                        line.spawn(label(format!("This project sets it to {}", setting_catalogue::say(setting, over))));
                    }
                });
        })),
    )
}
