//! **A person's own files, moved from RON to YAML once** (GD-03, #365; M3 of the review).
//!
//! `settings.ron`, `recent.ron` and `trusted.ron` (a person's keymap, theme and layout; the projects they opened; the guides
//! they allowed) are read from `.yaml` now. A person who upgrades has the old files and nothing reads them, so without this their
//! settings reset, their recents empty and every trusted project asks again. [`upgrade_user_files`] runs at startup: each old file
//! is converted **by value, read back and compared** ([`crate::migrate::convert_value`]), written beside its place, and the original
//! moved into `migrated-ron/` next to it. **A file that cannot be converted is never silent**: the notice names it and why, and the
//! original stays where it is.

use std::path::{Path, PathBuf};

use crate::migrate::{RON_EXT, YAML_EXT, convert_value};

/// **The folder beside a user's file the RON originals are moved into.**
pub const USER_ARCHIVE_DIR: &str = "migrated-ron";

/// **How a file failed to upgrade**, because the two are different answers to a person.
#[derive(Debug)]
enum Unconverted {
    /// It did not convert, so nothing was written and this build reads no settings from it.
    Refused(String),
    /// **It did convert** — the YAML is written and is read — and only the original's move aside failed.
    WrittenBut(String),
}

/// **Convert each `<dir>/<stem>.ron` that has no `<dir>/<stem>.yaml`**, and say what happened to each one.
///
/// `command` is only what a refusal names. Returns one sentence per file touched (converted, or refused and why); nothing for a
/// stem with no old file.
pub fn upgrade_user_files(dirs_and_stems: &[(PathBuf, &str)], command: &str) -> Vec<String> {
    let mut notices = Vec::new();
    for (dir, stem) in dirs_and_stems {
        let ron = dir.join(format!("{stem}.{RON_EXT}"));
        if !ron.is_file() {
            continue;
        }
        let yaml = dir.join(format!("{stem}.{YAML_EXT}"));
        let shown = ron.display().to_string();
        if yaml.exists() {
            notices.push(format!("{shown} was left alone: {} already exists and is the one this build reads", yaml.display()));
            continue;
        }
        match upgrade_one(&ron, &yaml, dir, command) {
            Ok(archived) => notices.push(format!("{shown} was converted to {} (the original is at {})", yaml.display(), archived.display())),
            // **The two failures are not the same sentence.** A file that did not convert wrote nothing, so this build has no
            // settings from it and "could not be converted" is exactly true. A file that *did* convert but whose original could not
            // be moved aside is the opposite: the YAML is there and is read, so saying it could not be converted would tell a person
            // their keymap was lost when it was not.
            Err(Unconverted::Refused(why)) => notices.push(format!("{shown} could not be converted, so this build ignores it: {why}")),
            Err(Unconverted::WrittenBut(why)) => notices.push(format!("{shown} was converted to {}, but {why}", yaml.display())),
        }
    }
    notices
}

fn upgrade_one(ron: &Path, yaml: &Path, dir: &Path, command: &str) -> Result<PathBuf, Unconverted> {
    let text = std::fs::read_to_string(ron).map_err(|e| Unconverted::Refused(e.to_string()))?;
    let done = convert_value(&text, &ron.display().to_string(), command).map_err(|p| Unconverted::Refused(p.line()))?;
    bevy_grimoire_core::atomic_file::write_new(yaml, &done.text).map_err(|e| Unconverted::Refused(format!("{}: {e}", yaml.display())))?;
    let name = ron.file_name().map(PathBuf::from).unwrap_or_default();
    let into = dir.join(USER_ARCHIVE_DIR).join(name);
    let moved = std::fs::create_dir_all(dir.join(USER_ARCHIVE_DIR))
        .and_then(|()| bevy_grimoire_core::atomic_file::write_cache(&into, text.as_bytes()))
        .and_then(|()| std::fs::remove_file(ron));
    if let Err(e) = moved {
        // **The YAML is written and is what this build reads**; only the archiving of the original failed. That is its own answer,
        // because the person's settings are here.
        return Err(Unconverted::WrittenBut(format!("the original could not be moved aside: {e}. The original is still at {}", ron.display())));
    }
    Ok(into)
}
