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
//! **The shell is a POSIX `sh`, pinned.** These tests leave the emulator out — they drain the PTY and
//! answer nothing — and `fish` 4, the push gate's `$SHELL` on big, sends a Primary Device Attributes
//! query (`ESC [ 0 c`) at startup and draws no prompt and runs no command until it is answered or 10 s
//! pass (measured 2026-11-06: it prints "fish could not read response to Primary Device Attribute
//! query after waiting for 10 seconds"). So under `fish` the first drain read nothing and the gate
//! failed `a_flood_never_holds_one_drain`. The editor itself answers that query (the screen's replies
//! are written back every frame), so this is a property of the harness, not of the terminal.

#![cfg(unix)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::grid::TerminalScreen;
use super::pty::TerminalSession;

/// How long one `drain` may take before the test calls it a freeze. A frame is 16 ms; a drain that
/// takes five seconds is not slow, it is not coming back.
const FREEZE: Duration = Duration::from_secs(5);

/// What `yes` writes, three lines of it: seen in the output, the flood is running. The flood is
/// waited for rather than a fixed warm-up, because a shell on a loaded gate can take longer than any
/// warm-up to start a command.
const FLOOD_SIGN: &[u8] = b"y\r\ny\r\ny\r\n";

/// **`SHELL` is process-wide**, so the tests here that set it take turns.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// How long the shell must write nothing before it counts as quiet.
const QUIET: Duration = Duration::from_millis(500);

/// How long `Ctrl+C` may take to quiet the flood, frames of draining included.
const PATIENCE: Duration = Duration::from_secs(15);

/// The gap between two drains when the test plays at frames.
const FRAME: Duration = Duration::from_millis(16);

/// **Drain once, on another thread, and hand the session back** — or say it never came back.
fn drain_once(session: TerminalSession) -> (TerminalSession, usize) {
    let (session, bytes) = drain_bytes(session);
    (session, bytes.len())
}

/// The same drain, keeping what it read.
fn drain_bytes(mut session: TerminalSession) -> (TerminalSession, Vec<u8>) {
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = session.drain(&mut bytes);
        let _ = sent.send((session, read, bytes));
    });
    let (session, read, bytes) = received.recv_timeout(FREEZE).unwrap_or_else(|_| {
        panic!(
            "one drain did not return in {FREEZE:?} while `yes` was running: the read loop chases a \
             producer that refills the PTY as fast as it is emptied, so the frame never ends"
        )
    });
    let read = read.expect("the drain reads the flood without an error");
    assert_eq!(read, bytes.len(), "drain's count is what it appended");
    (session, bytes)
}

/// Start `yes` in a fresh POSIX shell and drain, frame by frame, until its lines are pouring out.
fn flooding() -> TerminalSession {
    let mut session = {
        let _env = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        unsafe { std::env::set_var("SHELL", "/bin/sh") };
        TerminalSession::spawn(80, 24, std::path::Path::new("/tmp")).expect("a shell starts")
    };
    session.write(b"yes\r").expect("the command reaches the shell");
    let deadline = Instant::now() + PATIENCE;
    let mut seen: Vec<u8> = Vec::new();
    loop {
        let (back, bytes) = drain_bytes(session);
        session = back;
        seen.extend_from_slice(&bytes);
        if seen.windows(FLOOD_SIGN.len()).any(|w| w == FLOOD_SIGN) {
            return session;
        }
        assert!(
            Instant::now() < deadline,
            "`yes` never started in {PATIENCE:?}; the shell wrote: {:?}",
            String::from_utf8_lossy(&seen)
        );
        std::thread::sleep(FRAME);
    }
}

/// **The next frame's drain**, a frame after the last: it reads what `yes` wrote in between, waiting
/// up to [`PATIENCE`] of frames for the producer to be scheduled at all. Two drains back to back, with
/// no frame between them, can find the kernel's buffer empty on a loaded machine — measured: with 20
/// busy CPUs, a drain right after another read 0 bytes in 3 runs of 5.
fn next_frame(mut session: TerminalSession) -> (TerminalSession, usize) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        std::thread::sleep(FRAME);
        let (back, read) = drain_once(session);
        session = back;
        if read > 0 || Instant::now() >= deadline {
            return (session, read);
        }
    }
}

#[test]
fn a_flood_never_holds_one_drain() {
    let session = flooding();
    let (session, first) = next_frame(session);
    assert!(first > 0, "`yes` was running, so the next frame's drain read something");
    // The flood is still there for the next frame: a drain that returned left the rest in the kernel.
    let (_session, second) = next_frame(session);
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
