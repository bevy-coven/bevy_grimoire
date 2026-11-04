//! **The operating system's own file chooser** (#298): one way to ask a person for a folder or for files.
//!
//! Open folder…, Locate… (a project whose folder moved), a new project's Location, and Add meshes… all ask
//! the same way: write a [`PickRequest`], and the answer arrives as a [`Picked`] naming what the request was
//! for. A cancelled chooser answers nothing.
//!
//! # Why `rfd` with the `xdg-portal` backend
//!
//! A typed path was the first answer (`launcher.rs` before #295), because `rfd`'s default Linux backends pull
//! GTK into every build. Its `xdg-portal` backend does not: it talks D-Bus to the desktop's own chooser (the
//! one a Flatpak or a GNOME, KDE or Hyprland session already has) through `libdbus`, loaded when a chooser is
//! asked for, and falls back to `zenity`. macOS and Windows use the system's own panels and need no feature.
//!
//! # The chooser never blocks the frame
//!
//! `rfd::AsyncFileDialog` runs on Bevy's [`IoTaskPool`]; a system polls the task each frame.
//!
//! # A system with no chooser says so
//!
//! `rfd` answers "nothing" for a cancelled dialog and for a service that is not there alike, so a person with
//! no portal would press Open folder… and see nothing happen. On Linux the request therefore first checks that
//! a chooser can exist ([`chooser_installed`]: the portal's D-Bus service file, or `zenity`, is installed),
//! and when none can it raises an error toast ([`NO_CHOOSER`]) rather than a typed-path field. The check is
//! that a service is *installed*, not that it is running: a portal that is installed and fails to start still
//! reads as a cancel. [`PickerOff`] makes a run report no chooser, so a test can see the toast.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};

/// What a request is for; it comes back unchanged on the [`Picked`] that answers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickFor {
    /// Open folder…: a project's folder, to open.
    OpenFolder,
    /// Locate…: where the folder of the recent project `0` went.
    Locate(PathBuf),
    /// A new project's Location: the folder the project's own folder is made in.
    Location,
    /// Add meshes…: `.glb` and `.gltf` files to put into the project.
    AddMeshes,
}

impl PickFor {
    /// Whether the chooser picks a folder rather than files.
    pub fn is_folder(&self) -> bool {
        !matches!(self, PickFor::AddMeshes)
    }

    /// The chooser's title.
    fn title(&self) -> &'static str {
        match self {
            PickFor::OpenFolder => "Open a project folder",
            PickFor::Locate(_) => "Find the project's folder",
            PickFor::Location => "Choose where the project goes",
            PickFor::AddMeshes => "Add meshes",
        }
    }
}

/// Asks for a chooser to open.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct PickRequest(pub PickFor);

/// A person chose: the folder (one path) or the files, for the request that asked.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    pub for_what: PickFor,
    pub paths: Vec<PathBuf>,
}

/// **A run with no file chooser**: every request is answered with the [`NO_CHOOSER`] toast. A test inserts it,
/// so a headless run never opens a real dialog and the toast can be read.
#[derive(Resource)]
pub struct PickerOff;

/// The toast's words. It names the service a person installs and the other way in, with no program name.
pub const NO_CHOOSER: &str = "This system has no file chooser service (xdg-desktop-portal). Install it, or open a project from a terminal by giving its folder as the argument.";

/// The extensions Add meshes… offers.
pub const MESH_EXTENSIONS: [&str; 2] = ["glb", "gltf"];

/// The chooser in flight: what it is for, and its answer when it has one.
#[derive(Resource, Default)]
struct Pending(Option<(PickFor, Task<Option<Vec<PathBuf>>>)>);

pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PickRequest>()
            .add_message::<Picked>()
            .init_resource::<Pending>()
            .add_systems(Update, (start_a_chooser, collect_the_answer).chain());
    }
}

fn start_a_chooser(
    mut commands: Commands,
    mut requests: MessageReader<PickRequest>,
    mut pending: ResMut<Pending>,
    off: Option<Res<PickerOff>>,
) {
    for PickRequest(for_what) in requests.read() {
        // One chooser at a time: a second press while one is open would stack two dialogs.
        if pending.0.is_some() {
            continue;
        }
        if off.is_some() || !chooser_installed() {
            commands.spawn(bevy_gooey::toast_alert(NO_CHOOSER).sticky());
            continue;
        }
        let asked = for_what.clone();
        let task = IoTaskPool::get_or_init(bevy::tasks::TaskPool::new).spawn(ask(asked));
        pending.0 = Some((for_what.clone(), task));
    }
}

/// The chooser itself, for `for_what`; `None` when it was cancelled.
async fn ask(for_what: PickFor) -> Option<Vec<PathBuf>> {
    let dialog = rfd::AsyncFileDialog::new().set_title(for_what.title());
    if for_what.is_folder() {
        let folder = dialog.pick_folder().await?;
        Some(vec![folder.path().to_path_buf()])
    } else {
        let files = dialog.add_filter("3D meshes", &MESH_EXTENSIONS).pick_files().await?;
        Some(files.into_iter().map(|f| f.path().to_path_buf()).collect())
    }
}

fn collect_the_answer(mut pending: ResMut<Pending>, mut out: MessageWriter<Picked>) {
    let Some((_, task)) = pending.0.as_mut() else { return };
    let Some(answer) = bevy::tasks::futures::check_ready(task) else { return };
    let Some((for_what, _)) = pending.0.take() else { return };
    if let Some(paths) = answer.filter(|p| !p.is_empty()) {
        out.write(Picked { for_what, paths });
    }
}

/// Whether a file chooser can exist on this system. Always on macOS and Windows; on Linux when the portal's
/// D-Bus service or `zenity` is installed (see the module note for what that does and does not prove).
pub fn chooser_installed() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| dirs::home_dir().map(|h| h.join(".local/share")));
    let path = std::env::var_os("PATH").unwrap_or_default();
    portal_installed(data_home.as_deref(), &data_dirs) || std::env::split_paths(&path).any(|d| d.join("zenity").is_file())
}

/// The portal's service file, in any of the places D-Bus looks for services: the user's data folder and each of
/// `XDG_DATA_DIRS` (the system's two when it is empty).
pub fn portal_installed(data_home: Option<&Path>, xdg_data_dirs: &str) -> bool {
    const SERVICE: &str = "dbus-1/services/org.freedesktop.portal.Desktop.service";
    const SYSTEM_DIRS: &str = "/usr/local/share:/usr/share";
    let dirs = if xdg_data_dirs.is_empty() { SYSTEM_DIRS } else { xdg_data_dirs };
    data_home.into_iter().map(Path::to_path_buf).chain(dirs.split(':').map(PathBuf::from)).any(|d| d.join(SERVICE).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_is_installed_where_its_service_file_is() {
        let dir = std::env::temp_dir().join(format!("grimoire_picker_{}", std::process::id()));
        let services = dir.join("dbus-1/services");
        std::fs::create_dir_all(&services).unwrap_or_else(|e| panic!("{e}"));
        assert!(!portal_installed(None, &dir.to_string_lossy()), "an empty services folder has no portal");
        std::fs::write(services.join("org.freedesktop.portal.Desktop.service"), "").unwrap_or_else(|e| panic!("{e}"));
        assert!(portal_installed(None, &dir.to_string_lossy()), "found through XDG_DATA_DIRS");
        assert!(portal_installed(Some(&dir), ""), "found through the user's data folder");
        let _ = std::fs::remove_dir_all(dir);
    }
}
