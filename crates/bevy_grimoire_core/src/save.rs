//! **The one way a project file reaches the disk** (#199, H2): atomically, and — when the name is
//! derived from something a person typed — refusing to replace what is already there.
//!
//! Both functions are [`crate::atomic_file`] and nothing else. They exist under these names because
//! every operation's save goes through them, and because the *choice* between replacing and refusing is
//! a decision each caller has to make: a save to a path the caller chose replaces, and a write to a name
//! built from an author's typed name refuses.
//!
//! **This was `ron_surgery`** until the project's files became YAML (GD-03, #365). The rest of that module
//! — a RON span scanner, a RON line rewriter and the comment-preserving splice they served — went with
//! RON: a project file is edited by [`crate::yaml_surgery`] now, which works on YAML's own structure
//! instead of on a line's punctuation.

use std::path::Path;

/// Write `text` to `path` atomically: a reader sees the old file or the new one, never half of
/// either (#199). All of it is [`crate::atomic_file::write`]: a uniquely named hidden temp file
/// beside the target, flushed, renamed over it, then the directory flushed.
///
/// The caller is expected to re-load and re-validate afterwards; this only guarantees the bytes
/// landed.
///
/// **And it replaces.** `fs::rename` is POSIX `rename()`, and the standard specifies that it
/// removes an existing target: *"if the directory entry named by `new` exists, it shall be removed
/// and `old` renamed to `new`"* (IEEE Std 1003.1-2024). So a save to a path that already holds a
/// file **overwrites it, silently**. That is the right default for a save — the caller chose the
/// path — and the wrong default for anything where the path is *derived* from a name the author
/// typed. **Use [`save_atomic_no_replace`] there.**
pub fn save_atomic(path: &Path, text: &str) -> Result<(), String> {
    crate::atomic_file::write(path, text).map_err(|e| format!("save: {}: {e}", path.display()))
}

/// **Write `text` to `path`, and refuse if something is already there.**
///
/// # Why the refusal is the kernel's
///
/// The obvious implementation is a check and then a write:
///
/// ```text
/// if path.exists() { return Err(...) }
/// save_atomic(path, text)
/// ```
///
/// **and it is wrong, in a way that only shows up under a race.** The check and the write are two
/// calls, and POSIX gives atomicity to the single call only: *"if the `rename()` function fails for
/// any reason other than [EIO], any file named by `new` shall be unaffected"*. Between the check
/// and the write, the target can appear — and then `save_atomic`'s `fs::rename` removes it, which
/// is the exact failure this function exists to prevent.
///
/// **So the check and the write are one call.** The primitive is `link(2)`: creating a hard link at
/// the target **fails with `EEXIST` if the target exists**, and it never modifies the target. It is
/// POSIX, it is on every filesystem this crate runs on, and it is **not** `rename` — so the
/// "replace" behaviour is not available to it at all.
///
/// # Why `link` and not `renameat2(RENAME_NOREPLACE)`
///
/// `renameat2` with `RENAME_NOREPLACE` is the exact primitive, and it is the right one **where it
/// exists**. It is Linux-only, and the editor runs on macOS. `renamex_np(RENAME_EXCL)` is the
/// macOS equivalent, and it is a libc call with an `unsafe` declaration — and this crate has no
/// `unsafe` today and no `libc` dependency, and adding both for one call is a larger change than
/// the problem needs.
///
/// **`link` is the portable primitive that has the property.** It creates the target as a second
/// name for the same inode, atomically, and refuses if the name is taken.
///
/// # The one thing `link` cannot do
///
/// **A hard link cannot cross a filesystem**, and `EXDEV` says so. So this refuses rather than
/// falling back — because a fallback is the defect this function exists to remove, and Lampson's
/// rule is the whole of it: *"Make actions atomic or restartable."* **A refusal leaves both names
/// standing; a fallback replaces one.**
///
/// The tmp file is written beside the target, so the link is always on the same filesystem as the
/// target and `EXDEV` cannot happen for the link itself — only for a target that is already
/// somewhere else, which is a refusal the author can act on.
///
/// # What it leaves behind
///
/// The tmp file is removed on every path, including the refusal, so a failed write does not leave a
/// hidden `.tmp` in the project. After a successful link the removal is best effort (the bytes are in
/// the target already, and the next write of this target clears a leftover once it is an hour old):
/// removing the tmp name is removing a *second name for the same inode*, not the data. The check and
/// the write are [`crate::atomic_file::write_new`].
pub fn save_atomic_no_replace(path: &Path, text: &str) -> Result<(), String> {
    crate::atomic_file::write_new(path, text).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("save: {} already exists, and this write refuses to replace it", path.display())
        } else {
            format!("save: {}: {e}", path.display())
        }
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    /// **A save replaces what is there**, which is the caller's choice of path, and the bytes are the
    /// bytes it was given.
    #[test]
    fn a_save_replaces_the_file_it_names() {
        let dir = std::env::temp_dir().join(format!("grimoire-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("kits.yaml");
        save_atomic(&path, "first").unwrap_or_else(|e| panic!("{e}"));
        save_atomic(&path, "second").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}")), "second");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A no-replace write refuses a name that is taken, and leaves what was there untouched.**
    #[test]
    fn a_write_that_refuses_to_replace_leaves_the_file_it_met() {
        let dir = std::env::temp_dir().join(format!("grimoire-save-nr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("maps").join("map.map.yaml");
        save_atomic_no_replace(&path, "the map").unwrap_or_else(|e| panic!("{e}"));
        let said = save_atomic_no_replace(&path, "the new map").expect_err("the name is taken");
        assert!(said.contains("already exists"), "{said}");
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}")), "the map");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
