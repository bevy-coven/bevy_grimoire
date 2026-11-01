#![cfg(feature = "asset_spike")]
//! **The #204 spike's tests: `standards.ron` through Bevy's asset system against the editor's own
//! watcher** (hot-reload H7). They run only with `--features asset_spike`:
//! `cargo test -p bevy_grimoire_mapper --features asset_spike --test editor asset_spike::`.
//!
//! The last test is the finding that decided the recommendation in `docs/guide/reload-contract.md`
//! ("The asset system against the watcher"): since #199 every writer replaces a file by renaming a
//! temp file over it, and Bevy's unprocessed asset server does not reload on a rename. If it fails,
//! Bevy's watcher now sees an atomic replace and the recommendation is worth reading again.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_core::atomic_file;
use bevy_grimoire_core::live::{FileWatch, POLL_SECS, SETTLE_SECS};
use bevy_grimoire_core::standards::{Dim, Rule, Standards};
use bevy_grimoire_mapper::asset_spike::{Heard, Heard1, StandardsAsset, app_over};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("asset_spike_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }

    fn file(&self) -> PathBuf {
        self.0.join("standards.ron")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn standards(min: f32) -> Standards {
    Standards { version: 1, rules: vec![Rule { name: "door".into(), kind: None, dim: Dim::Width, min: Some(min), max: None, multiple_of: None }] }
}

fn text(min: f32) -> String {
    standards(min).to_ron().unwrap_or_else(|e| panic!("{e}"))
}

/// Steps the app every 4 ms until `done` or `within` has passed.
fn pump(app: &mut App, within: Duration, done: impl Fn(&App) -> bool) -> bool {
    let end = Instant::now() + within;
    while Instant::now() < end {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    false
}

fn heard(app: &App) -> Vec<Heard1> {
    app.world().resource::<Heard>().events.clone()
}

fn saw(app: &App, want: &Standards) -> bool {
    heard(app).iter().any(|h| *h == Heard1::Loaded(want.clone()))
}

/// **The loader reads with the type's own reader**, so a refusal is the one the reload verdict names
/// (`standards: version 2 is not the 1 this build reads`), and a good file is the same value
/// `Standards::parse` gives.
#[test]
fn the_asset_loader_reads_with_the_types_own_reader() {
    let good = Scratch::new("good");
    std::fs::write(good.file(), text(0.9)).unwrap_or_else(|e| panic!("{e}"));
    let (mut app, _handle) = app_over(&good.0);
    assert!(pump(&mut app, Duration::from_secs(10), |a| saw(a, &standards(0.9))), "the good file never loaded: {:?}", heard(&app));

    let bad = Scratch::new("bad");
    std::fs::write(bad.file(), text(0.9).replace("version: 1", "version: 2")).unwrap_or_else(|e| panic!("{e}"));
    let (mut app, _handle) = app_over(&bad.0);
    let refused = |a: &App| heard(a).iter().any(|h| matches!(h, Heard1::Failed(why) if why.contains("standards: version 2 is not the 1 this build reads")));
    assert!(pump(&mut app, Duration::from_secs(10), refused), "the refusal never arrived: {:?}", heard(&app));
}

/// **A refused edit leaves the last good asset in place**, the rule every editor reader keeps. Reloaded
/// by hand here, so the test is about what Bevy does with a file that does not read, not about when it
/// notices.
#[test]
fn a_refused_reload_keeps_the_last_good_asset() {
    let dir = Scratch::new("keeps");
    std::fs::write(dir.file(), text(0.9)).unwrap_or_else(|e| panic!("{e}"));
    let (mut app, handle) = app_over(&dir.0);
    assert!(pump(&mut app, Duration::from_secs(10), |a| saw(a, &standards(0.9))), "the first text never loaded");

    std::fs::write(dir.file(), "(version: 1, rules: [(name: \"door\"").unwrap_or_else(|e| panic!("{e}"));
    app.world().resource::<AssetServer>().reload("standards.ron");
    let failed = |a: &App| heard(a).iter().any(|h| matches!(h, Heard1::Failed(_)));
    assert!(pump(&mut app, Duration::from_secs(10), failed), "the cut-off file was not refused");
    let held = app.world().resource::<Assets<StandardsAsset>>().get(&handle).map(|a| a.0.clone());
    assert_eq!(held, Some(standards(0.9)), "the refused edit replaced the last good asset");
}

/// **Bevy's watcher sees a write in place and misses a rename over the file**, which the editor's own
/// watch sees, and which is how every project file has been written since #199 (`atomic_file`).
///
/// The control comes first: the same asset, watched the same way, changes when the file is written in
/// place, so a miss below is the watcher and not a machine that has none. Then the file is replaced by
/// rename and the editor's `FileWatch`, fed a clock and not the wall's, says a reload is due once, while
/// the asset stays what it was for five times Bevy's own 300 ms debounce.
#[test]
fn bevys_watcher_misses_a_rename_over_the_file_the_editors_watch_sees() {
    let dir = Scratch::new("rename");
    std::fs::write(dir.file(), text(0.9)).unwrap_or_else(|e| panic!("{e}"));
    let (mut app, _handle) = app_over(&dir.0);
    assert!(pump(&mut app, Duration::from_secs(10), |a| saw(a, &standards(0.9))), "the first text never loaded");
    pump(&mut app, Duration::from_millis(800), |_| false);

    // The control: written in place, the asset follows.
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(dir.file(), text(1.1)).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        pump(&mut app, Duration::from_secs(5), |a| saw(a, &standards(1.1))),
        "Bevy's watcher did not see a write in place within 5 s, so a miss below proves nothing about renames"
    );
    pump(&mut app, Duration::from_millis(800), |_| false);

    // The finding: replaced by rename, the editor's watch fires and Bevy's asset does not move.
    let mut watch = FileWatch::new(&dir.0, &["ron"]);
    std::thread::sleep(Duration::from_millis(30));
    atomic_file::write(&dir.file(), text(1.3)).unwrap_or_else(|e| panic!("{e}"));
    let due = (0..12).filter(|step| watch.poll(*step as f64 * POLL_SECS.max(SETTLE_SECS))).count();
    assert_eq!(due, 1, "the editor's watch did not fire exactly once for a rename over the file");
    let moved = pump(&mut app, Duration::from_millis(1500), |a| saw(a, &standards(1.3)));
    assert!(
        !moved,
        "Bevy's watcher now reloads the asset after a rename over the file: its unprocessed server used to ignore \
         `RenamedAsset`. Re-read 'The asset system against the watcher' in docs/guide/reload-contract.md"
    );
}
