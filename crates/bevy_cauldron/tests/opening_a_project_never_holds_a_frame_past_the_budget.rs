//! **Opening a project never stops the window for longer than a frame budget without saying so** (#430).
//!
//! The owner, on 0.5.3: "Whenever I open a project, the game freezes while the current map state is loaded. We
//! should have a loading bar of some sort that communicates to the user the need to wait." The loading screen
//! (UI-20, #221) reads the project on a thread; what came after it, the editor's own first frames on the main
//! thread, was a stall with nothing on screen to say so.
//!
//! Two things are held here:
//!
//! 1. **The longest frame from the open to the first idle editor frame**, on an empty project, on the fixture
//!    project and on a large map: every frame over [`FRAME_BUDGET_MS`] is one of the editor's first
//!    [`BUILD_STAGE_FRAMES`] (the work that cannot leave the main thread), none is longer than
//!    [`BUILD_STAGE_BUDGET_MS`], and from then on the editor is idle.
//! 2. **That stage is on the loading screen, with its label and its place in the bar**, for [`HANDOFF_FRAMES`]
//!    drawn frames before the hand-over: the last thing on screen when the window stops answering says why.
//!
//! The figures come from the headless editor (`harness::build_headless`: the plugin graph the binary ships, no
//! window and no GPU), so a real window is slower, not faster; the budgets are for finding the editor's own
//! cost growing, and the label is for the person waiting.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::loading::{Entering, HANDOFF_FRAMES, LoadPhase, LoadReport, LoadRequest, LoadTiming, LoadingPlugin};
use bevy_cauldron::project::Stage;
use bevy_cauldron::screen::{Chosen, Screen};
use bevy_gooey::{ProgressText, Role};

use crate::fixtures::Fixture;

/// The fewest frames the words may be on screen before the hand-over: one to be laid out, one to be drawn.
const MIN_DRAWN_FRAMES: usize = 2;

/// How many editor frames are stepped after the open: far past the first-frame work, so the last of them are idle.
const FRAMES: usize = 40;

/// The slowest frame the window may be held for while a project opens, in milliseconds: a frame at 20 fps.
const FRAME_BUDGET_MS: f64 = 50.0;

/// How many of the editor's first frames may be over [`FRAME_BUDGET_MS`]: the ones that build its panels, strip and map.
const BUILD_STAGE_FRAMES: usize = 2;

/// The longest the build stage may hold the window, in milliseconds (headless: ~110 ms; a real window draws as well).
const BUILD_STAGE_BUDGET_MS: f64 = 1_000.0;

/// How many times each project is opened, the frame times kept being the fastest seen at each index, so a busy host
/// does not fail the budget.
const TAKES: usize = 3;

/// A project and what it holds, for the report: `place`s on a map.
struct Case {
    name: &'static str,
    fixture: fn(&str) -> Fixture,
}

fn empty(tag: &str) -> Fixture {
    Fixture::new(tag)
}

fn one_kit(tag: &str) -> Fixture {
    Fixture::new(tag).descriptor("bench", "props")
}

/// Sanctuary's town is the large map this stands for: four hundred placements.
fn large_map(tag: &str) -> Fixture {
    let mut fixture = Fixture::new(tag).descriptor("bench", "props");
    for i in 0..400 {
        fixture = fixture.place("bench", ((i % 20) as f32, (i / 20) as f32));
    }
    fixture
}

const CASES: [Case; 3] =
    [Case { name: "an empty project", fixture: empty }, Case { name: "the fixture project", fixture: one_kit }, Case { name: "a large map", fixture: large_map }];

/// The editor's first frames on `case`, in milliseconds: frame 0 is the first update after the open.
fn editor_frames(case: &Case, take: usize) -> Vec<f64> {
    let root = (case.fixture)(&format!("open_budget_{}_{take}_{}", case.name.replace(' ', "_"), std::process::id())).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{}: {e}", case.name));
    (0..FRAMES)
        .map(|_| {
            let start = Instant::now();
            app.update();
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect()
}

/// The fastest each frame index was over [`TAKES`] opens.
fn fastest_frames(case: &Case) -> Vec<f64> {
    let takes: Vec<Vec<f64>> = (0..TAKES).map(|take| editor_frames(case, take)).collect();
    (0..FRAMES).map(|i| takes.iter().map(|t| t[i]).fold(f64::MAX, f64::min)).collect()
}

#[test]
fn every_frame_over_the_budget_is_the_editors_build_and_the_editor_is_idle_after_it() {
    for case in &CASES {
        let frames = fastest_frames(case);
        let longest = frames.iter().copied().fold(0.0, f64::max);
        let slow: Vec<usize> = frames.iter().enumerate().filter(|(_, ms)| **ms > FRAME_BUDGET_MS).map(|(i, _)| i).collect();
        let first_idle = slow.last().map_or(0, |i| i + 1);
        eprintln!("{}: longest frame {longest:.0} ms; frames over {FRAME_BUDGET_MS} ms: {slow:?}; first idle frame {first_idle}", case.name);

        assert!(longest <= BUILD_STAGE_BUDGET_MS, "{}: the editor's first frames held the window {longest:.0} ms, over the {BUILD_STAGE_BUDGET_MS} ms a stage may", case.name);
        assert!(
            first_idle <= BUILD_STAGE_FRAMES,
            "{}: frames {slow:?} are over {FRAME_BUDGET_MS} ms; only the first {BUILD_STAGE_FRAMES} (the build stage the loading screen announces) may be. Frame times: {frames:.0?}",
            case.name
        );
    }
}

/// A host editor that opens `request` through the loading screen, with the thresholds shrunk as the other loading tests do.
fn loading_app(tag: &str, request: Vec<String>) -> App {
    let other = Fixture::new(&format!("open_budget_host_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&other, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins((LauncherPlugin, LoadingPlugin))
        .insert_resource(LoadRequest(request))
        .insert_resource(LoadTiming { spinner_after: Duration::from_secs(60), progress_after: Duration::from_secs(600) });
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Loading);
    app
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

fn project(tag: &str) -> PathBuf {
    Fixture::new(&format!("open_budget_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m").canonicalize().unwrap()
}

/// What the loading screen was saying on one frame.
#[derive(Debug)]
struct Said {
    screen: Screen,
    phase: LoadPhase,
    line: String,
    step: Option<(u32, u32, String)>,
}

fn said(app: &mut App) -> Said {
    let world = app.world_mut();
    let line = world.query_filtered::<&Text, With<bevy_gooey::Role>>().iter(world).map(|t| t.0.clone()).find(|t| t.starts_with(Stage::BuildEditor.label())).unwrap_or_default();
    let step = world.query::<&ProgressText>().iter(world).find_map(|t| t.step.clone());
    Said { screen: screen_of(app), phase: *app.world().resource::<LoadPhase>(), line, step }
}

/// Steps in real time until the editor is entered, recording what the screen said on every frame.
fn frames_until_the_editor(app: &mut App) -> Vec<Said> {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < Duration::from_secs(10) {
        app.update();
        let now = said(app);
        let done = now.screen == Screen::Editor;
        seen.push(now);
        if done {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the editor was never entered; the last frames said {:?}", &seen[seen.len().saturating_sub(3)..]);
}

#[test]
fn the_loading_screen_says_building_the_editor_for_a_few_frames_before_it_hands_over() {
    let root = project("announce");
    let mut app = loading_app("announce", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    let seen = frames_until_the_editor(&mut app);

    let building: Vec<&Said> = seen.iter().filter(|s| s.screen == Screen::Loading && s.line.starts_with(Stage::BuildEditor.label())).collect();
    assert!(
        building.len() >= MIN_DRAWN_FRAMES,
        "the screen says `{}` on at least {MIN_DRAWN_FRAMES} frames before the editor; it did on {}: {:?}",
        Stage::BuildEditor.label(),
        building.len(),
        seen.iter().map(|s| (s.screen, s.phase, s.line.as_str())).collect::<Vec<_>>()
    );
    assert!(building.iter().all(|s| s.phase != LoadPhase::Quiet), "it shows even for a project that opened at once: the stall that follows is certain");
    assert!(building.last().is_some_and(|s| s.line.contains(&root.file_name().unwrap().to_string_lossy().into_owned())), "and names the project");
    let last_loading = seen.iter().rposition(|s| s.screen == Screen::Loading).expect("the screen was on");
    assert!(seen[last_loading].line.starts_with(Stage::BuildEditor.label()), "the last frame drawn before the hand-over is the one that says it: {:?}", seen[last_loading]);
    assert_eq!(Stage::BuildEditor.index() + 1, Stage::ALL.len(), "the build is the last stage of the bar");
    assert_eq!(screen_of(&app), Screen::Editor);
    assert!(app.world().get_resource::<Entering>().is_none(), "the hand-over is done");
}

#[test]
fn the_bar_counts_the_build_as_its_last_stage() {
    let root = project("bar");
    let mut app = loading_app("bar", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    // Past the bar's threshold from the start, so the stage reads as "N of M" instead of a line under a spinner.
    app.insert_resource(LoadTiming { spinner_after: Duration::ZERO, progress_after: Duration::ZERO });
    let seen = frames_until_the_editor(&mut app);
    let steps: Vec<&(u32, u32, String)> = seen.iter().filter_map(|s| s.step.as_ref()).collect();
    let last = steps.iter().rev().find(|(_, _, label)| label == Stage::BuildEditor.label()).expect("the bar named the build");
    assert_eq!((last.0, last.1), (Stage::ALL.len() as u32, Stage::ALL.len() as u32), "the build is {} of {}", Stage::ALL.len(), Stage::ALL.len());
}

fn press(app: &mut App, text: &str) {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Children)>();
    let mut t = world.query::<(&Role, &Text)>();
    let button = q
        .iter(world)
        .find_map(|(e, kids)| kids.iter().find_map(|k| t.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == text).map(|_| e)))
        .unwrap_or_else(|| panic!("no button says `{text}`"));
    app.world_mut().commands().trigger(Activate { entity: button });
    app.update();
}

#[test]
fn open_anyway_hands_over_with_the_same_words_and_cancel_in_the_hand_over_returns_to_the_launcher() {
    // A terrain file that does not read, so the report card is the way in.
    let root = project("anyway");
    let kit = bevy_cauldron::project::Project::open(&root, None).unwrap_or_else(|e| panic!("{e}")).grimoire_dir;
    std::fs::create_dir_all(kit.join("terrain")).unwrap();
    std::fs::write(kit.join("terrain/m.yaml"), "{version: \"one\"}").unwrap();
    let mut app = loading_app("anyway", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    let start = Instant::now();
    while app.world().get_resource::<LoadReport>().is_none() && start.elapsed() < Duration::from_secs(10) {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    app.update();
    app.update();
    assert!(app.world().get_resource::<LoadReport>().is_some(), "the broken file is reported");

    press(&mut app, "Open anyway");
    app.update();
    let now = said(&mut app);
    assert_eq!(now.screen, Screen::Loading, "the hand-over takes a few frames: {now:?}");
    assert!(now.line.starts_with(Stage::BuildEditor.label()), "Open anyway says the build too: {now:?}");
    assert!(app.world().get_resource::<Entering>().is_some());

    // Cancel is still the way out until the editor is entered.
    press(&mut app, "Cancel");
    assert_eq!(screen_of(&app), Screen::Launcher, "Cancel in the hand-over is the launcher");
    app.update();
    assert!(app.world().get_resource::<Entering>().is_none(), "no hand-over is left waiting");
    assert!(app.world().get_resource::<Chosen>().is_none(), "and no door");
    for _ in 0..HANDOFF_FRAMES + 2 {
        app.update();
    }
    assert_eq!(screen_of(&app), Screen::Launcher, "a cancelled hand-over does not enter the editor later");
}
