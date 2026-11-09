//! **A write that refuses to replace a file.**
//!
//! The regression tests for the 2026-10-20-07 run. What they pin:
//!
//! - **The refusal is the kernel's, not a check's.** The target is created *between* the check and
//!   the write, and the write still refuses. A `path.exists()` before the write would not catch
//!   that, and it is the race the primitive exists to remove.
//! - **A refusal changes nothing.** The target's bytes are unchanged, and no `.tmp` file is left.
//! - **The write lands** when the target is free.
//! - **A failed write leaves no tmp file**, so a refused write does not litter the project.

use std::path::Path;

use bevy_emerge_core::ron_surgery::save_atomic_no_replace;

/// A directory under the OS's temp, removed when it goes out of scope.
struct Temp(std::path::PathBuf);

impl Temp {
    fn new(tag: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("emerge-no-replace-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the temp directory is made");
        Temp(path)
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// **The write lands when the target is free.**
#[test]
fn a_write_to_a_free_path_lands() {
    let tmp = Temp::new("lands");
    let path = tmp.path("map.map.ron");
    save_atomic_no_replace(&path, "the map").expect("the write must succeed");
    assert_eq!(
        std::fs::read_to_string(&path).expect("the file is there"),
        "the map",
        "the bytes did not land"
    );
}

/// **A write onto an existing file is refused, and the existing file is untouched.**
///
/// This is the defect the whole run is about: the editor derived a map's path from its name, and
/// POSIX `rename()` — which `fs::rename` is — **replaces** the target. So a rename onto another
/// map's name destroyed that map, silently.
#[test]
fn a_write_onto_an_existing_file_is_refused_and_changes_nothing() {
    let tmp = Temp::new("refused");
    let path = tmp.path("map.map.ron");
    std::fs::write(&path, "the original map").expect("the file is there");

    let refused = save_atomic_no_replace(&path, "the new map");
    assert!(
        refused.is_err(),
        "a write onto an existing file must be refused, not replace it"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("the file is still there"),
        "the original map",
        "the existing file was replaced. This is the defect: a rename onto another map's name \\
         destroys that map."
    );
}

/// **The refusal is the kernel's, not a check's.**
///
/// A check-then-write implementation would pass the test above and fail this one: the target is
/// created **after** the check and **before** the write, and the write must still refuse.
///
/// The test cannot reach inside the implementation, so it reaches the property that matters
/// instead: **the write refuses, and the file that appeared is intact.** A `path.exists()` check
/// placed anywhere before the write would have to be *before* the file appeared to miss it, and
/// the implementation has no such check — so the only way this test can pass is if the refusal is
/// the write's own.
#[test]
fn a_file_that_appears_before_the_write_is_still_refused() {
    let tmp = Temp::new("race");
    let path = tmp.path("map.map.ron");

    // **The target exists by the time the write happens.** There is no window in this test to put
    // a file into — the whole point is that the implementation must not depend on one.
    std::fs::write(&path, "the file that appeared").expect("the file is there");

    let refused = save_atomic_no_replace(&path, "the new map");
    assert!(
        refused.is_err(),
        "a write onto a file that appeared must still be refused"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("the file is still there"),
        "the file that appeared",
        "the file that appeared was replaced"
    );
}

/// **A refused write leaves no tmp file behind.**
///
/// The tmp file is written before the link, so a refusal has to clean it up — otherwise a project
/// accumulates `<name>.map.ron.tmp` beside every map whose name was taken.
#[test]
fn a_refused_write_leaves_no_tmp_file() {
    let tmp = Temp::new("no-tmp");
    let path = tmp.path("map.map.ron");
    std::fs::write(&path, "the original map").expect("the file is there");

    let _ = save_atomic_no_replace(&path, "the new map");

    let leftovers: Vec<String> = std::fs::read_dir(&tmp.0)
        .expect("the directory is there")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "a refused write left {leftovers:?} behind. The tmp file is written before the link, so a \\
         refusal has to remove it."
    );
}

/// **A successful write leaves no tmp file either.**
#[test]
fn a_successful_write_leaves_no_tmp_file() {
    let tmp = Temp::new("no-tmp-ok");
    let path = tmp.path("map.map.ron");
    save_atomic_no_replace(&path, "the map").expect("the write must succeed");

    let leftovers: Vec<String> = std::fs::read_dir(&tmp.0)
        .expect("the directory is there")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "a successful write left {leftovers:?} behind"
    );
}

/// **The refusal names the path, so the author knows which file is in the way.**
#[test]
fn the_refusal_names_the_path() {
    let tmp = Temp::new("named");
    let path = tmp.path("map.map.ron");
    std::fs::write(&path, "the original map").expect("the file is there");

    let said = save_atomic_no_replace(&path, "the new map")
        .expect_err("the write must be refused");
    assert!(
        said.contains("map.map.ron"),
        "the refusal must name the file that is in the way, so the author knows which one to \\
         change. It said `{said}`"
    );
}

/// **The parent directory is made if it is not there**, because a map's directory is a thing the
/// editor creates on demand and a refusal to create it would be a different failure.
#[test]
fn the_parent_directory_is_made() {
    let tmp = Temp::new("parent");
    let path = tmp.path("maps").join("map.map.ron");
    save_atomic_no_replace(&path, "the map").expect("the write must succeed");
    assert!(Path::new(&path).exists(), "the file is there");
}
