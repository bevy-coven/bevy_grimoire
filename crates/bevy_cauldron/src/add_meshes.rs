//! **An empty library leads to adding meshes** (#301).
//!
//! A project with no pieces has an empty palette, and the way out has to be on the screen: the map's empty state, the PLACE
//! list and the kit's Meshes view each say so and offer **Add meshes…** ([`AddMeshesButton`]). One function does the work for
//! every way in — the button's file chooser ([`crate::picker`]), a `.glb` dropped on the window, and a test:
//!
//! 1. each file is copied into `assets/<authoring kit>/` ([`bevy_grimoire_core::atomic_file::write_new`]; a name that is
//!    taken gets ` 2` before the extension, then ` 3`), unless it already lies inside the project's assets, where it is used
//!    where it is;
//! 2. the copy is imported with `piece_import`, the operation an agent calls, so the piece is measured, logged and undoable;
//! 3. the first piece added is armed, so the next click on the floor places it.
//!
//! A mesh that cannot be imported (a refusal from `piece_import`) takes its copy away again: the file was put there by this call
//! and nothing else knows it. A `.gltf` is refused by name before anything is copied: it is a file plus its `.bin`, and
//! `piece_import` reads one `.glb`.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::FileDragAndDrop;

use crate::editor::EditorState;
use crate::picker::{PickFor, PickRequest, Picked};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;

/// The button's label, wherever it stands.
pub const ACTION: &str = "Add meshes…";

/// What the map's empty state and the PLACE list say when the library has no pieces.
pub const NO_PIECES: &str = "This project has no pieces yet";

/// What the kit's Meshes view says when the project has no meshes and nothing to import.
pub const NO_MESHES: &str = "No meshes in this project yet";

/// Where meshes go when the project's authoring kit is the project's own folder rather than a kit inside it.
const FALLBACK_FOLDER: &str = "meshes";

/// The files a person wants in the project, by any way in: the chooser, a drop, a test.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct AddMeshes(pub Vec<PathBuf>);

/// **Add meshes…**, on a standing prompt, in the PLACE list or in the Meshes view.
#[derive(Component)]
pub struct AddMeshesButton;

/// What one call added and what it refused, each refusal as a sentence.
#[derive(Debug, Default, PartialEq)]
pub struct Added {
    /// The pieces now in the library, in file order, with the log line each import wrote.
    pub pieces: Vec<Imported>,
    pub refused: Vec<String>,
}

/// One piece an import made.
#[derive(Debug, PartialEq, Eq)]
pub struct Imported {
    pub id: String,
    pub summary: String,
    /// The operation-log line of the import: what Cmd+Z takes back.
    pub line: String,
}

/// Whether a file is one this feature takes: a `.glb`, or a `.gltf` it will explain it cannot import.
pub fn is_mesh_file(path: &Path) -> bool {
    extension(path).is_some_and(|e| e == "glb" || e == "gltf")
}

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

/// The kit's folder under `assets/`: the authoring kit's own name.
fn kit_folder(project: &Project) -> String {
    if project.grimoire_dir == project.project_dir {
        return FALLBACK_FOLDER.to_owned();
    }
    project.grimoire_dir.file_name().map_or_else(|| FALLBACK_FOLDER.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// **Puts `files` in the project's library**, adopting the library each import wrote. One file's refusal never stops the others.
pub fn add_meshes(project: &mut Project, files: &[PathBuf]) -> Added {
    let mut out = Added::default();
    for file in files {
        match add_one(project, file) {
            Ok(piece) => out.pieces.push(piece),
            Err(e) => out.refused.push(e),
        }
    }
    out
}

fn add_one(project: &mut Project, file: &Path) -> Result<Imported, String> {
    let shown = file.file_name().map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned());
    match extension(file).as_deref() {
        Some("glb") => {}
        Some("gltf") => return Err(format!("{shown} is a .gltf, a file with a .bin beside it. Export it as one .glb and add that")),
        _ => return Err(format!("{shown} is not a .glb mesh")),
    }
    let (rel, copy) = place_in_project(project, file, &shown)?;
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let applied = crate::ops::dispatch(&mut ctx, "piece_import", &serde_json::json!({ "glb": rel }));
    match applied {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            if let Some(fresh) = ctx.project.take() {
                crate::editor::adopt_library(project, &fresh);
            }
            let id = outcome.after.as_ref().and_then(|a| a["id"].as_str()).map(str::to_owned).ok_or_else(|| format!("{shown} was added but its id was not answered"))?;
            Ok(Imported { id, summary: outcome.summary, line: operation_id })
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            if let Some(copy) = copy {
                // Made a moment ago by this call: nothing else knows it.
                let _ = std::fs::remove_file(copy);
            }
            Err(format!("{shown} was not added: {}", e.0))
        }
    }
}

/// The path `piece_import` is given for `file` (relative to the project), and the copy this call made, if it made one.
fn place_in_project(project: &Project, file: &Path, shown: &str) -> Result<(String, Option<PathBuf>), String> {
    let assets = project.root.join("assets");
    std::fs::create_dir_all(&assets).map_err(|e| format!("{shown} was not added: {} cannot be made ({e})", assets.display()))?;
    let assets = assets.canonicalize().map_err(|e| format!("{shown} was not added: {} cannot be read ({e})", assets.display()))?;
    let source = file.canonicalize().map_err(|e| format!("{shown} was not added: it cannot be read ({e})"))?;
    if let Ok(inside) = source.strip_prefix(&assets) {
        return Ok((relative_to_project(inside), None));
    }
    let folder = assets.join(kit_folder(project));
    std::fs::create_dir_all(&folder).map_err(|e| format!("{shown} was not added: {} cannot be made ({e})", folder.display()))?;
    let bytes = std::fs::read(&source).map_err(|e| format!("{shown} was not added: it cannot be read ({e})"))?;
    let stem = source.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    // `write_new` refuses a taken name in the kernel, so two additions at once cannot take the same one.
    for serial in 1u32.. {
        let name = if serial == 1 { format!("{stem}.glb") } else { format!("{stem} {serial}.glb") };
        let target = folder.join(&name);
        match bevy_grimoire_core::atomic_file::write_new(&target, &bytes) {
            Ok(()) => {
                let inside = target.strip_prefix(&assets).map_err(|e| e.to_string())?;
                return Ok((relative_to_project(inside), Some(target)));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("{shown} was not added: {} cannot be written ({e})", target.display())),
        }
    }
    Err(format!("{shown} was not added: every name for it is taken"))
}

/// `assets/<inside>` with `/` separators, the form `piece_import` takes.
fn relative_to_project(inside: &Path) -> String {
    let parts: Vec<String> = inside.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    format!("assets/{}", parts.join("/"))
}

pub struct AddMeshesPlugin;

impl Plugin for AddMeshesPlugin {
    fn build(&self, app: &mut App) {
        // The chooser itself is `LauncherPlugin`'s (`picker::PickerPlugin`); the editor only asks it and reads its answers, so a
        // stepped editor without the launcher has the messages and no chooser.
        app.add_message::<PickRequest>().add_message::<Picked>().add_message::<AddMeshes>().add_observer(ask_for_files).add_systems(
            Update,
            (take_the_picked, take_the_dropped, add_what_was_asked).chain().run_if(in_state(Screen::Editor)),
        );
    }
}

/// A press on any **Add meshes…** opens the system's file chooser.
fn ask_for_files(activate: On<Activate>, buttons: Query<(), With<AddMeshesButton>>, mut out: MessageWriter<PickRequest>) {
    if buttons.contains(activate.entity) {
        out.write(PickRequest(PickFor::AddMeshes));
    }
}

fn take_the_picked(mut picked: MessageReader<Picked>, mut out: MessageWriter<AddMeshes>) {
    for Picked { for_what, paths } in picked.read() {
        if *for_what == PickFor::AddMeshes {
            out.write(AddMeshes(paths.clone()));
        }
    }
}

/// A `.glb` or `.gltf` dropped on the window is added like a chosen one; other files are not this feature's (a dropped image is a reference).
fn take_the_dropped(mut drops: MessageReader<FileDragAndDrop>, mut out: MessageWriter<AddMeshes>) {
    let files: Vec<PathBuf> = drops
        .read()
        .filter_map(|drop| match drop {
            FileDragAndDrop::DroppedFile { path_buf, .. } if is_mesh_file(path_buf) => Some(path_buf.clone()),
            _ => None,
        })
        .collect();
    if !files.is_empty() {
        out.write(AddMeshes(files));
    }
}

fn add_what_was_asked(
    mut requests: MessageReader<AddMeshes>,
    project: Option<ResMut<Project>>,
    open: Option<Res<OpenMap>>,
    mut state: ResMut<EditorState>,
) {
    let Some(mut project) = project else {
        requests.clear();
        return;
    };
    for AddMeshes(files) in requests.read() {
        let added = add_meshes(&mut project, files);
        for piece in &added.pieces {
            state.record_logged(piece.line.clone(), piece.summary.clone(), false);
        }
        if let Some(first) = added.pieces.first() {
            crate::editor::arm_piece(&mut state, &project, open.as_deref(), &first.id);
            let said = match added.pieces.as_slice() {
                [one] => format!("added {} to the library", one.id),
                many => format!("added {} pieces to the library", many.len()),
            };
            state.status.note(said);
        }
        if !added.refused.is_empty() {
            state.status.problem(added.refused.join("; "));
        }
    }
}
