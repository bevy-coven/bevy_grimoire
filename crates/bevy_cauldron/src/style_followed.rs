//! **The style lines a hand-back says it followed** (SG-09, #408).
//!
//! `grimoire_present` takes an optional `followed: [{path, line}]`: the lines of the style `grimoire_style` served the agent that it worked to. The
//! person's hand-back card lists each line's text with a link that opens it on the Design page, so they can see whether the style was used and
//! argue with a specific line instead of a general impression. A handback with no style involved cites nothing.
//!
//! # A cited line must be a line the agent was served
//!
//! Each `{path, line}` must point at a non-empty line of a page of a chain `grimoire_style` answered in this editor session, and the page must
//! still say it: [`StyleServed`] holds each page's text as it was served, and [`check`] compares it with the page on disk. Anything else is refused
//! with the entry and the reason, so a citation is never decoration. **The reasons name paths and numbers and nothing a page says**: a page's text
//! is a prompt, and the editor does not hand it back to an agent in a refusal.
//!
//! # The text is for the person
//!
//! [`Followed::text`] is the line as served, drawn on the card. `grimoire_state` carries the `{path, line}` entries and never the text.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// **How many followed lines the hand-back card lists**; the rest are a count, and every line stays in `grimoire_state`.
pub const FOLLOWED_SHOWN_MAX: usize = 5;

/// **The most lines one hand-back may cite.** A citation is a handful of the lines the work went by; past this it is a copy of the style.
pub const FOLLOWED_MAX: usize = 30;

/// **The most pages the editor notes as served**, oldest dropped first: a chain is a few pages (the game's, a region's, a level's and an area's, each with a topic), so this is
/// many chains, and an agent that reads more asks again.
pub const STYLE_SERVED_MAX: usize = 64;

/// **The pages a `grimoire_style` answer served, with the text that was trust-checked for each**: a file the answer lists as served, paired with the text its decision was made
/// about. A file withheld or unread is not here, and nothing is read from disk, so what is noted is what was served.
pub fn served_pages(files: &[bevy_grimoire_core::style::File], checked: &[(String, String)]) -> Vec<(String, String)> {
    files
        .iter()
        .filter(|f| f.state == bevy_grimoire_core::style::FileState::Served)
        .filter_map(|f| checked.iter().find(|(path, _)| *path == f.path).cloned())
        .collect()
}

/// **One style line, as an agent cites it.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FollowedParam {
    /// The page, relative to the project root, as `grimoire_style` headed it.
    pub path: String,
    /// The line, counted from 1 as an editor shows it.
    pub line: u32,
}

/// **A cited line that was checked**, with its text as served for the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followed {
    pub path: String,
    pub line: u32,
    /// The line's text as the page said it when it was served. For the person's card; never in `grimoire_state`.
    pub text: String,
}

/// **The pages of the style `grimoire_style` has served this session**, with their text as served. `Session`: a hand-back on any door cites a
/// chain it was served on any other.
#[derive(Resource, Default)]
pub struct StyleServed {
    pages: Vec<ServedPage>,
}

struct ServedPage {
    root: PathBuf,
    path: String,
    text: String,
}

impl StyleServed {
    /// **Note that this page was served**, as it reads now. A page served again replaces what was noted for it.
    pub fn record(&mut self, root: &Path, path: &str, text: String) {
        // **A project that closes takes what it served with it**: only the open project's pages are kept, and a page served again moves to the newest.
        self.pages.retain(|p| p.root == root && p.path != path);
        self.pages.push(ServedPage { root: root.to_path_buf(), path: path.to_owned(), text });
        if self.pages.len() > STYLE_SERVED_MAX {
            self.pages.remove(0);
        }
    }

    /// **Forget everything served**: a client that said hello again is a new agent, which must ask `grimoire_style` again before it cites a line.
    pub fn clear(&mut self) {
        self.pages.clear();
    }

    /// How many pages are noted.
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether nothing is noted.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// The text this page had when it was last served, when it was.
    fn text_of(&self, root: &Path, path: &str) -> Option<&str> {
        self.pages.iter().find(|p| p.root == root && p.path == path).map(|p| p.text.as_str())
    }
}

/// **Check every cited line, or say which one is not a line the agent was served** — the first refusal, naming the entry (counted from 1) and why.
pub fn check(served: &StyleServed, root: &Path, entries: &[FollowedParam]) -> Result<Vec<Followed>, String> {
    if entries.len() > FOLLOWED_MAX {
        return Err(format!("`followed` cites {} lines, and at most {FOLLOWED_MAX} may be cited: name the lines the work went by, not the style.", entries.len()));
    }
    let mut out: Vec<Followed> = Vec::new();
    for (n, entry) in entries.iter().enumerate() {
        let which = format!("followed entry {} (`{}`, line {})", n + 1, entry.path, entry.line);
        if out.iter().any(|f| f.path == entry.path && f.line == entry.line) {
            return Err(format!("{which} is listed twice."));
        }
        let Some(text) = served.text_of(root, &entry.path) else {
            return Err(format!(
                "{which}: `{}` is not a page of any chain `grimoire_style` has served you in this session, so it is not a line you followed. Call `grimoire_style` for the place you worked on and cite lines it returned.",
                entry.path
            ));
        };
        if entry.line == 0 {
            return Err(format!("{which}: lines count from 1."));
        }
        let lines: Vec<&str> = text.lines().collect();
        let Some(served_line) = lines.get(entry.line as usize - 1) else {
            return Err(format!("{which}: the page was served with {} lines, so there is no line {}.", lines.len(), entry.line));
        };
        if served_line.trim().is_empty() {
            return Err(format!("{which}: that line is blank, so there is nothing in it to have followed."));
        }
        let now = std::fs::read_to_string(root.join(&entry.path)).ok();
        if now.as_deref().and_then(|t| t.lines().nth(entry.line as usize - 1)) != Some(*served_line) {
            return Err(format!("{which}: `{}` no longer says that at that line; it changed after it was served. Call `grimoire_style` again and cite the lines it returns now.", entry.path));
        }
        out.push(Followed { path: entry.path.clone(), line: entry.line, text: (*served_line).to_owned() });
    }
    Ok(out)
}

/// **The card's words for the lines beyond the ones it lists.**
pub fn more_says(followed: usize) -> Option<String> {
    let more = followed.checked_sub(FOLLOWED_SHOWN_MAX).filter(|more| *more > 0)?;
    Some(format!("and {more} more, which the state keeps"))
}
