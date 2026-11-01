//! **Measures #204's spike: how fast, and whether at all, each watcher notices a write to
//! `standards.ron`.**
//!
//! ```text
//! cargo run --release -p bevy_grimoire_mapper --features asset_spike --example asset_spike_measure -- [trials] [gap between the three files of a save, ms]
//! ```
//!
//! For each way of writing (in place, as the writers did before #199; replace by rename, as every
//! writer does since) it makes a fresh directory, starts Bevy's asset watcher, two [`FileWatch`]es
//! over it (the project's default timing, and the 0.1 s one the family settings use) and the editor's
//! own 250 ms stat poll, writes a new text and steps all of them every 4 ms until each has seen it or three seconds have
//! passed. Where the write falls in each watcher's cycle is spread over the trials. A frame here is one 4 ms step; an editor's frame is longer, which adds the same to every
//! column. Prints one row per way of writing and watcher: how many trials saw the write, and the
//! latency in ms (min, median, max) and in frames.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy_grimoire_core::atomic_file;
use bevy_grimoire_core::live::FileWatch;
use bevy_grimoire_core::standards::{Dim, Rule, Standards};
use bevy_grimoire_mapper::asset_spike::{Heard, Heard1, app_over};

const STEP: Duration = Duration::from_millis(4);
const GIVE_UP: Duration = Duration::from_secs(3);

/// The scratch directory's parent, with symlinks resolved. Bevy's watcher strips its root from the paths the
/// platform reports, and macOS reports `/private/var/...` for a root given as `/var/...` (the temp dir), which
/// panics Bevy's watcher thread (`get_asset_path() failed to strip prefix`): a root has to be canonical.
fn scratch_root() -> PathBuf {
    std::env::temp_dir().canonicalize().expect("a temp dir")
}

fn standards(n: u32) -> Standards {
    Standards { version: 1, rules: vec![Rule { name: format!("r{n}"), kind: None, dim: Dim::Width, min: Some(n as f32), max: None, multiple_of: None }] }
}

fn text(n: u32) -> String {
    standards(n).to_ron().expect("a standards text")
}

#[derive(Clone, Copy)]
enum Way {
    InPlace,
    Rename,
}

impl Way {
    fn write(self, path: &Path, text: &str) {
        match self {
            Way::InPlace => std::fs::write(path, text).expect("write in place"),
            Way::Rename => atomic_file::write(path, text).expect("write by rename"),
        }
    }
}

/// `(ms, frames)` at which a backend saw the write, or `None`.
type Seen = Option<(f64, u32)>;

fn trial(way: Way, n: u32, dir: &Path) -> [Seen; 4] {
    let file = dir.join("standards.ron");
    way.write(&file, &text(0));
    let (mut app, _handles) = app_over(dir, &["standards.ron"]);
    let begin = Instant::now();
    while !app.world().resource::<Heard>().events.iter().any(|h| matches!(h, Heard1::Loaded(_))) {
        assert!(begin.elapsed() < Duration::from_secs(10), "the asset never loaded the first text");
        app.update();
        std::thread::sleep(STEP);
    }
    // Let the watcher's start-up events pass, and the file system's clock tick, before the write.
    for _ in 0..250 {
        app.update();
        std::thread::sleep(STEP);
    }
    app.world_mut().resource_mut::<Heard>().events.clear();
    // A watch stats when it is made, so these two start from the file as it is before the write.
    let mut default_watch = FileWatch::new(dir, &["ron"]);
    let mut quick_watch = FileWatch::new(dir, &["ron"]).with_timing(0.1, 0.1);
    // The editor's own watcher (`mapper::watch::tick_watch`): the file's size and modification time
    // every 250 ms of the wall clock, a change at once and no settle time. The rule, on the one file.
    let stamp = || std::fs::metadata(&file).ok().map(|m| (m.len(), m.modified().ok()));
    let mut editor_stamp = stamp();
    // Where the write falls in each polling watcher's cycle is spread over trials (a golden-ratio sequence, so
    // every phase is covered): a write just after a poll waits a whole interval, one just before waits none.
    // Bevy's debouncer ticks (75 ms) on a clock of its own, started when it was made, so the write is delayed by
    // a varying fraction of a tick while the app steps; which phase of that tick it lands in is not controlled.
    let phase = (f64::from(n) * 0.618_033_988_75).fract();
    default_watch.poll(-phase * 0.5);
    quick_watch.poll(-phase * 0.1);
    let tick_wait = Instant::now() + Duration::from_secs_f64(phase * 0.075);
    while Instant::now() < tick_wait {
        app.update();
        std::thread::sleep(STEP);
    }
    let want = standards(n);
    way.write(&file, &text(n));
    let t0 = Instant::now();
    let mut editor_last = t0 - Duration::from_secs_f64(phase * 0.25);
    let (mut asset, mut default_seen, mut quick_seen, mut editor_seen): (Seen, Seen, Seen, Seen) = (None, None, None, None);
    let mut frames = 0u32;
    while t0.elapsed() < GIVE_UP && (asset.is_none() || default_seen.is_none() || quick_seen.is_none() || editor_seen.is_none()) {
        app.update();
        frames += 1;
        let now = t0.elapsed();
        if asset.is_none() && app.world().resource::<Heard>().events.iter().any(|h| *h == Heard1::Loaded(want.clone())) {
            asset = Some((now.as_secs_f64() * 1e3, frames));
        }
        // A watch says "due"; the reload then reads the file, which here must hold the new text.
        for (watch, seen) in [(&mut default_watch, &mut default_seen), (&mut quick_watch, &mut quick_seen)] {
            if seen.is_none() && watch.poll(now.as_secs_f64()) {
                let read = std::fs::read_to_string(&file).ok().and_then(|t| Standards::parse(&t).ok());
                if read.as_ref() == Some(&want) {
                    *seen = Some((now.as_secs_f64() * 1e3, frames));
                }
            }
        }
        if editor_seen.is_none() && editor_last.elapsed() >= Duration::from_millis(250) {
            editor_last = Instant::now();
            if stamp() != editor_stamp {
                editor_stamp = stamp();
                editor_seen = Some((now.as_secs_f64() * 1e3, frames));
            }
        }
        std::thread::sleep(STEP);
    }
    [asset, default_seen, quick_seen, editor_seen]
}

/// **A save of three files**, `gap_ms` apart, as the editor writes a map, the library and the compositions.
/// Returns the milliseconds from the first write to each file's arrival in the asset system (`None` for a
/// file that never arrived), and how many reloads a `FileWatch` over the directory said were due.
fn set_trial(n: u32, gap_ms: u64, dir: &Path) -> (Vec<Option<f64>>, u32) {
    const FILES: [&str; 3] = ["a.ron", "b.ron", "c.ron"];
    for file in FILES {
        atomic_file::write(&dir.join(file), text(0)).expect("a first text");
    }
    let (mut app, _handles) = app_over(dir, &FILES);
    let begin = Instant::now();
    while app.world().resource::<Heard>().events.len() < FILES.len() {
        assert!(begin.elapsed() < Duration::from_secs(10), "the three files never loaded");
        app.update();
        std::thread::sleep(STEP);
    }
    for _ in 0..250 {
        app.update();
        std::thread::sleep(STEP);
    }
    app.world_mut().resource_mut::<Heard>().events.clear();
    let mut watch = FileWatch::new(dir, &["ron"]);
    let mut next_write = 0;
    let (mut arrived, mut reloads) = (vec![None; FILES.len()], 0u32);
    let t0 = Instant::now();
    while t0.elapsed() < GIVE_UP {
        let now = t0.elapsed();
        if next_write < FILES.len() && now >= Duration::from_millis(gap_ms * next_write as u64) {
            atomic_file::write(&dir.join(FILES[next_write]), text(n * 10 + next_write as u32)).expect("a new text");
            next_write += 1;
        }
        app.update();
        if watch.poll(now.as_secs_f64()) {
            reloads += 1;
        }
        let heard = app.world().resource::<Heard>().events.clone();
        for (i, slot) in arrived.iter_mut().enumerate() {
            if slot.is_none() && heard.contains(&Heard1::Loaded(standards(n * 10 + i as u32))) {
                *slot = Some(now.as_secs_f64() * 1e3);
            }
        }
        std::thread::sleep(STEP);
    }
    (arrived, reloads)
}

fn summary(seen: &[Seen]) -> String {
    let mut ms: Vec<f64> = seen.iter().flatten().map(|s| s.0).collect();
    let mut frames: Vec<u32> = seen.iter().flatten().map(|s| s.1).collect();
    ms.sort_by(f64::total_cmp);
    frames.sort_unstable();
    if ms.is_empty() {
        return format!("saw {}/{}", 0, seen.len());
    }
    format!(
        "saw {}/{} | ms {:.0} / {:.0} / {:.0} | frames {} / {} / {}",
        ms.len(),
        seen.len(),
        ms[0],
        ms[ms.len() / 2],
        ms[ms.len() - 1],
        frames[0],
        frames[frames.len() / 2],
        frames[frames.len() - 1]
    )
}

fn main() {
    let trials: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(20);
    let gap_ms: u64 = std::env::args().nth(2).and_then(|a| a.parse().ok()).unwrap_or(25);
    println!("platform {} {}, {trials} trials per way of writing, step {} ms, give up after {} s", std::env::consts::OS, std::env::consts::ARCH, STEP.as_millis(), GIVE_UP.as_secs());
    for (name, way) in [("in place   ", Way::InPlace), ("by rename  ", Way::Rename)] {
        let mut rows: [Vec<Seen>; 4] = Default::default();
        for n in 1..=trials {
            let dir: PathBuf = scratch_root().join(format!("asset_spike_{}_{}", std::process::id(), n));
            std::fs::create_dir_all(&dir).expect("a scratch directory");
            let seen = trial(way, n, &dir);
            for (row, seen) in rows.iter_mut().zip(seen) {
                row.push(seen);
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        for (backend, row) in ["Bevy asset watcher     ", "FileWatch 0.5 s / 0.5 s ", "FileWatch 0.1 s / 0.1 s ", "editor stat, 250 ms     "].iter().zip(&rows) {
            println!("{name} {backend} {}", summary(row));
        }
    }
    let (mut spread, mut missing, mut reloads) = (Vec::new(), 0, Vec::new());
    for n in 1..=trials {
        let dir: PathBuf = scratch_root().join(format!("asset_spike_set_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let (arrived, due) = set_trial(n, gap_ms, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        reloads.push(due);
        let times: Vec<f64> = arrived.iter().flatten().copied().collect();
        missing += arrived.len() - times.len();
        if times.len() == arrived.len() {
            spread.push(times.iter().cloned().fold(f64::MIN, f64::max) - times.iter().cloned().fold(f64::MAX, f64::min));
        }
    }
    spread.sort_by(f64::total_cmp);
    reloads.sort_unstable();
    println!(
        "a save of 3 files {gap_ms} ms apart: Bevy arrivals {missing} missing; spread first to last file ms {:.0} / {:.0} / {:.0} (min / median / max), {} of {} saves split over more than one 60 fps frame; FileWatch reloads due per save {} / {} / {} (min / median / max)",
        spread.first().copied().unwrap_or(f64::NAN),
        spread.get(spread.len() / 2).copied().unwrap_or(f64::NAN),
        spread.last().copied().unwrap_or(f64::NAN),
        spread.iter().filter(|ms| **ms > 1e3 / 60.0).count(),
        spread.len(),
        reloads[0],
        reloads[reloads.len() / 2],
        reloads[reloads.len() - 1]
    );
}
