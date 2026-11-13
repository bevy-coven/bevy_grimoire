//! **The project a trial edits, and the files the suite lays over it** (BM-01, #411).
//!
//! **The fixture is the repository's own project.** `assets/` is a real project — a real library, real
//! meshes, a real map — so a task's arithmetic is measured against the sizes a person actually works
//! with rather than against numbers a test made up. `bench/fixture/` is laid over the copy, and that is
//! where the suite's own map lives: a person editing `assets/grimoire/maps/` cannot move a task's
//! starting position, and the tests that count the shipped project's maps do not move either, because
//! **nothing under `assets/grimoire/maps/` changes**.
//!
//! # The copy is the trial's, and the copy is all it gets
//!
//! A trial never edits the repository. It copies `assets/` to its own directory, lays the fixture's tree
//! over the copy file by file, and hands an agent that directory. That is what makes trials independent
//! of each other and of a person's editor: two trials in the same run cannot see each other's work, and a
//! run that goes wrong leaves the repository alone.

use std::path::{Path, PathBuf};

/// **Where the suite's fixture tree sits**, relative to the repository or game root — a tree of the
/// project's own shape, so `bench/fixture/assets/grimoire/maps/x.map.yaml` lands at
/// `assets/grimoire/maps/x.map.yaml` in a trial's copy.
pub const FIXTURE_DIR: &str = "bench/fixture";

/// **What a refusal calls the fixture's half of a project**, since a person reads it in a task's refusal.
pub const FIXTURE_OF_GRIMOIRE: &str = "bench/fixture/assets/grimoire";

/// **The project a trial copies**: `assets/`, relative to the repository or game root.
pub const PROJECT_DIR: &str = "assets";

/// **Copy the project at `root` into `into`, then lay the fixture over the copy.**
///
/// `into` is the trial's directory, which this call makes; the project lands at `into/assets/`, which is
/// the same shape the repository has, so an agent working in it sees the project it expects and nothing
/// about the benchmark.
///
/// Returns the files the fixture laid over, relative to `into`, with `/` separators, so a report can say
/// exactly what the suite added. Refuses when `into` already exists — a trial's directory is made by this
/// call, and two trials sharing one would be one trial.
pub fn lay_out(root: &Path, into: &Path) -> Result<Vec<String>, String> {
    if into.exists() {
        return Err(format!(
            "{} already exists, so this trial's project would be laid over another's",
            into.display()
        ));
    }
    let project = root.join(PROJECT_DIR);
    if !project.is_dir() {
        return Err(format!(
            "{} holds no {PROJECT_DIR}/ directory, so there is no project to copy",
            root.display()
        ));
    }
    let copied = into.join(PROJECT_DIR);
    copy_tree(&project, &copied)?;
    let fixture = root.join(FIXTURE_DIR).join(PROJECT_DIR);
    let mut laid = Vec::new();
    if fixture.is_dir() {
        lay_over(&fixture, &copied, into, &mut laid)?;
    }
    laid.sort();
    Ok(laid)
}

/// **Copy `from` to `to`**, making `to` and every directory under it.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let target = to.join(&name);
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            copy_tree(&path, &target)?;
        } else if kind.is_file() {
            std::fs::copy(&path, &target).map_err(|e| format!("{} -> {}: {e}", path.display(), target.display()))?;
        }
    }
    Ok(())
}

/// **Lay the fixture's tree over a copy of the project**, file by file, recording what was replaced.
///
/// Every file goes through [`bevy_grimoire_core::atomic_file::write`], so a reader of the trial's project
/// never sees a half-written file — the same rule every writer of a project file follows.
fn lay_over(from: &Path, into: &Path, trial: &Path, laid: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let target = into.join(&name);
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
            lay_over(&path, &target, trial, laid)?;
        } else if kind.is_file() {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            bevy_grimoire_core::atomic_file::write(&target, bytes)
                .map_err(|e| format!("{}: {e}", target.display()))?;
            // **Relative to the trial's own root**, so the list names the project's file a person would
            // open (`assets/grimoire/maps/x.map.yaml`) rather than a machine's temporary path.
            if let Ok(rel) = target.strip_prefix(trial) {
                laid.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

/// **The fixture's own map files**, whose stems are the maps a task may open by name.
///
/// Read from the fixture's tree rather than from the project's, because a task's map belongs to the suite:
/// two tasks that both wrote `bedroom` and shared the project's file would silently be one task.
pub fn fixture_maps(root: &Path) -> Vec<String> {
    let maps = root
        .join(FIXTURE_DIR)
        .join(PROJECT_DIR)
        .join("grimoire")
        .join("maps");
    let Ok(entries) = std::fs::read_dir(&maps) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.strip_suffix(bevy_grimoire_core::map::MAP_SUFFIX).map(str::to_owned)
        })
        .collect();
    out.sort();
    out
}

/// **A fixture file's path**, for a caller naming one: `bench/fixture/assets/<rel>`.
pub fn fixture_file(root: &Path, rel: &str) -> PathBuf {
    root.join(FIXTURE_DIR).join(PROJECT_DIR).join(rel)
}
