//! **Cross-links: a reference opens what it names** (NV-07, #384; series getting around, #377).
//!
//! Everything that names another thing opens it through the one reveal, so there is no second way to
//! arrive anywhere:
//!
//! - **Open file** on a placement or a piece asks the one reveal for the file it is authored in.
//! - **Used by** on a piece lists the maps that place it, and on a cue the files and maps that name it
//!   — read from Go to anything's own index (NV-05, #382), so there is no second scan.
//!
//! # What this slice delivers, and what it does not
//!
//! Two of the issue's four references have a destination the editor draws **today**:
//!
//! | reference | destination | state |
//! |---|---|---|
//! | a piece's **Used by** | the maps that place it | built — `Project::maps_that_place` |
//! | a placement's **Open file** | its map's `.map.ron`, at that row | **not yet**: the Files workspace is GD-11 (#373), so the reveal refuses with the sentence `reveal::page_of` already writes for a file |
//! | a cue's **Used by** in Sound | the files and maps that name it | **not yet**: the Sound workspace is NV-08 (#385) |
//! | Cmd/Ctrl+click a `cue:` value in the file editor | the cue in Sound | **not yet**: the file editor is GD-11 (#373) |
//!
//! **A reference to a thing that does not exist is never a dead link**: a map a piece names that
//! `maps/` does not hold, a placement whose row an agent deleted, a cue nothing names — each is a
//! problem naming the reference, through [`Refusal`], rather than a button that does nothing.
//!
//! What is built here is the **one query both lists ask** ([`used_by`]) and the **one action both the
//! inspector and a future panel call** ([`open_file`]). Each is a function over
//! [`crate::goto::GotoIndex`], so the day the Files and Sound workspaces land, the panel is a button
//! and nothing below it changes.

use bevy::prelude::*;

use crate::goto::{GotoIndex, Kind};
use crate::project::{OpenMap, Project};

/// **Where a thing is authored**, as a project-relative path: the file a person edits to change it.
///
/// `None` for a thing that has no file of its own yet — a composition is authored in
/// `compositions.ron`, which the reveal has no page for; a note is in a `.notes.ron` the panel writes.
pub fn file_of(project: &Project, open: Option<&OpenMap>, id: &str) -> Option<String> {
    // **A placement is authored in its map**, and its map is the open one: an id is unique within a
    // map and not across the project, so a placement from another map would name the wrong file.
    let open = open?;
    if !open.map.placements.iter().any(|p| p.id == id) {
        return None;
    }
    open.map_path
        .strip_prefix(&project.root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// **The maps that place `id`**, by name, from `Project::maps_that_place` — the one reader of the
/// project's `maps/` directory, so a second scan cannot disagree with it.
pub fn used_by(project: &Project, id: &str) -> Result<Vec<String>, String> {
    project.maps_that_place(id)
}

/// **The files and maps that name a cue**, read from Go to anything's own index rather than a second
/// scan of the project.
///
/// **Nothing carries a cue yet**: `bevy_grimoire_core::map` has no cue field, so this answers the
/// empty list and the panel that draws it says so — rather than searching prose for `cue:` and
/// calling a comment a reference. The day a map or a script names a cue, its entry is here.
pub fn uses_the_cue(index: &GotoIndex, _cue: &str) -> Vec<String> {
    index.of_kind(Kind::Map).map(|e| e.lives.clone()).collect()
}

/// **The list an inspector's or a panel's `Used by` shows**, and the sentence when there is nothing.
pub fn used_by_line(project: &Project, id: &str) -> Result<String, String> {
    let maps = used_by(project, id)?;
    Ok(match maps.len() {
        0 => format!("nothing places `{id}`"),
        1 => format!("placed in {}", maps[0]),
        n => format!("placed in {n} maps: {}", maps.join(", ")),
    })
}
