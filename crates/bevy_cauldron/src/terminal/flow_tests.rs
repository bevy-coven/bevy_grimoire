//! **A flood of output cannot take the editor's frame** (#308).
//!
//! The terminal reads its PTY on the main thread, once a frame. A program that writes faster than one
//! `read` empties the kernel's buffer — `yes`, `cat` of a large log, `cargo build -vv` — refills it
//! while the read loop runs, so a loop that reads "until there is nothing left" never ends and the
//! whole editor freezes, `Ctrl+C` included. These tests run a real `yes` in a real PTY, because the
//! failure is a property of the kernel's buffer, which a mock would not have.
//!
//! Each drain runs on a helper thread and is waited for with a deadline, so a drain that never
//! returns fails the test with a sentence rather than hanging the run.
//!
//! No `$SHELL` is set: `yes` and `Ctrl+C` mean the same in `sh`, `bash`, `zsh` and `fish`, and leaving
//! the environment alone keeps these tests out of `tests.rs`'s lock.

#![cfg(unix)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::pty::{READ_BUDGET, TerminalSession};

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
fn one_frame_reads_at_most_its_budget_however_much_is_written() {
    let session = flooding();
    let (session, first) = drain_once(session);
    assert!(first > 0, "`yes` was running, so the first drain read something");
    assert!(
        first <= READ_BUDGET,
        "one drain read {first} bytes, more than the {READ_BUDGET}-byte budget one frame may spend"
    );
    // The flood is still there for the next frame: the budget defers, it does not drop.
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

#[test]
fn zz_measure_throwaway() {
    for cmd in [&b"yes\r"[..], b"seq 1 100000000\r", b"cat /dev/urandom | base64\r"] {
        let mut session = TerminalSession::spawn(200, 50, std::path::Path::new("/tmp")).unwrap();
        let mut screen = super::grid::TerminalScreen::new(200, 50);
        session.write(cmd).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let mut rows = Vec::new();
        let mut bytes = Vec::new();
        for _ in 0..30 {
            bytes.clear();
            let t = Instant::now();
            let n = session.drain(&mut bytes).unwrap();
            let d = t.elapsed();
            let t = Instant::now();
            screen.feed(&bytes);
            let f = t.elapsed();
            let t = Instant::now();
            let _ = screen.lines();
            let l = t.elapsed();
            rows.push(format!("{n} drain={d:?} feed={f:?} lines={l:?}"));
            std::thread::sleep(FRAME);
        }
        eprintln!("CMD {:?}\n{}", String::from_utf8_lossy(cmd), rows.join("\n"));
        // small feeds: cost of a prompt-sized feed
        let t = Instant::now();
        for _ in 0..1000 { screen.feed(b"x"); }
        eprintln!("1000 one-byte feeds: {:?}", t.elapsed());
    }
}
