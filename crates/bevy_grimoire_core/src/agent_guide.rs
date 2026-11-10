//! **The agent's guide, in the layers a person writes it in** (#347, agent guidance AG-02).
//!
//! The shipped guide (`bevy_cauldron_mcp/src/guide.md`) says how to work in Cauldron; it knows nothing
//! about one person's habits or one project's conventions. Those are two more layers, each a file:
//!
//! - **the person's**, `<config_dir>/Grimoire/AGENTS.md` — `~/Library/Application Support/Grimoire/AGENTS.md`
//!   on macOS — which follows a person from project to project;
//! - **the project's**, `<root>/.dark_factory/AGENTS.md`, which travels with the project and can only be
//!   trusted once a person has allowed it (`crate::trust`).
//!
//! **A later layer adds to the earlier ones; nothing is merged or overridden key by key.** A guide is
//! prose a person wrote, in the order they wrote it, and nothing here rewrites a word of it. Each layer
//! is introduced by a heading that names its source and path, so an agent can tell a project rule from a
//! shipped one and from another person's habit.
//!
//! # What is refused, and what is only absent
//!
//! A file that is not there is **an absent layer**, and not a problem: most projects have no guide, and
//! neither does most people. A file that **cannot be read**, is **not UTF-8**, or is **larger than
//! [`GUIDE_MAX_BYTES`]** is refused: it is left out of the guide and the refusal is a
//! [`Problem`] naming the path and the reason. **It is never truncated, decoded lossily or guessed at** —
//! a guide an agent half-reads is worse than one it was told is missing, because the agent cannot tell
//! which half it has.
//!
//! Engine-free: `std`, `serde` and the one problem shape.

use std::path::{Path, PathBuf};

use crate::problem::{Detail, Location, Problem};

/// **The largest a guide file may be, in bytes.** 64 KiB is about ten thousand words, which is more
/// prose than any project's conventions take; past it the file is refused rather than served, because an
/// agent's context is the thing this limit protects.
pub const GUIDE_MAX_BYTES: usize = 65_536;

/// **The row count of a text a box cannot be trusted to show**, whatever its length: large enough that no
/// box's row limit holds it, small enough that adding it to another row count cannot overflow.
pub const UNSHOWABLE_ROWS: usize = 1_000_000;

/// **Whether a character can hide text or break a line the box does not count**: a control character, a tab
/// (a variable width), a Unicode line or paragraph separator (a text layout breaks on them where
/// [`str::lines`] does not), and the zero-width and bidirectional controls that make characters invisible
/// or reorder them.
fn hides_or_breaks(c: char) -> bool {
    c.is_control()
        || c == '\t'
        || matches!(c, '\u{2028}' | '\u{2029}')
        || ('\u{200b}'..='\u{200f}').contains(&c)
        || ('\u{202a}'..='\u{202e}').contains(&c)
        || ('\u{2060}'..='\u{2064}').contains(&c)
        || ('\u{2066}'..='\u{206f}').contains(&c)
        || c == '\u{feff}'
}

/// **How many visual rows `text` occupies in a box `columns` characters wide**, counting a line that wraps
/// as the rows it wraps onto and an empty line as one row, **never fewer than it needs and never guessing
/// kindly**.
///
/// **Characters are not rows.** A file of twenty short lines and a file of three long ones can hold the same
/// number of characters and need very different amounts of room, and a box that clips shows the first and
/// hides the rest. Every place that decides "can a person see all of this?" asks this, so the answer is one
/// answer: `ceil(columns_in_line / columns)`, at least one row per line, where a character outside ASCII
/// takes two columns (full-width and wide glyphs are about twice an average advance).
///
/// **A text with a character that hides or breaks** ([`hides_or_breaks`]) is [`UNSHOWABLE_ROWS`]: a person
/// cannot be shown what a layout would break or hide where this counts one line, so the answer fails
/// closed. A line ending `\r\n` is the one exception, being what a Windows file's lines end with.
pub fn visual_rows(text: &str, columns: usize) -> usize {
    let columns = columns.max(1);
    let mut rows = 0usize;
    for line in text.lines() {
        // `lines` drops one `\r` before `\n`; any other control character, a lone `\r` included, is a break
        // or a hidden character the layout would act on.
        if line.chars().any(hides_or_breaks) {
            return UNSHOWABLE_ROWS;
        }
        let width: usize = line.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
        rows += width.div_ceil(columns).max(1);
    }
    rows
}

/// The person's file, inside `user_dirs::config_dir()`.
pub const USER_FILE: &str = "AGENTS.md";

/// The project's file, relative to the project's root.
pub const PROJECT_PATH: &str = ".dark_factory/AGENTS.md";

/// **Which of the two layers a guide came from.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The person's own file: it needs no trust, because it is theirs.
    User,
    /// The open project's file: it is served only once a person has allowed it (`crate::trust`).
    Project,
}

impl Source {
    /// The word an answer carries.
    pub fn word(self) -> &'static str {
        match self {
            Source::User => "user",
            Source::Project => "project",
        }
    }

    /// **What the layer's heading calls it**, so an agent reading two layers knows which is which.
    pub fn heading(self) -> &'static str {
        match self {
            Source::User => "Your guide",
            Source::Project => "This project's guide",
        }
    }

    /// How the path inside the heading is written: the person's file in full, because it is outside the
    /// project and an agent cannot guess it; the project's as the project-relative path, which is what a
    /// person types and what `reload` classifies it as.
    pub fn shown_path(self, path: &Path) -> String {
        match self {
            Source::User => path.display().to_string(),
            Source::Project => PROJECT_PATH.to_owned(),
        }
    }
}

/// **One layer of the guide**: which file, and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub source: Source,
    /// The file it was read from.
    pub path: PathBuf,
    /// The file's own text, unchanged.
    pub text: String,
}

impl Layer {
    /// **The layer as an agent reads it**: a heading that names the source and its path, then the text.
    ///
    /// The heading is what makes two layers distinguishable, so it is never dropped and never reworded
    /// per caller — the shipped guide's own `##` sections and a person's are otherwise one run of prose.
    pub fn rendered(&self) -> String {
        format!(
            "## {} ({})\n\n{}\n",
            self.source.heading(),
            self.source.shown_path(&self.path),
            self.text.trim_end()
        )
    }
}

/// **What the editor adds to the shipped guide**: the layers, in order, and what it refused.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guide {
    pub layers: Vec<Layer>,
    pub problems: Vec<Problem>,
}

impl Guide {
    /// Every layer's rendered text, one after another: what a caller appends to the shipped guide.
    pub fn rendered(&self) -> String {
        let mut out = String::new();
        for layer in &self.layers {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&layer.rendered());
        }
        out
    }
}

/// **Read one layer.** `Ok(None)` for a file that is not there — the one absence that is not a problem.
/// `Ok(Some(text))` for a guide. `Err(problem)` for a file that cannot be read, **is not a regular file**, is
/// not UTF-8, or is over [`GUIDE_MAX_BYTES`]: the problem names the path and the reason, and the layer is
/// left out.
///
/// # Why this does not call `std::fs::read`
///
/// **A guide path is attacker-controlled**: it arrives with a cloned project, and its text is read inside a
/// BRP call while an agent waits. `std::fs::read` on a FIFO blocks forever and on `/dev/zero` grows until
/// memory is gone — one symlink would hang or kill the editor for every agent and for the person, and the
/// limit would be checked only after the damage. So the path is `stat`ed first and **anything that is not a
/// regular file is refused without being opened**, and what is a regular file is then read through
/// `.take(GUIDE_MAX_BYTES + 1)` so a file that grew between the two is still bounded.
pub fn read_layer(source: Source, path: &Path) -> Result<Option<String>, Box<Problem>> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(refused(source, path, format!("it cannot be read: {e}"), "make the file readable, or remove it")),
    };
    // **Not a regular file: a FIFO, a socket, a device, or a directory named `AGENTS.md`.**
    if !metadata.is_file() {
        return Err(refused(
            source,
            path,
            "it is not a regular file".to_owned(),
            "replace it with a text file: a guide is prose, and a pipe or a device has no end to read",
        ));
    }
    // **A file that is already too large is refused from its size**, so a 4 GB guide is never opened.
    if metadata.len() > GUIDE_MAX_BYTES as u64 {
        return Err(refused(
            source,
            path,
            format!("it is {} bytes and the limit is {GUIDE_MAX_BYTES}", metadata.len()),
            "shorten it: a guide past that limit is more than an agent can be given",
        ));
    }
    let bytes = {
        use std::io::Read as _;
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) => return Err(refused(source, path, format!("it cannot be read: {e}"), "make the file readable, or remove it")),
        };
        let mut bytes = Vec::new();
        // **The bound is on what is read, not on what was stat'ed**: a file that grew between the two is
        // still refused, and one that shrank is read as it now is.
        if let Err(e) = file.take(GUIDE_MAX_BYTES as u64 + 1).read_to_end(&mut bytes) {
            return Err(refused(source, path, format!("it cannot be read: {e}"), "make the file readable, or remove it"));
        }
        bytes
    };
    if bytes.len() > GUIDE_MAX_BYTES {
        return Err(refused(
            source,
            path,
            format!("it is more than {GUIDE_MAX_BYTES} bytes"),
            "shorten it: a guide past that limit is more than an agent can be given",
        ));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(Some(text)),
        Err(e) => Err(refused(
            source,
            path,
            format!("it is not UTF-8 text: {}", e.utf8_error()),
            "save it as UTF-8",
        )),
    }
}

/// **One layer's refusal**, so every way a guide can be refused reads the same and names the same path.
fn refused(source: Source, path: &Path, why: String, fix: &'static str) -> Box<Problem> {
    Box::new(Problem::with(
        format!("{} was left out of the agent's guide", what(source)),
        Detail::default().at(Location::file(path.display().to_string())).because(why).fix(fix),
    ))
}

/// What a refusal calls the layer, so the editor's status line and the tool's answer are one sentence.
fn what(source: Source) -> &'static str {
    match source {
        Source::User => "your agent guide",
        Source::Project => "this project's agent guide",
    }
}

/// **The person's layer and the project's, in that order.** A missing path is a layer this caller has
/// none of — no project is open, or the person has written none.
pub fn read(user_file: Option<&Path>, project_file: Option<&Path>) -> Guide {
    let mut guide = Guide::default();
    for (source, path) in [(Source::User, user_file), (Source::Project, project_file)] {
        let Some(path) = path else { continue };
        match read_layer(source, path) {
            Ok(Some(text)) => guide.layers.push(Layer { source, path: path.to_path_buf(), text }),
            Ok(None) => {}
            Err(problem) => guide.problems.push(*problem),
        }
    }
    guide
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("grimoire-guide-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn a_missing_file_is_an_absent_layer_and_never_a_problem() {
        let dir = scratch("absent");
        let guide = read(Some(&dir.join("nothing.md")), Some(&dir.join("nothing-else.md")));
        assert!(guide.layers.is_empty(), "no file, no layer: {guide:?}");
        assert!(guide.problems.is_empty(), "and nothing was refused: {guide:?}");
    }

    #[test]
    fn a_layer_renders_under_a_heading_that_names_it_and_its_path() {
        let path = PathBuf::from("/somewhere/Grimoire/AGENTS.md");
        let layer = Layer { source: Source::User, path: path.clone(), text: "Be brief.\n".to_owned() };
        let rendered = layer.rendered();
        assert_eq!(rendered, format!("## Your guide ({})\n\nBe brief.\n", path.display()));
        let project = Layer { source: Source::Project, path: PathBuf::from("/p/.dark_factory/AGENTS.md"), text: "Walls are 2.4 m.".to_owned() };
        assert_eq!(project.rendered(), "## This project's guide (.dark_factory/AGENTS.md)\n\nWalls are 2.4 m.\n");
    }

    #[test]
    fn a_file_over_the_limit_is_refused_rather_than_truncated() {
        let dir = scratch("large");
        let path = dir.join("AGENTS.md");
        std::fs::write(&path, "a".repeat(GUIDE_MAX_BYTES + 1)).expect("the file is written");
        let problem = read_layer(Source::Project, &path).expect_err("a file over the limit is refused");
        assert!(problem.line().contains("left out"), "{}", problem.line());
        assert!(problem.line().contains(&(GUIDE_MAX_BYTES + 1).to_string()), "the size is named: {}", problem.line());
    }

    /// **A guide that is not a regular file is refused, never read.** A symlink to `/dev/zero` or a FIFO is
    /// the shape of the attack: `std::fs::read` on either never returns, and this runs inside a BRP call —
    /// so one path in a cloned project would hang the editor for every agent and for the person.
    #[test]
    #[cfg(unix)]
    fn a_guide_that_is_not_a_regular_file_is_refused_without_being_read() {
        let dir = scratch("notafile");
        let fifo = dir.join("AGENTS.md");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status().map(|s| s.success()).unwrap_or(false);
        if !made {
            // No `mkfifo`: a symlink to `/dev/zero` is the same shape and is always available on macOS.
            let _ = std::fs::remove_file(&fifo);
            std::os::unix::fs::symlink("/dev/zero", &fifo).expect("a symlink to /dev/zero");
        }
        // **With a deadline**, so a regression fails here instead of hanging the suite: a read that returned
        // would finish in microseconds, and one that does not is the bug.
        let (tx, rx) = std::sync::mpsc::channel();
        let path = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(read_layer(Source::Project, &path));
        });
        let refused = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("reading a guide that is not a regular file never returned: the editor would hang on a BRP call");
        let problem = refused.expect_err("a FIFO or a device is not a guide");
        assert!(problem.line().contains("not a regular file") || problem.line().contains("regular"), "{}", problem.line());
        assert!(problem.line().contains(&fifo.display().to_string()), "the refusal names the path: {}", problem.line());
    }

    #[test]
    fn a_file_that_is_not_utf8_is_refused() {
        let dir = scratch("bytes");
        let path = dir.join("AGENTS.md");
        std::fs::write(&path, [0x23, 0x20, 0xff, 0xfe]).expect("the file is written");
        let problem = read_layer(Source::User, &path).expect_err("invalid UTF-8 is refused");
        assert!(problem.line().contains("UTF-8"), "{}", problem.line());
        assert!(problem.line().contains("your agent guide"), "{}", problem.line());
    }
}
