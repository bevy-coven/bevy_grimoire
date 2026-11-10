//! **A new project starts with an agent guide, and the person who made it is its author** (#349, AG-04).
//!
//! The New project wizard writes `<root>/.dark_factory/AGENTS.md` from a template, so a person finds the
//! file and its shape without reading a document about it. The template has four sections, each one italic
//! line of prompt text and nothing else — **This world** (the setting and scale), **Style** (look,
//! materials, density), **Rules** (what an agent must always or never do here) and **Lessons** (kept from
//! agents' proposals, which AG-06 appends under [`LESSONS_HEADING`]).
//!
//! **The person who creates the project authored the template**, so the wizard writes a guide grant for
//! that exact content in the same step: a new project's guide is never withheld from the person who just
//! made it. An existing project opened without the wizard gets no file — **nothing is written into a
//! project the editor did not create** — and the project page's **Create guide** is how such a project
//! gets one. Both call [`write`], so there is one writer of this file and one template.
//!
//! **Two calls, because the store is the person's and not the project's.** [`write_file`] puts the
//! template on disk and is what the project builder (`new_project::fill`) calls, which cannot know where a
//! person's trust file is. [`write`] is the whole act — the file *and* its grant — and is what every
//! caller that holds the person's store uses. Nothing writes the file twice: `write` calls `write_file`.
//!
//! Engine-free by nature (it is a file and a grant), but it lives in the editor because the wizard does.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::agent_guide::PROJECT_PATH;
use bevy_grimoire_core::trust::{self, TrustStore};

/// **The template, compiled in**, so a build that has no assets directory still writes it. One file, and
/// `include_str!` is what keeps the shape a person reads and the shape the test asserts the same text.
pub const TEMPLATE: &str = include_str!("guide_template.md");

/// **The heading AG-06 appends lessons under.** Named here so a later slice that proposes a lesson appends
/// under this heading rather than restating it and drifting.
pub const LESSONS_HEADING: &str = "## Lessons";

/// **The four sections, in the order the template writes them**, as a heading and the line under it. A test
/// reads this list against [`TEMPLATE`] so a section cannot be dropped or reordered without failing here.
pub const SECTIONS: [( &str, &str); 4] = [
    ("## This world", "*What this game is: its setting, its scale, and what a person walking through it should notice first.*"),
    ("## Style", "*The look: materials, density, how worn things are, and what this project never looks like.*"),
    ("## Rules", "*What an agent must always do here, and what it must never do. One rule a line.*"),
    ("## Lessons", "*Kept from agents' proposals: what a task made hard, and what to do differently next time.*"),
];

/// **Where the guide goes**, inside a project's root.
pub fn path(root: &Path) -> PathBuf {
    root.join(PROJECT_PATH)
}

/// **Put the template on disk**, and nothing else. Called by the project builder, which has no store to
/// write to; a person is never given a guide file whose grant was silently skipped, because every caller
/// that has a store calls [`write`] instead.
pub fn write_file(root: &Path) -> Result<PathBuf, String> {
    let file = path(root);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    bevy_grimoire_core::atomic_file::write(&file, TEMPLATE)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(file)
}

/// **Write the template into `root` and grant it**, which is one act: a person who made the project has
/// written this text, so it is allowed. The grant goes to `store` — the person's own trust file — and a
/// store that cannot be read or written is an `Err` naming it, never a silent grant-less guide.
pub fn write(root: &Path, store: &Path) -> Result<PathBuf, String> {
    let file = write_file(root)?;
    grant(root, store)?;
    Ok(file)
}

/// **Grant the template as it stands in `root`**, for a caller that wrote the file already.
pub fn grant(root: &Path, store: &Path) -> Result<(), String> {
    let mut trusted = TrustStore::load(store)?;
    trust::grant_guide(&mut trusted, root, TEMPLATE)?;
    trusted.save(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_holds_the_four_sections_in_order_each_with_its_line() {
        let mut at = 0;
        for (heading, line) in SECTIONS {
            let found = TEMPLATE[at..]
                .find(heading)
                .unwrap_or_else(|| panic!("the template has no `{heading}` after character {at}:\n{TEMPLATE}"));
            let from = at + found + heading.len();
            let after = &TEMPLATE[from..];
            let next = after.trim_start();
            assert!(
                next.starts_with(line),
                "`{heading}` is not followed by its prompt line.\nExpected: {line}\nThe template has:\n{}",
                after.lines().take(3).collect::<Vec<_>>().join("\n")
            );
            at = from;
        }
    }

    #[test]
    fn the_lessons_heading_is_the_one_the_template_writes() {
        assert!(TEMPLATE.contains(LESSONS_HEADING), "the template's Lessons heading moved: {TEMPLATE}");
    }
}
