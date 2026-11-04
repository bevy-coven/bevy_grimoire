//! **A project opens on the loading screen as named stages** (UI-20, #221): nothing under a threshold, then a
//! spinner, then a percent-done bar with "N of M: stage", then the editor; Cancel returns to the launcher with
//! the project's files untouched; a project that will not open says why and offers a way back.
//!
//! The thresholds are shrunk (`LoadTiming`) and one stage is slowed (`StageDelay`) so that the three phases are
//! seen in well under a second; the shipped ones are 1 s and 10 s.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::screen::Chosen;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::loading::{LoadFailed, LoadPhase, LoadReport, LoadRequest, LoadTiming, LoadingPlugin, ReportRow, StageDelay};
use bevy_cauldron::project::{Project, Stage};
use bevy_cauldron::screen::Screen;
use bevy_gooey::{ProgressText, Role};

use crate::fixtures::Fixture;

fn loading_app(tag: &str, request: Vec<String>, pause_ms: u64) -> App {
    let other = Fixture::new(&format!("load_host_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&other, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins((LauncherPlugin, LoadingPlugin))
        .insert_resource(LoadRequest(request))
        .insert_resource(StageDelay(Duration::from_millis(pause_ms)))
        .insert_resource(LoadTiming { spinner_after: Duration::from_millis(100), progress_after: Duration::from_millis(350) });
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Loading);
    app
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

/// Steps the app in real time until `done` or five seconds pass.
fn until(app: &mut App, mut done: impl FnMut(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the screen never got there: it is on {:?}, phase {:?}", screen_of(app), app.world().resource::<LoadPhase>());
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
    for _ in 0..3 {
        app.update();
    }
}

fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(dir).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn project(tag: &str) -> PathBuf {
    Fixture::new(&format!("load_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m").canonicalize().unwrap()
}

#[test]
fn a_slowed_stage_shows_nothing_then_a_spinner_then_progress_then_the_editor() {
    let root = project("slow");
    let mut app = loading_app("slow", vec![root.to_string_lossy().into_owned(), "m".to_owned()], 150);

    let mut phases = vec![LoadPhase::Quiet];
    let mut steps: Vec<(u32, u32, String)> = Vec::new();
    until(&mut app, |app| {
        let phase = *app.world().resource::<LoadPhase>();
        if phases.last() != Some(&phase) {
            phases.push(phase);
        }
        let mut q = app.world_mut().query::<&ProgressText>();
        if phase == LoadPhase::Progress
            && let Some(step) = q.iter(app.world()).find_map(|t| t.step.clone())
            && steps.last() != Some(&step)
        {
            steps.push(step);
        }
        screen_of(app) == Screen::Editor
    });

    assert_eq!(&phases[..3], [LoadPhase::Quiet, LoadPhase::Spinner, LoadPhase::Progress], "under the threshold nothing, then a spinner, then a bar");
    assert!(!steps.is_empty(), "the bar says which stage of how many");
    assert!(steps.iter().all(|(n, m, label)| *m == Stage::ALL.len() as u32 && (1..=*m).contains(n) && !label.is_empty()), "{steps:?}");
    assert_eq!(app.world().resource::<Project>().root, root, "and it opened the project that was asked for");
    assert!(app.world().get_resource::<Chosen>().is_none(), "the editor consumed the door");
}

#[test]
fn cancel_mid_load_returns_to_the_launcher_and_leaves_the_project_byte_identical() {
    let root = project("cancel");
    let before = files(&root);
    let mut app = loading_app("cancel", vec![root.to_string_lossy().into_owned(), "m".to_owned()], 300);

    until(&mut app, |app| *app.world().resource::<LoadPhase>() == LoadPhase::Spinner);
    assert_eq!(screen_of(&app), Screen::Loading, "still loading");
    press(&mut app, "Cancel");
    assert_eq!(screen_of(&app), Screen::Launcher, "Cancel is the launcher");

    // The open was reading on its thread; let it finish and see that nothing it did reaches the screen.
    std::thread::sleep(Duration::from_millis(1500));
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(screen_of(&app), Screen::Launcher, "a cancelled open does not arrive later");
    assert!(app.world().get_resource::<Chosen>().is_none(), "and leaves no door waiting");
    assert_eq!(files(&root), before, "every file of the project is byte-identical");
}

#[test]
fn a_project_that_will_not_open_says_why_on_the_screen_and_back_returns_to_the_launcher() {
    let nowhere = std::env::temp_dir().join(format!("grimoire_no_such_project_{}", std::process::id()));
    let mut app = loading_app("fail", vec![nowhere.to_string_lossy().into_owned(), "--door".to_owned(), "kit".to_owned()], 0);
    until(&mut app, |app| app.world().get_resource::<LoadFailed>().is_some());
    app.update();
    app.update();

    let why = app.world().resource::<LoadFailed>().0.clone();
    assert!(why.contains("cannot open") && why.contains(nowhere.to_str().unwrap()), "{why}");
    let mut q = app.world_mut().query::<(&Role, &Text)>();
    assert!(q.iter(app.world()).any(|(r, t)| *r == Role::Label && t.0 == why), "the reason is on the screen");
    assert_eq!(screen_of(&app), Screen::Loading, "it stays, rather than closing the window");

    press(&mut app, "Back");
    assert_eq!(screen_of(&app), Screen::Launcher);
}

/// A project with one terrain file that does not read, at the path a kit keeps its terrain.
fn project_with_a_broken_terrain_file(tag: &str) -> (PathBuf, String) {
    let root = project(tag);
    let kit = Project::open(&root, None).unwrap_or_else(|e| panic!("{e}")).grimoire_dir;
    std::fs::create_dir_all(kit.join("terrain")).unwrap();
    std::fs::write(kit.join("terrain/m.ron"), "(version: \"one\")").unwrap();
    let file = format!("{}/terrain/m.ron", kit.file_name().unwrap().to_string_lossy());
    (root, file)
}

fn rows(app: &mut App) -> Vec<String> {
    app.world_mut().query_filtered::<&Text, With<ReportRow>>().iter(app.world()).map(|t| t.0.clone()).collect()
}

#[test]
fn a_project_with_one_broken_terrain_file_shows_one_report_row_naming_the_file_and_field_and_open_anyway_enters_it() {
    let (root, file) = project_with_a_broken_terrain_file("report");
    let mut app = loading_app("report", vec![root.to_string_lossy().into_owned(), "m".to_owned()], 0);
    until(&mut app, |app| app.world().get_resource::<LoadReport>().is_some());
    app.update();
    app.update();

    assert_eq!(app.world().resource::<LoadReport>().0.len(), 1, "one file does not read");
    let rows = rows(&mut app);
    assert_eq!(rows.len(), 1, "one card row: {rows:?}");
    assert!(rows[0].contains(&file), "the row names the file: {}", rows[0]);
    assert!(rows[0].contains("(version)"), "and the field it is wrong at: {}", rows[0]);
    assert_eq!(screen_of(&app), Screen::Loading, "the card holds the project until the person says");
    assert!(app.world().get_resource::<Chosen>().is_some(), "the project is open and waiting");

    press(&mut app, "Open anyway");
    app.update();
    assert_eq!(screen_of(&app), Screen::Editor, "Open anyway enters the editor");
    assert_eq!(app.world().resource::<Project>().root, root);
    assert!(std::fs::read_to_string(root.join(format!("assets/grimoire/{file}"))).unwrap().contains("one"), "and the file is as it was");
}

#[test]
fn back_from_the_report_returns_to_the_launcher_with_no_door_left_open() {
    let (root, _) = project_with_a_broken_terrain_file("report_back");
    let mut app = loading_app("report_back", vec![root.to_string_lossy().into_owned(), "m".to_owned()], 0);
    until(&mut app, |app| app.world().get_resource::<LoadReport>().is_some());
    app.update();
    app.update();
    press(&mut app, "Back");
    assert_eq!(screen_of(&app), Screen::Launcher);
    assert!(app.world().get_resource::<Chosen>().is_none(), "the project that was held is let go");
}

#[test]
fn cancel_after_the_answer_was_taken_lets_the_opened_project_go() {
    let (root, _) = project_with_a_broken_terrain_file("report_cancel");
    let mut app = loading_app("report_cancel", vec![root.to_string_lossy().into_owned(), "m".to_owned()], 0);
    until(&mut app, |app| app.world().get_resource::<LoadReport>().is_some());
    assert!(app.world().get_resource::<Chosen>().is_some(), "the premise: the project is open and waiting");
    // The loading panel's own Cancel is still on the screen (hidden by the card), as it is for the frame the answer is taken in.
    press(&mut app, "Cancel");
    assert_eq!(screen_of(&app), Screen::Launcher);
    assert!(app.world().get_resource::<Chosen>().is_none(), "a cancelled open leaves no project waiting in the world");
}
