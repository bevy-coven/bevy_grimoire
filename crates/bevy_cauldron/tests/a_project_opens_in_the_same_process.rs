//! **A project opens in the process that is already running** (#294).
//!
//! Two projects each carry `assets/a.glb`, with different triangle counts. Opening A, going home and
//! opening B must show B's mesh: the asset server's root moved with the project, and nothing of A's
//! outlived the door that loaded it. The `App` is never exited across the switch.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::loading::LoadingPlugin;
use bevy_cauldron::project::Project;
use bevy_cauldron::project_assets::{ProjectRoot, open_project};
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

/// How long a screen change or a load may take before the test calls it stuck.
const PATIENCE: Duration = Duration::from_secs(20);

/// Triangles in project A's mesh, and in B's. Chosen so that neither equals a mesh the editor builds for itself.
const TRIANGLES_A: usize = 17;
const TRIANGLES_B: usize = 23;

/// A binary glTF of one mesh with `triangles` unindexed triangles, laid out as `bevy_grimoire_core::glb` reads one.
fn glb(triangles: usize) -> Vec<u8> {
    let count = triangles * 3;
    let mut bin = Vec::new();
    for i in 0..count {
        for c in [i as f32 * 0.01, (i % 3) as f32, 0.0f32] {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"accessors":[{{"type":"VEC3","componentType":5126,"count":{count},"bufferView":0,
            "min":[0.0,0.0,0.0],"max":[{:.4},2.0,0.0]}}],
            "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{}}}],
            "buffers":[{{"byteLength":{}}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}}}]}}],
            "nodes":[{{"mesh":0}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#,
        (count - 1) as f32 * 0.01,
        bin.len(),
        bin.len()
    );
    let mut j = json.into_bytes();
    while j.len() % 4 != 0 {
        j.push(b' ');
    }
    let total = 12 + 8 + j.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&j);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

/// A project with one piece, `bench`, placed on map `m`, whose mesh — and `assets/a.glb` — has `triangles` triangles.
fn project(tag: &str, triangles: usize) -> PathBuf {
    let root = Fixture::new(&format!("same_process_{tag}_{}", std::process::id())).descriptor("bench", "props").place("bench", (2.0, 2.0)).build("m");
    let bytes = glb(triangles);
    std::fs::write(root.join("assets/props/bench.glb"), &bytes).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/a.glb"), bytes).unwrap_or_else(|e| panic!("{e}"));
    root.canonicalize().unwrap_or_else(|e| panic!("{e}"))
}

fn editor_on(root: &Path) -> App {
    let mut app = bevy_cauldron::harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins((LauncherPlugin, LoadingPlugin));
    // The glTF loader is registered when plugins finish, which a stepped app does not do on its own.
    app.finish();
    app.cleanup();
    app
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

/// Steps the app until `done`, in real time, because a project opens on a thread and a mesh loads on the task pool.
fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed() < PATIENCE {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("never got there: {what} (screen {:?})", screen_of(app));
}

/// Every mesh alive, as its triangle count: the unindexed fixture meshes are three vertices a triangle.
fn triangle_counts(app: &App) -> Vec<usize> {
    app.world().resource::<Assets<Mesh>>().iter().map(|(_, m)| m.indices().map_or_else(|| m.count_vertices() / 3, |i| i.len() / 3)).collect()
}

/// Loads `a.glb`'s mesh by path and returns its triangle count, holding no handle afterwards.
fn load_a(app: &mut App) -> usize {
    let handle: Handle<Mesh> = app.world().resource::<AssetServer>().load("a.glb#Mesh0/Primitive0");
    until(app, "a.glb loads", |app| app.world().resource::<AssetServer>().is_loaded(&handle));
    let mesh = app.world().resource::<Assets<Mesh>>().get(&handle).unwrap_or_else(|| panic!("a.glb has a mesh"));
    mesh.count_vertices() / 3
}

/// The editor on A has A's piece drawn: a mesh of A's size is alive.
fn open_a_and_see_its_mesh() -> (App, PathBuf, PathBuf) {
    let (a, b) = (project("a", TRIANGLES_A), project("b", TRIANGLES_B));
    let mut app = editor_on(&a);
    until(&mut app, "A's placed piece loads", |app| triangle_counts(app).contains(&TRIANGLES_A));
    assert_eq!(load_a(&mut app), TRIANGLES_A, "a.glb is A's while A is open");
    (app, a, b)
}

fn assert_b_not_a(app: &mut App, b: &Path) {
    assert_eq!(screen_of(app), Screen::Editor);
    assert_eq!(app.world().resource::<Project>().root, b, "the editor is on B");
    assert_eq!(app.world().resource::<ProjectRoot>().current().as_deref(), Some(b), "and the asset server reads B");
    until(app, "B's placed piece loads", |app| triangle_counts(app).contains(&TRIANGLES_B));
    assert!(!triangle_counts(app).contains(&TRIANGLES_A), "nothing of A's mesh is still alive: {:?}", triangle_counts(app));
    assert_eq!(load_a(app), TRIANGLES_B, "a.glb is B's now");
    assert!(app.should_exit().is_none(), "the application never exited across the switch");
}

#[test]
fn a_project_opened_after_going_home_reads_its_own_meshes() {
    let (mut app, _a, b) = open_a_and_see_its_mesh();

    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    until(&mut app, "the window is home", |app| screen_of(app) == Screen::Launcher);
    assert!(app.world().get_resource::<Project>().is_none(), "going home dropped A's project");

    app.world_mut().commands().queue({
        let b = b.clone();
        move |world: &mut World| open_project(world, &b, Some("m")).unwrap_or_else(|e| panic!("{e}"))
    });
    until(&mut app, "B opens", |app| screen_of(app) == Screen::Editor);
    assert_b_not_a(&mut app, &b);
}

#[test]
fn a_project_opened_from_inside_the_editor_closes_the_door_first() {
    let (mut app, a, b) = open_a_and_see_its_mesh();

    let b_for_open = b.clone();
    app.world_mut().commands().queue(move |world: &mut World| {
        open_project(world, &b_for_open, Some("m")).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(world.resource::<ProjectRoot>().current(), Some(a), "the editor is still up, so the root has not moved under it");
    });
    until(&mut app, "B opens", |app| screen_of(app) == Screen::Editor && app.world().resource::<Project>().root == b);
    assert_b_not_a(&mut app, &b);
}
