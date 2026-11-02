//! **The editor's icon atlas** (UI-38, #232): one 16 px glyph per toolbar and panel command, in
//! `assets/ui/icons.png`, drawn white on transparent because bevy_gooey's `Icon` tints a glyph from the
//! skin (`IconTone`) and never from a number here.
//!
//! The index of an icon is its place in [`ICONS`]: the sheet is [`COLUMNS`] cells wide, filled left to
//! right and top to bottom. `tools/ui/make_icons.py` draws the sheet from the names in this file, and
//! `tests/every_toolbar_command_has_an_icon.rs` reads the PNG back, so a name with no art, art with no
//! name or two glyphs drawn alike fails a test rather than showing a blank button.
//!
//! An icon is named by the command it stands for (`edit.undo`, the id in [`crate::commands`]), or by
//! `panel.<id>` for a dock panel, which is not a command until the workbench (UI-30) registers them.
//! The toolbar (UI-37) takes its glyphs from [`TOOLBAR`] and a panel's tab from its `panel.` name.

use bevy::math::UVec2;
use bevy::prelude::TextureAtlasLayout;

/// The sheet, embedded so an editor with no project and no working directory still has its icons.
pub const ATLAS_PNG: &[u8] = include_bytes!("../assets/ui/icons.png");

/// A glyph is this many pixels on a side.
pub const CELL: u32 = 16;

/// The sheet is this many glyphs wide.
pub const COLUMNS: u32 = 8;

/// Every icon, in sheet order. `tools/ui/make_icons.py` reads this list between its two markers.
// icons:begin
pub const ICONS: &[&str] = &[
    "edit.undo",
    "edit.redo",
    "file.save",
    "map.fill",
    "map.remove_mode",
    "map.move_mode",
    "map.clone_mode",
    "map.note",
    "map.run_mode",
    "map.generate",
    "view.frame",
    "tools.snip",
    "panel.outliner",
    "panel.library",
    "panel.inspector",
    "panel.history",
    "panel.problems",
    "panel.notes",
    "panel.data",
    "panel.familiar",
    "panel.reload",
    "panel.log",
];
// icons:end

/// The commands the toolbar shows: the tool modes, undo and redo, save, and the view and capture
/// helpers. Each is a registered command and has an icon.
pub const TOOLBAR: &[&str] = &[
    "edit.undo",
    "edit.redo",
    "file.save",
    "map.fill",
    "map.remove_mode",
    "map.move_mode",
    "map.clone_mode",
    "map.note",
    "map.run_mode",
    "map.generate",
    "view.frame",
    "tools.snip",
];

/// The atlas index of the icon named `name`.
pub fn index(name: &str) -> Option<usize> {
    ICONS.iter().position(|icon| *icon == name)
}

/// The names in `names` that have no icon, in the order given: the refusal a toolbar command without
/// a glyph gets.
pub fn without_an_icon<'a>(names: &[&'a str]) -> Vec<&'a str> {
    names.iter().copied().filter(|name| index(name).is_none()).collect()
}

/// How the sheet is cut up, for `bevy_gooey`'s `IconSource::Atlas { index, .. }`.
pub fn layout() -> TextureAtlasLayout {
    let rows = (ICONS.len() as u32).div_ceil(COLUMNS);
    TextureAtlasLayout::from_grid(UVec2::splat(CELL), COLUMNS, rows, None, None)
}
