//! **Live editing's engine-free half: notice that a project changed on disk, and say what changed.**
//!
//! A running game reloads its grimoire project when the editor writes it. Two questions come before
//! any engine work, and both are answered here so the game and the runtime answer them the same way:
//!
//! - **Is a reload due?** [`FileWatch`] stats the project's files and says yes once, after a change
//!   has settled. It stats the files itself rather than through an engine's asset watcher because
//!   the editor writes a project as several files (a map, the library, the compositions): reading
//!   the whole set once, after the writes stop, is what keeps a reload consistent.
//! - **What changed?** [`diff_maps`] compares two versions of a map by placement id, so a reload can
//!   be reported, and later applied, as the handful of rows it touched.
//!
//! Moved from Sanctuary's `grimoire_data::watch` (its `due` rule and its three tests unchanged) and
//! generalised to any directory and any set of extensions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::map::Map;

#[path = "live_verdict.rs"]
mod verdict;
pub use verdict::{KindVerdict, Outcome, Verdict, VERDICT_VERSION};

/// Seconds between looks at the files.
pub const POLL_SECS: f64 = 0.5;
/// How long the files must stay as they are before a reload is due.
pub const SETTLE_SECS: f64 = 0.5;

/// **Watches every file under a directory with one of the given extensions.**
///
/// A file or directory whose name starts with `.` is never watched: a game writes its own verdict
/// into `<grimoire_dir>/.live/`, and that write must not trigger the reload it reports on. A file
/// with another extension is skipped too, so the editor's write-then-rename (a hidden
/// `.<name>.<pid>-<n>.tmp`, see [`crate::atomic_file`]) is not a change until the rename lands.
pub struct FileWatch {
    dir: PathBuf,
    extensions: Vec<String>,
    seen: BTreeMap<PathBuf, SystemTime>,
    changed_at: Option<f64>,
    last_poll: Option<f64>,
    poll_secs: f64,
    settle_secs: f64,
}

impl FileWatch {
    /// Stats now. Call it BEFORE the first load, so a write between this stat and the load is a
    /// change the first poll sees.
    pub fn new(dir: &Path, extensions: &[&str]) -> FileWatch {
        let mut watch = FileWatch {
            dir: dir.to_path_buf(),
            extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
            seen: BTreeMap::new(),
            changed_at: None,
            last_poll: None,
            poll_secs: POLL_SECS,
            settle_secs: SETTLE_SECS,
        };
        watch.seen = watch.stat();
        watch
    }

    /// The same watch with its own poll interval and settle time, for a few small files whose
    /// reload should not wait the project's half second twice.
    pub fn with_timing(mut self, poll_secs: f64, settle_secs: f64) -> FileWatch {
        self.poll_secs = poll_secs;
        self.settle_secs = settle_secs;
        self
    }

    /// Stats at most once per [`POLL_SECS`] of `now_secs`. True exactly once, when the set of
    /// `(path, mtime)` changed and then stayed the same for [`SETTLE_SECS`].
    pub fn poll(&mut self, now_secs: f64) -> bool {
        if self
            .last_poll
            .is_some_and(|last| now_secs - last < self.poll_secs)
        {
            return false;
        }
        self.last_poll = Some(now_secs);
        let stats = self.stat();
        due(
            &mut self.seen,
            &mut self.changed_at,
            stats,
            now_secs,
            self.settle_secs,
        )
    }

    fn stat(&self) -> BTreeMap<PathBuf, SystemTime> {
        let mut out = BTreeMap::new();
        stat_dir(&self.dir, &self.extensions, &mut out);
        out
    }
}

/// Every watched file under `dir`, recursively, with its modification time.
fn stat_dir(dir: &Path, extensions: &[String], out: &mut BTreeMap<PathBuf, SystemTime>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            stat_dir(&path, extensions, out);
        } else if path
            .extension()
            .is_some_and(|e| extensions.iter().any(|want| e == want.as_str()))
            && let Ok(modified) = meta.modified()
        {
            out.insert(path, modified);
        }
    }
}

/// Feed one poll's stats in at time `now`. Returns true when a reload is due: the files differ
/// from the last poll's, and have stayed as they are for `settle` seconds. `seen` starts as the
/// stats taken before the project was loaded.
fn due(
    seen: &mut BTreeMap<PathBuf, SystemTime>,
    changed_at: &mut Option<f64>,
    stats: BTreeMap<PathBuf, SystemTime>,
    now: f64,
    settle: f64,
) -> bool {
    if *seen != stats {
        *seen = stats;
        *changed_at = Some(now);
        return false;
    }
    match *changed_at {
        Some(at) if now - at >= settle => {
            *changed_at = None;
            true
        }
        _ => false,
    }
}

/// **What changed between two versions of a map**, by placement id. Each list is sorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// Ids in both maps whose [`crate::map::Placed`] differs.
    pub changed: Vec<String>,
}

/// How many ids [`MapDiff::summary`] names before it counts the rest.
const SUMMARY_IDS: usize = 12;

impl MapDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// `"1 added, 0 removed, 2 changed: +smithy ~wagon ~well"`. At most twelve ids, then
    /// `"…and N more"`.
    pub fn summary(&self) -> String {
        let counts = format!(
            "{} added, {} removed, {} changed",
            self.added.len(),
            self.removed.len(),
            self.changed.len()
        );
        let ids: Vec<String> = self
            .added
            .iter()
            .map(|id| format!("+{id}"))
            .chain(self.removed.iter().map(|id| format!("-{id}")))
            .chain(self.changed.iter().map(|id| format!("~{id}")))
            .collect();
        if ids.is_empty() {
            return counts;
        }
        let shown = ids.iter().take(SUMMARY_IDS).cloned().collect::<Vec<_>>().join(" ");
        match ids.len().saturating_sub(SUMMARY_IDS) {
            0 => format!("{counts}: {shown}"),
            more => format!("{counts}: {shown} …and {more} more"),
        }
    }
}

/// **The placements `new` added, removed and changed against `old`**, keyed by
/// [`crate::map::Placed::id`]. A row a run lays (`<run id>/<n>`, see [`crate::run::post_id`]) is
/// not named: its run is, once, as added, removed or changed. Run ids share the placement
/// namespace (`Map::validate` refuses a collision), so the lists stay unambiguous.
pub fn diff_maps(old: &Map, new: &Map) -> MapDiff {
    let runs: BTreeSet<&str> = old.runs.iter().chain(&new.runs).map(|r| r.id.as_str()).collect();
    fn run_of<'a>(runs: &BTreeSet<&'a str>, id: &str) -> Option<&'a str> {
        let (run, n) = id.rsplit_once('/')?;
        let run = runs.get(run).copied()?;
        (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(run)
    }
    let before: BTreeMap<&str, _> = old.placements.iter().map(|p| (p.id.as_str(), p)).collect();
    let after: BTreeMap<&str, _> = new.placements.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut diff = MapDiff::default();
    // A run is changed when its definition differs or any of its rows does (a library edit moves
    // posts without touching the run).
    let mut moved: BTreeSet<&str> = BTreeSet::new();
    for (id, placed) in &after {
        let differs = before.get(id).is_none_or(|was| *was != *placed);
        match (run_of(&runs, id), differs) {
            (Some(run), true) => {
                moved.insert(run);
            }
            (None, true) if before.contains_key(id) => diff.changed.push((*id).to_owned()),
            (None, true) => diff.added.push((*id).to_owned()),
            _ => {}
        }
    }
    for id in before.keys() {
        if !after.contains_key(id) {
            match run_of(&runs, id) {
                Some(run) => {
                    moved.insert(run);
                }
                None => diff.removed.push((*id).to_owned()),
            }
        }
    }
    for id in &runs {
        let was = old.runs.iter().find(|r| r.id == *id);
        let now = new.runs.iter().find(|r| r.id == *id);
        match (was, now) {
            (None, Some(_)) => diff.added.push((*id).to_owned()),
            (Some(_), None) => diff.removed.push((*id).to_owned()),
            (Some(a), Some(b)) if a != b || moved.contains(id) => diff.changed.push((*id).to_owned()),
            _ => {}
        }
    }
    diff.added.sort();
    diff.removed.sort();
    diff.changed.sort();
    diff
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::map::Placed;

    fn stats(stamp: u64) -> BTreeMap<PathBuf, SystemTime> {
        let mut m = BTreeMap::new();
        m.insert(
            PathBuf::from("a.ron"),
            SystemTime::UNIX_EPOCH + Duration::from_secs(stamp),
        );
        m
    }

    #[test]
    fn a_change_reloads_once_after_it_settles() {
        let (mut seen, mut at) = (stats(1), None);
        // Nothing changed since the files were stat'ed before the load.
        assert!(!due(&mut seen, &mut at, stats(1), 0.0, 0.5));
        assert!(!due(&mut seen, &mut at, stats(1), 0.5, 0.5));
        // A change starts the clock; a further change restarts it.
        assert!(!due(&mut seen, &mut at, stats(2), 1.0, 0.5));
        assert!(!due(&mut seen, &mut at, stats(3), 1.5, 0.5));
        assert!(!due(&mut seen, &mut at, stats(3), 1.9, 0.5));
        assert!(due(&mut seen, &mut at, stats(3), 2.0, 0.5));
        // Once reloaded it stays quiet.
        assert!(!due(&mut seen, &mut at, stats(3), 2.5, 0.5));
    }

    #[test]
    fn a_removed_file_counts_as_a_change() {
        let (mut seen, mut at) = (stats(1), None);
        assert!(!due(&mut seen, &mut at, BTreeMap::new(), 0.5, 0.5));
        assert!(due(&mut seen, &mut at, BTreeMap::new(), 1.0, 0.5));
    }

    #[test]
    fn a_save_between_the_load_and_the_first_poll_is_seen() {
        // The watcher was seeded with the stats from before the load (stamp 1); the editor
        // saved (stamp 2) before the first poll.
        let (mut seen, mut at) = (stats(1), None);
        assert!(!due(&mut seen, &mut at, stats(2), 0.0, 0.5));
        assert!(due(&mut seen, &mut at, stats(2), 0.5, 0.5));
    }

    /// A scratch directory for one test, removed when the guard drops.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test_name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "grimoire_live_{}_{}",
                std::process::id(),
                test_name
            ));
            let _ = std::fs::remove_dir_all(&dir);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                panic!("scratch dir {}: {e}", dir.display());
            }
            Scratch(dir)
        }

        /// Write `name` under the scratch dir with a modification time of `stamp` seconds, so a
        /// test never depends on the filesystem's clock resolution.
        fn write(&self, name: &str, stamp: u64) {
            let path = self.0.join(name);
            if let Some(parent) = path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                panic!("{}: {e}", parent.display());
            }
            let file = std::fs::File::create(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            if let Err(e) = file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(stamp))
            {
                panic!("{}: {e}", path.display());
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// **The game's own verdict file never triggers a reload.** It is written into `.live/` after
    /// every reload, so a watch that saw it would reload forever.
    #[test]
    fn a_dotfile_change_never_fires() {
        let dir = Scratch::new("dotfile");
        dir.write("maps/town.map.ron", 1);
        let mut watch = FileWatch::new(&dir.0, &["ron"]);
        dir.write(".live/game.ron", 2);
        dir.write("maps/.town.map.ron", 2);
        for step in 0..8 {
            assert!(
                !watch.poll(step as f64 * POLL_SECS),
                "a dotfile change fired a reload at poll {step}"
            );
        }
        // And the watch is not blind: a real change still fires.
        dir.write("maps/town.map.ron", 3);
        let fired = (8..16).any(|step| watch.poll(step as f64 * POLL_SECS));
        assert!(fired, "a change to the map never fired");
    }

    #[test]
    fn a_file_of_another_extension_is_ignored() {
        let dir = Scratch::new("extension");
        dir.write("town.map.ron", 1);
        let mut watch = FileWatch::new(&dir.0, &["ron"]);
        dir.write("pieces.yaml", 2);
        dir.write("town.map.ron.tmp", 2);
        for step in 0..8 {
            assert!(!watch.poll(step as f64 * POLL_SECS));
        }
    }

    /// **Two polls inside [`POLL_SECS`] stat once.** A change made between them is first seen by
    /// the next poll that is allowed to stat, so the settle starts there and not at the early poll.
    #[test]
    fn two_polls_within_the_poll_interval_stat_once() {
        let dir = Scratch::new("interval");
        dir.write("town.map.ron", 1);
        let mut watch = FileWatch::new(&dir.0, &["ron"]);
        assert!(!watch.poll(0.0));
        dir.write("town.map.ron", 2);
        // Too soon after the last poll: no stat, so the change is not yet seen.
        assert!(!watch.poll(0.1));
        // Had 0.1 stat'ed, the change would have settled by 0.6 and fired here.
        assert!(!watch.poll(0.6));
        assert!(watch.poll(1.1));
        assert!(!watch.poll(1.6));
    }

    fn placed(id: &str, yaw: f32) -> Placed {
        Placed {
            id: id.to_owned(),
            descriptor: "town/crate".to_owned(),
            at: (0.0, 0.0),
            yaw,
            ..Default::default()
        }
    }

    fn map(rows: &[Placed]) -> Map {
        Map { placements: rows.to_vec(), ..Default::default() }
    }

    #[test]
    fn diff_maps_names_what_was_added_removed_and_changed() {
        let old = map(&[placed("well", 0.0), placed("wagon", 20.0), placed("gone", 0.0)]);
        let new = map(&[placed("wagon", 110.0), placed("well", 0.0), placed("smithy", 0.0)]);
        let diff = diff_maps(&old, &new);
        assert_eq!(
            diff,
            MapDiff {
                added: vec!["smithy".to_owned()],
                removed: vec!["gone".to_owned()],
                changed: vec!["wagon".to_owned()],
            }
        );
        assert_eq!(diff.summary(), "1 added, 1 removed, 1 changed: +smithy -gone ~wagon");
    }

    #[test]
    fn diff_maps_of_one_map_is_empty() {
        let m = map(&[placed("well", 0.0), placed("wagon", 20.0)]);
        let diff = diff_maps(&m, &m.clone());
        assert!(diff.is_empty());
        assert_eq!(diff.summary(), "0 added, 0 removed, 0 changed");
    }

    #[test]
    fn a_long_summary_names_twelve_and_counts_the_rest() {
        let new = map(&(0..15).map(|i| placed(&format!("p{i:02}"), 0.0)).collect::<Vec<_>>());
        let summary = diff_maps(&Map::default(), &new).summary();
        assert!(summary.starts_with("15 added, 0 removed, 0 changed: +p00 "), "{summary}");
        assert!(summary.contains("+p11"), "{summary}");
        assert!(!summary.contains("+p12"), "{summary}");
        assert!(summary.ends_with("…and 3 more"), "{summary}");
    }
}
