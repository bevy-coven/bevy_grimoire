//! **The projects the editor has opened, most recent first** (UI-11, #217).
//!
//! One file per user, `<data_dir>/Grimoire/recent.ron`, where `<data_dir>` is the platform's own
//! (`dirs::data_dir`: `~/.local/share` on Linux, `~/Library/Application Support` on macOS,
//! `%APPDATA%` on Windows). It is not in any project: a project is a folder a person owns and may
//! share, and where one author opened it from is no business of its files.
//!
//! The list is data like every other file here: versioned, and a file of another version is refused
//! by name rather than read as this one's ([`Recent::from_ron`]). A path whose folder has gone is
//! listed as [`Listed::missing`], never dropped: the author moved it, and [`Recent::locate`] points
//! the entry at where it went.
//!
//! The editor records an opening in one place, `screen::open_the_door`, which every way into the
//! editor (the menu, the command line, an agent's `map_open`) goes through. It records only when a
//! [`RecentProjects`] resource names the file, which `main` inserts; a test's editor does not, so no
//! test writes to the real data directory.

use std::path::{Path, PathBuf};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const RECENT_VERSION: u32 = 1;

/// The directory under the platform's data directory that holds the editor's own files.
const APP_DIR: &str = "Grimoire";
const FILE: &str = "recent.ron";

/// Where the recent list lives for this user, or `None` on a platform with no data directory.
pub fn user_file() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP_DIR).join(FILE))
}

/// Names the file the editor records openings in. Inserted by `main`; absent in a test.
#[derive(Resource, Clone, Debug)]
pub struct RecentProjects(pub PathBuf);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recent {
    pub version: u32,
    /// Most recently opened first. A pinned entry keeps its own place in this order; [`Recent::listing`]
    /// is what puts the pinned ones above the rest.
    pub projects: Vec<RecentProject>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentProject {
    pub path: PathBuf,
    #[serde(default)]
    pub pinned: bool,
    /// Seconds since the Unix epoch, when it was last opened.
    pub last_opened: u64,
}

/// One row of the list as a person sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub project: RecentProject,
    /// The folder is not there now: shown as missing, with [`Recent::locate`] to point it at the new place.
    pub missing: bool,
}

impl Default for Recent {
    fn default() -> Self {
        Recent { version: RECENT_VERSION, projects: Vec::new() }
    }
}

/// The key two spellings of one folder share: its canonical path when it exists, else the path as given.
fn key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl Recent {
    pub fn from_ron(text: &str) -> Result<Recent, String> {
        let r: Recent = ron::from_str(text).map_err(|e| format!("recent: parse: {e}"))?;
        if r.version != RECENT_VERSION {
            return Err(format!("recent: version {} is not the {RECENT_VERSION} this build reads", r.version));
        }
        Ok(r)
    }

    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| format!("recent: write: {e}"))
    }

    /// Reads the list at `file`; no file is an empty list, a file this build cannot read is an error naming it.
    pub fn load(file: &Path) -> Result<Recent, String> {
        match std::fs::read_to_string(file) {
            Ok(text) => Recent::from_ron(&text).map_err(|e| format!("{}: {e}", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Recent::default()),
            Err(e) => Err(format!("recent: {}: {e}", file.display())),
        }
    }

    /// Writes the list to `file` through a temporary beside it, so a crash leaves the old list or the new one.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        let text = self.to_ron()?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("recent: {}: {e}", dir.display()))?;
        }
        let tmp = file.with_extension("ron.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("recent: {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, file).map_err(|e| format!("recent: {}: {e}", file.display()))
    }

    fn position(&self, path: &Path) -> Option<usize> {
        let want = key(path);
        self.projects.iter().position(|p| key(&p.path) == want)
    }

    /// `path` was opened at `now`: it moves to the top, keeping whether it was pinned, and a first
    /// opening adds it there.
    pub fn opened(&mut self, path: &Path, now: u64) {
        let mut entry = match self.position(path) {
            Some(i) => self.projects.remove(i),
            None => RecentProject { path: key(path), pinned: false, last_opened: now },
        };
        entry.last_opened = now;
        self.projects.insert(0, entry);
    }

    /// Pins or unpins an entry; `false` when the path is not in the list.
    pub fn pin(&mut self, path: &Path, pinned: bool) -> bool {
        match self.position(path) {
            Some(i) => {
                self.projects[i].pinned = pinned;
                true
            }
            None => false,
        }
    }

    /// Takes an entry out of the list (the folder itself is untouched); `false` when it is not there.
    pub fn remove(&mut self, path: &Path) -> bool {
        match self.position(path) {
            Some(i) => {
                self.projects.remove(i);
                true
            }
            None => false,
        }
    }

    /// Points the entry for `old` at `new`, where a person found the folder again. The entry keeps its
    /// place, its pin and its time. Refused when `old` is not listed, when `new` is not a folder, or when
    /// `new` is already another entry (two rows for one folder).
    pub fn locate(&mut self, old: &Path, new: &Path) -> Result<(), String> {
        let Some(i) = self.position(old) else {
            return Err(format!("{} is not in the recent projects", old.display()));
        };
        if !new.is_dir() {
            return Err(format!("{} is not a folder", new.display()));
        }
        if self.position(new).is_some_and(|j| j != i) {
            return Err(format!("{} is already in the recent projects", new.display()));
        }
        self.projects[i].path = key(new);
        Ok(())
    }

    /// The list as shown: pinned entries first, then the rest, each group most recently opened first;
    /// every entry whose folder is gone is marked missing, none is dropped.
    pub fn listing(&self) -> Vec<Listed> {
        let row = |p: &RecentProject| Listed { project: p.clone(), missing: !p.path.is_dir() };
        self.projects
            .iter()
            .filter(|p| p.pinned)
            .chain(self.projects.iter().filter(|p| !p.pinned))
            .map(row)
            .collect()
    }
}

/// Seconds since the Unix epoch now.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Records that `root` was opened in the list at `file`, reading and writing it. A list this build
/// cannot read (another version, damaged) is left as it is and named in the error, never overwritten:
/// it may be a newer build's, and the opening itself is not stopped by it.
pub fn record(file: &Path, root: &Path) -> Result<(), String> {
    let mut recent = Recent::load(file)?;
    recent.opened(root, now());
    recent.save(file)
}
