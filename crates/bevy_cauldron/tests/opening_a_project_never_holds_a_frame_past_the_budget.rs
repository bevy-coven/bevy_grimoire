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
use bevy_cauldron::loading::{Entering, HANDOFF_FRAMES, LoadPhase, LoadReport, LoadRequest, LoadTiming, LoadingPlugin, PREPARING_WORDS, PreparingBand, SettleBudget};
use bevy_cauldron::project::Stage;
use bevy_cauldron::screen::{Chosen, Screen};
use bevy_gooey::{ProgressText, Role};

use crate::fixtures::Fixture;

/// The fewest frames the words may be on screen before the hand-over: one to be laid out, one to be drawn.
const MIN_DRAWN_FRAMES: usize = 2;

/// How many editor frames are stepped after the open: far past the first-frame work, so the last of them are idle.
const FRAMES: usize = 40;

/// The slowest frame the window may be held for while a project opens, in milliseconds: a frame at 20 fps.
const FRAME_BUDGET_MS: f64 = 100.0;

/// How many of the editor's first frames may be over [`FRAME_BUDGET_MS`]: the ones that build its panels, strip and map.
const BUILD_STAGE_FRAMES: usize = 2;

/// The longest the build stage may hold the window, in milliseconds: six times what it took when this was set (105 to 135
/// ms headless, first frame, on an empty project, the fixture project and a 2000-placement map; see `examples/open_cost.rs`), so
/// that the gate's parallel load does not fail it. A real window draws as well, which is what the settle band is for.
const BUILD_STAGE_BUDGET_MS: f64 = 700.0;

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

/// The largest map to be had: the repository's own maps are a dozen placements (`assets` `bedroom`: 12; `games/sample` `courtyard`: 2), so
/// this is built, 2000 placements on a map 100 m on a side: far more than the town in Sanctuary's saves.
fn large_map(tag: &str) -> Fixture {
    let mut fixture = Fixture::new(tag).descriptor("bench", "props").side(100.0);
    for i in 0..2000 {
        fixture = fixture.place("bench", ((i % 50) as f32, (i / 50) as f32));
    }
    fixture
}

const CASES: [Case; 3] = [
    Case { name: "an empty project", fixture: empty },
    Case { name: "the fixture project", fixture: one_kit },
    Case { name: "a 2000-placement map", fixture: large_map },
];

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
        .insert_resource(LoadTiming { spinner_after: Duration::from_secs(60), progress_after: Duration::from_secs(600) })
        .insert_resource(SlowFrames { left: Some(0), pause: Duration::ZERO })
        .add_systems(Update, slow_frames.run_if(in_state(Screen::Editor)));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Loading);
    app
}

/// **A stand-in for what follows the hand-over** (the GPU compiling pipelines, thumbnails, the first frame's asset work): the
/// editor's frames take `pause` each, for `left` frames, or for ever when `left` is `None`. A headless test has no GPU, so this
/// is the slow system that proves the band waits for the frames to settle and cannot wait for ever.
#[derive(Resource)]
struct SlowFrames {
    left: Option<u32>,
    pause: Duration,
}

fn slow_frames(mut slow: ResMut<SlowFrames>) {
    match slow.left {
        Some(0) => {}
        Some(n) => {
            slow.left = Some(n - 1);
            std::thread::sleep(slow.pause);
        }
        None => std::thread::sleep(slow.pause),
    }
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

fn bands(app: &mut App) -> usize {
    app.world_mut().query_filtered::<Entity, With<PreparingBand>>().iter(app.world()).count()
}

/// Steps to the first frame the editor is the screen, and returns whether the band was already up on the last loading frame.
fn into_the_editor(app: &mut App) -> bool {
    let start = Instant::now();
    let mut band_before = false;
    while start.elapsed() < Duration::from_secs(10) {
        app.update();
        if screen_of(app) == Screen::Editor {
            return band_before;
        }
        band_before = bands(app) == 1;
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the editor was never entered");
}

#[test]
fn the_view_is_prepared_under_a_band_until_the_editors_frames_settle() {
    let root = project("settle");
    let mut app = loading_app("settle", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    // The stand-in: the first four editor frames take 80 ms each, as a pipeline compile does; then they are quiet.
    let slow_frames_count = 4;
    app.insert_resource(SettleBudget { frame: Duration::from_millis(150), quiet_frames: 5, cap: Duration::from_secs(60) })
        .insert_resource(SlowFrames { left: Some(slow_frames_count), pause: Duration::from_millis(300) });

    let up_before_the_stall = into_the_editor(&mut app);
    assert!(up_before_the_stall, "the band is already up on the last frame the loading screen draws, so it is on screen when the stall begins");
    assert_eq!(bands(&mut app), 1, "and stays on the first editor frame");
    let mut words = app.world_mut().query::<&Text>();
    assert!(words.iter(app.world()).any(|t| t.0 == PREPARING_WORDS), "and says `{PREPARING_WORDS}`");

    let mut with_band = 1;
    for _ in 0..40 {
        app.update();
        if bands(&mut app) == 0 {
            break;
        }
        with_band += 1;
    }
    assert_eq!(bands(&mut app), 0, "the band goes once the frames settle");
    // It stood through every slow frame and the quiet frames that proved them over, and not much longer.
    let (slow, quiet) = (slow_frames_count as usize, 5);
    assert!(with_band >= slow + quiet - 1, "the band went after {with_band} frames: it stayed through {slow} slow frames and {quiet} quiet ones");
    assert!(with_band <= slow + quiet + 6, "and not much longer: {with_band} frames");
}

#[test]
fn the_band_cannot_stick_when_the_frames_never_settle() {
    let root = project("stick");
    let mut app = loading_app("stick", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    // The cap counts from the hand-over, so it includes the stall of the editor's own first frames (here about half a second).
    let cap = Duration::from_millis(3_000);
    app.insert_resource(SettleBudget { frame: Duration::from_millis(150), quiet_frames: 5, cap })
        .insert_resource(SlowFrames { left: None, pause: Duration::from_millis(250) });
    into_the_editor(&mut app);
    let entered = Instant::now();
    let mut frames = 0;
    while bands(&mut app) > 0 && entered.elapsed() < Duration::from_secs(10) {
        app.update();
        frames += 1;
    }
    let stayed = entered.elapsed();
    assert_eq!(bands(&mut app), 0, "the band is taken down at the cap though every frame is slow");
    assert!(frames >= 5, "it stood through slow frames ({frames}), and was not taken down by a quiet one");
    assert!(stayed <= cap + Duration::from_secs(1), "it stayed {stayed:?} against a cap of {cap:?}");
}

#[test]
fn leaving_the_editor_takes_the_band_with_it() {
    let root = project("leave");
    let mut app = loading_app("leave", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    app.insert_resource(SlowFrames { left: None, pause: Duration::from_millis(70) });
    into_the_editor(&mut app);
    assert_eq!(bands(&mut app), 1);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    app.update();
    app.update();
    assert_eq!(bands(&mut app), 0, "a band does not outlive the editor it was preparing");
}

/// The slowest the shipped project's first editor frame may be, in milliseconds: six times what it took when this was set
/// (299 ms headless, M1 Max, `examples/open_cost.rs`: 90 descriptors, 2 kits).
const SHIPPED_BUDGET_MS: f64 = 1_800.0;

/// **The shipped project is the realistic one** (the repository's own maps are a dozen placements, so it is its libraries that
/// are large): `assets/` with `bedroom`. Like the other contract tests that read the shipped project, it follows the project it
/// ships, and its budget is three times the first frame measured.
#[test]
fn the_shipped_project_holds_the_editors_first_frame_inside_three_times_its_measured_cost() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("{e}"));
    let takes: Vec<Vec<f64>> = (0..TAKES)
        .map(|_| {
            let mut app = bevy_cauldron::harness::build_headless(&root, "bedroom", None).unwrap_or_else(|e| panic!("the shipped project: {e}"));
            (0..FRAMES)
                .map(|_| {
                    let start = Instant::now();
                    app.update();
                    start.elapsed().as_secs_f64() * 1000.0
                })
                .collect()
        })
        .collect();
    let frames: Vec<f64> = (0..FRAMES).map(|i| takes.iter().map(|t| t[i]).fold(f64::MAX, f64::min)).collect();
    let longest = frames.iter().copied().fold(0.0, f64::max);
    let slow: Vec<usize> = frames.iter().enumerate().filter(|(_, ms)| **ms > FRAME_BUDGET_MS).map(|(i, _)| i).collect();
    eprintln!("the shipped project: longest frame {longest:.0} ms; frames over {FRAME_BUDGET_MS} ms: {slow:?}");
    assert!(longest <= SHIPPED_BUDGET_MS, "the shipped project's first editor frame took {longest:.0} ms, over {SHIPPED_BUDGET_MS} ms");
    assert!(slow.last().is_none_or(|last| *last < BUILD_STAGE_FRAMES), "frames {slow:?} are over {FRAME_BUDGET_MS} ms; only the first {BUILD_STAGE_FRAMES} may be: {frames:.0?}");
}

#[test]
fn nothing_of_the_preparing_band_takes_a_click() {
    let root = project("pick");
    let mut app = loading_app("pick", vec![root.to_string_lossy().into_owned(), "m".to_owned()]);
    app.insert_resource(SlowFrames { left: None, pause: Duration::from_millis(5) });
    into_the_editor(&mut app);
    let world = app.world_mut();
    let band = world.query_filtered::<Entity, With<PreparingBand>>().single(world).expect("the band is up");
    let mut pending = vec![band];
    let mut seen = 0;
    while let Some(entity) = pending.pop() {
        seen += 1;
        let pickable = world.get::<Pickable>(entity).copied();
        assert_eq!(pickable, Some(Pickable::IGNORE), "a node of the band ({entity:?}) could take a click: {pickable:?}");
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    assert!(seen >= 4, "the band, its panel, its spinner and its words were all looked at ({seen})");
}
