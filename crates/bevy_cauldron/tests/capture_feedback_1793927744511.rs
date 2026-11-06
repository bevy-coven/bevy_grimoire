//! **Every glyph a chord label can draw is in the face the editor installs** —
//! `feedback-1793927744511`, relation: *coverage of the declaration*.
//!
//! The note, from the editor at 2560x1604: *"Why is there a "?" here?"* — pointing at the Undo tooltip's
//! keycap, which draws `?Z` where its own accessibility text reads `⌘Z`. The `?` is a tofu box's fallback
//! glyph, not a character anybody typed.
//!
//! # The relation, and why this one
//!
//! A picture has no expected output, so the oracle is a relation between two things that both exist in
//! one build: **the strings the UI will draw, and the face it draws them with**. Segura, Fraser, Sánchez &
//! Ruiz-Cortés, *A Survey on Metamorphic Testing* (`10.1109/TSE.2016.2532875`), find the effective relations
//! are those "strongly inspired by the semantics of the program under test" — here, that text is legible
//! exactly when its codepoints are mapped by the font. An unmapped codepoint cannot render as anything but
//! a replacement box, whatever the layout does.
//!
//! The survey's other finding is why this asks the *font* and not a screenshot: a pixel test would have to
//! guess what a tofu box looks like at every size and would pass or fail on antialiasing. The cmap is the
//! font's own statement of what it can draw, and it is the same data the shaper consults.
//!
//! # What the capture shows
//!
//! Read from `FiraMono-Regular.ttf`'s own `cmap` (1,350 codepoints) against the thirteen glyphs
//! `shortcut_label` can emit on a Mac — `⌃⌥⇧⌘` for modifiers and `↩⌫⌦⎋⇥↑↓←→` for named keys:
//!
//! | present | missing |
//! |---|---|
//! | `⇧ ⌫ ⌦ ↑ ↓ ← →` | **`⌘ ⌃ ⌥ ↩ ⎋ ⇥`** |
//!
//! Seven of thirteen. `⌘` is on nearly every chord on a Mac, which is why the first person to hover a
//! toolbar button reported it.
//!
//! # Why this test ignores rather than fixes
//!
//! The glyphs are chosen in `bevy_gooey::shortcut_label`, which has no API by which a host can ask for a
//! spelling it can draw; filed as [bevy-coven/bevy_gooey#45](https://github.com/bevy-coven/bevy_gooey/issues/45).
//! This repo must not patch, wrap or vendor a family crate. So the test is the **receipt**: it reads the
//! font this build installs and names every missing glyph, and it is what will prove the pin bump or the
//! face swap that closes the issue.

/// **The codepoints a chord label can draw on a Mac**, as `bevy_gooey::shortcut_label` spells them.
///
/// Written out here rather than imported because the point is to be independent of that function: if the
/// crate changes its spelling, this list is what says whether the new one is drawable.
const MAC_CHORD_GLYPHS: &[(char, &str)] = &[
    ('⌃', "control"),
    ('⌥', "option"),
    ('⇧', "shift"),
    ('⌘', "command"),
    ('↩', "return"),
    ('⌫', "delete"),
    ('⌦', "forward delete"),
    ('⎋', "escape"),
    ('⇥', "tab"),
    ('↑', "up arrow"),
    ('↓', "down arrow"),
    ('←', "left arrow"),
    ('→', "right arrow"),
];

/// `(codepoint, present)` for each codepoint in the face at `path`, read from the font's own `cmap`.
///
/// A hand-written reader because this crate forbids a font-parsing dependency for one test, and because the
/// two formats a modern face uses — 4 (BMP segments) and 12 (full repertoire) — are a page of code each.
/// Returns `None` when the file has no `cmap` the reader understands, so a test can say so rather than
/// silently pass.
fn cmap_codepoints(bytes: &[u8]) -> Option<Vec<u32>> {
    fn u16_at(d: &[u8], at: usize) -> Option<u16> {
        d.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32_at(d: &[u8], at: usize) -> Option<u32> {
        d.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    let tables = u16_at(bytes, 4)? as usize;
    let mut cmap_at = None;
    for i in 0..tables {
        let off = 12 + 16 * i;
        let tag = bytes.get(off..off + 4)?;
        if tag == b"cmap" {
            cmap_at = Some(u32_at(bytes, off + 8)? as usize);
        }
    }
    let cmap = cmap_at?;
    let subtables = u16_at(bytes, cmap + 2)? as usize;
    let mut out = Vec::new();
    for i in 0..subtables {
        let record = cmap + 4 + 8 * i;
        let at = cmap + u32_at(bytes, record + 4)? as usize;
        match u16_at(bytes, at)? {
            // Format 4: BMP, as segment ranges.
            4 => {
                let seg_x2 = u16_at(bytes, at + 6)? as usize;
                let segs = seg_x2 / 2;
                let ends = at + 14;
                let starts = ends + seg_x2 + 2;
                for k in 0..segs {
                    let (Some(end), Some(start)) = (u16_at(bytes, ends + 2 * k), u16_at(bytes, starts + 2 * k)) else {
                        continue;
                    };
                    if start == 0xFFFF {
                        continue;
                    }
                    out.extend((start..=end.min(0xFFFE)).map(u32::from));
                }
            }
            // Format 12: the whole repertoire, as groups.
            12 => {
                let groups = u32_at(bytes, at + 12)? as usize;
                for g in 0..groups {
                    let at = at + 16 + 12 * g;
                    let (Some(start), Some(end)) = (u32_at(bytes, at), u32_at(bytes, at + 4)) else {
                        continue;
                    };
                    out.extend(start..=end);
                }
            }
            _ => {}
        }
    }
    (!out.is_empty()).then_some(out)
}

/// **The face this build installs covers every glyph a chord label can draw.**
///
/// The font is `harness::install_font_in`'s: a project's own `assets/fonts/FiraMono-Regular.ttf` when it
/// has one, this crate's embedded copy otherwise. This reads the same file the editor reads.
#[test]
// **Ignored until bevy-coven/bevy_gooey#45 lands**, and the reason is in the doc comment above: the spelling
// is the crate's choice and this repo must not patch, wrap or vendor it. The test is the **receipt** — it
// reads the font this build installs, names every glyph a chord cannot draw, and is what proves the pin bump
// or the face swap that closes the issue. `ignore`d rather than deleted (a deleted test is a forgotten
// defect) and rather than left red (a permanently red suite teaches a loop to ignore red).
#[ignore = "blocked on bevy-coven/bevy_gooey#45: Mac chord glyphs are not in the shipped face"]
fn the_installed_face_covers_every_chord_glyph() {
    // **What the fixture project carries**, which is what a project without its own font gets: the
    // fixture writes nothing to `assets/fonts/`, so this is the embedded face's own bytes by the same
    // rule `install_font_in` applies. Reading the embedded copy directly is the honest subject — the
    // capture was taken on a project with no face of its own.
    let bytes: &[u8] = include_bytes!("../assets/fonts/FiraMono-Regular.ttf");
    let Some(codepoints) = cmap_codepoints(bytes) else {
        panic!(
            "the installed face has no cmap this test can read, so it cannot say whether a chord's \
             glyphs are drawable — which is itself the finding: a chord label is drawn with whatever \
             this file maps"
        );
    };

    let missing: Vec<String> = MAC_CHORD_GLYPHS
        .iter()
        .filter(|(glyph, _)| !codepoints.contains(&(*glyph as u32)))
        .map(|(glyph, name)| format!("`{glyph}` U+{:04X} ({name})", *glyph as u32))
        .collect();

    let total = MAC_CHORD_GLYPHS.len();
    let covered = total - missing.len();
    assert!(
        missing.is_empty(),
        "the face installed as Bevy's default font maps {covered} of {total} chord glyphs, and a chord \
         label cannot be drawn with a glyph the face does not have — it becomes a tofu box, which is what \
         the note reports: the Undo tooltip draws `?Z` where its accessibility text reads `⌘Z`.\n\n\
         missing: {}\n\n\
         blocked on bevy-coven/bevy_gooey#45: `shortcut_label` chooses the Mac spelling and offers a host \
         no way to ask for one it can draw.",
        missing.join(", ")
    );
}
