//! **The one function a project file is read through, and the only place two formats meet** (GD-08, #370).
//!
//! A development build reads the YAML a person edits; a release (a game built with the `baked` feature of
//! `bevy_grimoire`) reads the files [`crate::bake`] made from it. **The split is [`load`] and nowhere else**: a
//! caller names a type and a path, and what comes back is the same value whichever format held it. Nothing above
//! this function asks which one it got, and a test holds the two to the same answer.
//!
//! ```ignore
//! let map: Map = bevy_grimoire_core::load::load(&root, "assets/grimoire/maps/yard.map.yaml")?;
//! ```
//!
//! # Which source a process reads
//!
//! One, chosen once at start: [`use_baked`] opens a bake, refuses it when the YAML it was made from has changed
//! ([`crate::bake::Baked::refuse_if_stale`]) and from then on every [`load`] reads the binaries. Until it is called a
//! process reads YAML. **A release that cannot open its bake does not start**: the caller of [`use_baked`] has the
//! problem, naming the file, and the game decides how to stop; nothing falls back to YAML, because a release that
//! quietly ran other data than it was built from is the failure this exists to prevent.

use std::path::Path;
use std::sync::RwLock;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::bake::Baked;
use crate::problem::{Detail, Location, Problem};

/// **Where a process reads its project files from.**
enum Source {
    /// The YAML on disk: what a development build reads.
    Yaml,
    /// A bake: what a release reads.
    Baked(Baked),
}

/// The one source of this process. A lock, because a game reloads on a thread of its own.
static SOURCE: RwLock<Source> = RwLock::new(Source::Yaml);

/// **A type a project file reads as**, from its YAML.
///
/// The default is the one YAML reader ([`crate::yaml::from_str`]); a type with a validator of its own (a map's
/// `validate`, the rules' `validate_numbers`) says so by overriding [`Loadable::from_yaml`] to call its `parse`, so
/// what a bake proves is what a load of the YAML would have accepted.
pub trait Loadable: DeserializeOwned {
    /// The type, read from the YAML `text` of a file: the type's own `parse`, which is where its validators are.
    fn from_yaml(text: &str) -> Result<Self, String>;
}

/// **Say a type is `Loadable` by its own `parse`.**
macro_rules! loadable {
    ($($t:ty),* $(,)?) => {$(
        impl Loadable for $t {
            fn from_yaml(text: &str) -> Result<Self, String> {
                <$t>::parse(text)
            }
        }
    )*};
}

loadable!(
    crate::kits::Kits,
    crate::vocab::Vocabularies,
    crate::library::Library,
    crate::policy::Policy,
    crate::rules::Rules,
    crate::composition::Compositions,
    crate::map::Map,
);

/// **Make this process read the bake in `dir`**, for the project at `root`.
///
/// Refuses, naming the file, a bake that is missing, of another format, or made from YAML that has changed since.
#[allow(clippy::result_large_err)]
pub fn use_baked(dir: &Path, root: &Path) -> Result<(), Problem> {
    let baked = Baked::open(dir, root)?;
    baked.refuse_if_stale()?;
    *SOURCE.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Source::Baked(baked);
    Ok(())
}

/// **Make this process read YAML** (the default). A test that changed the source puts it back with this.
pub fn use_yaml() {
    *SOURCE.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Source::Yaml;
}

/// **Whether this process reads a bake.**
pub fn is_baked() -> bool {
    matches!(*SOURCE.read().unwrap_or_else(std::sync::PoisonError::into_inner), Source::Baked(_))
}

/// **A project file as a `T`**: `rel` is its path relative to the project `root`, written with `/`.
#[allow(clippy::result_large_err)]
pub fn load<T: Loadable>(root: &Path, rel: &str) -> Result<T, Problem> {
    parsed(&root.join(rel)).map_err(|why| {
        Problem::with(
            format!("{rel} does not read"),
            Detail::default().at(Location::file(rel.to_owned())).because(why).fix(format!("fix {rel}, or bake the project again if the game is a release")),
        )
    })
}

/// **A project file as a `T`, for a caller that spells the path itself and answers with a sentence.**
///
/// The same single function [`load`] is: a development build reads the YAML at `path` through the type's own `parse`;
/// a release reads the baked value of the file at that path. The sentence names the path.
pub fn parsed<T: Loadable>(path: &Path) -> Result<T, String> {
    match &*SOURCE.read().unwrap_or_else(std::sync::PoisonError::into_inner) {
        Source::Yaml => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            T::from_yaml(&text).map_err(|e| format!("{}: {e}", path.display()))
        }
        Source::Baked(baked) => {
            let value = baked_value(baked, path).map_err(|p| p.line())?;
            serde_json::from_value(value).map_err(|e| format!("{}: the baked file does not read as the type asked for: {e}", path.display()))
        }
    }
}

/// **Whether a project file is there**: on disk for YAML, in the bake for a release.
pub fn exists(path: &Path) -> bool {
    match &*SOURCE.read().unwrap_or_else(std::sync::PoisonError::into_inner) {
        Source::Yaml => path.is_file(),
        Source::Baked(baked) => baked.rel_of(path).is_some_and(|rel| baked.has(&rel)),
    }
}

/// **A project file as the value it holds**, for a caller whose type the game makes up (an entity file, a game's own data).
#[allow(clippy::result_large_err)]
pub fn load_value(path: &Path) -> Result<Value, Problem> {
    match &*SOURCE.read().unwrap_or_else(std::sync::PoisonError::into_inner) {
        Source::Yaml => {
            let shown = path.display().to_string();
            let text = std::fs::read_to_string(path).map_err(|e| unreadable(&shown, &e))?;
            crate::yaml::to_json(&text, &shown)
        }
        Source::Baked(baked) => baked_value(baked, path),
    }
}

/// **The files of `kind` the project at `root` holds**, as paths under `root`, in path order: the YAML on disk for a
/// development build, the manifest for a release.
#[allow(clippy::result_large_err)]
pub fn files_of_kind(root: &Path, kind: &str) -> Result<Vec<std::path::PathBuf>, Problem> {
    match &*SOURCE.read().unwrap_or_else(std::sync::PoisonError::into_inner) {
        Source::Yaml => Ok(crate::bake::project_files(root)?.into_iter().filter(|(_, k)| k == kind).map(|(rel, _)| root.join(rel)).collect()),
        Source::Baked(baked) => Ok(baked.files().filter(|e| e.kind == kind).map(|e| root.join(&e.file)).collect()),
    }
}

#[allow(clippy::result_large_err)]
fn baked_value(baked: &Baked, path: &Path) -> Result<Value, Problem> {
    let shown = path.display().to_string();
    let rel = baked.rel_of(path).ok_or_else(|| {
        Problem::with(
            format!("{shown} is not in the bake"),
            Detail::default().at(Location::file(shown.clone())).because("this build loads only baked files, and the path is outside the project").fix("bake the project again"),
        )
    })?;
    baked.value(&rel)
}

fn unreadable(rel: &str, e: &std::io::Error) -> Problem {
    Problem::with(
        format!("{rel} could not be read"),
        Detail::default().at(Location::file(rel.to_owned())).because(e.to_string()).fix(format!("check that {rel} is in the project and can be read")),
    )
}
