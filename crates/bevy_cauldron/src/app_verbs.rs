//! **The verbs of an application, not of a map** (#299, and #257's Help half): New project, Open project, Open recent, Add
//! meshes, Quit, the keys, the written guide, What's new, Report a problem and About.
//!
//! `commands.rs` is the registry of what a *key* does inside a map, and `keys::BINDINGS` is its one census, so a verb that
//! leaves the map (a project is not a map) and is no chord of the editor's does not belong there: a row in that census would
//! be a binding the dispatcher could never see on the screens that have no editor. These verbs are rows of the menu bar that
//! are not commands, as View → Show statistics is a setting and not a command ([`crate::menu_bar::StatisticsItem`]).
//!
//! # Leaving for the home saves first
//!
//! New project…, Open project…, What's new and Open recent leave the open project. The deliberate exits save what is open and
//! go, as `Cmd+O` does ([`crate::editor::save_and_leave`]): a map that will not save keeps the person where they are with the
//! reason, because leaving would drop exactly the work the save was promising to keep.
//!
//! # Quit asks only when there is something to lose
//!
//! `Cmd/Ctrl+Q` and File → Quit quit at once from a screen with no map, and from a clean one. With unsaved edits they ask
//! the leave question's three answers — save and quit, discard, stay — through the one prompt ([`crate::confirm`]).

use std::path::{Path, PathBuf};

use bevy::input_focus::AutoFocus;
use bevy::prelude::*;
use bevy_gooey::{Activate, Cancel, Shortcut, button, dialog, heading, label, menu, panel, toast_alert};

use crate::confirm::{Asked, Choice, Confirm, ConfirmKeys};
use crate::editor::EditorState;
use crate::launcher::{self, HomeTab, Launcher};
use crate::picker::{PickFor, PickRequest};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;

/// Where "Report a problem" opens. The version and the system are the body the form starts with.
pub const REPORT_URL: &str = "https://github.com/bevy-coven/bevy_grimoire/issues/new";

/// The recent projects File → Open recent lists.
pub const RECENT_SHOWN: usize = 5;

/// The chord that quits: the platform's own modifier and Q.
pub const QUIT: Shortcut = Shortcut::primary(KeyCode::KeyQ);

/// A row of the menu bar that is a verb of the application and no chord of the editor's.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum AppVerb {
    /// Leave for the home and its New project form.
    NewProject,
    /// Leave for the home's list of projects.
    OpenProject,
    /// Open the project in this folder, on its project page.
    OpenRecent(PathBuf),
    /// Ask for meshes to add to the library.
    AddMeshes,
    /// Start the first-map tour (`tour.rs`).
    #[cfg(feature = "debugger")]
    TakeTour,
    /// Quit, asking first when a map has unsaved edits.
    Quit,
    /// Show the keymap: every command and its chord.
    KeyboardShortcuts,
    /// **Open one page of the written guide** (UI-91, #257), by [`crate::help`]'s id. The row says the
    /// page's title, and the address is the page's own file, so the menu and the guide's index cannot
    /// name different pages.
    GuidePage(&'static str),
    /// Open the written guide in the browser.
    GettingStarted,
    /// Leave for the home's What's new page.
    WhatsNew,
    /// Open a new issue in the browser, with the version in it.
    ReportProblem,
    /// Show the About dialog.
    About,
    /// **Put the shell back on its default arrangement** (UI-33, #227): Window ▸ Reset layout, undoing
    /// every divider drag and panel move this door has had.
    ResetLayout,
    /// **Bring the PROBLEMS list on screen** (NV-11, #397): View ▸ Show problems, the row of the `Cmd/Ctrl+Shift+X` chord.
    ShowProblems,
    /// **Make the terminal cover the window below the menu bar, or give it back its height** (#426): View ▸ Maximise terminal, the row of the
    /// `terminal.maximise` chord (Shift+Cmd+J, Shift+Ctrl+J off a mac).
    MaximiseTerminal,
}

impl AppVerb {
    /// What the row says.
    pub fn title(&self) -> String {
        match self {
            AppVerb::NewProject => "New project…".to_owned(),
            AppVerb::OpenProject => "Open project…".to_owned(),
            AppVerb::OpenRecent(path) => launcher::name_of(path),
            AppVerb::AddMeshes => crate::add_meshes::ACTION.to_owned(),
            #[cfg(feature = "debugger")]
            AppVerb::TakeTour => "Take the tour".to_owned(),
            AppVerb::Quit => "Quit".to_owned(),
            AppVerb::KeyboardShortcuts => "Keyboard shortcuts".to_owned(),
            AppVerb::GuidePage(id) => crate::help::page(id).map_or_else(|| "Guide".to_owned(), |page| page.title.to_owned()),
            AppVerb::GettingStarted => "Getting started guide".to_owned(),
            AppVerb::WhatsNew => "What's new".to_owned(),
            AppVerb::ReportProblem => "Report a problem".to_owned(),
            AppVerb::About => "About Cauldron".to_owned(),
            AppVerb::ResetLayout => "Reset layout".to_owned(),
            AppVerb::ShowProblems => "Show problems".to_owned(),
            AppVerb::MaximiseTerminal => "Maximise terminal".to_owned(),
        }
    }

    /// The chord the row shows at its end, when the verb has one.
    pub fn shortcut(&self) -> Option<Shortcut> {
        match self {
            AppVerb::Quit => Some(QUIT),
            AppVerb::ShowProblems => Some(crate::problems_view::show_problems_chord()),
            AppVerb::MaximiseTerminal => Some(crate::terminal_pane::maximise_chord()),
            _ => None,
        }
    }
}

/// **A person asked to quit**: the chord, or File → Quit.
#[derive(Message, Clone, Copy, Debug)]
pub struct QuitRequested;

/// Ask for the About dialog.
#[derive(Message, Clone, Copy, Debug)]
pub struct OpenAbout;

/// **A run that opens no browser**: a test inserts it, and an address that would have been opened is recorded here
/// instead, so a headless run never launches the system's opener. ([`launcher::ClipboardOff`] is the same for the clipboard.)
#[derive(Resource, Default, Debug)]
pub struct LinksOff(pub Vec<String>);

pub struct AppVerbsPlugin;

impl Plugin for AppVerbsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<QuitRequested>()
            .add_message::<OpenAbout>()
            .add_observer(on_verb_activate)
            .add_systems(
                Update,
                (
                    // Before the verbs: the chord is Ctrl+Q and the editor's plain `Q` turns the view, but a press that is
                    // this one's is taken all the same.
                    quit_chord.before(crate::keys::Phase::Act),
                    quit_requested.after(quit_chord),
                    answer_the_quit_question.after(ConfirmKeys).before(crate::keys::Phase::Act),
                )
                    .chain(),
            )
            .add_systems(Update, open_about.run_if(in_state(Screen::Editor)));
    }
}

/// The press of a verb's row.
fn on_verb_activate(activate: On<Activate>, verbs: Query<&AppVerb>, mut commands: Commands) {
    let Ok(verb) = verbs.get(activate.entity) else { return };
    let verb = verb.clone();
    commands.queue(move |world: &mut World| run(world, verb));
}

/// Do what the verb says.
pub fn run(world: &mut World, verb: AppVerb) {
    match verb {
        AppVerb::NewProject => leave_for_the_home(world, HomeTab::Projects, true),
        AppVerb::OpenProject => leave_for_the_home(world, HomeTab::Projects, false),
        AppVerb::WhatsNew => leave_for_the_home(world, HomeTab::WhatsNew, false),
        AppVerb::OpenRecent(path) => open_recent(world, &path),
        AppVerb::AddMeshes => {
            world.write_message(PickRequest(PickFor::AddMeshes));
        }
        #[cfg(feature = "debugger")]
        AppVerb::TakeTour => crate::tour::start_or_say_why(world),
        AppVerb::Quit => {
            world.write_message(QuitRequested);
        }
        AppVerb::KeyboardShortcuts => {
            world.write_message(crate::keymap_screen::OpenKeymap);
        }
        AppVerb::GuidePage(id) => match crate::help::page(id) {
            Some(page) => open_link(world, &crate::help::url(page)),
            // A row whose page the catalog lost cannot open anything: say so where the person reads
            // it rather than opening the guide's front page and pretending it was the answer.
            None => {
                let said = format!("the guide has no page `{id}`, so this row has nothing to open");
                world.spawn(bevy_gooey::toast_alert(said.clone()).sticky());
                bevy::log::warn!("{said}");
            }
        },
        AppVerb::GettingStarted => open_link(world, launcher::GUIDE_URL),
        AppVerb::ReportProblem => open_link(world, &report_url()),
        AppVerb::About => {
            world.write_message(OpenAbout);
        }
        AppVerb::ResetLayout => reset_the_layout(world),
        AppVerb::ShowProblems => crate::problems_view::ask_for_the_problems(world),
        AppVerb::MaximiseTerminal => crate::terminal_pane::maximise_from_world(world),
    }
}

/// **Window ▸ Reset layout**: the default arrangement back, and the person's file made to agree with it.
///
/// Two writes, one intent. The running `Dock` is set to the current workspace's default so the change
/// is on screen this frame; the user's file loses that workspace's entry, which is what its default
/// *is* (a person who has never moved a panel has no key). Writing the file is what makes it survive a restart; the
/// key goes rather than being written out as the default's RON, because a saved copy of the default
/// would stop tracking a change to it.
fn reset_the_layout(world: &mut World) {
    let workspace = world
        .get_resource::<crate::workspace::Workspace>()
        .copied()
        .unwrap_or_default();
    let default = crate::ui::shell_layout(workspace);
    // **The arrangement's dock, and only it** (review of #247). The frame holds a second gooey `Dock`
    // at the foot of the workbench (UI-61), and an unfiltered loop wrote the workspace's *split* into
    // it: Window ▸ Reset layout then deleted the reload panel and built three placeholder pages in a
    // 148-px strip. The workspace arrangement is `chrome::ArrangementDock`'s alone.
    let mut docks = world.query_filtered::<&mut bevy_gooey::Dock, With<crate::chrome::ArrangementDock>>();
    for mut dock in docks.iter_mut(world) {
        if dock.0 != default {
            dock.0 = default.clone();
        }
    }
    if let Some(settings) = world.get_resource_mut::<crate::settings::EditorSettings>() {
        settings.into_inner().workspace_layout.remove(workspace.id());
    }
    let Some(file) = world.get_resource::<crate::settings::UserSettingsFile>().map(|f| f.0.clone()) else { return };
    if let Err(why) = crate::setting_catalogue::edit_layer(&file, |layer| {
        layer.workspace_layout.remove(workspace.id());
        Ok(())
    }) {
        world.resource_mut::<EditorState>().status.problem(format!("cannot reset the layout: {why}"));
    }
}

/// **Write what is open before leaving it**, as the deliberate exits do. `Err` says why not, and the person stays.
pub fn save_the_open_map(world: &mut World) -> Result<(), String> {
    let Some(open) = world.get_resource::<OpenMap>() else { return Ok(()) };
    if !open.dirty() {
        return Ok(());
    }
    let saved = world.resource_scope(|world: &mut World, mut open: Mut<OpenMap>| match world.get_resource::<Project>() {
        Some(project) => open.save(project),
        None => Ok(()),
    });
    match saved {
        Ok(()) => {
            world.resource_mut::<EditorState>().status.note("saved");
            Ok(())
        }
        Err(e) => {
            let why = format!("NOT SAVED: {e} — staying on this map");
            world.resource_mut::<EditorState>().status.problem(why.clone());
            Err(why)
        }
    }
}

/// Leave the open project for the home on `tab`, and its New project form when `new_project` is asked for.
fn leave_for_the_home(world: &mut World, tab: HomeTab, new_project: bool) {
    if save_the_open_map(world).is_err() {
        return;
    }
    let mut launcher = world.resource_mut::<Launcher>();
    launcher.tab = tab;
    launcher.new_project_asked = new_project;
    world.resource_mut::<NextState<Screen>>().set(Screen::Launcher);
}

/// Open a project from the recent list: save what is open, then the same opening the home makes.
fn open_recent(world: &mut World, path: &Path) {
    if save_the_open_map(world).is_err() {
        return;
    }
    let opened = launcher::check_project(path).and_then(|root| {
        if let Some(file) = world.get_resource::<crate::recent::RecentProjects>().map(|r| r.0.clone())
            && let Err(e) = crate::recent::record(&file, &root)
        {
            warn!("recent projects not updated: {e}");
        }
        crate::project_assets::open_project(world, &root, None)
    });
    if let Err(e) = opened {
        world.spawn(toast_alert(e).sticky());
    }
}

/// Open an address in the browser, or record it when this run opens none ([`LinksOff`]).
pub fn open_link(world: &mut World, url: &str) {
    if let Err(e) = open_in_this_run(world, url) {
        world.spawn(toast_alert(e).sticky());
    }
}

/// **Open an address, and say whether it opened** — the answer rather than the toast, for a caller
/// that has to know (#354).
///
/// **One opener**: this is the whole body of [`open_link`], and a run that opens no browser
/// ([`LinksOff`]) records the address and answers `Ok`. A caller that needs the address recorded
/// *and* the answer — a wish's Open on GitHub, which leaves the wish waiting when the page did not
/// open — asks here instead of launching a second `open_in_os` and losing the seam a headless run
/// needs.
pub fn open_in_this_run(world: &mut World, url: &str) -> Result<(), String> {
    if let Some(mut off) = world.get_resource_mut::<LinksOff>() {
        off.0.push(url.to_owned());
        return Ok(());
    }
    launcher::open_in_os(url)
}

/// The address "Report a problem" opens: a new issue whose text already says which build and system it came from.
pub fn report_url() -> String {
    let body = format!("Cauldron {} on {}\n\nWhat happened:\n", env!("CARGO_PKG_VERSION"), std::env::consts::OS);
    format!("{REPORT_URL}?body={}", percent_encode(&body))
}

/// `text` as a query value: everything but the unreserved characters is written as `%XX` bytes.
fn percent_encode(text: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(char::from(byte)),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// `Cmd/Ctrl+Q` is a quit request on every screen. The press is taken, so nothing behind it reads the Q.
fn quit_chord(mut keyboard: ResMut<ButtonInput<KeyCode>>, mut out: MessageWriter<QuitRequested>) {
    if QUIT.just_pressed(&keyboard) {
        crate::confirm::take(&mut keyboard, KeyCode::KeyQ);
        out.write(QuitRequested);
    }
}

/// **Quit now, or ask first.** A map with unsaved edits asks the leave question's three answers; anything else quits.
fn quit_requested(
    mut requests: MessageReader<QuitRequested>,
    open: Option<Res<OpenMap>>,
    mut state: ResMut<EditorState>,
    mut confirm: ResMut<Confirm>,
    mut exit: MessageWriter<AppExit>,
) {
    if requests.read().count() == 0 {
        return;
    }
    // A question is already waiting for its answer; quitting is asked for again once it has one.
    if confirm.is_open() {
        return;
    }
    if open.is_some_and(|open| open.dirty()) || crate::files::any_unsaved() {
        state.leaving = true;
        confirm.ask_three(
            Asked::QuitApp,
            "Quit Cauldron?",
            "It has unsaved edits. Quitting saves them first, unless you discard them.",
            "Save and quit",
            "Discard changes",
            KeyCode::KeyD,
            "Stay",
        );
    } else {
        exit.write(AppExit::Success);
    }
}

/// The quit question's answer: save and quit, quit without saving, or stay. A map that will not save keeps the person here.
fn answer_the_quit_question(
    mut confirm: ResMut<Confirm>,
    project: Option<Res<Project>>,
    open: Option<ResMut<OpenMap>>,
    mut state: ResMut<EditorState>,
    mut exit: MessageWriter<AppExit>,
    mut files: Option<ResMut<crate::files::Files>>,
) {
    let Some(choice) = confirm.answer_choice(Asked::QuitApp) else { return };
    state.leaving = false;
    // **The files' unsaved edits are asked about with the map's** (GD-11, #373): saved with it, or let go with it.
    if choice == Choice::Yes
        && let (Some(files), Some(project)) = (files.as_deref_mut(), project.as_deref())
        && let Err(e) = files.save_all(&project.root)
    {
        state.status.problem(format!("NOT SAVED: {e} — staying on this map"));
        return;
    }
    match choice {
        Choice::No => state.status.note("staying on this map"),
        Choice::Alt => {
            exit.write(AppExit::Success);
        }
        Choice::Yes => {
            let saved = match (open, project) {
                (Some(mut open), Some(project)) if open.dirty() => open.save(&project),
                _ => Ok(()),
            };
            match saved {
                Ok(()) => {
                    exit.write(AppExit::Success);
                }
                Err(e) => state.status.problem(format!("NOT SAVED: {e} — staying on this map")),
            }
        }
    }
}

/// The About dialog: what this is, which build, and under which licence.
fn open_about(
    mut asks: MessageReader<OpenAbout>,
    standing: Query<(), With<AboutDialog>>,
    images: Option<ResMut<Assets<Image>>>,
    splash: Option<ResMut<launcher::SplashArt>>,
    mut commands: Commands,
) {
    if asks.read().count() == 0 || !standing.is_empty() {
        return;
    }
    // The splash is the home's (`LauncherPlugin` owns it); an editor stood up without the home has none, and About is its words.
    let art = match (images, splash) {
        (Some(mut images), Some(mut splash)) => {
            launcher::ensure_splash(&mut images, &mut splash);
            splash.0.clone()
        }
        _ => None,
    };
    commands
        .spawn((dialog(), AboutDialog, Name::new("about")))
        .insert(GlobalZIndex(crate::chrome::MODAL_Z))
        .observe(|cancel: On<Cancel>, mut keyboard: ResMut<ButtonInput<KeyCode>>, mut commands: Commands| {
            // The `Esc` that closes this is this dialog's alone.
            crate::confirm::take(&mut keyboard, KeyCode::Escape);
            commands.entity(cancel.entity).insert(bevy_gooey::Leave);
        })
        .with_children(|scrim| {
            scrim.spawn((panel(), bevy::picking::hover::Hovered::default())).with_children(|card| {
                if let Some((handle, size)) = art.as_ref() {
                    card.spawn((
                        bevy_gooey::aspect_image(handle.clone(), size.x, size.x / size.y),
                        Name::new("about splash"),
                    ));
                }
                card.spawn(heading("Cauldron"));
                card.spawn(label(format!("Version {}", env!("CARGO_PKG_VERSION"))));
                card.spawn(label("Build levels from your own pieces, and hand your game a grimoire it can load."));
                card.spawn(label(format!("Licence: {}", env!("CARGO_PKG_LICENSE"))));
                card.spawn(menu()).with_children(|answers| {
                    answers.spawn((
                        button("Close"),
                        AutoFocus,
                        bevy_gooey::observe(|_: On<Activate>, dialogs: Query<Entity, With<AboutDialog>>, mut commands: Commands| {
                            for dialog in &dialogs {
                                commands.entity(dialog).insert(bevy_gooey::Leave);
                            }
                        }),
                    ));
                });
            });
        });
}

/// The About dialog, so a test finds it and a second one is not opened over it.
#[derive(Component)]
pub struct AboutDialog;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_written_as_percent_bytes() {
        assert_eq!(percent_encode("Cauldron 0.2.0 on linux\n\nWhat?"), "Cauldron%200.2.0%20on%20linux%0A%0AWhat%3F");
    }
}
