//! **The What's new page's prose is set at a readable measure** (`feedback-1793905698793`, filed against
//! 0.2.6 on `mac_air`).
//!
//! The note: *"These boxes like this are are hard to read. The prose are bad. And there are several
//! separate boxes. Could we actually use a decision model to run all labels and prose through to make
//! sure it is human readable?"* Three complaints, and only the first two are the defect:
//!
//! - **hard to read**: each entry's sentence runs the full width of the page. The capture is 2560
//!   physical / 1280 logical wide, so a sentence is set at roughly 130 characters a line — and at that
//!   measure the eye loses the line it was on between the end of one and the start of the next. The
//!   crop of the capture shows exactly this: four entries, each one long line of prose spanning the
//!   whole window.
//! - **the prose are bad**: each entry's sentence is written as a changelog line ("Press Check for
//!   updates and it now tells you which version is the newest, instead of looking like nothing
//!   happened") rather than as the thing a person would do. That is authoring, not layout, and it is
//!   fixed where the sentences are written.
//! - *the decision-model sentence is the suggested fix*, not the defect: recorded in the ledger's
//!   `reason` and **not** in this pass.
//!
//! **The relation is a bound on the measure**, chosen from what the person said they saw rather than
//! from the code (metamorphic testing; Segura, Fraser, Sánchez & Ruiz-Cortés, *A Survey on Metamorphic
//! Testing*, IEEE TSE 2016, `10.1109/TSE.2016.2532875`). "Hard to read" at this width is a line that is
//! too long, so the assertion is:
//!
//! > no sentence on the page is set wider than [`MAX_MEASURE_CHARS`] average characters to a line,
//! > and the page's column is no wider than that bound allows.
//!
//! The bound is not a taste: Bringhurst, *The Elements of Typographic Style*, gives 45–75 characters
//! as the comfortable measure for a single column and calls a longer line disagreeable, and WCAG's
//! reflow criterion is written for the same failure — content that must be read by moving the eye
//! across a very wide line. **75** is taken as the ceiling: the comfortable range's upper end, and the
//! number a monospace face needs rather than a narrower one.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::launcher::{ChangelogText, HomeTab, Launcher, LauncherPlugin, NEWS_SHOWN};
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

/// The capture's window: `physical` 2560 x 1406 at `scale_factor` 2.0 is 1280 x 703 logical, which at
/// the harness's `UiScale` of 1.2 is this many texels.
const CAPTURE_SURFACE: (u32, u32) = (1280, 703);

/// **The most average characters a line of this page's prose may carry** — Bringhurst's comfortable
/// range ends at 75, and past 90 he calls the measure disagreeable.
const MAX_MEASURE_CHARS: f32 = 75.0;

/// The width of one character as a fraction of the font's size, for the monospace face this editor
/// ships (FiraMono). A monospace advance is a fixed proportion of the em, so a line's character count
/// is its width over that advance — no text shaping needed to measure a measure.
const MONO_ADVANCE_PER_EM: f32 = 0.6;

/// The face's size in logical pixels: `chrome` sets body text at 11 px and this page's sentences are
/// body text. Read from the drawn node rather than assumed if the constants change is the job of the
/// test that pins the skin; here the number only has to be close enough to count characters.
const BODY_PX: f32 = 11.0;

/// Ten entries, each with a sentence of the length the real ones have, so the page is the page the
/// person was looking at.
fn entries(count: usize) -> String {
    let mut text = String::from("# Changelog\n\n");
    for i in 0..count {
        text.push_str(&format!(
            "- **an-entry-{i}** `{i:07}` Something that changed *For you: entry {i} says what changed, \
             in the two or three lines a person would actually read, because every sentence on this \
             page is set as body text and every one of them is as long as this one.*\n"
        ));
    }
    text
}

fn the_page(tag: &str) -> App {
    let root = Fixture::new(&format!("measure_{tag}_{}", std::process::id()))
        .descriptor("bench", "props")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(ChangelogText(entries(NEWS_SHOWN)));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    bevy_cauldron::harness::resize_surface(&mut app, CAPTURE_SURFACE.0, CAPTURE_SURFACE.1)
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<Launcher>().tab = HomeTab::WhatsNew;
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Every long sentence on the page, with the width it was given**, in logical pixels.
///
/// "Long" is more than a couple of words: a heading or a button's label is short enough that its width
/// says nothing about a measure. The page's prose is what the note is about.
fn sentences_and_their_widths(app: &mut App) -> Vec<(String, f32)> {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode)>();
    let mut rows = q
        .iter(world)
        .filter(|(text, node)| text.0.chars().count() > 60 && node.size() != Vec2::ZERO)
        .map(|(text, node)| (text.0.clone(), node.size().x * node.inverse_scale_factor))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    rows
}

#[test]
fn the_whats_new_pages_prose_fits_a_readable_measure() {
    let mut app = the_page("measure");
    let rows = sentences_and_their_widths(&mut app);
    assert!(
        !rows.is_empty(),
        "the page sets no long sentence, so this test would prove nothing about a measure"
    );

    let advance = BODY_PX * MONO_ADVANCE_PER_EM;
    let mut too_wide = Vec::new();
    for (text, width) in &rows {
        // The face is monospace, so the line's character count is its width over one advance.
        let per_line = width / advance;
        if per_line > MAX_MEASURE_CHARS {
            too_wide.push(format!(
                "{per_line:.0} characters a line in {width:.0} px: {:?}",
                text.chars().take(60).collect::<String>()
            ));
        }
    }
    assert!(
        too_wide.is_empty(),
        "the page's prose is set wider than {MAX_MEASURE_CHARS:.0} characters a line, which is what \
         the note calls \"hard to read\" — the eye loses its place between one line and the next. Give \
         the entries a column narrower than the window:\n{}",
        too_wide.join("\n")
    );
}

#[test]
fn the_prose_column_does_not_grow_with_the_window() {
    // The same bound at a wider window, because a measure set as a percentage of the window is not a
    // measure: the capture's own window is 1280 logical wide, and a maximised one would be worse.
    let mut app = the_page("wide");
    bevy_cauldron::harness::resize_surface(&mut app, 1920, 1080).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let rows = sentences_and_their_widths(&mut app);
    assert!(!rows.is_empty(), "the page sets no long sentence at a wide window");
    let advance = BODY_PX * MONO_ADVANCE_PER_EM;
    let widest = rows.iter().map(|(_, width)| width / advance).fold(0.0_f32, f32::max);
    assert!(
        widest <= MAX_MEASURE_CHARS,
        "at a 1920 px window the page's prose is set {widest:.0} characters a line, over the {MAX_MEASURE_CHARS:.0} \
         this page allows: the column follows the window, so a bigger window makes it harder to read"
    );
}
