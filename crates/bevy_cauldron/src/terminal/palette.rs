//! **The 256 colours a terminal may name, resolved to this editor's ink.**
//!
//! A shell writes *indices*, not colours: `ESC[31m` means "the red slot", and which red that is
//! belongs to whoever is drawing. So the table lives here, at the boundary `terminal/grid.rs` calls
//! [`resolve`] at, and everything above `grid.rs` carries a `Color` and has never heard of a palette
//! index. That is deliberate: a pane that knew what index 203 was would be a second place the ink is
//! decided, and the first thing to drift when the editor's own palette moves.
//!
//! # The first sixteen are the editor's, the rest are the standard's
//!
//! Indices 0–15 are the ANSI slots a program names semantically ("this is a filename", "this is a
//! warning"), and they are the ones a person actually sees all day. Mapping them onto
//! [`crate::chrome`]'s palette is what makes a shell in this window look like part of this window
//! rather than like somebody else's terminal pasted in — the same argument `ui.rs` makes for
//! `editor_skin.yaml`, one layer down.
//!
//! Indices 16–255 are not a style choice. The 6×6×6 colour cube and the 24-step grey ramp are a
//! *standard*: `cargo`, `ls --color` and every TUI compute against those exact values, and remapping
//! them would silently change a program's own colour arithmetic. So they are the xterm values,
//! restated here as data.
//!
//! # The cube and the ramp are computed once, at compile time
//!
//! They are a pure function of the index, so they are a `const fn` rather than 240 transcribed
//! numbers: hand-typed colour tables are exactly the class of thing that is wrong in the middle and
//! never noticed.

use bevy::prelude::*;

use crate::chrome;

/// **The ANSI slots 0–15**, in the standard's order: eight normal, then the same eight bright.
///
/// The picks are the editor's own words wherever one exists — `chrome::ink().text` for "foreground" ink,
/// `chrome::ink().danger` for red, `chrome::ink().accent` for yellow — and the slots this palette has no word
/// for are chosen to sit at the same lightness as the ones beside them, so a line mixing `31` and
/// `33` does not flicker between two eras of design.
/// **The ANSI slots 0–15**, in the standard's order: eight normal, then the same eight bright.
///
/// **A function and not a `const`** (UI-02, #210): four of the sixteen are the editor's own ink, and
/// the ink is the skin's now — read from `src/editor_skin.yaml` at run time, so the table cannot be a
/// compile-time constant any more. The twelve that are nobody's word are still written here, each
/// with the reason it has no palette name.
fn ansi(ink: &chrome::Ink) -> [Color; 16] {
    [
        // 0–7: black, red, green, yellow, blue, magenta, cyan, white.
        Color::srgb(0.129, 0.122, 0.110), // CHROME-OK: ANSI slot 0 is a shade below `ink.panel_bg`, so a
        // `ESC[40m` block reads as a block against the panel rather than as the panel.
        ink.danger,
        Color::srgb(0.545, 0.780, 0.490), // CHROME-OK: ANSI slot 2 — "success" green, which the
        // editor's palette has no word for (it has DANGER and ACCENT, and a third severity would be a
        // second green nobody asked for).
        ink.accent,
        Color::srgb(0.451, 0.647, 0.851), // CHROME-OK: ANSI slot 4 — the blue a compiler's `help:`
        // lines arrive in, and the editor's palette is warm by design and has no blue at all.
        Color::srgb(0.788, 0.573, 0.792), // CHROME-OK: ANSI slot 5, magenta; no palette word.
        Color::srgb(0.478, 0.757, 0.776), // CHROME-OK: ANSI slot 6, cyan; no palette word.
        ink.text,
        // 8–15: the bright half.
        ink.muted,
        Color::srgb(0.969, 0.635, 0.588), // CHROME-OK: bright red, `ink.danger` lifted toward white so
        // bold red still reads as an error rather than as a second red.
        Color::srgb(0.663, 0.878, 0.604), // CHROME-OK: bright green.
        Color::srgb(1.000, 0.831, 0.502), // CHROME-OK: bright yellow.
        Color::srgb(0.573, 0.749, 0.937), // CHROME-OK: bright blue.
        Color::srgb(0.878, 0.686, 0.882), // CHROME-OK: bright magenta.
        Color::srgb(0.612, 0.855, 0.871), // CHROME-OK: bright cyan.
        Color::srgb(0.976, 0.965, 0.941), // CHROME-OK: bright white — brighter than `ink.text`, because
                                          // `ESC[97m` on a `ESC[1m` heading has to be the brightest thing
                                          // on the pane or the escape did nothing.
    ]
}

/// What a cell draws when the child said nothing: the shell's own default.
///
/// Taken from [`crate::chrome`] so a shell looks like the panel it is docked in — the same reasoning
/// `docs/ui.md` §5 gives for keeping Bevy's Feathers greys out of the editor.
pub fn default_fg() -> Color {
    chrome::ink().text
}

/// The pane's own ground. `chrome::ink().panel_bg`, because the terminal is a panel.
pub fn default_bg() -> Color {
    chrome::ink().panel_bg
}

/// **The selection's ground**, under the selected cells' own ink (#309). `chrome::ink().row_selected`,
/// the editor's one word for "selected", so a terminal selection reads like a chosen row anywhere
/// else in the window and the cells keep their colours (a red error stays red).
pub fn selection_bg() -> Color {
    chrome::ink().row_selected
}

/// **A find match's ground** (#321): the accent, faint, so every match on screen is lit without
/// drowning the text, and the current one ([`find_current_bg`]) stands out among them. Mixed from the
/// pane's ground toward `chrome::ink().accent` rather than a new hue, so the terminal keeps the editor's one
/// accent.
pub fn find_bg() -> Color {
    chrome::ink().panel_bg.mix(&chrome::ink().accent, FIND_MIX)
}

/// **The current find match's ground**: the same accent, stronger.
pub fn find_current_bg() -> Color {
    chrome::ink().panel_bg.mix(&chrome::ink().accent, FIND_CURRENT_MIX)
}

/// How far toward the accent a match's ground is mixed.
const FIND_MIX: f32 = 0.3;

/// How far toward the accent the current match's ground is mixed.
const FIND_CURRENT_MIX: f32 = 0.65;

/// **Index 16–231: the 6×6×6 cube.** `0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff` are the xterm levels —
/// not 0/51/102/153/204/255, which is the same idea computed with the wrong step and is why two
/// terminals disagree about what `ESC[38;5;100m` looks like.
const CUBE_LEVELS: [u8; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];

/// **Index 232–255: 24 greys**, evenly spaced from 8/255 to 238/255.
const GREY_FIRST: u8 = 8;
const GREY_STEP: u8 = 10;

/// The colour a program named by index.
///
/// Out-of-range is impossible — the type is `u8` and every value 0–255 has an answer — so this
/// returns a `Color` rather than an `Option`, and a caller has no branch to get wrong.
pub fn resolve(index: u8) -> Color {
    if index < 16 {
        // **Built once per ink** (review, 2026-11-10; #428): `ansi()` mixes the ink into sixteen colours, and a terminal repaints a
        // whole screen of cells per frame, so a rebuild per cell was work repeated for an answer that cannot change while the
        // ink does not. The ink is the theme's and changes live, so the table is built again when the four inks it takes
        // (danger, accent, text, muted) are not the ones it was built from, and only then.
        static ANSI: std::sync::Mutex<Option<([Color; 4], [Color; 16])>> = std::sync::Mutex::new(None);
        let ink = chrome::ink();
        let key = [ink.danger, ink.accent, ink.text, ink.muted];
        let mut built = ANSI.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let table = match &*built {
            Some((from, table)) if *from == key => *table,
            _ => {
                let table = ansi(&ink);
                *built = Some((key, table));
                table
            }
        };
        return table[index as usize];
    }
    let (r, g, b) = rgb(index);
    Color::srgb_u8(r, g, b)
}

/// The xterm RGB triple for an index: the cube for 16–231, the grey ramp above it, and the cube's
/// first entry as the answer for anything under 16 (which [`resolve`] never asks).
pub const fn rgb(index: u8) -> (u8, u8, u8) {
    if index >= 232 {
        let level = GREY_FIRST + (index - 232) * GREY_STEP;
        return (level, level, level);
    }
    if index < 16 {
        return (0, 0, 0);
    }
    let cube = index - 16;
    let r = CUBE_LEVELS[(cube / 36) as usize];
    let g = CUBE_LEVELS[((cube % 36) / 6) as usize];
    let b = CUBE_LEVELS[(cube % 6) as usize];
    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two indices every xterm-compatible terminal agrees on, because they are the ones a
    /// program's own colour arithmetic lands on: the cube's first and last cell.
    #[test]
    fn the_cube_ends_are_the_standard_ones() {
        assert_eq!(rgb(16), (0x00, 0x00, 0x00), "index 16 is the cube's origin");
        assert_eq!(rgb(231), (0xff, 0xff, 0xff), "index 231 is the cube's far corner");
        assert_eq!(rgb(196), (0xff, 0x00, 0x00), "index 196 is pure red");
    }

    /// The grey ramp is monotone and spans the documented range. A ramp that stopped short would
    /// make `ESC[38;5;255m` dimmer than `ESC[38;5;231m`, and a diff viewer that greys old lines by
    /// index would look broken.
    #[test]
    fn the_grey_ramp_climbs_to_almost_white() {
        let mut last = 0;
        for index in 232..=255u8 {
            let (r, g, b) = rgb(index);
            assert_eq!((r, g, b), (r, r, r), "index {index} is not grey");
            assert!(r > last, "index {index} did not climb");
            last = r;
        }
        assert_eq!(last, 238, "the ramp ends at 238/255, as xterm's does");
    }

    /// The first sixteen are the editor's ink, not a black hole. This is the assertion that would
    /// have caught a transcribed table whose first entry was left at `Color::BLACK`.
    #[test]
    fn the_named_slots_are_not_all_the_same_colour() {
        let mut seen = std::collections::BTreeSet::new();
        for index in 0..16u8 {
            seen.insert(format!("{:?}", resolve(index)));
        }
        assert_eq!(seen.len(), 16, "the sixteen named slots must be sixteen colours");
    }
}
