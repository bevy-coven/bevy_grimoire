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
/// `Ok(Some(text))` for a guide. `Err(problem)` for a file that cannot be read, is not UTF-8, or is over
/// [`GUIDE_MAX_BYTES`]: the problem names the path and the reason, and the layer is left out.
pub fn read_layer(source: Source, path: &Path) -> Result<Option<String>, Box<Problem>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(Box::new(Problem::with(
                format!("{} was left out of the agent's guide", what(source)),
                Detail::default()
                    .at(Location::file(path.display().to_string()))
                    .because(format!("it cannot be read: {e}"))
                    .fix("make the file readable, or remove it"),
            )));
        }
    };
    if bytes.len() > GUIDE_MAX_BYTES {
        return Err(Box::new(Problem::with(
            format!("{} was left out of the agent's guide", what(source)),
            Detail::default()
                .at(Location::file(path.display().to_string()))
                .because(format!(
                    "it is {} bytes and the limit is {GUIDE_MAX_BYTES}",
                    bytes.len()
                ))
                .fix("shorten it: a guide past that limit is more than an agent can be given"),
        )));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(Some(text)),
        Err(e) => Err(Box::new(Problem::with(
            format!("{} was left out of the agent's guide", what(source)),
            Detail::default()
                .at(Location::file(path.display().to_string()))
                .because(format!("it is not UTF-8 text: {}", e.utf8_error()))
                .fix("save it as UTF-8"),
        ))),
    }
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
