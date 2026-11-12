//! **The sample game is a game made of YAML and Rhai, and a test plays it** (GD-10, #372; GD-08, #370).
//!
//! `games/sample/` holds a courtyard map, a lever, a gate, a crown behind the gate, a guard on a beat, and a
//! referee script that says `win`. Its Rust is `src/main.rs`, which is one line of app and nothing more
//! ([`the_games_only_source_file_is_a_short_main`]).
//!
//! **The playthrough is scripted input**: the hero is walked with the keyboard resource (`D` held), onto the lever pad,
//! through the open gate and onto the crown, and the test reads the events the scripts emit. **The same playthrough runs
//! from the YAML and from the bake** (`cauldron bake`), and both reach `win` with the same number of entities after the
//! first frame.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use bevy::prelude::*;
use bevy_grimoire::game::{GameDataPlugin, StartRefused};
use bevy_grimoire_core::{bake, load};
use bevy_grimoire_rhai::ScriptEvent;

/// **The most lines `src/main.rs` may have**: the game is data, and a game whose Rust grows past this has stopped being one.
const SAMPLE_MAIN_MAX_LINES: usize = 10;

/// **The step of one frame**, in seconds: the hero walks `4 m/s`, so a held key is a metre a frame.
const FRAME_S: f32 = 0.25;

/// **The most frames a playthrough may take** before it is a failure. The crown is twelve metres from the start.
const MAX_FRAMES: usize = 60;

/// **The load source is the process's**, so the tests that choose one take turns.
static SOURCE: Mutex<()> = Mutex::new(());

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// **A copy of the game's data**, in a scratch folder of this test's own.
fn copy_of_the_game(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("grimoire-sample-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["assets", "game", "scripts"] {
        copy_tree(&sample().join(dir), &root.join(dir));
    }
    root
}

fn copy_tree(from: &Path, into: &Path) {
    std::fs::create_dir_all(into).unwrap_or_else(|e| panic!("{into:?}: {e}"));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| panic!("{from:?}: {e}")).flatten() {
        let (source, target) = (entry.path(), into.join(entry.file_name()));
        if source.is_dir() {
            copy_tree(&source, &target);
        } else {
            std::fs::copy(&source, &target).unwrap_or_else(|e| panic!("{source:?}: {e}"));
        }
    }
}

/// **The names of every script event seen**, in order.
#[derive(Resource, Default)]
struct Seen(Vec<String>);

fn collect(mut events: MessageReader<ScriptEvent>, mut seen: ResMut<Seen>) {
    for event in events.read() {
        seen.0.push(event.name.clone());
    }
}

fn game(root: &Path, bake: Option<PathBuf>) -> App {
    let mut app = App::new();
    app.init_resource::<Time>().init_resource::<ButtonInput<KeyCode>>().init_resource::<Seen>();
    app.add_plugins(bevy::transform::TransformPlugin).add_plugins(GameDataPlugin { root: root.to_path_buf(), bake });
    app.add_systems(Last, collect);
    app
}

fn frame(app: &mut App) {
    app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs_f32(FRAME_S));
    app.update();
    app.finish();
    app.cleanup();
}

/// **Play to the win**: `D` held. Returns the events seen and the number of entities after the first frame.
fn play(root: &Path, bake: Option<PathBuf>) -> (Vec<String>, usize) {
    let mut app = game(root, bake);
    frame(&mut app);
    let entities = app.world().iter_entities().count();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyD);
    for _ in 0..MAX_FRAMES {
        frame(&mut app);
        if app.world().resource::<Seen>().0.iter().any(|name| name == "win") {
            break;
        }
    }
    (app.world().resource::<Seen>().0.clone(), entities)
}

/// **The playthrough reaches `win` from the YAML and from the bake, and the world is the same size.**
#[test]
fn the_sample_game_is_played_to_its_win_from_the_yaml_and_from_the_bake() {
    let _turn = SOURCE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    load::use_yaml();
    let root = copy_of_the_game("play");

    let (yaml_events, yaml_entities) = play(&root, None);
    assert!(yaml_events.iter().any(|e| e == "through_gate"), "the hero came through the open gate: {yaml_events:?}");
    assert!(yaml_events.iter().any(|e| e == "win"), "the YAML run reaches the win: {yaml_events:?}");
    assert!(yaml_entities >= 7, "the courtyard's seven entities stand after the first frame: {yaml_entities}");

    let out = root.join("baked");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    // **A release has no YAML to read for the data it bakes**: the folder it loads from is the bake's.
    let (baked_events, baked_entities) = play(&root, Some(out));
    assert!(load::is_baked(), "the baked run read the bake");
    load::use_yaml();
    assert!(baked_events.iter().any(|e| e == "win"), "the baked run reaches the win: {baked_events:?}");
    assert_eq!(baked_entities, yaml_entities, "the first frame has the same entities from the bake as from the YAML");
}

/// **After one YAML byte changes, the baked game refuses to start**, naming the file.
#[test]
fn a_changed_yaml_byte_stops_the_baked_game_naming_the_file() {
    let _turn = SOURCE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    load::use_yaml();
    let root = copy_of_the_game("stale");
    let out = root.join("baked");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    let entities = root.join("game/courtyard.entities.yaml");
    let mut text = std::fs::read_to_string(&entities).unwrap_or_else(|e| panic!("{e}"));
    text.push('#');
    std::fs::write(&entities, text).unwrap_or_else(|e| panic!("{e}"));

    let app = game(&root, Some(out));
    let refused = app.world().get_resource::<StartRefused>().unwrap_or_else(|| panic!("the game refused to start"));
    assert!(refused.0.contains("courtyard.entities.yaml"), "the file is named: {}", refused.0);
    assert!(!load::is_baked(), "and nothing fell back to reading the bake it did not trust");
    load::use_yaml();
}

/// **The game's only source file is a short `main.rs`**: no game-specific Rust, ever.
#[test]
fn the_games_only_source_file_is_a_short_main() {
    let src = sample().join("src");
    let files: Vec<String> =
        std::fs::read_dir(&src).unwrap_or_else(|e| panic!("{e}")).flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(files, ["main.rs"], "the crate's one source file");
    let lines = std::fs::read_to_string(src.join("main.rs")).unwrap_or_else(|e| panic!("{e}")).lines().count();
    assert!(lines <= SAMPLE_MAIN_MAX_LINES, "main.rs is {lines} lines, and a game made of data keeps it to {SAMPLE_MAIN_MAX_LINES}");
}
