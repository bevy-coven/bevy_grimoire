//! **A new project opens on a map you can edit at once** (#297).
//!
//! The Starter template copies the Prototype Kit out of the example and opens `first_map` with its pieces in the
//! palette. The Empty template opens the same map on a kit with no pieces. The dialog says which folder it will make,
//! offers Choose… for the place, and opens clean: what the home last refused is not carried into it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_cauldron::editor::PaletteRow;
use bevy_cauldron::launcher::{ClipboardOff, Launcher, LauncherPlugin};
use bevy_cauldron::loading::LoadingPlugin;
use bevy_cauldron::new_project::{self, FIRST_MAP, FolderLine, LocationField, NameField, NewProject, Template, TemplateChoice, TemplateNote};
use bevy_cauldron::picker::{PickFor, PickRequest, Picked};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::recent::RecentProjects;
use bevy_cauldron::screen::Screen;
use bevy_gooey::{Role, TextInputValue, Toast, ToastKind};

use crate::fixtures::Fixture;

/// How long a screen change or a load may take before the test calls it stuck.
const PATIENCE: Duration = Duration::from_secs(30);

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_new_project_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// The Projects home, with the loader that takes a created project to its map.
fn on_the_home(tag: &str, dir: &Path) -> App {
    let root = Fixture::new(&format!("new_project_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(dir.join("recent.yaml"))).insert_resource(ClipboardOff).add_plugins((LauncherPlugin, LoadingPlugin));
    // The glTF loader is registered when plugins finish, which a stepped app does not do on its own.
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
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

fn press(app: &mut App, text: &str) {
    let button = bevy_gooey::testing::find_by_label(app, text).unwrap_or_else(|e| panic!("{e:?}"));
    app.world_mut().commands().trigger(Activate { entity: button });
    for _ in 0..3 {
        app.update();
    }
}

fn type_into<M: Component>(app: &mut App, text: &str) {
    let field = app.world_mut().query_filtered::<Entity, With<M>>().single(app.world()).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().commands().trigger(ValueChange::<String> { source: field, value: text.to_owned(), is_final: false });
    for _ in 0..3 {
        app.update();
    }
}

fn line_of<M: Component>(app: &mut App) -> String {
    app.world_mut().query_filtered::<&Text, With<M>>().single(app.world()).unwrap_or_else(|e| panic!("{e}")).0.clone()
}

fn fill_in(app: &mut App, place: &Path, name: &str) {
    press(app, "New project");
    type_into::<LocationField>(app, &place.to_string_lossy());
    type_into::<NameField>(app, name);
}

/// The image files a `.glb` names by path (`images[].uri`) rather than embeds: the asset server reads each beside the mesh.
fn external_images(glb: &Path) -> Vec<String> {
    let bytes = std::fs::read(glb).unwrap_or_else(|e| panic!("{}: {e}", glb.display()));
    let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap_or_else(|e| panic!("{}: {e}", glb.display()));
    json["images"].as_array().into_iter().flatten().filter_map(|i| i["uri"].as_str()).filter(|uri| !uri.starts_with("data:")).map(str::to_owned).collect()
}

#[test]
fn a_starter_project_opens_on_first_map_with_its_pieces_in_the_palette() {
    let dir = scratch("starter");
    let mut app = on_the_home("starter", &dir);
    fill_in(&mut app, &dir, "Harbour Town");
    assert_eq!(line_of::<FolderLine>(&mut app), "Folder: harbour_town", "the dialog says which folder it will make");
    press(&mut app, "Create");

    let made = dir.join("harbour_town");
    until(&mut app, "the new project's map opens", |app| screen_of(app) == Screen::Editor);
    let open = app.world().resource::<OpenMap>();
    assert_eq!(open.map.name, FIRST_MAP, "it opens on first_map");
    assert_eq!(open.map.bounds, (10.0, 4.0, 10.0), "ten metres by ten, four tall");
    let project = app.world().resource::<Project>();
    assert_eq!(project.root, made.canonicalize().unwrap_or_else(|e| panic!("{e}")));
    assert!(!project.library.descriptors.is_empty(), "the Starter kit has pieces");
    assert_eq!(project.triangles.len(), project.library.descriptors.len());
    assert!(project.triangles.iter().all(|t| *t > 0), "every row's mesh path resolves after the copy: {:?}", project.triangles);
    assert!(made.join("assets/kenney_prototype-kit/License.txt").is_file(), "the licence travels with the meshes");
    for descriptor in &project.library.descriptors {
        let mesh = made.join("assets").join(descriptor.mesh.as_deref().unwrap_or_default());
        for image in external_images(&mesh) {
            assert!(mesh.parent().is_some_and(|dir| dir.join(&image).is_file()), "{} draws with {image}, which the copy must hold beside it", mesh.display());
        }
    }
    for _ in 0..6 {
        app.update();
    }
    let rows = app.world_mut().query::<&PaletteRow>().iter(app.world()).count();
    assert!(rows >= 1, "the palette lists at least one piece to place");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_project_opens_on_first_map_with_an_empty_palette() {
    let dir = scratch("empty");
    let mut app = on_the_home("empty", &dir);
    fill_in(&mut app, &dir, "blank");
    let choice = app.world_mut().query_filtered::<Entity, With<TemplateChoice>>().single(app.world()).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().commands().trigger(ValueChange::<usize> { source: choice, value: 1, is_final: true });
    app.update();
    assert_eq!(line_of::<TemplateNote>(&mut app), Template::Empty.describe(), "the line under the choice describes the selected template");
    press(&mut app, "Create");
    until(&mut app, "the new project's map opens", |app| screen_of(app) == Screen::Editor);
    let project = app.world().resource::<Project>();
    assert!(project.library.descriptors.is_empty(), "an Empty project has no pieces");
    assert_eq!(project.grimoire_dir.file_name().and_then(|n| n.to_str()), Some("main"), "its one kit is main");
    assert_eq!(app.world().resource::<OpenMap>().map.name, FIRST_MAP);
    let catalog = bevy_cauldron::catalog::Catalog::scan(&dir.join("blank")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(catalog.authoring.as_deref(), Some("main"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_dialog_opened_after_a_refused_folder_shows_no_problem() {
    let dir = scratch("refused");
    let plain = dir.join("plain");
    std::fs::create_dir_all(&plain).unwrap_or_else(|e| panic!("{e}"));
    let mut app = on_the_home("refused", &dir);

    // Open folder… on a folder that is not a project: the home answers with a toast.
    app.world_mut().write_message(Picked { for_what: PickFor::OpenFolder, paths: vec![plain.clone()] });
    for _ in 0..4 {
        app.update();
    }
    let alert = |app: &mut App| app.world_mut().query::<&Toast>().iter(app.world()).filter(|t| t.kind() == ToastKind::Alert).count();
    assert_eq!(alert(&mut app), 1, "the refusal is up");
    app.world_mut().resource_mut::<Launcher>().problem = Some(format!("{} is not a project", plain.display()));

    press(&mut app, "New project");
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(alert(&mut app), 0, "the dialog opens with no refusal toast");
    assert_eq!(app.world().resource::<Launcher>().problem, None);
    let mut texts = app.world_mut().query::<(&Role, &Text)>();
    let said: Vec<String> = texts.iter(app.world()).map(|(_, t)| t.0.clone()).filter(|t| t.contains("not a project") || t.contains("not a Cauldron project")).collect();
    assert!(said.is_empty(), "no problem text on screen: {said:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_location_starts_in_the_cauldron_projects_folder_and_choose_asks_the_chooser() {
    let form = NewProject::default();
    assert_eq!(form.location, new_project::default_location().to_string_lossy(), "the place is never filled from anywhere else");
    assert!(new_project::default_location().ends_with("Cauldron Projects"));
    assert_eq!(new_project::check(&NewProject { name: "x".into(), ..form }).location, None, "a default place that is not made yet is not a problem");

    let dir = scratch("choose");
    let mut app = on_the_home("choose", &dir);
    press(&mut app, "New project");
    #[derive(Resource, Default)]
    struct Asked(Vec<PickFor>);
    app.init_resource::<Asked>().add_systems(Update, |mut requests: MessageReader<PickRequest>, mut asked: ResMut<Asked>| asked.0.extend(requests.read().map(|r| r.0.clone())));
    press(&mut app, "Choose…");
    assert_eq!(app.world().resource::<Asked>().0, [PickFor::Location], "Choose… asks for a folder");

    let chosen = dir.join("elsewhere");
    app.world_mut().write_message(Picked { for_what: PickFor::Location, paths: vec![chosen.clone()] });
    for _ in 0..3 {
        app.update();
    }
    let shown = app.world_mut().query_filtered::<&TextInputValue, With<LocationField>>().single(app.world()).unwrap_or_else(|e| panic!("{e}")).0.clone();
    assert_eq!(shown, chosen.to_string_lossy(), "the chosen folder is the Location a person sees");
    let _ = std::fs::remove_dir_all(&dir);
}
