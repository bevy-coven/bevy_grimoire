//! **A write of a project file is all or nothing, for a reader, for a crash and for a second writer**
//! (#199, H2). Everything here goes through `ron_surgery::save_atomic`, the call every operation
//! makes, so each test measures what an editor save does.
//!
//! - A reader that parses the map in a loop while it is replaced never sees a torn file.
//! - A writer **killed at random points** leaves the old file or the new one, never a mix.
//! - Two writers of one path do not corrupt each other: they never share a temp file.
//! - The temp file is never named like a watched file, and a rename-replace is one reload.
//!
//! What is *not* claimed here is durability against power loss (`atomic_file`'s module docs say why).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bevy_grimoire_core::live::{FileWatch, POLL_SECS};
use bevy_grimoire_core::map::{Map, Placed};
use bevy_grimoire_core::ron_surgery::save_atomic;

/// A scratch directory under the OS's temp, removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_atomic_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A map with `rows` placements at `yaw`: a few kilobytes, so a torn write is a plausible one.
fn map_text(rows: usize, yaw: f32) -> String {
    let placements = (0..rows)
        .map(|i| Placed {
            id: format!("p{i}"),
            descriptor: "town/crate".to_owned(),
            at: (i as f32, 0.0),
            yaw,
            ..Default::default()
        })
        .collect();
    Map { placements, ..Default::default() }.to_ron().unwrap_or_else(|e| panic!("{e}"))
}

/// **Replace `target` `times` times while a reader thread parses it in a loop.** Every read must
/// parse, and must be one of the two whole maps.
fn read_while_replacing(target: &Path, times: usize, replace: impl Fn(&Path, &str) -> Result<(), String>) -> usize {
    let (a, b) = (map_text(60, 0.0), map_text(60, 90.0));
    replace(target, &a).unwrap_or_else(|e| panic!("{e}"));
    let done = AtomicBool::new(false);
    let reads = std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mut reads = 0usize;
            while !done.load(Ordering::Acquire) {
                let text = std::fs::read_to_string(target)
                    .unwrap_or_else(|e| panic!("the target vanished mid-replace: {e}"));
                assert!(
                    text == a || text == b,
                    "a reader saw a file that is neither whole map ({} bytes, expected {} or {})",
                    text.len(),
                    a.len(),
                    b.len()
                );
                Map::parse(&text).unwrap_or_else(|e| panic!("a reader saw a map that does not parse: {e}"));
                reads += 1;
            }
            reads
        });
        for i in 0..times {
            replace(target, if i % 2 == 0 { &b } else { &a }).unwrap_or_else(|e| panic!("{e}"));
        }
        done.store(true, Ordering::Release);
        reader.join().unwrap_or_else(|_| panic!("the reader thread panicked"))
    });
    assert!(reads > 20, "the reader only read {reads} times, so the test did not overlap the writer");
    reads
}

#[test]
fn a_reader_never_sees_a_torn_map_while_it_is_replaced() {
    let dir = Scratch::new("reader");
    // The editor's own save, flushes included.
    read_while_replacing(&dir.0.join("town.map.ron"), 300, |p, t| save_atomic(p, t));
}

#[test]
fn a_reader_never_sees_a_torn_map_while_it_is_replaced_ten_thousand_times() {
    let dir = Scratch::new("reader_10k");
    // The same replace without the flushes: a flush does not change what a *reader* can see, and
    // ten thousand of them is minutes on a Mac.
    read_while_replacing(&dir.0.join("town.map.ron"), 10_000, |p, t| {
        bevy_grimoire_core::atomic_file::write_cache(p, t).map_err(|e| e.to_string())
    });
}

// ---------------------------------------------------------------------------------------------
// A writer that dies
// ---------------------------------------------------------------------------------------------

const KILL_TARGET_ENV: &str = "GRIMOIRE_ATOMIC_KILL_TARGET";
const PAYLOAD: usize = 2 * 1024 * 1024;

fn payload(byte: u8) -> String {
    (byte as char).to_string().repeat(PAYLOAD)
}

/// **Not a test by itself**: the child process the kill test starts. Run bare it does nothing.
/// Handed a target in the environment it replaces it forever, with two whole payloads in turn.
#[test]
fn a_writer_to_kill() {
    let Ok(target) = std::env::var(KILL_TARGET_ENV) else { return };
    let target = PathBuf::from(target);
    std::fs::write(target.with_extension("started"), "1").unwrap_or_else(|e| panic!("{e}"));
    let (a, b) = (payload(b'a'), payload(b'b'));
    loop {
        let _ = save_atomic(&target, &a);
        let _ = save_atomic(&target, &b);
    }
}

#[test]
fn a_writer_killed_at_random_points_leaves_the_old_file_or_the_new_one() {
    let dir = Scratch::new("kill");
    let target = dir.0.join("town.map.ron");
    let started = target.with_extension("started");
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("{e}"));
    let (a, b, old) = (payload(b'a'), payload(b'b'), payload(b'o'));
    std::fs::write(&target, &old).unwrap_or_else(|e| panic!("{e}"));

    // A small deterministic generator: the delays vary, and a failure reruns the same way.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let (mut saw_old, mut saw_a, mut saw_b) = (0, 0, 0);
    for trial in 0..40 {
        let _ = std::fs::remove_file(&started);
        let mut child = Command::new(&exe)
            .args(["--exact", "a_writer_to_kill", "--test-threads=1"])
            .env(KILL_TARGET_ENV, &target)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start the writer: {e}"));
        let waited = Instant::now();
        while !started.exists() {
            assert!(waited.elapsed() < Duration::from_secs(30), "the writer never started (trial {trial})");
            std::thread::sleep(Duration::from_millis(1));
        }
        // Somewhere inside the writer's loop, anywhere from its first write to the thirtieth.
        std::thread::sleep(Duration::from_micros(next() % 60_000));
        child.kill().unwrap_or_else(|e| panic!("cannot kill the writer: {e}"));
        let _ = child.wait();

        let got = std::fs::read_to_string(&target).unwrap_or_else(|e| panic!("trial {trial}: the target is gone: {e}"));
        if got == old {
            saw_old += 1;
        } else if got == a {
            saw_a += 1;
        } else if got == b {
            saw_b += 1;
        } else {
            panic!(
                "trial {trial}: the writer was killed and the target is neither file ({} bytes, first byte {:?})",
                got.len(),
                got.bytes().next()
            );
        }
    }
    eprintln!("kill trials: {saw_old} old, {saw_a} a, {saw_b} b");
    assert!(saw_a + saw_b > 0, "no trial ever got a write in, so the writer was killed too early to test anything");
}

// ---------------------------------------------------------------------------------------------
// Two writers
// ---------------------------------------------------------------------------------------------

#[test]
fn two_writers_of_one_path_do_not_corrupt_each_other() {
    let dir = Scratch::new("writers");
    let target = dir.0.join("town.map.ron");
    let payloads: Vec<String> = [b'w', b'x', b'y', b'z'].into_iter().map(|byte| (byte as char).to_string().repeat(256 * 1024)).collect();
    save_atomic(&target, &payloads[0]).unwrap_or_else(|e| panic!("{e}"));

    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            while !done.load(Ordering::Acquire) {
                let text = std::fs::read_to_string(&target).unwrap_or_else(|e| panic!("the target vanished: {e}"));
                assert!(payloads.contains(&text), "a reader saw a mix of two writers ({} bytes)", text.len());
            }
        });
        let writers: Vec<_> = payloads
            .iter()
            .map(|text| {
                let target = &target;
                scope.spawn(move || {
                    for _ in 0..100 {
                        save_atomic(target, text).unwrap_or_else(|e| panic!("a writer was refused by another: {e}"));
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap_or_else(|_| panic!("a writer panicked"));
        }
        done.store(true, Ordering::Release);
        reader.join().unwrap_or_else(|_| panic!("the reader panicked"));
    });

    let last = std::fs::read_to_string(&target).unwrap_or_else(|e| panic!("{e}"));
    assert!(payloads.contains(&last), "the file that remains is a mix");
    let left: Vec<String> = std::fs::read_dir(&dir.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != "town.map.ron")
        .collect();
    assert!(left.is_empty(), "the writers left files behind: {left:?}");
}

// ---------------------------------------------------------------------------------------------
// The watcher
// ---------------------------------------------------------------------------------------------

/// **No name a write ever has, other than the target's, is a `.ron` file**, so a watch over `ron`
/// files and a loader that scans for them never meet a half-written file under any name.
#[test]
fn no_file_a_write_makes_but_the_target_is_a_ron_file() {
    let dir = Scratch::new("names");
    let target = dir.0.join("town.map.ron");
    let done = AtomicBool::new(false);
    let seen: BTreeSet<String> = std::thread::scope(|scope| {
        let looker = scope.spawn(|| {
            let mut seen = BTreeSet::new();
            while !done.load(Ordering::Acquire) {
                for entry in std::fs::read_dir(&dir.0).into_iter().flatten().flatten() {
                    seen.insert(entry.file_name().to_string_lossy().into_owned());
                }
            }
            seen
        });
        let text = map_text(200, 0.0);
        for _ in 0..200 {
            save_atomic(&target, &text).unwrap_or_else(|e| panic!("{e}"));
        }
        done.store(true, Ordering::Release);
        looker.join().unwrap_or_else(|_| panic!("the looker panicked"))
    });
    for name in &seen {
        if name == "town.map.ron" {
            continue;
        }
        assert!(
            Path::new(name).extension().is_none_or(|e| e != "ron"),
            "a write made `{name}`, which a watch of `ron` files would load"
        );
    }
    assert!(seen.len() > 1, "the looker never caught a temp file ({seen:?}), so it proved nothing");
}

/// **A rename-replace is one reload**: the new file arrives under the old name in one step, and the
/// watch fires once after it settles. Two replaces inside the settle window are still one.
#[test]
fn a_rename_replaced_file_is_loaded_once() {
    let dir = Scratch::new("once");
    let target = dir.0.join("maps").join("town.map.ron");
    save_atomic(&target, &map_text(5, 0.0)).unwrap_or_else(|e| panic!("{e}"));
    let mut watch = FileWatch::new(&dir.0, &["ron"]);

    // A file system's clock can be as coarse as a few milliseconds, so a replace made at once after
    // the last stat would share its mtime and be invisible to a watch that compares mtimes.
    let tick = || std::thread::sleep(Duration::from_millis(30));
    let fired = |watch: &mut FileWatch, from: usize| (from..from + 12).filter(|step| watch.poll(*step as f64 * POLL_SECS)).count();
    assert_eq!(fired(&mut watch, 0), 0, "nothing changed and the watch fired");

    tick();
    save_atomic(&target, &map_text(5, 90.0)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(fired(&mut watch, 12), 1, "one replace must be one reload");

    // Two saves back to back, before the watch has looked at all.
    tick();
    save_atomic(&target, &map_text(5, 180.0)).unwrap_or_else(|e| panic!("{e}"));
    save_atomic(&target, &map_text(5, 270.0)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(fired(&mut watch, 24), 1, "two replaces inside the settle window must be one reload");
}
