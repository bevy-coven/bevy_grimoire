//! **A person's RON files from before the cutover, converted once at startup** (GD-03, #365).
//!
//! `settings.ron` (config folder), `recent.ron` and `trusted.ron` (data folder) are `.yaml` now. The conversion is
//! `bevy_grimoire_migrate::upgrade_user_files`: by value, read back and compared, the original moved into `migrated-ron/`, and a
//! file that cannot be converted named loudly rather than ignored.

use std::path::PathBuf;

/// **The user files this build reads**, as the stems of the `.ron` they used to be.
pub const SETTINGS_STEM: &str = "settings";
/// The recent-projects file's stem.
pub const RECENT_STEM: &str = "recent";
/// The trust store's stem.
pub const TRUSTED_STEM: &str = "trusted";

/// **Convert the old files in `config` and `data`**, returning one sentence per file touched.
pub fn upgrade_user_files(config: Option<PathBuf>, data: Option<PathBuf>) -> Vec<String> {
    let mut places: Vec<(PathBuf, &str)> = Vec::new();
    if let Some(config) = config {
        places.push((config, SETTINGS_STEM));
    }
    if let Some(data) = data {
        places.push((data.clone(), RECENT_STEM));
        places.push((data, TRUSTED_STEM));
    }
    bevy_grimoire_migrate::upgrade_user_files(&places, &format!("{} migrate <project folder>", crate::args::PROGRAM))
}
