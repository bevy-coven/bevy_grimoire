//! **A flood of output cannot take the editor's frame** (#308).
//!
//! The terminal reads its PTY on the main thread, once a frame, until the kernel says there is
//! nothing left. That is only safe while a program cannot refill the buffer as fast as one `read`
//! empties it — the line discipline's own processing is what slows it — and these tests pin that
//! with a real `yes` in a real PTY, because the property belongs to the kernel and a mock would not
//! have it. Measured 2026-11-06 on big (Linux): a frame of `yes`, `seq` or `base64` reads 21–62 KB in
//! under a millisecond, so the freeze #308 first claimed does not happen there; these tests are what
//! would say so on another kernel.
//!
//! Each drain runs on a helper thread and is waited for with a deadline, so a drain that never
//! returns fails the test with a sentence rather than hanging the run.
//!
//! No `$SHELL` is set: `yes` and `Ctrl+C` mean the same in `sh`, `bash`, `zsh` and `fish`, and leaving
//! the environment alone keeps these tests out of `tests.rs`'s lock.

#![cfg(unix)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::grid::TerminalScreen;
use super::pty::TerminalSession;

/// How long one `drain` may take before the test calls it a freeze. A frame is 16 ms; a drain that
/// takes five seconds is not slow, it is not coming back.
const FREEZE: Duration = Duration::from_secs(5);

/// How long the flood runs before the drain is measured, so the PTY's buffer is full when it starts.
const FLOOD_WARMUP: Duration = Duration::from_millis(500);

/// How long the shell must write nothing before it counts as quiet.
const QUIET: Duration = Duration::from_millis(500);

/// How long `Ctrl+C` may take to quiet the flood, frames of draining included.
const PATIENCE: Duration = Duration::from_secs(15);

/// The gap between two drains when the test plays at frames.
const FRAME: Duration = Duration::from_millis(16);

/// **Drain once, on another thread, and hand the session back** — or say it never came back.
fn drain_once(mut session: TerminalSession) -> (TerminalSession, usize) {
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = session.drain(&mut bytes);
        let _ = sent.send((session, read, bytes.len()));
    });
    let (session, read, buffered) = received.recv_timeout(FREEZE).unwrap_or_else(|_| {
        panic!(
            "one drain did not return in {FREEZE:?} while `yes` was running: the read loop chases a \
             producer that refills the PTY as fast as it is emptied, so the frame never ends"
        )
    });
    let read = read.expect("the drain reads the flood without an error");
    assert_eq!(read, buffered, "drain's count is what it appended");
    (session, read)
}

/// Start `yes` in a fresh shell and let it fill the PTY.
fn flooding() -> TerminalSession {
    let mut session = TerminalSession::spawn(80, 24, std::path::Path::new("/tmp")).expect("a shell starts");
    session.write(b"yes\r").expect("the command reaches the shell");
    std::thread::sleep(FLOOD_WARMUP);
    session
}

#[test]
fn a_flood_never_holds_one_drain() {
    let session = flooding();
    let (session, first) = drain_once(session);
    assert!(first > 0, "`yes` was running, so the first drain read something");
    // The flood is still there for the next frame: a drain that returned left the rest in the kernel.
    let (_session, second) = drain_once(session);
    assert!(second > 0, "the next frame reads on where the last one stopped");
}

#[test]
fn ctrl_c_stops_a_flood_and_the_shell_goes_quiet() {
    let mut session = flooding();
    // A frame or two of the flood first, as the person would see it before they reach for the key.
    for _ in 0..2 {
        let (back, _) = drain_once(session);
        session = back;
    }
    session.write(b"\x03").expect("Ctrl+C reaches the PTY");

    let deadline = Instant::now() + PATIENCE;
    let mut quiet_since = Instant::now();
    loop {
        let (back, read) = drain_once(session);
        session = back;
        if read > 0 {
            quiet_since = Instant::now();
        } else if quiet_since.elapsed() >= QUIET {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "`yes` was still writing {PATIENCE:?} after Ctrl+C: the interrupt never reached it, or the \
             drain is too slow to empty what it wrote before it stopped"
        );
        std::thread::sleep(FRAME);
    }
}


/// **The stamp is what lets the pane skip a still frame**, so it must move on every change and on
/// nothing else: a feed that kept the old stamp would leave the pane drawing the previous screen.
#[test]
fn a_screen_is_restamped_on_a_change_and_only_then() {
    let mut first = TerminalScreen::new(20, 5);
    let mut second = TerminalScreen::new(20, 5);
    first.lines();
    second.lines();
    let built = first.stamp();
    assert_ne!(built, second.stamp(), "two screens never share a stamp, so a switch of tabs repaints");

    first.lines();
    assert_eq!(first.stamp(), built, "a screen nothing was fed keeps its stamp");

    first.feed(b"hi");
    first.lines();
    assert_ne!(first.stamp(), built, "a feed is a new build");
    assert_eq!(first.built_lines()[0].cells.len(), 2, "and the built rows are the new ones");
}
