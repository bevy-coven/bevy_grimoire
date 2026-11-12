//! **Cross-links: a reference opens what it names** (NV-07, #384; series getting around, #377).
//!
//! Everything that names another thing opens it through the one reveal, so there is no second way to
//! arrive anywhere. This module holds the one query a `Used by` list asks; the references themselves
//! wait on workspaces the editor does not draw yet.
//!
//! # What this slice delivers, and what it does not
//!
//! | reference | destination | state |
//! |---|---|---|
//! | a piece's **Used by** | the maps that place it | the query and its sentence are here ([`used_by`], [`used_by_line`], from `Project::maps_that_place`); **no panel draws them yet** |
//! | a placement's **Open file** | its map's `.map.yaml`, at that row | **not yet**: the Files workspace is GD-11 (#373), so a `Shown::File` is refused with the sentence `reveal::page_of` already writes for a file |
//! | a cue's **Used by** in Sound | the files and maps that name it | **not yet**: the Sound workspace is NV-08 (#385), and no map, script or file carries a cue to name |
//! | Cmd/Ctrl+click a `cue:` value in the file editor | the cue in Sound | **not yet**: the file editor is GD-11 (#373) |
//!
//! **A reference to a thing that does not exist is never a dead link**: a map a piece names that
//! `maps/` does not hold, a placement whose row an agent deleted — each is a problem naming the
//! reference rather than a button that does nothing. That rule has nothing to apply to until a panel
//! draws a reference, and is part of #384's remainder with the rest of the table's *not yet* rows.

use crate::project::Project;

/// **The maps that place `id`**, by name, from `Project::maps_that_place` — the one reader of the
/// project's `maps/` directory, so a second scan cannot disagree with it.
pub fn used_by(project: &Project, id: &str) -> Result<Vec<String>, String> {
    project.maps_that_place(id)
}

/// **The list a panel's `Used by` shows**, and the sentence when there is nothing.
pub fn used_by_line(project: &Project, id: &str) -> Result<String, String> {
    let maps = used_by(project, id)?;
    Ok(match maps.len() {
        0 => format!("nothing places `{id}`"),
        1 => format!("placed in {}", maps[0]),
        n => format!("placed in {n} maps: {}", maps.join(", ")),
    })
}
