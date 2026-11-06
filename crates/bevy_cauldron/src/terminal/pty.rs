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

/// **The most one frame reads from one shell**, in bytes (#308).
///
/// Alacritty's own event loop reads at most this much (`READ_BUFFER_SIZE`, 1 MiB) before it renders.
/// At 60 frames a second it is 60 MiB/s per shell — more than any program a person is reading — and
/// it keeps the parse a few milliseconds of the frame.
pub const READ_BUDGET: usize = 1 << 20;

/// **The shell this session runs.** `$SHELL`, or `/bin/sh` when the environment does not say.
///
/// `$SHELL` first because a person's shell is their environment — `fish`, `zsh` and `bash` differ in
/// ways that matter to the commands they type. The fallback is POSIX `sh` and not `$SHELL`'s
/// emptiness answered with an error: a login shell that is not configured is a shell, and refusing
/// to open a pane over it would be a refusal to be useful on a fresh machine.
fn shell_of_environment() -> Result<Shell, String> {
    let vars = std::env::var("SHELL").ok().filter(|s| !s.is_empty());
    let program = match vars {
        Some(shell) => shell,
        None if Path::new("/bin/sh").exists() => "/bin/sh".to_owned(),
        // Named rather than guessed: a build with neither `$SHELL` nor `/bin/sh` is not Unix as this
        // code understands it, and inventing a path here would produce a spawn error nobody can act on.
        None => {
            return Err("$SHELL is unset and /bin/sh does not exist, so there is no shell to run".to_owned());
        }
    };
    Ok(Shell::new(program, Vec::new()))
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

impl TerminalSession {
    /// **Start a shell.** `cols` and `rows` are the pane's cell count, which the child is told before
    /// it ever draws a prompt — a shell that starts at 80×24 and is then resized prints `cargo`'s
    /// progress bar twice.
    #[cfg(unix)]
    pub fn spawn(cols: u16, rows: u16, cwd: &Path) -> Result<TerminalSession, String> {
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
            shell: Some(shell_of_environment()?),
            working_directory: Some(cwd.clone()),
            // **`drain_on_exit: false`** — a shell that has exited has nothing left worth waiting
            // for, and waiting would block the frame that noticed.
            drain_on_exit: false,
            // **The two variables that make colour work.** Without `TERM` a program's library sees
            // `dumb` and prints no escapes at all, so `cargo`'s output arrives uncoloured and the
            // palette above is never asked; `COLORTERM=truecolor` is what makes a program emit
            // 24-bit sequences rather than rounding to the 256 slots. Set here rather than inherited,
            // because the editor may itself have been launched from a `.desktop` file with neither.
            env: [
                ("TERM".to_owned(), "xterm-256color".to_owned()),
                ("COLORTERM".to_owned(), "truecolor".to_owned()),
            ]
            .into_iter()
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
    pub fn spawn(_cols: u16, _rows: u16, _cwd: &Path) -> Result<TerminalSession, String> {
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
