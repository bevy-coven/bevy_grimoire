//! **The engine, proved with no window.**
//!
//! Four tests, and each one fails if a different layer of `terminal/` is wrong. They are here rather
//! than in `tests/` because they need no `App`: a PTY and a parser are the whole subject, and an
//! integration test that built a Bevy app to prove `printf` works would be testing the harness.
//!
//! **These run a real shell.** That is the point — a mock PTY proves nothing about whether
//! `TIOCSWINSZ` reached the child — and it costs about a second each. They are `#[cfg(unix)]`
//! because that is where the PTY layer exists at all.
//!
//! # Why the shell is `sh` and not `$SHELL`
//!
//! The person's shell may be `fish`, whose `printf` is a builtin with different spelling and whose
//! startup files print a greeting. These tests are about the PTY, not about which shell somebody
//! configured, so they run a POSIX `sh` with `--norc`-equivalent behaviour
//! (`ENV` unset) and assert on what it writes.

#![cfg(unix)]

use std::path::Path;
use std::time::{Duration, Instant};

use super::grid::TerminalScreen;
use super::pty::TerminalSession;

/// How long a test waits for a shell to answer before calling it broken.
///
/// Generous on purpose: this is a `cargo test` in a workspace where a hundred other tests are
/// running, and `sh` on a loaded machine taking 1.5 s to print a line is slow, not wrong.
const PATIENCE: Duration = Duration::from_secs(10);

/// **The environment is process-global, so the shell tests take turns.**
///
/// Two `App`s in one process is ordinary here; two tests setting `SHELL` at once is not. The lock is
/// held for the length of a test body and the guard is re-entrant through `drop`, which is what makes
/// `env::set_var` sound on the Rust 2024 edition.
static THE_SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Point `$SHELL` at a POSIX shell for the length of one test.
fn use_posix_shell() {
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
}

/// **Read until `wanted` is on screen**, or give up.
///
/// The single piece of polling the tests share: a shell answers asynchronously, so every assertion
/// here is "it eventually said this", never "it said this within one read".
fn read_until(session: &mut TerminalSession, screen: &mut TerminalScreen, wanted: &str) -> bool {
    let deadline = Instant::now() + PATIENCE;
    let mut bytes = Vec::new();
    while Instant::now() < deadline {
        bytes.clear();
        if let Ok(n) = session.drain(&mut bytes)
            && n > 0
        {
            screen.feed(&bytes);
            let replies = screen.take_replies();
            if !replies.is_empty() {
                let _ = session.write(&replies);
            }
        }
        let text: String = screen
            .lines()
            .iter()
            .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        if text.contains(wanted) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Every row's text, joined — the test's view of the screen.
fn screen_text(screen: &mut TerminalScreen) -> String {
    screen
        .lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// **The end-to-end proof the feature rests on**: a PTY, a child, its bytes, the parser and the grid
/// are wired to each other.
///
/// It is deliberately the dumbest possible command — `printf 'READY\n'` — because everything clever
/// about a terminal is downstream of "a shell's output arrives". If this fails, nothing above it can
/// work, and the failure names which layer by whether the shell said anything at all.
#[test]
fn a_shell_starts_and_answers() {
    let _guard = THE_SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    use_posix_shell();

    let mut session = match TerminalSession::spawn(80, 24, Path::new("/tmp"), &[]) {
        Ok(session) => session,
        Err(why) => panic!(
            "a shell must start in a test: {why}\nIf this is a machine with no /bin/sh, the \
             terminal's own refusal is what a person would see in the pane."
        ),
    };
    let mut screen = TerminalScreen::new(80, 24);
    // **`-i` and the explicit unset**: `sh` reads no startup file when it is not interactive and
    // `ENV` is unset, so the only thing on screen is what the test asked for.
    session
        .write(b"unset ENV; printf 'READY\\n'\n")
        .expect("the shell takes input");

    assert!(
        read_until(&mut session, &mut screen, "READY"),
        "the shell never printed READY. Screen was:\n{}",
        screen_text(&mut screen)
    );
}

/// **`TIOCSWINSZ` is real, not decorative.** A child that is told its width through the ioctl answers
/// with that width; one whose resize went nowhere answers `80`.
///
/// `tput cols` reads the terminal's own size, so it is the independent measurement: it does not share
/// a code path with the resize call, which is what makes this a proof rather than a restatement.
#[test]
fn a_resize_reaches_the_child() {
    let _guard = THE_SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    use_posix_shell();

    let mut session = TerminalSession::spawn(80, 24, Path::new("/tmp"), &[]).expect("a shell starts");
    let mut screen = TerminalScreen::new(80, 24);
    session.resize(100, 30).expect("the child is told the new size");
    screen.resize(100, 30);

    // `tput` reads the terminal's size; `\033[6n` and the shell's own `COLUMNS` both come from
    // elsewhere, so this is the one command whose answer is the ioctl and nothing else.
    session.write(b"tput cols\n").expect("the shell takes input");

    assert!(
        read_until(&mut session, &mut screen, "100"),
        "`tput cols` after a resize to 100 never said 100. Screen was:\n{}",
        screen_text(&mut screen)
    );
}

/// **A wrapped row says so**, which is what lets the pane treat it as one logical line when the
/// child's output is longer than the screen is wide.
#[test]
fn a_line_that_wraps_is_marked_so() {
    let mut screen = TerminalScreen::new(10, 4);
    // Twenty characters into a ten-column screen: the first row must be marked as continuing.
    screen.feed(b"0123456789ABCDEFGHIJ");

    let lines = screen.lines();
    assert!(lines.len() >= 2, "twenty characters did not wrap into two rows: {lines:?}");
    assert!(
        lines[0].wraps,
        "the first row ran off the right edge and must be marked as wrapping, or the pane draws a \
         broken sentence as two: {lines:?}"
    );
    assert!(
        !lines[1].wraps,
        "the second row is the end of the line and must not claim to continue: {lines:?}"
    );
}

/// **A closed session reaps its child.** Without this, every `×` on the bar leaks a shell — and a
/// leaked shell is a `cargo build` running forever with nothing drawing it.
#[test]
fn closing_the_session_reaps_the_child() {
    let _guard = THE_SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    use_posix_shell();

    let session = TerminalSession::spawn(80, 24, Path::new("/tmp"), &[]).expect("a shell starts");
    let pid = session.pid();
    assert!(pid > 0, "a started shell has a pid");
    // The shell is alive right now: `kill(pid, 0)` answers whether the signal could be sent.
    assert!(alive(pid), "the shell should be running before it is closed");

    drop(session);

    // **The reaping is synchronous** — `Pty`'s `Drop` sends `SIGHUP` and then `wait`s — so by the
    // time `drop` returns the pid is gone. One read of `/proc` on a Linux host proves it; the
    // deadline is there for a machine where the wait raced the kernel's own cleanup.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if !alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the shell (pid {pid}) outlived its session; a closed pane would leak a child");
}

/// Is this pid still running? `kill(pid, 0)` in its `ESRCH` sense.
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    // `/proc` is Linux's; the signal-0 probe is the portable one and needs no dependency, because
    // `std` has no `kill` and `rustix` is already a dependency of this crate.
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(pid as i32).expect("a live child has a positive pid"),
        rustix::process::Signal::CONT,
    )
    .is_ok()
}

/// **Select all is every line the shell drew, and no blank row under them** (feedback-1794024642593): Cmd+A in
/// the terminal, then Cmd+C, copies the session without a tail of empty lines.
#[test]
fn select_all_takes_every_drawn_line_and_no_blank_tail() {
    let mut screen = TerminalScreen::new(20, 6);
    assert_eq!(screen.selection_text(), None);
    screen.select_all();
    assert_eq!(screen.selection_text(), None, "a blank terminal has nothing to select");
    screen.feed(b"one\r\ntwo\r\nthree");
    screen.select_all();
    assert_eq!(screen.selection_text().as_deref(), Some("one\ntwo\nthree"));
}

/// **Pump until `done`, or give up.** The shell answers asynchronously, so every assertion about what a tab does when its
/// shell leaves is "it eventually did", never "within one pump".
fn pump_until(terminal: &mut super::Terminal, done: impl Fn(&super::Terminal) -> bool) -> bool {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        terminal.pump();
        if done(terminal) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// **A session knows its shell has left** (feedback-1794112080244): the pane has nothing else to tell it. A shell that
/// was told `exit` is a zombie the session holds, and its master reads end-of-file without ever saying why.
#[test]
fn a_session_knows_when_its_shell_has_left() {
    let _guard = THE_SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    use_posix_shell();

    let mut session = TerminalSession::spawn(80, 24, Path::new("/tmp"), &[]).expect("a shell starts");
    assert!(!session.has_exited(), "a shell that has just started has not left");
    session.write(b"exit\n").expect("the shell takes input");
    let deadline = Instant::now() + PATIENCE;
    while !session.has_exited() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(session.has_exited(), "the shell was told `exit` and the session never noticed it had left");
}

/// **A shell told to leave takes its tab with it** (feedback-1794112080244: *"If I exit omp, then type "exit" from the
/// shell, it freezes. We should close this terminal, then allow the user to create another one"*). The tab stayed, over a
/// shell that was gone, and looked frozen.
#[test]
fn a_shell_told_to_exit_takes_its_tab_with_it() {
    let _guard = THE_SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    use_posix_shell();

    let mut terminal = super::Terminal::default();
    terminal.spawn(80, 24, Path::new("/tmp"));
    assert_eq!(terminal.len(), 1);
    terminal.write(b"exit\n");
    assert!(
        pump_until(&mut terminal, |t| t.is_empty()),
        "the shell was told `exit`, and its tab is still there ({} open)",
        terminal.len()
    );
}

/// **Another tab's shell leaving never moves the one being drawn**: tab 0 exits while tab 1 is the one the pane shows, and
/// the pane is still showing that same shell afterwards.
#[test]
fn a_tab_that_leaves_behind_the_drawn_one_keeps_the_drawn_one_drawn() {
    let _guard = THE_SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    use_posix_shell();

    let mut terminal = super::Terminal::default();
    terminal.spawn(80, 24, Path::new("/tmp"));
    terminal.spawn(80, 24, Path::new("/tmp"));
    let shown = terminal.tab().and_then(|tab| tab.session.as_ref().map(TerminalSession::pid)).expect("the second tab has a shell");
    terminal.select(0);
    terminal.write(b"exit\n");
    terminal.select(1);
    assert!(pump_until(&mut terminal, |t| t.len() == 1), "the first tab's shell was told `exit` and its tab stayed");
    assert_eq!(terminal.active(), 0, "the drawn tab is now the only one");
    assert_eq!(terminal.tab().and_then(|tab| tab.session.as_ref().map(TerminalSession::pid)), Some(shown), "and it is the same shell that was drawn");
}

/// **A shell that leaves before anyone typed to it keeps its tab and says so**: closing it would open a fresh shell the next
/// frame, and a shell that cannot stay up (a bad `terminal_shell`) would be opened for ever.
#[test]
fn a_shell_that_cannot_stay_up_keeps_its_tab_and_says_why() {
    let mut terminal = super::Terminal::default();
    terminal.set_shell(Some(super::ShellChoice { program: "/bin/sh".to_owned(), args: vec!["-c".to_owned(), "exit 3".to_owned()] }));
    terminal.spawn(80, 24, Path::new("/tmp"));
    assert!(
        pump_until(&mut terminal, |t| t.tab().is_some_and(|tab| tab.problem().is_some())),
        "a shell that ended by itself left no word on its tab"
    );
    assert_eq!(terminal.len(), 1, "and its tab is still there to close");
    let said = terminal.tab().and_then(|tab| tab.problem()).unwrap_or_default().to_owned();
    assert!(said.contains("ended before anything was typed"), "{said}");
}
