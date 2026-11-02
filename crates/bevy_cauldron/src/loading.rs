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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;
use bevy_gooey::{Progress, ProgressText, button, heading, label, observe, panel, progress, screen, spinner};

use crate::args::Opened;
use crate::project::Stage;
use crate::screen::Screen;

/// What to open: the argv [`crate::args::open`] reads. Inserted by `main` for a command line that names a door.
#[derive(Resource, Clone, Debug)]
pub struct LoadRequest(pub Vec<String>);

/// A pause after each stage, so a project that opens at once can be watched. `main` sets it from
/// `GRIMOIRE_LOAD_STAGE_DELAY_MS`; a test sets it to slow one stage down.
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

/// The open in flight.
#[derive(Resource)]
pub struct LoadJob {
    result: Mutex<Receiver<Result<Opened, String>>>,
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
struct LoadingPanel;
#[derive(Component)]
struct SpinnerRow;
#[derive(Component)]
struct StageLine;
#[derive(Component)]
struct ProgressRow;
#[derive(Component)]
struct FailureLine;
#[derive(Component)]
struct BackButton;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadTiming>()
            .init_resource::<LoadPhase>()
            .add_systems(OnEnter(Screen::Loading), (start_the_load, build_the_screen).chain())
            .add_systems(OnExit(Screen::Loading), |mut commands: Commands| {
                commands.remove_resource::<LoadJob>();
                commands.remove_resource::<LoadFailed>();
                commands.insert_resource(LoadPhase::Quiet);
            })
            .add_systems(Update, (poll_the_load, show_the_phase).chain().run_if(in_state(Screen::Loading)));
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
        // The screen may be gone (cancelled); an answer nobody reads is dropped with the channel.
        let _ = send.send(opened);
    });
    commands.insert_resource(LoadJob { result: Mutex::new(receive), stage, cancel, started: time.elapsed(), name });
}

fn build_the_screen(mut commands: Commands) {
    let hidden = |flex_direction| Node { display: Display::None, flex_direction, ..default() };
    commands.spawn((screen(), DespawnOnExit(Screen::Loading), Name::new("loading"))).with_children(|root| {
        // `insert` after the widget's own `Node`: a bundle cannot carry two.
        root.spawn((panel(), LoadingPanel)).insert(hidden(FlexDirection::Column)).with_children(|card| {
            card.spawn(heading("Opening the project"));
            card.spawn((label(""), StageLine));
            card.spawn((spinner(), SpinnerRow)).insert(hidden(FlexDirection::Row));
            card.spawn((progress(0.0), ProgressRow)).insert(hidden(FlexDirection::Column));
            card.spawn((label(""), FailureLine));
            card.spawn((
                button("Cancel"),
                observe(|_: On<bevy_gooey::Activate>, mut commands: Commands, job: Option<Res<LoadJob>>, mut next: ResMut<NextState<Screen>>| {
                    if let Some(job) = job {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                    commands.remove_resource::<LoadJob>();
                    next.set(Screen::Launcher);
                }),
            ));
            card.spawn((
                button("Back"),
                BackButton,
                observe(|_: On<bevy_gooey::Activate>, mut next: ResMut<NextState<Screen>>| next.set(Screen::Launcher)),
            ))
            .insert(Node { display: Display::None, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() });
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
        Ok(Ok(opened)) => {
            commands.insert_resource(crate::chooser::Chosen(opened));
            commands.remove_resource::<LoadJob>();
            next.set(Screen::Editor);
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

/// What the screen shows now: nothing under the spinner's threshold, then the stage with a spinner, then a bar; or
/// the reason a project did not open.
#[allow(clippy::too_many_arguments)]
fn show_the_phase(
    job: Option<Res<LoadJob>>,
    failed: Option<Res<LoadFailed>>,
    timing: Res<LoadTiming>,
    time: Res<Time<Real>>,
    mut phase: ResMut<LoadPhase>,
    mut panels: Query<&mut Node, (With<LoadingPanel>, Without<SpinnerRow>, Without<ProgressRow>, Without<BackButton>)>,
    mut spinners: Query<&mut Node, (With<SpinnerRow>, Without<LoadingPanel>, Without<ProgressRow>, Without<BackButton>)>,
    mut bars: Query<(&mut Node, &mut Progress, &mut ProgressText), (With<ProgressRow>, Without<LoadingPanel>, Without<SpinnerRow>, Without<BackButton>)>,
    mut back: Query<&mut Node, (With<BackButton>, Without<LoadingPanel>, Without<SpinnerRow>, Without<ProgressRow>)>,
    mut lines: Query<&mut Text, (With<StageLine>, Without<FailureLine>)>,
    mut failures: Query<&mut Text, (With<FailureLine>, Without<StageLine>)>,
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
    let showing = failed.is_some() || want != LoadPhase::Quiet;
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
    for mut node in &mut back {
        set(&mut node, failed.is_some());
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
    for mut text in &mut failures {
        let said = failed.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        if text.0 != said {
            text.0 = said;
        }
    }
}
