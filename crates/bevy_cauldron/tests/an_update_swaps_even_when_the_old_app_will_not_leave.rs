//! **An update's swap script finishes the swap whether or not the old app ever leaves** (#337).
//!
//! The script waited for the old process with `while kill -0 "$pid"` and no limit, and it ran in the app's
//! own process group. An old app that hung on its way out (its window gone, its process alive) kept the
//! script waiting, and the only thing a person could do, kill the app by name or by group, killed the script
//! too, so `/Applications` kept the old version and the app had to be started by hand. Now the script asks the
//! old process to end after `term_after_s`, makes it after `kill_after_s`, and is started in a group of its own.
#![cfg(unix)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use bevy_cauldron::update::Swap;
use rustix::process::{Pid, getpgid};

/// Seconds the old process has before it is asked to end, and before it is made to: short, so the test is.
const TERM_AFTER_S: u32 = 1;
const KILL_AFTER_S: u32 = 2;

/// The most the test waits for anything to happen, seconds. A swap that never ends fails here, not hangs.
const PATIENCE_S: u64 = 30;

/// A scratch folder laid out as an install: the running app, the staged one, and a stand-in for `open`.
struct Install {
    root: PathBuf,
    running: PathBuf,
    staged: PathBuf,
    opener: PathBuf,
    script: PathBuf,
}

impl Install {
    fn new(name: &str) -> Install {
        let root = std::env::temp_dir().join(format!("swap_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let running = root.join("Applications").join("Cauldron.app");
        let staged = root.join("updates").join("9.9.9").join("unpacked").join("Cauldron.app");
        for (dir, says) in [(&running, "old"), (&staged, "new")] {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            std::fs::write(dir.join("version"), says).unwrap_or_else(|e| panic!("{e}"));
        }
        let opener = root.join("fake_open.sh");
        // What the relaunch was asked to open, one argument a line.
        std::fs::write(&opener, format!("#!/bin/sh\nfor a in \"$@\"; do echo \"$a\" >> '{}'; done\n", root.join("opened.txt").display()))
            .unwrap_or_else(|e| panic!("{e}"));
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
        }
        let script = root.join("updates").join("9.9.9").join("swap.sh");
        Install { root, running, staged, opener, script }
    }

    fn swap(&self, old_pid: u32) -> Swap<'_> {
        Swap {
            pid: old_pid,
            running: &self.running,
            new: &self.staged,
            relaunch: self.opener.to_str().unwrap_or_else(|| panic!("a utf-8 path")),
            term_after_s: TERM_AFTER_S,
            kill_after_s: KILL_AFTER_S,
            args: vec!["--project".into(), "a b".into()],
        }
    }

    fn version_installed(&self) -> String {
        std::fs::read_to_string(self.running.join("version")).unwrap_or_default()
    }

    fn opened(&self) -> String {
        std::fs::read_to_string(self.root.join("opened.txt")).unwrap_or_default()
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Waits for `child` to end, failing the test rather than hanging it.
fn finished(child: &mut Child, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(PATIENCE_S);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                panic!("{what} was still running after {PATIENCE_S} s");
            }
            Err(e) => panic!("{what}: {e}"),
        }
    }
}

/// A stand-in for the old app that will not leave when asked: it ignores `SIGTERM` and `SIGHUP`.
fn the_old_app_that_will_not_leave() -> Child {
    let old = Command::new("/bin/sh")
        .args(["-c", "trap '' TERM HUP; exec sleep 120"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("{e}"));
    // The trap and the exec both happen in the first moments; the script reads the program's name when it starts.
    std::thread::sleep(Duration::from_millis(300));
    old
}

#[test]
fn the_swap_ends_an_old_app_that_will_not_leave_and_finishes() {
    let install = Install::new("stubborn");
    let mut old = the_old_app_that_will_not_leave();
    let mut swap = install.swap(old.id()).start(&install.script).unwrap_or_else(|e| panic!("{e}"));

    // The old app is the test's child, so the test collects it once the script has ended it.
    finished(&mut old, "the old app that ignores SIGTERM");
    finished(&mut swap, "the swap script");

    assert_eq!(install.version_installed(), "new", "the new app is in place of the old one");
    assert!(!install.staged.exists(), "the staged app was moved, not copied");
    assert!(!install.running.with_extension("app.previous").exists(), "the old app is not left beside it");
    assert_eq!(
        install.opened().lines().collect::<Vec<_>>(),
        [install.running.to_str().unwrap_or_default(), "--args", "--project", "a b"],
        "the new app was opened with the old one's arguments"
    );
}

#[test]
fn an_old_app_that_has_already_left_does_not_cost_the_wait() {
    let install = Install::new("gone");
    let mut old = Command::new("/bin/sh").args(["-c", "exit 0"]).spawn().unwrap_or_else(|e| panic!("{e}"));
    let old_pid = old.id();
    finished(&mut old, "the old app that exits");

    let started = Instant::now();
    let mut swap = install.swap(old_pid).start(&install.script).unwrap_or_else(|e| panic!("{e}"));
    finished(&mut swap, "the swap script");
    assert!(
        started.elapsed() < Duration::from_secs(u64::from(TERM_AFTER_S)),
        "a process that is gone is not waited for: took {:?}",
        started.elapsed()
    );
    assert_eq!(install.version_installed(), "new");
}

#[test]
fn the_script_does_not_share_the_apps_process_group() {
    let install = Install::new("group");
    let mut old = the_old_app_that_will_not_leave();
    let mut swap = install.swap(old.id()).start(&install.script).unwrap_or_else(|e| panic!("{e}"));

    let ours = getpgid(None).unwrap_or_else(|e| panic!("{e}"));
    let script = i32::try_from(swap.id()).ok().and_then(Pid::from_raw).unwrap_or_else(|| panic!("a pid"));
    let its = getpgid(Some(script)).unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(its, ours, "a group kill of the app would end the swap with it");
    assert_eq!(its, script, "the script leads a group of its own");

    finished(&mut old, "the old app");
    finished(&mut swap, "the swap script");
}
