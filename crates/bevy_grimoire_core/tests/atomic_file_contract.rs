//! **`atomic_file`'s own contract** (#199, H2): what each of its calls leaves behind, on success and
//! on failure. The behaviour a *reader* sees (no torn read, a killed writer, two writers, the
//! watch) is `atomic_writes.rs`; this is what the files in the directory look like afterwards.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy_grimoire_core::atomic_file::{self, STALE_AFTER};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_atomic_file_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap_or_else(|e| panic!("{e}"))
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn age(path: &Path, by: Duration) {
    let file = std::fs::File::options().write(true).open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    file.set_modified(SystemTime::now() - by).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

#[test]
fn a_write_makes_the_directory_and_leaves_only_the_target() {
    let dir = Scratch::new("makes");
    let target = dir.0.join("deep").join("er").join("town.map.ron");
    atomic_file::write(&target, "one").unwrap_or_else(|e| panic!("{e}"));
    atomic_file::write(&target, "two").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(std::fs::read_to_string(&target).unwrap_or_default(), "two");
    let beside: Vec<String> = std::fs::read_dir(target.parent().unwrap_or(&dir.0))
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(beside, ["town.map.ron"], "a write left something behind");
}

/// **A write that cannot land leaves the target alone and no temp file.** The target here is a
/// directory, which `rename` of a file will not replace.
#[test]
fn a_refused_write_changes_nothing_and_leaves_no_temp_file() {
    let dir = Scratch::new("refused");
    let target = dir.0.join("town.map.ron");
    std::fs::create_dir(&target).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(target.join("inside.txt"), "kept").unwrap_or_else(|e| panic!("{e}"));

    assert!(atomic_file::write(&target, "new").is_err(), "a file was written over a directory");
    assert_eq!(std::fs::read_to_string(target.join("inside.txt")).unwrap_or_default(), "kept");
    assert_eq!(dir.names(), ["town.map.ron"], "the refused write left a temp file");
}

#[test]
fn write_new_refuses_to_replace_and_says_why() {
    let dir = Scratch::new("new");
    let target = dir.0.join("town.map.ron");
    atomic_file::write_new(&target, "first").unwrap_or_else(|e| panic!("{e}"));
    let refused = atomic_file::write_new(&target, "second").expect_err("a second write_new must be refused");
    assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_to_string(&target).unwrap_or_default(), "first");
    assert_eq!(dir.names(), ["town.map.ron"], "write_new left a temp file");
}

/// **A temp name is hidden and is not `.ron`**, so a loader that scans for `ron` files, and a watch
/// that skips dotfiles, never meet it; and two serials never share a name.
#[test]
fn a_temp_name_is_hidden_and_never_ron() {
    let target = Path::new("/project/maps/town.map.ron");
    let (a, b) = (atomic_file::temp_path(target, 0), atomic_file::temp_path(target, 1));
    assert_ne!(a, b, "two serials share a temp name");
    for tmp in [&a, &b] {
        assert_eq!(tmp.parent(), target.parent(), "the temp file must sit beside the target (a rename is atomic within one file system only)");
        assert!(atomic_file::is_temp(tmp), "{} is not recognised as a temp name", tmp.display());
        let name = tmp.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        assert!(name.starts_with('.'), "{name} is not hidden");
        assert!(tmp.extension().is_some_and(|e| e != "ron"), "{name} would be loaded as a ron file");
    }
    assert!(!atomic_file::is_temp(target), "the target itself is not a temp file");
    assert!(!atomic_file::is_temp(Path::new("/project/.live/game.ron")), "a hidden file is not a temp file");
}

/// **A dead writer's temp file goes at the next write, and a live one's does not.**
#[test]
fn the_next_write_clears_a_dead_writers_temp_file_and_spares_a_fresh_one() {
    let dir = Scratch::new("sweep");
    let target = dir.0.join("town.map.ron");
    let stale = atomic_file::temp_path(&target, 900_001);
    let fresh = atomic_file::temp_path(&target, 900_002);
    let other_targets = atomic_file::temp_path(&dir.0.join("kits.ron"), 900_003);
    for p in [&stale, &fresh, &other_targets] {
        std::fs::write(p, "half a file").unwrap_or_else(|e| panic!("{e}"));
    }
    age(&stale, STALE_AFTER + Duration::from_secs(60));
    age(&other_targets, STALE_AFTER + Duration::from_secs(60));

    atomic_file::write(&target, "whole").unwrap_or_else(|e| panic!("{e}"));

    assert!(!stale.exists(), "a dead writer's temp file was not cleared");
    assert!(fresh.exists(), "a writer that may still be running had its temp file removed");
    assert!(other_targets.exists(), "a write of one file cleared another file's temp file");
    assert_eq!(std::fs::read_to_string(&target).unwrap_or_default(), "whole");
}

/// **A leftover with the very name a write would pick is skipped over**, not overwritten and not an
/// error: a writer killed earlier by a process with this pid can leave one.
#[test]
fn a_leftover_temp_name_is_stepped_over() {
    let dir = Scratch::new("collide");
    let target = dir.0.join("town.map.ron");
    // The first 32 serials, which is where this binary's counter is (it has made a handful of writes);
    // a write tries 64 names before it gives up, so it must step over all of these and land.
    let mut taken = Vec::new();
    for serial in 0..32u64 {
        let p = atomic_file::temp_path(&target, serial);
        std::fs::write(&p, "someone else's bytes").unwrap_or_else(|e| panic!("{e}"));
        taken.push(p);
    }
    atomic_file::write(&target, "mine").unwrap_or_else(|e| panic!("a write refused to step over leftovers: {e}"));
    assert_eq!(std::fs::read_to_string(&target).unwrap_or_default(), "mine");
    for p in &taken {
        assert_eq!(std::fs::read_to_string(p).unwrap_or_default(), "someone else's bytes", "{} was overwritten", p.display());
    }
}

#[test]
fn write_cache_replaces_whole_too() {
    let dir = Scratch::new("cache");
    let target = dir.0.join("thumb.png");
    atomic_file::write_cache(&target, [1u8, 2, 3]).unwrap_or_else(|e| panic!("{e}"));
    atomic_file::write_cache(&target, [4u8, 5]).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(std::fs::read(&target).unwrap_or_default(), [4, 5]);
    assert_eq!(dir.names(), ["thumb.png"]);
}
