//! **The loading screen: a project opened as named stages** (UI-20, #221).
//!
//! `bevy_cauldron <root> --door kit` used to open the project on the main thread before the window existed, so
//! a big project was a window that did not answer. [`Screen::Loading`] opens it on a thread
//! ([`crate::args::open_staged`], which tells each [`Stage`] as it starts) while the screen answers:
//!
//! * **under [`LoadTiming::spinner_after`] (1 s) nothing shows**: a project that opens at once is never flashed;
//! * **from then, a spinner and the stage's name**;
//! * **from [`LoadTiming::progress_after`] (10 s), a percent-done bar** with "N of M: stage", the stages being
//!   known from the start ([`Stage::ALL`]).
//!
//! **When it ends, a report card** (UI-21, #222) lists every file of the project that does not read, one row each
//! with the file, the field and why ([`report`]: [`bevy_grimoire_core::reload_check`] over the files the reload
//! contract names), with **Open anyway** and **Back**. A project whose files all read goes straight to the editor.
//!
//! Cancel returns to the launcher. An open only reads, so there is nothing to undo: the thread is left to finish
//! and its answer is dropped, and a test pins that the project's files are byte-identical afterwards. A project that
//! will not open stays on this screen with the reason and a Back button, and the reason is on the terminal too.
//!
//! The chooser's own open (a kit and a map picked on the menu) is still synchronous: it is one frame of a project
//! already chosen, and it moves into the workbench (UI-30) rather than through here.
//!
//! # What is not a stage yet
//!
//! The issue names six stages. The four here are the ones an open runs; the thumbnails and the family crates are
//! built after the editor's screen is up, by systems that do not run in the loader, so naming them here would be a
//! stage with nothing in it.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;
use bevy_gooey::{Progress, ProgressText, button, heading, label, observe, panel, progress, screen, spinner};

use bevy_grimoire_core::problem::{Detail, Location, Problem};
use bevy_grimoire_core::reload::{self, Base};
use bevy_grimoire_core::reload_check::{self, Check};

use crate::args::Opened;
use crate::project::Stage;
use crate::screen::Screen;

/// What to open: the argv [`crate::args::open`] reads. Inserted by `main` for a command line that names a door.
#[derive(Resource, Clone, Debug)]
pub struct LoadRequest(pub Vec<String>);

/// A pause as each stage starts, before its work, so a project that opens at once can be watched; the same pause for every
/// stage. `main` sets it from `GRIMOIRE_LOAD_STAGE_DELAY_MS`; a test sets it to make the phases visible.
#[derive(Resource, Clone, Copy, Debug)]
pub struct StageDelay(pub Duration);

/// When the screen shows what: nothing, a spinner, a bar.
#[derive(Resource, Clone, Copy, Debug)]
pub struct LoadTiming {
    pub spinner_after: Duration,
    pub progress_after: Duration,
}

impl Default for LoadTiming {
    fn default() -> Self {
        LoadTiming { spinner_after: Duration::from_secs(1), progress_after: Duration::from_secs(10) }
    }
}

/// What the screen is showing.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadPhase {
    /// Nothing yet: under the spinner's threshold, or not loading.
    #[default]
    Quiet,
    Spinner,
    Progress,
}

/// What the thread hands back: the door, and every file of the project that did not read.
struct Loaded {
    opened: Opened,
    problems: Vec<Problem>,
}

/// **The files of an opened project that do not read**, while the report card is up. Each is one [`Problem`]
/// whose location is the file and the field.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LoadReport(pub Vec<Problem>);

/// Every file of the project at `root` that its kind's parser refuses, in file order: the ones the open itself
/// does not need (a terrain, a water plan, a map not opened, the notes) and so cannot have failed it.
pub fn report(root: &Path) -> Vec<Problem> {
    let project_dir = root.join(crate::chooser::GRIMOIRE_DIR);
    let mut out = Vec::new();
    for file in crate::reload_files::files(&project_dir, root, None, &[]) {
        let Some(kind) = reload::kind_by_id(&file.kind) else { continue };
        let base = if kind.base == Base::Root { root } else { project_dir.as_path() };
        let Ok(text) = std::fs::read_to_string(base.join(&file.file)) else { continue };
        if let Check::Refused(refusal) = reload_check::check(kind, &file.file, &text) {
            out.push(Problem::with(
                format!("a {} file did not read", kind.id),
                Detail::default()
                    .at(Location::file(&file.file).field(refusal.field.clone()))
                    .because(refusal.reason.clone())
                    .fix(format!("save a {} that reads", kind.id)),
            ));
        }
    }
    out
}

/// The open in flight.
#[derive(Resource)]
pub struct LoadJob {
    result: Mutex<Receiver<Result<Loaded, String>>>,
    stage: Arc<Mutex<Option<Stage>>>,
    cancel: Arc<AtomicBool>,
    started: Duration,
    /// What the project was asked to open, for the heading.
    pub name: String,
}

/// Why a project did not open, while the screen says so.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LoadFailed(pub String);

#[derive(Component)]
struct LoadingRoot;
#[derive(Component)]
struct ReportCard;
#[derive(Component)]
struct LoadingPanel;
#[derive(Component)]
struct SpinnerRow;
#[derive(Component)]
struct StageLine;
#[derive(Component)]
struct ProgressRow;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadTiming>()
            .init_resource::<LoadPhase>()
            .add_systems(OnEnter(Screen::Loading), (start_the_load, build_the_screen).chain())
            .add_systems(OnExit(Screen::Loading), |mut commands: Commands| {
                commands.remove_resource::<LoadJob>();
                commands.remove_resource::<LoadFailed>();
                commands.remove_resource::<LoadReport>();
                commands.insert_resource(LoadPhase::Quiet);
            })
            .add_systems(Update, (poll_the_load, show_the_report, show_the_phase).chain().run_if(in_state(Screen::Loading)));
    }
}

/// Starts the open on a thread. Without a [`LoadRequest`] there is nothing to open, and the screen says so.
fn start_the_load(mut commands: Commands, request: Option<Res<LoadRequest>>, delay: Option<Res<StageDelay>>, time: Res<Time<Real>>) {
    let Some(request) = request else {
        commands.insert_resource(LoadFailed("nothing was asked to be opened".to_owned()));
        return;
    };
    let argv = request.0.clone();
    let pause = delay.map_or(Duration::ZERO, |d| d.0);
    let (send, receive) = std::sync::mpsc::channel();
    let stage = Arc::new(Mutex::new(None));
    let cancel = Arc::new(AtomicBool::new(false));
    let (seen, stopped) = (stage.clone(), cancel.clone());
    let name = crate::args::root_of(&argv).file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    std::thread::spawn(move || {
        let opened = crate::args::open_staged(&argv, &mut |entering| {
            if let Ok(mut at) = seen.lock() {
                *at = Some(entering);
            }
            if !pause.is_zero() && !stopped.load(Ordering::Relaxed) {
                std::thread::sleep(pause);
            }
        });
        let loaded = opened.map(|opened| {
            let problems = report(&opened.project.root);
            Loaded { opened, problems }
        });
        // The screen may be gone (cancelled); an answer nobody reads is dropped with the channel.
        let _ = send.send(loaded);
    });
    commands.insert_resource(LoadJob { result: Mutex::new(receive), stage, cancel, started: time.elapsed(), name });
}

fn build_the_screen(mut commands: Commands) {
    let hidden = |flex_direction| Node { display: Display::None, flex_direction, ..default() };
    commands.spawn((screen(), DespawnOnExit(Screen::Loading), LoadingRoot, Name::new("loading"))).with_children(|root| {
        // `insert` after the widget's own `Node`: a bundle cannot carry two.
        root.spawn((panel(), LoadingPanel)).insert(hidden(FlexDirection::Column)).with_children(|card| {
            card.spawn(heading("Opening the project"));
            card.spawn((label(""), StageLine));
            card.spawn((spinner(), SpinnerRow)).insert(hidden(FlexDirection::Row));
            card.spawn((progress(0.0), ProgressRow)).insert(hidden(FlexDirection::Column));
            card.spawn((
                button("Cancel"),
                observe(|_: On<bevy_gooey::Activate>, mut commands: Commands, job: Option<Res<LoadJob>>, mut next: ResMut<NextState<Screen>>| {
                    if let Some(job) = job {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                    commands.remove_resource::<LoadJob>();
                    // An answer already taken this frame left the opened project waiting as `Chosen`; cancelling lets it go.
                    commands.remove_resource::<crate::chooser::Chosen>();
                    next.set(Screen::Launcher);
                }),
            ));
        });
    });
}

/// Takes the answer when the thread has one: into the editor on a door, or onto the screen as the reason.
fn poll_the_load(mut commands: Commands, job: Option<Res<LoadJob>>, mut next: ResMut<NextState<Screen>>) {
    let Some(job) = job else { return };
    let answer = match job.result.lock() {
        Ok(rx) => rx.try_recv(),
        Err(_) => Err(TryRecvError::Disconnected),
    };
    match answer {
        Ok(Ok(Loaded { opened, problems })) => {
            commands.insert_resource(crate::chooser::Chosen(opened));
            commands.remove_resource::<LoadJob>();
            if problems.is_empty() {
                next.set(Screen::Editor);
            } else {
                commands.insert_resource(LoadReport(problems));
            }
        }
        Ok(Err(reason)) => fail(&mut commands, reason),
        Err(TryRecvError::Disconnected) => fail(&mut commands, "the project's opening stopped without an answer".to_owned()),
        Err(TryRecvError::Empty) => {}
    }
}

fn fail(commands: &mut Commands, reason: String) {
    eprintln!("bevy_cauldron: {reason}");
    commands.remove_resource::<LoadJob>();
    commands.insert_resource(LoadFailed(reason));
}

/// The three nodes `show_the_phase` changes the display of, told apart so their queries cannot overlap.
type PanelNode = (With<LoadingPanel>, Without<SpinnerRow>, Without<ProgressRow>);
type SpinnerNode = (With<SpinnerRow>, Without<LoadingPanel>, Without<ProgressRow>);
type BarNode = (With<ProgressRow>, Without<LoadingPanel>, Without<SpinnerRow>);

/// What the screen shows now: nothing under the spinner's threshold, then the stage with a spinner, then a bar; or
/// the reason a project did not open.
#[allow(clippy::too_many_arguments)]
fn show_the_phase(
    job: Option<Res<LoadJob>>,
    failed: Option<Res<LoadFailed>>,
    report: Option<Res<LoadReport>>,
    timing: Res<LoadTiming>,
    time: Res<Time<Real>>,
    mut phase: ResMut<LoadPhase>,
    mut panels: Query<&mut Node, PanelNode>,
    mut spinners: Query<&mut Node, SpinnerNode>,
    mut bars: Query<(&mut Node, &mut Progress, &mut ProgressText), BarNode>,
    mut lines: Query<&mut Text, With<StageLine>>,
) {
    let (want, stage) = match (&job, &failed) {
        (_, Some(_)) => (LoadPhase::Quiet, None),
        (Some(job), None) => {
            let elapsed = time.elapsed().saturating_sub(job.started);
            let phase = if elapsed >= timing.progress_after {
                LoadPhase::Progress
            } else if elapsed >= timing.spinner_after {
                LoadPhase::Spinner
            } else {
                LoadPhase::Quiet
            };
            (phase, job.stage.lock().ok().and_then(|s| *s))
        }
        (None, None) => (*phase, None),
    };
    if *phase != want {
        *phase = want;
    }
    let showing = report.is_none() && failed.is_none() && want != LoadPhase::Quiet;
    let set = |node: &mut Node, on: bool| {
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    };
    for mut node in &mut panels {
        set(&mut node, showing);
    }
    for mut node in &mut spinners {
        set(&mut node, failed.is_none() && want == LoadPhase::Spinner);
    }
    let name = job.as_ref().map(|j| j.name.clone()).unwrap_or_default();
    for mut text in &mut lines {
        let said = match (failed.is_some(), stage) {
            (true, _) => String::new(),
            (false, Some(stage)) if want != LoadPhase::Progress => format!("{} ({name})", stage.label()),
            _ => String::new(),
        };
        if text.0 != said {
            text.0 = said;
        }
    }
    for (mut node, mut bar, mut words) in bars.iter_mut() {
        set(&mut node, failed.is_none() && want == LoadPhase::Progress);
        if let Some(stage) = stage {
            let total = Stage::ALL.len();
            let fraction = stage.index() as f32 / total as f32;
            if bar.0 != fraction {
                bar.0 = fraction;
            }
            let step = Some((stage.index() as u32 + 1, total as u32, stage.label().to_owned()));
            if words.step != step {
                words.step = step;
            }
        }
    }
}

/// The report card, built once when [`LoadReport`] appears: one row per file that did not read, then Open anyway
/// and Back.
fn show_the_report(
    mut commands: Commands,
    report: Option<Res<LoadReport>>,
    failed: Option<Res<LoadFailed>>,
    cards: Query<(), With<ReportCard>>,
    root: Query<Entity, With<LoadingRoot>>,
) {
    if !cards.is_empty() {
        return;
    }
    let Ok(root) = root.single() else { return };
    // A project that did not open is a card too: its reason, and the way back. One that opened with files that
    // do not read adds Open anyway.
    let (head, rows, open_anyway) = match (&report, &failed) {
        (Some(report), _) => {
            let n = report.0.len();
            (format!("{n} file{} did not read", if n == 1 { "" } else { "s" }), report.0.iter().map(Problem::line).collect::<Vec<_>>(), true)
        }
        (None, Some(failed)) => ("The project did not open".to_owned(), vec![failed.0.clone()], false),
        (None, None) => return,
    };
    commands.entity(root).with_children(|root| {
        root.spawn((panel(), ReportCard))
            .insert(Node { flex_direction: FlexDirection::Column, width: Val::Px(crate::launcher::CARD_WIDTH), max_width: Val::Percent(100.0), ..default() })
            .with_children(|card| {
            card.spawn(heading(head));
            for row in rows {
                card.spawn((label(row), ReportRow));
            }
            card.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(crate::chrome::GAP_GROUP), ..default() }).with_children(|row| {
                if open_anyway {
                    row.spawn((
                        button("Open anyway"),
                        observe(|_: On<bevy_gooey::Activate>, mut next: ResMut<NextState<Screen>>| next.set(Screen::Editor)),
                    ));
                }
                row.spawn((
                    button("Back"),
                    observe(|_: On<bevy_gooey::Activate>, mut commands: Commands, mut next: ResMut<NextState<Screen>>| {
                        commands.remove_resource::<crate::chooser::Chosen>();
                        next.set(Screen::Launcher);
                    }),
                ));
            });
        });
    });
}

/// One file of the report.
#[derive(Component)]
pub struct ReportRow;
