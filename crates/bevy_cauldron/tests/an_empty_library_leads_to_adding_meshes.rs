//! **An empty library leads to adding meshes** (#301).
//!
//! An Empty project's PLACE list says it has no pieces and offers Add meshes…; the map is left clear (`feedback-1794230462401`).
//! Meshes put in through the one function — a `.glb` dropped on the window, the file chooser's answer — are copied into the
//! authoring kit, imported with `piece_import`, and the first is armed: the palette then has the piece as its one row, and it
//! is the brush.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_cauldron::add_meshes::{self, ACTION, AddMeshesButton, NO_PIECES};
use bevy_cauldron::editor::{EditorState, PaletteRow};
use bevy_cauldron::empty_state::EmptyMapPrompt;
use bevy_cauldron::new_project::{self, NewProject, Template};
use bevy_cauldron::picker::{PickFor, PickRequest, Picked};
use bevy_cauldron::project::Project;
use bevy_gooey::testing::click;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_add_meshes_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// A real mesh to add: one of the Prototype Kit's, copied to `dir` as `name`, so it lies outside the project.
fn a_mesh_outside(dir: &Path, name: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/kenney_prototype-kit/Models/GLB format/crate.glb");
    let to = dir.join(name);
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
    std::fs::copy(&from, &to).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    to
}

/// The editor on a new Empty project's first map.
fn on_an_empty_project(tag: &str, dir: &Path) -> (App, PathBuf) {
    let form = NewProject { name: tag.to_owned(), location: dir.to_string_lossy().into_owned(), template: Template::Empty };
    let root = new_project::create(&form).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_cauldron::harness::build_headless(&root, new_project::FIRST_MAP, None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    (app, root)
}

fn steps(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn prompt(app: &mut App) -> Option<EmptyMapPrompt> {
    app.world_mut().query::<&EmptyMapPrompt>().iter(app.world()).next().copied()
}

fn texts(app: &mut App) -> Vec<String> {
    app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect()
}

fn rows(app: &mut App) -> usize {
    app.world_mut().query::<&PaletteRow>().iter(app.world()).count()
}

fn brush_id(app: &App) -> Option<String> {
    let ix = app.world().resource::<EditorState>().brush?;
    app.world().resource::<Project>().library.descriptors.get(ix).map(|d| d.id.clone())
}

#[test]
fn an_empty_project_says_it_has_no_pieces_and_offers_to_add_meshes_in_place_and_leaves_the_map_clear() {
    let dir = scratch("says");
    let (mut app, _) = on_an_empty_project("blank", &dir);
    assert_eq!(prompt(&mut app), None, "the map draws no empty-state prompt when the library has no pieces to place");
    let said = texts(&mut app);
    assert!(said.iter().filter(|t| t.as_str() == NO_PIECES).count() == 1, "the PLACE list alone says {NO_PIECES:?}: {said:?}");
    // The menu bar's File → Add meshes… (#299) says it too, so the count is of the *buttons* the empty states draw: the PLACE list's one.
    let buttons: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<AddMeshesButton>>().iter(app.world()).collect();
    assert_eq!(buttons.len(), 1, "the PLACE list offers the one button");
    for button in buttons {
        let says = app.world().get::<Children>(button).is_some_and(|kids| kids.iter().any(|k| app.world().get::<Text>(k).is_some_and(|t| t.0 == ACTION)));
        assert!(says, "the button says {ACTION:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_mesh_dropped_on_the_window_becomes_the_one_row_of_the_palette_and_the_brush() {
    let dir = scratch("drop");
    let (mut app, root) = on_an_empty_project("blank", &dir);
    let mesh = a_mesh_outside(&dir.join("downloads"), "crate.glb");
    assert_eq!(rows(&mut app), 0);

    app.world_mut().write_message(FileDragAndDrop::DroppedFile { window: Entity::PLACEHOLDER, path_buf: mesh.clone() });
    steps(&mut app, 8);

    assert!(root.join("assets/main/crate.glb").is_file(), "the file is copied into the authoring kit's folder");
    assert!(mesh.is_file(), "and the person's own file is left where it was");
    assert_eq!(app.world().resource::<Project>().library.descriptors.len(), 1, "piece_import put it in the library");
    assert_eq!(rows(&mut app), 1, "the palette has one row");
    assert_eq!(brush_id(&app).as_deref(), Some("crate"), "and it is the brush");
    assert_eq!(prompt(&mut app), Some(EmptyMapPrompt::NoPlacements), "the map now says to place a piece, not to add meshes");
    assert!(!texts(&mut app).iter().any(|t| t == NO_PIECES));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_that_is_taken_gets_a_two_before_its_extension() {
    let dir = scratch("clash");
    let (mut app, root) = on_an_empty_project("blank", &dir);
    let first = a_mesh_outside(&dir.join("one"), "crate.glb");
    let second = a_mesh_outside(&dir.join("two"), "crate.glb");

    let mut project = app.world_mut().resource_mut::<Project>();
    let added = add_meshes::add_meshes(&mut project, &[first, second]);
    assert!(added.refused.is_empty(), "{:?}", added.refused);
    let ids: Vec<&str> = added.pieces.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["crate", "crate_2"], "the second is its own piece");
    assert!(root.join("assets/main/crate.glb").is_file() && root.join("assets/main/crate 2.glb").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gltf_is_refused_by_name_and_copies_nothing() {
    let dir = scratch("gltf");
    let (mut app, root) = on_an_empty_project("blank", &dir);
    let gltf = dir.join("downloads").join("hut.gltf");
    std::fs::create_dir_all(gltf.parent().unwrap_or_else(|| panic!("a parent"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&gltf, "{}").unwrap_or_else(|e| panic!("{e}"));

    app.world_mut().write_message(FileDragAndDrop::DroppedFile { window: Entity::PLACEHOLDER, path_buf: gltf });
    steps(&mut app, 4);

    let problem = app.world().resource::<EditorState>().status.problem_text();
    assert!(problem.contains("hut.gltf") && problem.contains(".glb"), "the refusal names the file and what to export: {problem}");
    assert!(!root.join("assets/main/hut.gltf").exists(), "nothing was copied");
    assert_eq!(rows(&mut app), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_choosers_answer_adds_meshes_and_the_button_asks_the_chooser() {
    let dir = scratch("picked");
    let (mut app, _) = on_an_empty_project("blank", &dir);
    #[derive(Resource, Default)]
    struct Asked(Vec<PickFor>);
    app.init_resource::<Asked>().add_systems(Update, |mut requests: MessageReader<PickRequest>, mut asked: ResMut<Asked>| asked.0.extend(requests.read().map(|r| r.0.clone())));
    let button = app.world_mut().query_filtered::<Entity, With<AddMeshesButton>>().iter(app.world()).next().unwrap_or_else(|| panic!("an Add meshes… button"));
    click(&mut app, button);
    steps(&mut app, 3);
    assert_eq!(app.world().resource::<Asked>().0, [PickFor::AddMeshes], "a press asks the system's chooser for meshes");

    let mesh = a_mesh_outside(&dir.join("chosen"), "barrel.glb");
    app.world_mut().write_message(Picked { for_what: PickFor::AddMeshes, paths: vec![mesh] });
    steps(&mut app, 8);
    assert_eq!(rows(&mut app), 1);
    assert_eq!(brush_id(&app).as_deref(), Some("barrel"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_kits_meshes_view_of_an_empty_project_says_so_and_offers_the_same_button() {
    let dir = scratch("meshes_view");
    let form = NewProject { name: "blank".to_owned(), location: dir.to_string_lossy().into_owned(), template: Template::Empty };
    let root = new_project::create(&form).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_cauldron::harness::build_headless_at(&root, new_project::FIRST_MAP, None, bevy_cauldron::tiles::Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    steps(&mut app, 8);
    let said = texts(&mut app);
    assert!(said.iter().any(|t| t == add_meshes::NO_MESHES), "the view says {:?}: {said:?}", add_meshes::NO_MESHES);
    assert!(!said.iter().any(|t| t.contains("already in the library")), "and no longer claims every mesh is in the library");
    // The button stands under the sentence, in the same list.
    let world = app.world_mut();
    let buttons: Vec<Entity> = world.query_filtered::<Entity, With<AddMeshesButton>>().iter(world).collect();
    let mut texts_in = world.query::<&Text>();
    let beside_the_sentence = buttons.iter().any(|b| {
        let Some(parent) = world.get::<ChildOf>(*b).map(|c| c.parent()) else { return false };
        world.get::<Children>(parent).is_some_and(|kids| kids.iter().any(|k| texts_in.get(world, k).is_ok_and(|t| t.0 == add_meshes::NO_MESHES)))
    });
    assert!(beside_the_sentence, "an {ACTION:?} button stands beside {:?}", add_meshes::NO_MESHES);
    let _ = std::fs::remove_dir_all(&dir);
}
