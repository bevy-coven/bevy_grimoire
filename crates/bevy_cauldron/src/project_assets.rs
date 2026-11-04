//! **The project is the asset root, and the root can move** (#294).
//!
//! A descriptor's `mesh` is named relative to the project (`kenney/chair.glb`), so every
//! `AssetServer::load` in this crate resolves against `<project>/assets`. Bevy builds its asset sources
//! once, when `AssetPlugin` is added, and `AssetPlugin::file_path` is a fixed string — so a second
//! project could only be opened by starting a second process. This module is what lets one process open
//! any number of projects:
//!
//! * [`ProjectAssetReader`] is the **default** asset source. It holds a shared [`ProjectRoot`] and, on every
//!   read, hands the work to a [`FileAssetReader`] rooted at `<root>/assets`. With no project open it
//!   answers `NotFound`, which is the truth.
//! * [`open_project`] is the **one** way into a project, whether the command line named it or a person chose
//!   it on the launcher: it points the root at the project, installs the project's font, tells the chooser
//!   where it is, and sends the window to the screen the arguments ask for.
//!
//! Nothing here keeps a path prefix per project or a second asset server. An asset whose path is the
//! same in two projects (`a.glb`) is the same *path* to Bevy, so it stays correct only while no strong
//! handle outlives the door that loaded it; `tests/a_project_opens_in_the_same_process.rs` pins that.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use bevy::asset::AssetApp;
use bevy::asset::io::file::FileAssetReader;
use bevy::asset::io::{AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader, VecReader};
use bevy::prelude::*;

use crate::screen::Screen;

/// The folder inside a project that its meshes are named relative to.
const ASSETS_DIR: &str = "assets";

/// Which project the asset server reads from, shared with [`ProjectAssetReader`]. `None` while no project is
/// open (the launcher).
#[derive(Resource, Clone, Default, Debug)]
pub struct ProjectRoot(Arc<RwLock<Option<PathBuf>>>);

impl ProjectRoot {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self(Arc::new(RwLock::new(root)))
    }

    /// The project assets are read from now.
    pub fn current(&self) -> Option<PathBuf> {
        // A poisoned lock still holds a whole `Option<PathBuf>`: a writer only ever replaces it.
        self.0.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Points the asset server at another project, or at none.
    pub fn set(&self, root: Option<PathBuf>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = root;
    }
}

/// An [`AssetReader`] over `<project root>/assets` whose project can change between two reads.
pub struct ProjectAssetReader {
    root: ProjectRoot,
}

impl ProjectAssetReader {
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }

    /// A file reader on the project as it is now, or `NotFound` when none is open.
    fn files(&self, path: &Path) -> Result<FileAssetReader, AssetReaderError> {
        match self.root.current() {
            Some(root) => Ok(FileAssetReader::new(root.join(ASSETS_DIR))),
            None => Err(AssetReaderError::NotFound(path.to_path_buf())),
        }
    }
}

// The file reader is built per call, so what it returns cannot borrow from it: both file reads are
// finished inside the call and handed back as bytes.
impl AssetReader for ProjectAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let files = self.files(path)?;
        let mut file = files.read(path).await?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        Ok(VecReader::new(bytes))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let files = self.files(path)?;
        let mut file = files.read_meta(path).await?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        Ok(VecReader::new(bytes))
    }

    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> {
        self.files(path)?.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.files(path)?.is_directory(path).await
    }
}

/// Registers [`ProjectAssetReader`] as the default asset source and inserts the [`ProjectRoot`] it reads.
/// **Before `DefaultPlugins`**: Bevy builds its sources when `AssetPlugin` is added, and a default source that
/// is already registered is the one it keeps.
pub fn install(app: &mut App, root: Option<PathBuf>) {
    let root = ProjectRoot::new(root);
    let shared = root.clone();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(ProjectAssetReader::new(shared.clone()))),
    )
    .insert_resource(root);
}

/// **A project that is waiting for the open door to close.** [`open_project`] called while the editor is up
/// leaves the root alone — the door's entities still hold handles into the old project — and the editor's
/// exit applies it after [`crate::screen::close_the_door`].
#[derive(Resource, Clone, Debug)]
pub struct PendingOpen(pub Vec<String>);

/// The screen the arguments ask for: the loading screen for a door or a map, the chooser for a bare project.
pub fn screen_for(argv: &[String]) -> Screen {
    if crate::args::names_a_door(argv) { Screen::Loading } else { Screen::Menu }
}

/// **Everything about a project that was decided from its root when the process started**, done for
/// `argv`'s project: the data folder a project from before the rename gets moved, the asset root, the font, the
/// chooser's root and the loader's request. Returns the screen to show. Does not change the screen: `main` calls
/// this before the state machine runs, where the state it chose is already the one returned.
pub fn point_at(world: &mut World, argv: &[String]) -> Result<Screen, String> {
    let root = crate::args::root_of(argv);
    // A project from before the rename keeps its data in `assets/emerge`; move it once, or refuse to
    // guess when both folders exist.
    match bevy_grimoire_core::project_data::migrate_legacy_data_dir(&root) {
        Ok(Some(moved)) => println!("{moved}"),
        Ok(None) => {}
        Err(e) => return Err(e),
    }
    crate::harness::install_font_in(world, Some(&root))?;
    world.resource::<ProjectRoot>().set(Some(root.clone()));
    // The chooser's own state describes the project it scanned, which is no longer this one.
    world.remove_resource::<crate::chooser::Chooser>();
    world.insert_resource(crate::chooser::MenuOpening { root, preselect: crate::args::split(argv).kit.map(str::to_owned) });
    let screen = screen_for(argv);
    if screen == Screen::Loading {
        world.insert_resource(crate::loading::LoadRequest(argv.to_vec()));
    }
    Ok(screen)
}

/// **Opens the project `argv` names, in this process** — the one way in, for the command line and the launcher.
/// `argv` is what `bevy_cauldron` itself parses ([`crate::args`]): `<root> [map] [--door d] [--kit k]`.
///
/// A door being open is closed first (the editor's exit runs [`crate::screen::close_the_door`], and the project
/// is pointed at after it). An error is the reason the project cannot be pointed at.
pub fn open_project_argv(world: &mut World, argv: &[String]) -> Result<(), String> {
    let editor_is_up = world.get_resource::<State<Screen>>().is_some_and(|s| *s.get() == Screen::Editor);
    let screen = if editor_is_up {
        world.insert_resource(PendingOpen(argv.to_vec()));
        screen_for(argv)
    } else {
        point_at(world, argv)?
    };
    world.resource_mut::<NextState<Screen>>().set(screen);
    Ok(())
}

/// [`open_project_argv`] for `root`, on `map` when one is named.
pub fn open_project(world: &mut World, root: &Path, map: Option<&str>) -> Result<(), String> {
    let mut argv = vec![root.to_string_lossy().into_owned()];
    argv.extend(map.map(str::to_owned));
    open_project_argv(world, &argv)
}

/// The editor's exit, after its door is closed: points at the project that was waiting. A project that cannot
/// be pointed at leaves the window on the launcher with the reason in the log.
pub(crate) fn apply_pending_open(world: &mut World) {
    let Some(PendingOpen(argv)) = world.remove_resource::<PendingOpen>() else { return };
    if let Err(e) = point_at(world, &argv) {
        error!("cannot open {}: {e}", crate::args::root_of(&argv).display());
        world.resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_project_open_every_read_is_not_found() {
        let reader = ProjectAssetReader::new(ProjectRoot::new(None));
        let path = Path::new("a.glb");
        let read = bevy::tasks::block_on(AssetReader::read(&reader, path));
        assert!(matches!(read, Err(AssetReaderError::NotFound(p)) if p == path));
        let dir = bevy::tasks::block_on(AssetReader::is_directory(&reader, path));
        assert!(matches!(dir, Err(AssetReaderError::NotFound(_))));
    }

    #[test]
    fn the_root_a_read_uses_is_the_one_set_last() {
        let one = std::env::temp_dir().join(format!("grimoire_assets_one_{}", std::process::id()));
        let two = std::env::temp_dir().join(format!("grimoire_assets_two_{}", std::process::id()));
        for (dir, text) in [(&one, "one"), (&two, "two")] {
            std::fs::create_dir_all(dir.join(ASSETS_DIR)).unwrap_or_else(|e| panic!("{e}"));
            std::fs::write(dir.join(ASSETS_DIR).join("a.txt"), text).unwrap_or_else(|e| panic!("{e}"));
        }
        let root = ProjectRoot::new(Some(one.clone()));
        let reader = ProjectAssetReader::new(root.clone());
        let read = |reader: &ProjectAssetReader| {
            bevy::tasks::block_on(async {
                let mut bytes = Vec::new();
                let mut file = AssetReader::read(reader, Path::new("a.txt")).await.unwrap_or_else(|e| panic!("{e}"));
                file.read_to_end(&mut bytes).await.unwrap_or_else(|e| panic!("{e}"));
                String::from_utf8(bytes).unwrap_or_else(|e| panic!("{e}"))
            })
        };
        assert_eq!(read(&reader), "one");
        root.set(Some(two.clone()));
        assert_eq!(read(&reader), "two", "the same reader reads the project that is open now");
        let _ = std::fs::remove_dir_all(one);
        let _ = std::fs::remove_dir_all(two);
    }
}
