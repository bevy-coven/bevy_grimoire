//! **One PTY, and the shell on the other end of it.**
//!
//! The process boundary of the terminal: everything that touches a file descriptor lives here, and
//! nothing above this file knows that a `fork` happened.
//!
//! # Why there is no reader thread
//!
//! The obvious design is a thread per session reading the master fd into a channel. It is rejected,
//! and the reason is measurable rather than stylistic: the master fd is already non-blocking
//! (`alacritty_terminal` sets `O_NONBLOCK` when it spawns), so [`TerminalSession::drain`] is **one
//! `read` syscall per frame** that returns 0 when the shell is idle — about a microsecond. A thread
//! would cost a channel, a lock, a shutdown protocol, and a way for a thread to survive a `Drop`
//! that raced it, to replace that single syscall. Every failure mode of this feature would then be a
//! threading bug rather than a drawn thing.
//!
//! # Why `rustix::termios::tcsetwinsize` and not the PTY's own resize
//!
//! `alacritty_terminal`'s `Pty::on_resize` does the identical `TIOCSWINSZ`, and its failure branch is
//! `die!`, which is `std::process::exit(1)`. A library that kills the editor because a pane got
//! dragged is not one to put on a system path, so the ioctl is made directly and its `Result` reaches
//! the pane, which says so in the body.

use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::tty::{self, Options, Shell};

/// **The shell this session runs.** `$SHELL`, or `/bin/sh` when the environment does not say.
///
/// `$SHELL` first because a person's shell is their environment — `fish`, `zsh` and `bash` differ in
/// ways that matter to the commands they type. The fallback is POSIX `sh` and not `$SHELL`'s
/// emptiness answered with an error: a login shell that is not configured is a shell, and refusing
/// to open a pane over it would be a refusal to be useful on a fresh machine.
fn shell_of_environment() -> Result<Shell, String> {
    Ok(Shell::new(shell_program_of_environment()?, Vec::new()))
}

/// **The program [`shell_of_environment`] runs**, when only the program is asked for (#341).
///
/// The editor asks it once at start-up, to put a question about the machine to the shell its first tab
/// will run. One place answers "which shell is this person's", so the question and the tab cannot
/// disagree about `$SHELL` or about its absence.
pub(crate) fn shell_program_of_environment() -> Result<String, String> {
    match std::env::var("SHELL").ok().filter(|s| !s.is_empty()) {
        Some(shell) => Ok(shell),
        None if Path::new("/bin/sh").exists() => Ok("/bin/sh".to_owned()),
        // Named rather than guessed: a build with neither `$SHELL` nor `/bin/sh` is not Unix as this
        // code understands it, and inventing a path here would produce a spawn error nobody can act on.
        None => {
            Err("$SHELL is unset and /bin/sh does not exist, so there is no shell to run".to_owned())
        }
    }
}

/// **A shell the person chose** (`terminal_shell` and `terminal_shell_args`, #324): VS Code's default
/// profile. Without one a session runs [`shell_of_environment`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellChoice {
    pub program: String,
    pub args: Vec<String>,
}

/// **The shell a session runs**: the person's choice when they made one, else the environment's.
fn shell_to_run(choice: Option<&ShellChoice>) -> Result<Shell, String> {
    match choice {
        Some(choice) => Ok(Shell::new(choice.program.clone(), choice.args.clone())),
        None => shell_of_environment(),
    }
}

/// **One running shell, and the master side of its terminal.**
///
/// Dropped = closed: [`Drop`] hangs the child up and reaps it, which is what stops a closed pane
/// leaking a shell. That is not a nicety — the editor spawns a session per `+` chip, and a session
/// that outlived its pane would leave `cargo build` running with no window anywhere drawing it.
pub struct TerminalSession {
    pty: tty::Pty,
    /// The size last written to the child, so a resize to the size it already has is not a syscall.
    size: (u16, u16),
    /// The directory the shell was started in, for the bar's chip and for the `cd` offer.
    cwd: PathBuf,
}

/// **Start a shell the environment names.** The same as [`TerminalSession::spawn_with`] with no choice.
impl TerminalSession {
    pub fn spawn(cols: u16, rows: u16, cwd: &Path, extra_env: &[(String, String)]) -> Result<TerminalSession, String> {
        TerminalSession::spawn_with(cols, rows, cwd, extra_env, None)
    }
}

impl TerminalSession {
    /// **Start a shell.** `cols` and `rows` are the pane's cell count, which the child is told before
    /// it ever draws a prompt — a shell that starts at 80×24 and is then resized prints `cargo`'s
    /// progress bar twice. `extra_env` is laid over [`session_env`]: an agent tab's MCP port, token and
    /// config path (`crate::terminal_agent::AgentLaunch::env`); a plain shell passes none.
    #[cfg(unix)]
    pub fn spawn_with(cols: u16, rows: u16, cwd: &Path, extra_env: &[(String, String)], shell: Option<&ShellChoice>) -> Result<TerminalSession, String> {
        // **Zero cells is refused rather than clamped silently.** A zero-width terminal makes
        // `alacritty_terminal` divide by zero when it lays out a wide character, and a zero-height
        // one has nowhere to put a cursor; both are reachable from a pane dragged to nothing, and
        // both are the caller's bug. Reporting it means the pane can say "the pane is too small"
        // instead of the process dying in a library.
        if cols == 0 || rows == 0 {
            return Err(format!(
                "cannot start a shell in a {cols} x {rows} terminal: a terminal with no {} has nowhere \
                 to draw",
                if cols == 0 { "columns" } else { "rows" }
            ));
        }
        // **A working directory that is gone is not a reason to refuse a shell.** `cwd` comes from
        // the open project, and a project deleted from another window would otherwise take the
        // terminal with it. The home directory is a shell's own default answer to the same question.
        let cwd = if cwd.is_dir() {
            cwd.to_path_buf()
        } else {
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
        };

        let options = Options {
            shell: Some(shell_to_run(shell)?),
            working_directory: Some(cwd.clone()),
            // **`drain_on_exit: false`** — a shell that has exited has nothing left worth waiting
            // for, and waiting would block the frame that noticed.
            drain_on_exit: false,
            // **The two variables that make colour work.** Without `TERM` a program's library sees
            // `dumb` and prints no escapes at all, so `cargo`'s output arrives uncoloured and the
            // palette above is never asked; `COLORTERM=truecolor` is what makes a program emit
            // 24-bit sequences rather than rounding to the 256 slots. Set here rather than inherited,
            // because the editor may itself have been launched from a `.desktop` file with neither.
            //
            // **And the ones that say where it runs** (#324), as VS Code's integrated terminal says
            // `TERM_PROGRAM=vscode`: a prompt or a tool can tell it is in Cauldron, and which one.
            // A locale is given only when the inherited one cannot carry UTF-8, or every `—`, box
            // line and prompt icon arrives as `?`.
            env: session_env(std::env::var("LC_ALL").ok(), std::env::var("LANG").ok())
                .into_iter()
                .chain(extra_env.iter().cloned())
                .collect(),
        };

        let size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            // The pixel size is what a program reads to decide whether a picture fits; a cell is
            // measured by the pane (`terminal_pane::measure_the_cell`), and zero here means "unknown",
            // which every program treats as "use the cell count".
            cell_width: 0,
            cell_height: 0,
        };
        let pty = tty::new(&options, size, 0).map_err(|e| {
            format!(
                "the shell did not start: {e}. The terminal runs `$SHELL` in a PTY, so a build with no \
                 `/dev/ptmx` or a machine at its process limit cannot open one"
            )
        })?;
        Ok(TerminalSession { pty, size: (cols, rows), cwd })
    }

    /// Any non-Unix target: the PTY layer does not exist there in this build.
    ///
    /// A **refusal with a reason** rather than an empty pane or a fake prompt. The owner develops on
    /// macOS and on `big`, both Unix; a Windows build that silently drew nothing would be a lie
    /// discovered at the worst moment.
    #[cfg(not(unix))]
    pub fn spawn_with(_cols: u16, _rows: u16, _cwd: &Path, _extra_env: &[(String, String)], _shell: Option<&ShellChoice>) -> Result<TerminalSession, String> {
        Err("the terminal is Unix-only in this build: it needs a PTY, which this target has no \
             implementation for"
            .to_owned())
    }

    /// The directory the shell was started in.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The child's pid, for a test that has to prove it was reaped.
    #[cfg(unix)]
    pub fn pid(&self) -> u32 {
        self.pty.child().id()
    }

    /// **Has the shell left?** Asked without collecting it: the child stays this process's zombie until
    /// the session is dropped (`child_has_left`), so the pid is still the shell's and never a stranger's.
    ///
    /// The pane asks **before** it drains, so everything the shell wrote before it went is already in the
    /// pipe when the answer is yes, and one drain reads it all.
    #[cfg(unix)]
    pub fn has_exited(&self) -> bool {
        i32::try_from(self.pty.child().id()).ok().and_then(rustix::process::Pid::from_raw).is_some_and(child_has_left)
    }

    /// No session exists on this target (`spawn_with` refuses), so none has left.
    #[cfg(not(unix))]
    pub fn has_exited(&self) -> bool {
        false
    }

    /// Keystrokes to the child.
    ///
    /// A full pipe is **not** an error worth reporting: the child is not reading yet (a `cargo build`
    /// in the middle of a link), and a keystroke that could not be written is a keystroke the person
    /// pressed while the program was busy. It is dropped, which is what every terminal does.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self.pty.file().write_all(bytes) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(()),
            Err(e) if e.kind() == ErrorKind::BrokenPipe => {
                Err("the shell has exited; this terminal is no longer connected to anything".to_owned())
            }
            Err(e) => Err(format!("could not write to the shell: {e}")),
        }
    }

    /// Tell the child its window changed.
    ///
    /// **Compares first**: `TIOCSWINSZ` is a syscall, and the pane recomputes its cell count every
    /// frame it is visible. Writing unconditionally would be sixty ioctls a second to say nothing,
    /// and each one makes `cargo`'s progress line redraw.
    #[cfg(unix)]
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        if (cols, rows) == self.size {
            return Ok(());
        }
        if cols == 0 || rows == 0 {
            // The pane's own floor (`TERMINAL_MIN_H`) makes this unreachable from a drag; it is
            // reachable from a resize_surface call in a test, and clamping is right there because
            // the alternative is a dead shell over a bad frame.
            return Ok(());
        }
        use rustix::termios::{Winsize, tcsetwinsize};
        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        tcsetwinsize(self.pty.file(), winsize)
            .map_err(|e| format!("the shell was not told the pane resized to {cols}x{rows}: {e}"))?;
        self.size = (cols, rows);
        Ok(())
    }

    /// The same, on a target with no PTY.
    #[cfg(not(unix))]
    pub fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), String> {
        Ok(())
    }

    /// **Whatever the child has written since the last call**, appended to `into`.
    ///
    /// Non-blocking, and `Ok(0)` when idle — which is the common case, since a settled prompt writes
    /// nothing. Reads until the kernel has nothing left rather than once, so a `cargo` build's
    /// several kilobytes arrive in one frame instead of dribbling across forty.
    pub fn drain(&mut self, into: &mut Vec<u8>) -> Result<usize, String> {
        // 64 KiB per `read`: one frame's worth of a busy program, and small enough that a very large
        // paste does not allocate a megabyte to hand to the parser.
        let mut chunk = [0u8; 64 * 1024];
        let mut total = 0;
        loop {
            match self.pty.file().read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    into.extend_from_slice(&chunk[..n]);
                    total += n;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => break,
                // **A closed fd is how a shell says goodbye.** `EIO` on a PTY's master is the
                // documented shape of "the slave side is gone", so it is the ordinary exit path and
                // not an error to report every frame.
                Err(e) if e.raw_os_error() == Some(5) => break,
                Err(e) => return Err(format!("the terminal could not read from the shell: {e}")),
            }
        }
        Ok(total)
    }

}

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalSession")
            .field("cwd", &self.cwd)
            .field("cols", &self.size.0)
            .field("rows", &self.size.1)
            .finish()
    }
}

/// **How long a child has to leave after it is hung up on**, milliseconds, before it is killed.
pub const HANGUP_GRACE_MS: u64 = 1_000;

/// How often the wait for the child looks, milliseconds.
const HANGUP_POLL_MS: u64 = 10;

/// **How long the exit watchdog lets the hang-up wait run before it calls it overdue**, milliseconds: the grace, and a little over
/// it for the kill and the look that follows.
const HANGUP_WAIT_BOUND_MS: u64 = HANGUP_GRACE_MS + 500;

/// **Closing a session never waits on its child for ever** (#337).
///
/// `alacritty_terminal`'s `Pty` hangs the child up and then blocks in `Child::wait` with no limit
/// (`tty/unix.rs`, `impl Drop for Pty`). A shell leaves at once; an agent (the tab that runs `omp`)
/// may ignore `SIGHUP` or take its time, and the editor's main thread then sat in that `wait` after
/// its window was gone: the freeze a person met when an update quit the app, which stayed alive,
/// window-less, until it was killed by hand. This runs first (a type's own `Drop` runs before its
/// fields'): it hangs up, gives the child [`HANGUP_GRACE_MS`] to go, and kills it if it has not, so the
/// `Pty`'s own wait that follows returns at once. The pane that had it is already closed.
///
/// **The wait is listed for the exit watchdog** while it lasts, as `a terminal's shell hanging up`, so a quit that is stuck here
/// is cut short saying so and not saying nothing.
#[cfg(unix)]
impl Drop for TerminalSession {
    fn drop(&mut self) {
        let Some(pid) = i32::try_from(self.pty.child().id()).ok().and_then(rustix::process::Pid::from_raw) else {
            return;
        };
        let _wait = crate::exit_watchdog::waiting("a terminal's shell hanging up", std::time::Duration::from_millis(HANGUP_WAIT_BOUND_MS));
        hang_up_then_kill(pid, std::time::Duration::from_millis(HANGUP_GRACE_MS));
    }
}

/// **Has `pid` left?** The look is `waitid` with `NOWAIT`: it says a child has exited **without reaping
/// it**, so the pid stays this process's zombie until `Pty`'s `wait` collects it, and can never be another
/// process's in between (a signal to a reaped and reused pid would hit a stranger).
#[cfg(unix)]
fn child_has_left(pid: rustix::process::Pid) -> bool {
    use rustix::process::{WaitId, WaitIdOptions, waitid};
    match waitid(WaitId::Pid(pid), WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT) {
        // It left, and is waiting to be collected.
        Ok(Some(_)) => true,
        // Still running.
        Ok(None) => false,
        // Not this process's child any more (already collected): nothing is left to wait for.
        Err(_) => true,
    }
}

/// **Hangs up on `pid`, and kills it if it is still there after `grace`.** `true` when it had to.
#[cfg(unix)]
fn hang_up_then_kill(pid: rustix::process::Pid, grace: std::time::Duration) -> bool {
    use rustix::process::{Signal, kill_process};
    let _ = kill_process(pid, Signal::HUP);
    let deadline = std::time::Instant::now() + grace;
    loop {
        if child_has_left(pid) {
            return false;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(HANGUP_POLL_MS));
    }
    let _ = kill_process(pid, Signal::KILL);
    true
}

/// The name a program reads in `TERM_PROGRAM` to know it runs in this editor's terminal (#324).
pub const TERM_PROGRAM: &str = "Cauldron";

/// **The UTF-8 locale a shell is given when its own cannot carry UTF-8.** `C.UTF-8` is built into
/// glibc and musl on Linux; macOS has no `C.UTF-8`, and `en_US.UTF-8` is the locale every mac has.
#[cfg(target_os = "macos")]
const UTF8_LOCALE: &str = "en_US.UTF-8";
#[cfg(not(target_os = "macos"))]
const UTF8_LOCALE: &str = "C.UTF-8";

/// **The environment a shell starts with, beyond the editor's own** (#324), from the inherited
/// `LC_ALL` and `LANG`.
///
/// `TERM` and `COLORTERM` make colour work; `TERM_PROGRAM` and `TERM_PROGRAM_VERSION` say the terminal
/// is Cauldron's and which release, as VS Code says `vscode`. `LANG` is set to a UTF-8 locale only when
/// nothing inherited names one — no `LC_ALL`, and a `LANG` that is unset, `C` or `POSIX` — so a person's
/// own locale is never overridden.
pub fn session_env(lc_all: Option<String>, lang: Option<String>) -> Vec<(String, String)> {
    let mut env = vec![
        ("TERM".to_owned(), "xterm-256color".to_owned()),
        ("COLORTERM".to_owned(), "truecolor".to_owned()),
        ("TERM_PROGRAM".to_owned(), TERM_PROGRAM.to_owned()),
        ("TERM_PROGRAM_VERSION".to_owned(), env!("CARGO_PKG_VERSION").to_owned()),
    ];
    let names_a_locale = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.is_empty());
    let lang_is_plain = lang.as_deref().is_none_or(|v| v.is_empty() || v == "C" || v == "POSIX");
    if !names_a_locale(&lc_all) && lang_is_plain {
        env.push(("LANG".to_owned(), UTF8_LOCALE.to_owned()));
    }
    env
}
