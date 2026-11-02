//! **The editor's icon atlas has a glyph for every toolbar command, and the file says the same as the
//! list** (UI-38, #232).
//!
//! `assets/ui/icons.png` is a sheet of 16 px white-on-transparent glyphs (bevy_gooey's `Icon` tints
//! them from the skin). `icons::ICONS` names each cell, so the index of a command is its place in the
//! list; the assertions read the PNG itself, so a name with no art, art with no name, or two names
//! drawn alike fail here rather than showing a blank button.

use bevy_cauldron::{commands, icons};

fn atlas() -> image::RgbaImage {
    image::load_from_memory(icons::ATLAS_PNG).expect("icons.png decodes").to_rgba8()
}

fn rows() -> u32 {
    (icons::ICONS.len() as u32).div_ceil(icons::COLUMNS)
}

/// The pixels of cell `index`, row by row.
fn cell(sheet: &image::RgbaImage, index: usize) -> Vec<[u8; 4]> {
    let (cx, cy) = (index as u32 % icons::COLUMNS * icons::CELL, index as u32 / icons::COLUMNS * icons::CELL);
    let mut pixels = Vec::new();
    for y in cy..cy + icons::CELL {
        for x in cx..cx + icons::CELL {
            pixels.push(sheet.get_pixel(x, y).0);
        }
    }
    pixels
}

#[test]
fn every_toolbar_command_is_a_registered_command_with_an_icon() {
    assert!(!icons::TOOLBAR.is_empty(), "the toolbar has commands");
    for id in icons::TOOLBAR {
        assert!(commands::by_id(id).is_some(), "`{id}` is on the toolbar but is not a registered command");
    }
    assert_eq!(icons::without_an_icon(icons::TOOLBAR), Vec::<&str>::new());
}

#[test]
fn a_toolbar_command_without_an_atlas_index_is_named() {
    assert_eq!(
        icons::without_an_icon(&["edit.undo", "map.not_a_command", "file.save", "map.also_missing"]),
        vec!["map.not_a_command", "map.also_missing"]
    );
}

#[test]
fn every_icon_name_is_unique_and_indexes_its_own_place() {
    for (place, name) in icons::ICONS.iter().enumerate() {
        assert_eq!(icons::index(name), Some(place), "`{name}` is listed twice or its index drifted");
    }
    assert_eq!(icons::index("no.such.icon"), None);
}

#[test]
fn the_sheet_is_whole_cells_and_every_named_cell_is_drawn_white_on_transparent() {
    let sheet = atlas();
    assert_eq!(
        (sheet.width(), sheet.height()),
        (icons::COLUMNS * icons::CELL, rows() * icons::CELL),
        "the sheet is COLUMNS cells wide and as many rows as the list needs"
    );
    for (index, name) in icons::ICONS.iter().enumerate() {
        let pixels = cell(&sheet, index);
        let lit = pixels.iter().filter(|p| p[3] > 0).count();
        assert!(lit >= 8, "`{name}` (cell {index}) has {lit} lit pixels: it is not drawn");
        assert!(lit < pixels.len(), "`{name}` (cell {index}) is a solid block");
        for p in pixels.iter().filter(|p| p[3] > 0) {
            assert_eq!(&p[..3], &[255, 255, 255], "`{name}` has a coloured pixel: the skin tints icons, so they are white");
            assert_eq!(p[3], 255, "`{name}` has a soft edge: pixel art is on or off");
        }
    }
}

#[test]
fn no_two_icons_are_drawn_alike_and_a_spare_cell_is_empty() {
    let sheet = atlas();
    let drawn: Vec<Vec<[u8; 4]>> = (0..icons::ICONS.len()).map(|i| cell(&sheet, i)).collect();
    for a in 0..drawn.len() {
        for b in a + 1..drawn.len() {
            assert_ne!(drawn[a], drawn[b], "`{}` and `{}` are the same picture", icons::ICONS[a], icons::ICONS[b]);
        }
    }
    for spare in icons::ICONS.len()..(rows() * icons::COLUMNS) as usize {
        assert!(cell(&sheet, spare).iter().all(|p| p[3] == 0), "cell {spare} has art but no name in ICONS");
    }
}

#[test]
fn the_layout_cuts_the_sheet_into_the_same_cells() {
    let layout = icons::layout();
    assert_eq!(layout.len(), (rows() * icons::COLUMNS) as usize);
    let at = icons::index("map.fill").expect("fill has an icon");
    let rect = layout.textures[at];
    assert_eq!(rect.min.x, at as u32 % icons::COLUMNS * icons::CELL);
    assert_eq!(rect.min.y, at as u32 / icons::COLUMNS * icons::CELL);
    assert_eq!(rect.size(), bevy::math::UVec2::splat(icons::CELL));
}
