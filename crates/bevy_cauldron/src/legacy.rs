//! **A person's RON files from before the cutover, converted once at startup** (GD-03, #365).
//!
//! `settings.ron` (config folder), `recent.ron` and `trusted.ron` (data folder) are `.yaml` now. The conversion is
//! `bevy_grimoire_migrate::upgrade_user_files`: by value, read back and compared, the original moved into `migrated-ron/`, and a
//! file that cannot be converted named loudly rather than ignored.

use std::path::PathBuf;

use bevy::prelude::Resource;

/// **What the one-time upgrade said, carried into the running app** so it is on screen and not only in the log.
///
/// One sentence per file touched (converted, or refused and why). [`crate::launcher`] spawns a toast for each on
/// entering the Projects home, which is the first screen a person sees after an upgrade; a run that found nothing to
/// upgrade carries an empty list and says nothing.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct StartupNotices(pub Vec<String>);

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
