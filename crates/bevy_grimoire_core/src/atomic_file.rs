//! **The one way the editor writes a file a person or a running game may read: a reader sees the old
//! file or the new one, never a mix** (#199, H2).
//!
//! A running game, and the editor's own [`crate::live::FileWatch`], can read a file while the editor
//! is writing it. `std::fs::write` truncates the target and then fills it, so a reader in between
//! parses an empty or half-written file (#187, #158). Every write of a project file goes through
//! [`write`] (or [`write_new`], which refuses to replace), and a ratchet test
//! (`tests/atomic_writes_only.rs`) fails on a new plain `fs::write`, `File::create`, `fs::copy`
//! or `OpenOptions` in the non-test source of `bevy_grimoire_core` and `bevy_cauldron`
//! (a write through another API, such as a bare imported `write(`, is not seen by it).
//!
//! # What a write does
//!
//! 1. A temp file **in the target's own directory** (a rename is atomic only within one file
//!    system), with a name no other writer shares: `.<name>.<pid>-<serial>.tmp`, created with
//!    `create_new`. The pid separates processes and the serial separates threads, and a leftover
//!    of a dead process with the same pid is stepped over. Only the threads are tested.
//! 2. The bytes, then [`File::sync_all`](std::fs::File::sync_all).
//! 3. `rename` over the target (or `link`, for [`write_new`]).
//! 4. A flush of the directory.
//!
//! **The temp name is never a watched pattern.** It starts with `.` and ends in `.tmp`, so it is not
//! a `.ron` file and [`crate::live::FileWatch`] skips every dotfile: a reader that scans a project
//! never sees a half-written file under any name it loads. [`is_temp`] says which names these are.
//!
//! # What is claimed, and what is tested
//!
//! **Claimed and tested:** no torn read (a reader thread parses the target while it is replaced ten
//! thousand times without the flushes, and three hundred times with them), no half-written target
//! when the writer dies (a child process is killed at random points), and four writers of one path
//! do not corrupt each other (`tests/atomic_writes.rs`).
//!
//! **Not claimed: durability against power loss.** Steps 2 and 4 are issued, and on macOS
//! `File::sync_all` is `fcntl(F_FULLFSYNC)` (about 6 ms a call here, against 0.1 ms for plain
//! `fsync`, measured on APFS), which asks the drive to flush its cache too. What a given file
//! system then guarantees after a power cut is not something a test here can show, so nothing here
//! promises it. A failure to flush the *directory* is ignored: the file is already in place, and
//! not every file system lets a directory be opened.
//!
//! # A crash leaves a temp file, and the next write clears it
//!
//! A writer killed between steps 1 and 3 leaves its temp file. It is invisible to the editor and the
//! game, and the next [`write`] or [`write_new`] of the same target removes any such file older
//! than [`STALE_AFTER`] (never a fresh one, which may be another writer's, mid-write).
//! [`write_cache`] does not look: a cache directory can hold thousands of files.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

/// A temp file this old is a dead writer's: no write takes an hour.
pub const STALE_AFTER: Duration = Duration::from_secs(3600);

/// The suffix of every temp name, without its dot.
const TEMP_EXTENSION: &str = "tmp";

/// How many serials one write tries before it gives up on finding an unused temp name.
const TEMP_TRIES: u32 = 64;

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flush {
    Full,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Land {
    Replace,
    Refuse,
}

/// **Write `contents` to `path`, replacing what is there**, all or nothing. Same call shape as
/// `std::fs::write`, and the parent directory is created if it is missing.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    put(path, contents.as_ref(), Flush::Full, Land::Replace)
}

/// **Write `contents` to `path`, and refuse with `ErrorKind::AlreadyExists` if something is there.**
/// The refusal is the kernel's: `link(2)` fails with `EEXIST` and never touches the target, where a
/// check followed by a write leaves a window in which the target appears and is then replaced.
/// Use it where the path is *derived* from a name the author typed (a map's file, a kit's folder).
/// A hard link cannot cross a file system, and the temp file sits beside the target, so it never
/// has to.
pub fn write_new(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    put(path, contents.as_ref(), Flush::Full, Land::Refuse)
}

/// **[`write`] without the flushes**, for a cache the editor can rebuild (a thumbnail, the animation
/// bench's cache). A reader still never sees a mix; only a power cut may lose the file, and a cache
/// is not worth six milliseconds on every entry.
pub fn write_cache(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    put(path, contents.as_ref(), Flush::None, Land::Replace)
}

/// The temp name a write of `target` uses for `serial`: beside the target, hidden, and not `.ron`.
pub fn temp_path(target: &Path, serial: u64) -> PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = format!(".{name}.{}-{serial}.{TEMP_EXTENSION}", std::process::id());
    match target.parent() {
        Some(dir) => dir.join(temp),
        None => PathBuf::from(temp),
    }
}

/// Whether `path` is named the way a write's temp file is (a hidden `.tmp`). A reader that wants
/// "every file a person authored" skips these.
pub fn is_temp(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
        && path.extension().is_some_and(|e| e == TEMP_EXTENSION)
}

fn put(path: &Path, bytes: &[u8], flush: Flush, land: Land) -> io::Result<()> {
    let parent = match path.parent() {
        Some(p) if p.as_os_str().is_empty() => Path::new("."),
        Some(p) => p,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} has no parent directory", path.display()),
            ));
        }
    };
    fs::create_dir_all(parent)?;
    let (tmp, mut file) = create_temp(path)?;
    let written = file.write_all(bytes).and_then(|()| match flush {
        Flush::Full => file.sync_all(),
        Flush::None => Ok(()),
    });
    drop(file);
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let landed = match land {
        Land::Replace => fs::rename(&tmp, path),
        // The link is the check. The temp name goes either way: a refusal leaves the author's file
        // exactly as it was, and after a success the target is a second name for the same inode.
        Land::Refuse => fs::hard_link(&tmp, path),
    };
    if land == Land::Refuse || landed.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    landed?;
    if flush == Flush::Full {
        let _ = File::open(parent).and_then(|dir| dir.sync_all());
        sweep_stale(parent, path);
    }
    Ok(())
}

/// A new, empty temp file for `path`, on a name nothing else holds.
fn create_temp(path: &Path) -> io::Result<(PathBuf, File)> {
    let mut last = None;
    for _ in 0..TEMP_TRIES {
        let tmp = temp_path(path, SERIAL.fetch_add(1, Ordering::Relaxed));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            // A dead writer with this pid left this name: take the next serial.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("no unused temp name")))
}

/// Whether `file_name` is a temp file of the target called `target_name`, exactly: `.<target_name>.`,
/// then `<digits>-<digits>`, then `.tmp`. `town.map.ron.bak`'s temp files are not `town.map.ron`'s.
fn is_temp_of(target_name: &str, file_name: &str) -> bool {
    let Some(rest) = file_name.strip_prefix('.').and_then(|r| r.strip_prefix(target_name)).and_then(|r| r.strip_prefix('.'))
    else {
        return false;
    };
    let Some(pid_serial) = rest.strip_suffix(".tmp") else { return false };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    pid_serial.split_once('-').is_some_and(|(pid, serial)| digits(pid) && digits(serial))
}

/// Remove `target`'s temp files that a dead writer left: older than [`STALE_AFTER`]. Best effort.
fn sweep_stale(parent: &Path, target: &Path) {
    let Some(name) = target.file_name() else { return };
    let name = name.to_string_lossy();
    let Ok(entries) = fs::read_dir(parent) else { return };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        if !is_temp_of(&name, &entry.file_name().to_string_lossy()) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age >= STALE_AFTER);
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}
