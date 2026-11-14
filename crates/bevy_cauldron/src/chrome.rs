//! **The editor's shared furniture** — one palette, one panel, one row shape.
//!
//! Every tab is the same two shapes: a controls panel down one side, and a list down the other. They
//! were written twice and were already drifting. The census key-row block appeared in `editor.rs` and
//! `tiles.rs` byte-for-byte, *including its five-line comment about `min_width`*, and rendered in two
//! different pairs of colours — the same list, a shade darker in one tab, which nobody chose. The
//! colour table itself was declared twice, with two of its entries carrying the same value under two
//! names (`TEXT_DIM`/`DIM`, `ROW_ARMED`/`ROW_SELECTED`), so a change to one tab's greys silently was
//! not a change to the other's.
//!
//! That is the failure `docs/ui.md` §3.5 records for key bindings, in a second place: a fact stated
//! more than once drifts. Ousterhout's *A Philosophy of Software Design* ch. 17 is the general form —
//! *"once you have learned how something is done in one place, you can use that knowledge to
//! immediately understand other places that use the same approach."* Here that is literal: a third tab
//! costs ~30 lines of panel code instead of ~110, and it cannot come out looking like a different
//! program.
//!
//! Nothing here knows what a map or a tile is. It knows what a panel looks like.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;

use crate::keys::{self};

// ── the elevation ladder ─────────────────────────────────────────────────────────────────────────
//
// **A ground is a step on a ladder, and the step is measured.** The 2026-09-03 audit's finding was
// that this palette separated *ink from ground* superbly and *ground from ground* not at all:
// `PANEL_BG` stood 1.03:1 against `VOID`, `ROW_BG` 1.08:1 against `PANEL_BG`, and hover 1.19:1
// against a row — with no border and no radius to carry the boundary instead. Reported at the
// keyboard as *"the layout overall is muddy as hell"*, which is exactly what a screen looks like
// when every surface edge is below the just-noticeable difference.
//
// **WCAG's contrast ratio is the wrong instrument down here** and using it is how the old ladder
// passed review: the `+0.05` flare term dominates at near-black, so two clearly different darks and
// two identical darks both score about 1.0. The right instrument for a large flat field is CIE
// **L\***, where a just-noticeable difference is about 2 and a comfortable one about 4. Measured on
// the shipped ladder, `VOID`→`PANEL_BG` was **ΔL\* 1.60** — below the JND. Every adjacent pair below
// is **ΔL\* ≥ 2.95**, and `the_ladder_is_a_ladder` fails the build if one closes up again.
//
// Contrast ratio is still the right instrument for *text*, and every ink keeps ≥ 4.5:1 against the
// grounds it actually renders on — `the_ink_clears_its_grounds` names the pairing per ink, because
// "every ink on every ground" is a rule this palette cannot satisfy and should not pretend to.
//
//    VOID        L*  5.4   the window behind everything
//    PANEL_BG    L* 10.0   a panel's own ground              ΔL* 4.58
//    HEADER_BG   L* 13.0   a group heading band              ΔL* 2.95
//    ROW_BG      L* 15.9   a row, a chip, a field at rest    ΔL* 5.84 over PANEL_BG
//    SLOT_BG     L* 18.1   an inspector slot, a thumbnail
//    OVERLAY_BG  L* 19.2   a modal card, above the scrim
//    ROW_HOVER   L* 21.1   under the pointer                 ΔL* 5.25 over ROW_BG
//    ROW_SELECTED L* 25.0  the one being acted on            ΔL* 3.89 over ROW_HOVER
//    PANEL_EDGE  L* 33.1   the hairline that ends a surface
//
// Two grounds run the other way on purpose: `ROW_PRESSED` and `FOCUS_BG` are *recessed*, below the
// surface they sit in, because pushed-in and typed-into are the two states where the metaphor is a
// well rather than a card.

/// **The editor's own ink, and the only place a colour is named** (UI-02, #210; follows the theme, #428).
///
/// This module used to declare 28 `pub const … : Color` — a table the 2026-08-17 audit had already
/// caught drifting against itself twice. They are gone: every colour the chrome paints is a field
/// here, and every field is read from **the running skin** ([`Ink::of`]) — the same data `bevy_gooey`
/// restyles the widgets from and the same data `GooeySkin::problems()` measures. One palette, one
/// skin, one check. Tool dark's is `src/editor_skin.yaml`; Parchment's and High contrast's are
/// `src/editor_theme_*.yaml` laid over it by [`crate::ui::skin_for`].
///
/// # Why a struct, and why [`ink`] is a function of the skin and not a parameter
///
/// The chrome is sixty-one spawn systems and helpers that hold no `Res<GooeySkin>`, and threading a
/// skin through all of them would be a second parameter on every call. So the ink is one live value
/// ([`ink`]), rebuilt by `crate::theme_ink::follow_the_skin` whenever the running skin changes colour,
/// and the same system moves everything already on screen onto the new one: **a theme changes every
/// colour the editor draws, live** (#428). A project's skin never reaches it (UI-03, #211 — a game's
/// palette belongs inside the Game UI preview panel and nowhere else): only the running skin does.
///
/// # The tone names are the file's keys
///
/// Twelve of these have no `GooeySkin` field — `key`, `label`, `muted`, `suggest`, `labeled`,
/// `grid_line`, `bounds_line`, `veil`, `focus_bg`, `row_pressed`, `snip_wash`, `overlay_bg` — and
/// live in the skin's `tones` map (bevy-coven/bevy_gooey#33). A tone the skin does not carry is
/// answered in magenta, this module's convention for a colour that cannot be derived
/// ([`scaled`], [`ink`]); for the editor's own skin it cannot happen, and
/// `tests/the_skin_carries_the_editors_other_colours.rs` holds all twelve in place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    /// A panel's ground — the skin's `surface`.
    pub panel_bg: Color,
    /// A row, a chip, a field at rest — the skin's `surface_raised`.
    pub row_bg: Color,
    /// A row under the pointer — the skin's `hover`.
    pub row_hover: Color,
    /// A row that is the one being acted on — the ladder's seventh step.
    pub row_selected: Color,
    /// A control with the pointer down on it — the skin's `pressed`, recessed below [`Ink::row_bg`].
    pub row_pressed: Color,
    /// Body text: a value, an id, the thing that changes.
    pub text: Color,
    /// Quieter body text — the skin's `text_muted`.
    pub dim: Color,
    /// **The one live-edit colour**: a value being changed right now, and nothing else.
    pub accent: Color,
    /// The key column, brighter than the description beside it.
    pub key: Color,
    /// A label column, quieter than its value.
    pub label: Color,
    /// Destructive, and only destructive.
    pub danger: Color,
    /// The scrim behind a modal — black at 55%, so the question is the only lit thing.
    pub scrim: Color,
    /// The veil the shortcut badges are read through — lighter than the scrim on purpose.
    pub veil: Color,
    /// A row that exists on purpose and is deliberately not participating.
    pub muted: Color,
    /// Machine-proposed, human-unconfirmed — a cool slate, neither accent nor danger.
    pub suggest: Color,
    /// [`Ink::suggest`] one state later, lifted toward white so it reads on a chosen row.
    pub unchecked: Color,
    /// A mesh that has been judged — the one state that means *ready*.
    pub labeled: Color,
    /// An inspector slot, an empty thumbnail — the ladder's fifth step.
    pub slot_bg: Color,
    /// A modal card's ground, above the scrim — the skin's `card`.
    pub overlay_bg: Color,
    /// A text box that owns the keyboard: recessed below [`Ink::panel_bg`].
    pub focus_bg: Color,
    /// A group heading band — the ladder's third step.
    pub header_bg: Color,
    /// The working grid: the map's cell lattice and the seating lattice.
    pub grid_line: Color,
    /// A stated extent, brighter than the grid inside it.
    pub bounds_line: Color,
    /// The void the panels float over — the clear colour both entry points install.
    pub void: Color,
    /// The window's own chrome: the bar above the door strip and the band at its foot.
    pub bar_bg: Color,
    /// The hairline that ends any surface.
    pub panel_edge: Color,
    /// A louder edge, for the surface that owns the keyboard.
    pub edge_strong: Color,
    /// The snip's translucent wash over its rectangle.
    pub snip_wash: Color,
}

/// **How far [`Ink::bar_bg`] sits above a slot's ground.** `Ink::bar_bg` was `scaled(SLOT_BG, 0.78)`; the
/// relationship is the point (chrome a step above a panel, a step below a slot), so it is kept as a
/// derivation rather than transcribed.
const BAR_STEP_BACK: f32 = 0.78;

/// **How far [`Ink::unchecked`] is lifted off [`Ink::suggest`]** — the value a model wrote, one state
/// after the proposal, so it still reads on a chosen row.
const UNCHECKED_LIFT: f32 = 1.45;

/// The ladder's step for a selected row: `background, surface, header, raised, slot, hover,
/// selected, edge`, lowest first (`src/editor_skin.yaml`).
const ROW_SELECTED_STEP: u8 = 6;
/// The ladder's step for an inspector slot.
const SLOT_STEP: u8 = 4;
/// The ladder's step for a group heading band.
const HEADER_STEP: u8 = 2;
/// The ladder's step for the hairline that ends a surface.
const EDGE_STEP: u8 = 7;

impl Ink {
    /// **The ink as a skin says it.** Every field is one of the skin's own roles or tones, except
    /// the two that are *derived* and say so: [`Ink::bar_bg`] from a slot's ground and
    /// [`Ink::unchecked`] from the suggest tone.
    pub fn of(skin: &bevy_gooey::GooeySkin) -> Ink {
        let tone = |name: &str| skin.tone(name).unwrap_or_else(|| {
            // **Magenta, loudly.** This module answers a colour it cannot derive in fuchsia rather
            // than with a silent grey ([`scaled`], [`ink`]); a skin missing one of the editor's tones
            // is exactly that case. The editor's own skin carries all twelve, held by
            // `tests/the_skin_carries_the_editors_other_colours.rs`.
            Color::srgb(1.0, 0.0, 1.0)
        });
        let slot_bg = skin.surface_at(SLOT_STEP);
        let suggest = tone("suggest");
        Ink {
            panel_bg: skin.surface,
            row_bg: skin.surface_raised,
            row_hover: skin.hovered(),
            row_selected: skin.surface_at(ROW_SELECTED_STEP),
            row_pressed: skin.pressed(),
            text: skin.text,
            dim: skin.text_muted,
            accent: skin.accent,
            key: tone("key"),
            label: tone("label"),
            danger: skin.danger,
            scrim: skin.scrim,
            veil: tone("veil"),
            muted: tone("muted"),
            suggest,
            unchecked: scaled(suggest, UNCHECKED_LIFT),
            labeled: tone("labeled"),
            slot_bg,
            overlay_bg: skin.card(),
            focus_bg: tone("focus_bg"),
            header_bg: skin.surface_at(HEADER_STEP),
            grid_line: tone(GRID_TONE),
            bounds_line: tone("bounds_line"),
            void: skin.background,
            bar_bg: scaled(slot_bg, BAR_STEP_BACK),
            panel_edge: skin.surface_at(EDGE_STEP),
            edge_strong: skin.outline,
            snip_wash: tone("snip_wash"),
        }
    }

    /// **The ink the editor starts with**, from `src/editor_skin.yaml` — the Tool dark theme, and the
    /// value [`ink`] holds until a skin says otherwise.
    pub fn editor() -> Ink {
        Ink::of(&crate::ui::editor_skin())
    }

    /// **Every field, in declaration order**, so a theme change can say "the colour that was `panel_bg`
    /// is now this theme's `panel_bg`" ([`Ink::remap`]) without naming 28 fields twice. The pattern has
    /// no `..`, so a field added to [`Ink`] fails to compile here until it is listed.
    pub fn fields(&self) -> [Color; INK_FIELDS] {
        let Ink {
            panel_bg, row_bg, row_hover, row_selected, row_pressed, text, dim, accent, key, label, danger, scrim, veil, muted, suggest,
            unchecked, labeled, slot_bg, overlay_bg, focus_bg, header_bg, grid_line, bounds_line, void, bar_bg, panel_edge, edge_strong,
            snip_wash,
        } = *self;
        [
            panel_bg, row_bg, row_hover, row_selected, row_pressed, text, dim, accent, key, label, danger, scrim, veil, muted, suggest,
            unchecked, labeled, slot_bg, overlay_bg, focus_bg, header_bg, grid_line, bounds_line, void, bar_bg, panel_edge, edge_strong,
            snip_wash,
        ]
    }

    /// **What a colour drawn from `from` becomes when the ink is `to`** (#428), or `None` when it was
    /// not drawn from `from` at all.
    ///
    /// A colour that *is* a field of `from` takes the same field of `to`, alpha and all. One that has the
    /// field's colour at another alpha (a fade, the shortcut key's step back) keeps its own alpha and
    /// takes the new field's colour. Anything else — a measured colour, a game's, a terminal program's, a
    /// fully transparent one — is not the editor's ink and is left alone, which is the point of asking.
    ///
    /// **It matches by value, so the caller chooses what to ask about.** A component whose default colour is
    /// white (an `ImageNode`'s tint) cannot be told from High contrast's white `text`, which is why
    /// `crate::theme_ink` does not ask about images: their tint belongs to the picture, not to the theme.
    pub fn remap(from: &Ink, to: &Ink, colour: Color) -> Option<Color> {
        let (from, to) = (from.fields(), to.fields());
        let same = |a: Color, b: Color| {
            let (a, b) = (Srgba::from(a), Srgba::from(b));
            [a.red - b.red, a.green - b.green, a.blue - b.blue].iter().all(|d| d.abs() <= INK_MATCH_TOLERANCE)
        };
        let alpha = Srgba::from(colour).alpha;
        // **Nothing is drawn at alpha 0**, so it is no ink's: `Color::NONE` has the RGB of every black field
        // (`scrim`, High contrast's `void`) and must stay `NONE` through a theme change.
        if alpha <= 0.0 {
            return None;
        }
        let exact = from.iter().position(|f| same(*f, colour) && (Srgba::from(*f).alpha - alpha).abs() <= INK_MATCH_TOLERANCE);
        if let Some(at) = exact {
            return Some(to[at]);
        }
        let at = from.iter().position(|f| same(*f, colour))?;
        Some(to[at].with_alpha(alpha))
    }
}

/// How many colours an [`Ink`] holds.
pub const INK_FIELDS: usize = 28;

/// **How near two sRGB channels must be to be the same ink**: a thousandth, well under one 8-bit step
/// (1/255), so a colour that went through a `Color` conversion still matches and two inks that differ
/// by a step do not.
const INK_MATCH_TOLERANCE: f32 = 1.0e-3;

/// **The ink the editor is drawing with right now** — the running skin's, by [`Ink::of`] (#428).
///
/// There is one, and `crate::theme_ink::follow_the_skin` is the only thing that writes it: whenever the
/// running [`bevy_gooey::GooeySkin`] changes colour (a theme chosen in Settings, a settings file edited
/// by hand) the ink is rebuilt from it and everything already drawn is moved onto it. Until a skin says
/// otherwise it is the editor's own, `src/editor_skin.yaml`.
///
/// It is a process-wide value rather than a resource because 300 call sites paint from [`ink`] where no
/// system parameter reaches — the same reason [`text_scale`] is one atomic. A process has one editor.
static LIVE_INK: std::sync::LazyLock<std::sync::RwLock<Ink>> = std::sync::LazyLock::new(|| std::sync::RwLock::new(Ink::editor()));

/// **The editor's ink now**, by value (28 colours, `Copy`): what a painter reads, whatever theme is
/// running. A value and not a reference, because it changes under a running editor.
pub fn ink() -> Ink {
    // A poisoned lock still holds a whole `Ink`: it is only ever replaced, never edited in place.
    *LIVE_INK.read().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Replaces the editor's ink and returns the one it replaced. Only `crate::theme_ink` calls it.
pub(crate) fn swap_ink(next: Ink) -> Ink {
    std::mem::replace(&mut *LIVE_INK.write().unwrap_or_else(std::sync::PoisonError::into_inner), next)
}

/// **How far a ground fill steps back while the shortcuts key is held.**
///
/// A panel's fill is the ground its rows stand on ([`Ground`]), and while `K` is down that ground is
/// also what a badge may stand on. At full opacity the badge and the panel are two opaque boxes of
/// the same colour meeting at a border; at this alpha the `Ink::veil` and the world read faintly
/// through the ground, so the badge is plainly the nearest thing and the panel is plainly behind it.
///
/// **Only the fill.** Rows, fields, chips and every word keep their own opacity — a badge points at
/// words, and dimming those in the instant they are being read is the opposite of the point.
pub const GROUND_HELD: f32 = 0.55;

/// **How far the world's own wireframe steps back while the shortcut key is held.**
///
/// Deeper than [`GROUND_HELD`], because a panel's fill is a flat wash a badge sits on and the
/// world's envelope is a *line* crossing the same ground a badge stands on. On the Tiles tab a
/// 1 x 4 x 1 m tile projects to three near-full-height verticals that run straight through the
/// badge stack, and at full strength they read as leaders — which is the one thing on this screen a
/// hairline is supposed to mean. Reported from the keyboard: *"the tiles tab is super muddy."*
///
/// It is a fade and not a hide: the envelope is what the author is building, and a box that
/// vanished when they reached for a key would be a different confusion. `badges::WorldOnScreen`
/// still keeps boxes off it either way — this is about reading, not about placement.
pub const WORLD_HELD: f32 = 0.25;

/// A world gizmo's colour, faded while the shortcut key is held. See [`WORLD_HELD`].
///
/// A free function rather than a resource because the drawers are ordinary `Update` systems with no
/// stated order against anything in `badges`, and a resource written in one phase and read in
/// another would be a frame stale on some runs and not others — the trap `sense_world_ink`'s own
/// header records paying for.
pub fn stepped_back(color: Color, held: bool) -> Color {
    if held {
        color.with_alpha(color.alpha() * WORLD_HELD)
    } else {
        color
    }
}

/// Edge of a row's preview box, logical px. One home since 2026-09-04: the map palette and the
/// kit door's MESHES shelf both draw a portrait in this slot, and two copies of the size would
/// be two row heights for one picture.
pub const THUMB_SLOT: f32 = 30.0;
// ── world ink ────────────────────────────────────────────────────────────────────────────────────
//
// Gizmo colours the tabs draw INTO the scene — the exception to "nothing here knows what a map is",
// carried because each is a fact two tabs state: a colour defined per-tab drifted into two names
// for one value twice before this block existed (the audit's `BOUNDS_LINE == CELLS` finding).

// (`BAR_EDGE` was here until 2026-09-03. It was `ROW_SELECTED × 0.62` — an edge derived from a row
// fill, which stopped making sense the moment every surface got an edge: a chrome bar's hairline and
// a panel's hairline are one fact, and this module's whole argument is that a fact stated twice
// drifts. Its three call sites take `PANEL_EDGE`.)

// ── edges and corners ────────────────────────────────────────────────────────────────────────────
//
// **The other half of the elevation model.** A fill step of ΔL* 4 is a comfortable separation and it
// is still only a change of shade; what makes a surface read as an *object* is that it has an edge
// and the edge turns a corner. Before 2026-09-03 this crate had four `BorderRadius` in 58,000 lines
// (two of them the compass) and no panel border at all, so a panel was a rectangle of very slightly
// different paint. Gestalt closure does the work a fill cannot: an outline groups its contents
// whatever their contrast, which is why a bordered panel survives a bright thumbnail landing in it
// and a fill-only panel does not.

/// **Which panel the keyboard is in**, as an edge rather than as a second selected row.
///
/// Asked for 2026-09-03: in the chooser nothing said which of the three columns the arrows were
/// walking, so `↑` was a guess until it moved something. A focused *row* already exists and cannot
/// answer this — every column has one, all the time. The panel's own edge can, and it costs no
/// layout: the border is already there at `Ink::panel_edge`, and focus lights it.
pub fn focus_edge(focused: bool) -> Color {
    if focused { ink().accent } else { ink().panel_edge }
}

/// One hairline. Not two: a 2 px border reads as a frame, and the thing being framed here is a work
/// surface rather than a picture.
pub const EDGE_W: f32 = 1.0;

/// **A surface's corner.** Small on purpose — enough that the eye closes the shape, not so much that
/// a 20 px row turns into a lozenge. Blender's editor rounds at 4–5 px on a panel and 3 on a widget;
/// this is the same relationship at this editor's density.
pub const RADIUS_PANEL: f32 = 4.0;
/// A row, a chip, a field, a button.
pub const RADIUS_ROW: f32 = 3.0;

/// **The lattice an author reads a map by, as the running skin colours it** (UI-80, #254).
///
/// The map's grid and the kit door's seating lattice are the editor's two grids, and both used to
/// read `chrome::ink().grid_line` whatever skin was running. This is the one expression they share:
/// the skin's own `grid_line` tone where it carries one, the shipped tone where it does not, so the
/// editor's look is unchanged by construction and a theme or a project that names `grid_line` moves
/// both lattices together. A test reads it, which is why it is a function and not a line inside a
/// `Gizmos` call — a headless app renders no gizmos, so nothing else can ask what colour they are.
pub fn grid_ink(skin: Option<&bevy_gooey::GooeySkin>) -> Color {
    skin.and_then(|skin| skin.tone(GRID_TONE)).unwrap_or_else(|| ink().grid_line)
}

/// **The skin tone a lattice's colour is kept under.** One name for the two places it is read — the
/// lookup in [`grid_ink`] and the `Ink::grid_line` field's own read of it in `Ink::of` — so a rename in
/// `src/editor_skin.yaml` cannot leave one of them asking for a tone that no longer exists.
pub const GRID_TONE: &str = "grid_line";

/// **The rail that says "this one"** — `Ink::accent` down the leading edge of the selected row.
///
/// Selection used to be carried by fill and by amber *text*, and both were problems: the fill had to
/// be loud enough to be unambiguous on its own, which pushed `Ink::row_selected` bright enough to fail
/// contrast for `Ink::label` on it, and the amber text was one of `Ink::accent`'s five jobs. A rail is a
/// shape, so it is unambiguous at any fill, and it re-uses the vocabulary [`severity_rail`] already
/// established in this same interface — a tinted left edge means *this line is special*.
pub const SELECT_RAIL_W: f32 = 2.0;

/// The snip's drag-rectangle border. It is heavier than the rail's 2 px on purpose: at 2 px a drag
/// measured about 1% of a 600 px-wide capture and was hard to find, so the box gets 3 px.
pub const SNIP_EDGE_W: f32 = 3.0;
/// **A wash at `share` of a named colour's own strength**: `Ink::snip_wash` at `0.5` is the quieter tint
/// a report draws over a pick it has already taken, so a person adding a second can tell which is the
/// decision already made and which is the one being made now.
///
/// Derived rather than declared, because a second colour constant beside the first is the duplicate
/// this table already had once (`the_colour_table_only_shrinks` counts them, and it only shrinks).
pub fn wash_at(colour: Color, share: f32) -> Color {
    match colour {
        Color::Srgba(srgba) => Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha * share),
        other => other,
    }
}

/// **A surface-pixel coordinate as the overlay's `Val::Px`.**
///
/// The layout multiplies `Val::Px` by [`UiScale`](bevy::ui::UiScale), so an overlay drawing a
/// rectangle it was handed in surface (physical) pixels divides by it, as `compose.rs` places its own
/// rectangles. Two things draw such a rectangle — the snip's drag box and a feedback report's
/// outline — and they had a copy each before this was one function.
pub fn overlay_px(surface: f32, ui_scale: f32) -> f32 {
    let s = if ui_scale > 0.0 { ui_scale } else { 1.0 };
    surface / s
}

/// A chrome colour scaled toward black, for a quieter sibling of a named colour — so the derived
/// value cannot drift from its parent the way `compose`'s hand-halved ACCENT could have.
pub const fn scaled(colour: Color, k: f32) -> Color {
    match colour {
        Color::Srgba(c) => Color::srgb(c.red * k, c.green * k, c.blue * k),
        // Every chrome colour is authored as sRGB; anything else reaching here is a caller error,
        // answered loudly in magenta rather than with a silent guess.
        _ => Color::srgb(1.0, 0.0, 1.0),
    }
}

/// A chrome colour as raster bytes, for the CPU-drawn plots — one palette, two encodings, no
/// hand-transcribed mirror (the audit found three byte copies of chrome colours in `anim_plots`).
///
/// Named `ink_bytes` and not `ink`: [`ink`] is the palette itself, and a second `ink` that took a
/// colour as an argument was the one collision this rename cost.
pub const fn ink_bytes(colour: Color) -> [u8; 4] {
    match colour {
        Color::Srgba(c) => [
            (c.red * 255.0 + 0.5) as u8,
            (c.green * 255.0 + 0.5) as u8,
            (c.blue * 255.0 + 0.5) as u8,
            255,
        ],
        _ => [255, 0, 255, 255],
    }
}

// ── layout ───────────────────────────────────────────────────────────────────────────────────────

/// **The editor's interface scale**, and the one `main` starts the application at.
///
/// Named here rather than written as a number in two places: `main` starts at it and
/// `fit_surface_to_window` scales from it. The menu screen that once ran at its own scale (its
/// offscreen capture) went with the chooser (#296).
///
/// **Raised on 2026-09-03, then made a base rather than an answer.** `fit_surface_to_window`
/// multiplies this by the display's scale factor, which reports *density* and not *size*: on a
/// 3396 px window reporting a factor of 1, body text rendered at 13 physical pixels and the two
/// Meshes panels together used under 14 % of the width (the audit's F2).
///
/// A larger constant was the first fix and it was wrong in the other direction — it is the density
/// a **small** window also gets, and at 1280 x 800 two 380 px docks scaled by 1.45 leave almost no
/// stage, which the badge packer reported by covering its own legend. So the growth belongs where
/// the window's size is known: this is the density the design is drawn at, and
/// [`crate::surface::ui_scale_for`] is what a bigger window does with it.
pub const EDITOR_UI_SCALE: f32 = 1.2;

pub const TAB_STRIP_BOTTOM: f32 = 46.0;
/// The gap every panel keeps from the window edge.
pub const MARGIN: f32 = 12.0;
/// Inside a panel.
pub const PAD: f32 = 12.0;

/// **Width of a controls panel — one number for every left dock.**
///
/// It was two: `CONTROLS_W` 300 on the Map door and `TILES_CONTROLS_W` 380 on the other three, so
/// the viewport jumped 80 px when the author changed door, and three of the four callers used a
/// constant named for one tab. The wider value wins because the reason it exists is still true —
/// below the key list the Tiles inspector carries four rows of tag chips and a findings paragraph,
/// neither of which is a two-column table, and at 300 the chips wrapped to twice the height and
/// pushed the findings off the bottom of the screen. A panel that is 80 px wider than it needs costs
/// nothing; a viewport that changes size when you change tab costs the author their place.
pub const CONTROLS_W: f32 = 380.0;
/// Width of a list panel. Narrower than the controls: a row here is a thumbnail, an id and a number
/// rather than two text columns. Both panels together leave a little under half the screen for the
/// map at `UiScale(1.2)`, which is the thing the right edge was chosen to protect.
pub const LIST_W: f32 = 264.0;

/// **The narrowest a panel may be squeezed to before the viewport gets the rest** — B-014.
///
/// The two widths above are *preferred* widths, and a panel took its own whatever the window could
/// afford, because [`panel_root`] set `width: Val::Px(width)` and nothing could give it back. At a
/// 900 logical px window that left the map **69 px** — measured from the real layout in
/// `the_viewport_keeps_a_usable_width_in_a_narrow_window`, and the same number the item filed
/// ("about 70 logical px at 1800 x 1406 physical").
///
/// Two things make the overhead bigger than the constants suggest. `EDITOR_UI_SCALE` multiplies
/// every `Val::Px`, so `CONTROLS_W` 380 is 456 on screen at 1.2 before the panel's own `MARGIN` and
/// border; measured, the left dock was **485** and the right **346**. And a dock is as wide as its
/// widest *visible* panel, so it cannot answer for the sum.
///
/// The floor is not zero: a panel narrowed past this stops being a panel, its rows wrap and its tag
/// chips stack into a column of single words. So panels shrink **down to** this and no further, and
/// the viewport is guaranteed the remainder — see [`fit_docks_to_the_window`].
pub const DOCK_MIN_W: f32 = LIST_W * 0.6;

/// **The width the viewport is owed before either dock gets more than its floor** — B-014.
///
/// The docks were never *overflowing* a 900 px window; measured, they took 835 of it and the map got
/// the 65 that was left. So the defect is not "the panels do not fit", it is **"nothing reserved the
/// viewport"**: the map was the leftover of two fixed panels, and at a narrow window that leftover is
/// a sliver. Below this width the docks give ground down to [`DOCK_MIN_W`] each, because a map
/// narrower than two palette rows is not a map you can place a piece on.
///
/// It is a floor with a reason rather than a round number: `LIST_W` plus room for a piece of the
/// largest tile class and the ghost's footprint beside it, which is what the author needs to see to
/// judge where a piece will land.
///
/// **It is a reserve, not a guarantee, and the distinction is honest.** A window can be too narrow
/// for this *and* two readable docks at once — at 900 logical px, `1.2` scaling makes the pair owe
/// `(158 + 24) * 1.2 * 2 = 438` even at their floors, and 438 + 504 of viewport is more than the
/// window has. When that happens [`DOCK_MIN_W`] wins and the viewport takes the remainder: a panel
/// squeezed past its floor is a column of single words with nothing to read, while a viewport
/// smaller than this is merely cramped and still shows the map. Measured at a 900 px window, the map
/// went from **69 px to 379**: the sliver is gone, and the panels are still panels.
pub const VIEWPORT_MIN_W: f32 = 420.0;

/// **The share of the window the viewport is owed once the window is wide enough for that to be
/// more than [`VIEWPORT_MIN_W`]** (#282). The frame's own test says the viewport "must hold a usable
/// share of the window", and the shell's `SplitFraction(0.22)` used to give it 56% of a 1280 px
/// window by letting the left panel overflow its pane; a panel that fits its pane cannot, so the
/// share is asked of the docks instead.
pub const VIEWPORT_SHARE: f32 = 0.52;

/// **What the shell's divider costs**, in the `Val::Px` units the panel widths are written in:
/// gooey's divider is 29 px on screen at the editor's 1.2 scale, measured on big, 2026-11-03.
pub const DIVIDER_W: f32 = 24.0;

// ── the widget layer ─────────────────────────────────────────────────────────────────────────────

/// **`bevy_feathers`' machinery, with this editor's palette in it.**
///
/// Separate from [`ChromePlugin`] because `add_plugins` tuples cap at **15**
/// (`bevy_app-0.19.0/src/plugin.rs:186`) and `harness::add_editor_plugins` is near it — a plugin
/// that nests its own group keeps the shared list one list.
///
/// **`UiTheme` is inserted, never defaulted.** `UiTheme::default()` is empty and every token miss
/// renders fuchsia with a warning; a theme nobody seeded is a magenta editor, and no test would see
/// it because nothing here reads a colour back. [`theme`] is the seed and
/// `the_theme_is_seeded_from_the_palette` is what notices if it stops being.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::feathers::FeathersPlugins)
            .insert_resource(bevy::feathers::theme::UiTheme(theme()));
    }
}

// ── theme ────────────────────────────────────────────────────────────────────────────────────────

/// **This editor's palette, expressed as a `bevy_feathers` theme.**
///
/// # Machinery, not greys — and that reconciliation is written down here because nowhere else has it
///
/// `docs/ui.md` §5 says of `bevy_feathers`: *"its visuals are Bevy's editor skin — do not adopt
/// them"*, and *"Do not add a UI framework crate"*. Both stand. What is adopted is the **mechanism**
/// — a token table, a themed scrollbar, and focus outlines that would otherwise be hand-written
/// badly — and every colour this editor states comes from the constants above.
///
/// **What is NOT true, and was written here as if it were:** Feathers' greys are not overwritten
/// wholesale. `create_dark_theme()` populates 137 tokens and the table below replaces 14, so 123 of
/// Bevy's editor greys are live in the resource. Nothing in this crate spawns a Feathers-themed
/// widget today, so none of them reaches the screen — but that is a fact about the call sites, not
/// about this function, and the first themed widget anybody adds will render in somebody else's
/// skin. Seeding from `ThemeProps::default()` instead is what would make the paragraph above true;
/// it is a decision about what a missing token should look like, not a typo, so it is named here
/// rather than taken.
///
/// The editor already deviates from that section once, deliberately and on the record (the full font
/// face, where the game keeps Bevy's 95-codepoint default). This is the second, and the same shape:
/// take the machinery, keep the house's own decisions.
///
/// # Why this is not a fifth dialect
///
/// The 2026-08-17 audit's verdict is that four tabs were drifting into four dialects, and its remedy
/// is *one name per fact*. A token table that **restated** a colour — `srgb(0.098, 0.092, 0.082)`
/// beside `Ink::row_bg` — would be exactly the leak it measured, one indirection later. So every entry
/// below is a reference to a const, never a literal, and `tests/chrome_census.rs` fails on a literal
/// in this file's theme.
///
/// `UiTheme::color` renders **fuchsia** and warns once for a token it has no entry for — the house
/// convention, which [`scaled`] and [`ink`] also keep (*"loudly in magenta rather than with a silent
/// guess"*). **That alarm cannot currently fire here**, and the difference matters: seeding from
/// `create_dark_theme()` gives every token an entry, so `color`'s `None` arm is unreachable and a
/// token this table misses answers with Bevy's grey rather than shouting. The safety property this
/// paragraph used to claim is the one thing the seeding gives up.
pub fn theme() -> bevy::feathers::theme::ThemeProps {
    use bevy::feathers::theme::{SemanticToken, ThemeToken};
    use bevy::feathers::tokens;
    let mut props = bevy::feathers::dark_theme::create_dark_theme();
    // **Bevy 0.20 split a token from its colour.** A design token now maps to a `SemanticToken`
    // (`token_assignments`), and the semantic token maps to a colour, per surface level
    // (`semantic_base`, `semantic_overrides`). This editor has no surface levels, so each token it
    // restates gets a semantic token of its own — one colour, whatever the context — which is the
    // port the 0.20 migration guide (`contextual_theming.md`) names as the quick one.
    let mut assign = |token: ThemeToken, color: Color| {
        let semantic = SemanticToken::new(format!("bevy_cauldron.{token}").into());
        props.token_assignments.insert(token, semantic.clone());
        props.semantic_base.insert(semantic, color);
    };
    assign(tokens::WINDOW_BG, ink().void);
    assign(tokens::PANE_BODY_BG, ink().panel_bg);
    assign(tokens::PANE_HEADER_BG, ink().header_bg);
    assign(tokens::SUBPANE_BODY_BG, ink().slot_bg);
    assign(tokens::SUBPANE_HEADER_BG, ink().header_bg);
    assign(tokens::TEXT_MAIN, ink().text);
    assign(tokens::TEXT_DIM, ink().dim);
    // The focus ring is the one thing Feathers gives that this editor has never had. `ACCENT` at
    // half alpha, because a ring is a *state* and the audit's rule is that persistent signals sit
    // at medium contrast with the loud end held in reserve.
    assign(tokens::FOCUS_RING, ink().accent.with_alpha(0.5));
    assign(tokens::SCROLLBAR_BG, ink().header_bg);
    assign(tokens::SCROLLBAR_THUMB, scaled(ink().row_selected, 0.8));
    assign(tokens::SCROLLBAR_THUMB_HOVER, ink().row_selected);
    assign(tokens::LISTROW_BG, ink().row_bg);
    assign(tokens::LISTROW_BG_HOVER, ink().row_hover);
    assign(tokens::LISTROW_BG_SELECTED, ink().row_selected);
    props
}

// ── type ─────────────────────────────────────────────────────────────────────────────────────────

/// **The type scale, as roles rather than numbers.**
///
/// The palette got named constants and the spacing got a scale; **size never got a name**, and the
/// 2026-08-17 audit measured what that cost: six sizes across a hundred call sites with no rule
/// between them. Section headings were 9 (`chrome::section`), 10 (the editor's and anim's
/// hand-rolled ones) or 11 (Compose's three) — *three dialects for one role in one editor*. Compose
/// showed two "COMPOSITIONS" headings, the real `section` plus an 11 px twin. Label/value pairs were
/// 10/11, 10/10 or flat 11 depending on the tab. And the anim bench rendered its central pairing
/// **inverted** — declared at 10 over measured at 9 — so the number the author is checking was the
/// smaller of the two.
///
/// A role is not the same thing as a distinct size, and two roles sharing a value is not a defect:
/// [`HEADING`] and [`text::LABEL`] are both 10 because both are quiet supporting text, and what separates
/// a heading from a label is that it is uppercase, in a different ink, and preceded by
/// [`GAP_GROUP`]. `section`'s own doc says so — *"quiet, and separated from what came before it…
/// the separation is the work"* — which is why the fix for three heading dialects is the smallest
/// of them and not the largest. A scale where headings out-shouted values would be a different
/// editor, and nobody asked for that one.
///
/// **What actually moved**, so a capture diff can be read rather than guessed at: `section` 9 → 10,
/// Compose's three hand-made headings 11 → 10, the anim pair un-inverted, and the name box's 18 →
/// [`TITLE`]. Everything else keeps the size it had and gains a name.
pub mod text {
    /// **A type role, and the reason it is not an `f32`.**
    ///
    /// The 2026-09-03 audit found the size ratchet had a laundering hole: `chrome_census.rs` scans
    /// for `from_font_size(<digit>`, and `chip`/`text_field` took `px: f32` and called
    /// `from_font_size` *inside* `chrome.rs`, which the scan skips. Twenty call sites were passing
    /// bare `9.0` / `10.0` / `11.0` through a function argument, in a crate whose own test claims a
    /// size is *"a role, never a number"*.
    ///
    /// A regex cannot close that; a type can. A builder takes a [`Role`], a `Role` is only ever one
    /// of the constants below, and a bare number at a call site is a compile error rather than a
    /// finding in the next audit.
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub struct Role(f32);

    impl Role {
        /// The role's size in logical pixels. For [`super::font`] and for measurement
        /// (`chars * BODY_CHAR_W`); **not** a way back to writing numbers at a call site.
        pub const fn px(self) -> f32 {
            self.0
        }
    }

    /// The panel's own name — `GRIMOIRE MAPPER`, `MESHES AND TILES`. One per panel.
    pub const TITLE: Role = Role(15.0);
    /// A tab's word in the strip, and the chooser's column headers.
    pub const TAB: Role = Role(13.0);
    /// **The readable default.** A row's value, a field's contents, the problem banner, a census
    /// row — anything the author is reading rather than orienting by. The chord on a badge is
    /// this size on purpose: it is the one thing a badge exists to have read.
    pub const BODY: Role = Role(11.0);
    /// A block or list heading. Quiet: uppercase, dim ink, and [`super::GAP_GROUP`] above it.
    pub const HEADING: Role = Role(10.0);
    /// The dim half of a label/value pair, and a log line.
    pub const LABEL: Role = Role(10.0);
    /// **A control's own word** — the text on a chip, a button, a field. Ten pixels, the same
    /// number as [`LABEL`] and [`HEADING`], and a seventh role anyway.
    ///
    /// `the_type_scale_stays_short` says a seventh role *"is a decision somebody should have to make
    /// on purpose"*, so here is the purpose. The 2026-09-03 decision was **shape carries kind,
    /// colour carries severity**: a toggle and a command stopped being the same box. The word on a
    /// control is therefore a role in its own right — it is the only text in this interface that
    /// names an *action* rather than describing a value — and the twenty call sites that used to
    /// pass a bare `10.0` into `chip(px: f32)` had nothing else honest to say.
    pub const CONTROL: Role = Role(10.0);
    /// Badge descriptions, footnotes, and the always-on hint — the smallest thing on screen, and
    /// never the thing being read. (Badge *chords* moved up to [`BODY`]; the description beside a
    /// chord stays down here, quieter on both axes.)
    pub const HINT: Role = Role(9.0);
}

/// **The only way this crate makes a `TextFont`.**
///
/// `TextFont::from_font_size` takes an `f32` and will therefore always accept a number; this takes a
/// [`text::Role`] and cannot. `the_type_scale_is_a_type` fails the build on a `from_font_size`
/// outside this module, which is the rule the old digit-matching regex was reaching for.
pub fn font(role: text::Role) -> TextFont {
    TextFont::from_font_size(role.px() * text_scale())
}

/// **The role a drawn label was made at, so a change of text size can redraw it** (UI-73, #251).
///
/// A `TextFont` is written once, when a widget is spawned. [`font`] reads the running
/// [`text_scale`] at that moment, so a label spawned after a person chose 175% is drawn at 175% —
/// and one that was **already on screen** keeps the size it was built at, which is exactly what the
/// review of #251 measured and what this component exists to fix.
///
/// The marker is the role, not the size: a role is a fact of the type scale and does not go stale,
/// and [`resize_the_labels`] can therefore recompute every label from `role.px() * scale` when the
/// person's multiple changes, without knowing what any of them says.
///
/// **Spawn through [`typed`]**, which puts this beside the `TextFont` in one bundle. A label that
/// calls [`font`] alone is drawn at the size it was made and never follows the setting.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct TypeRole(text::Role);

impl TypeRole {
    /// The role this label is drawn at.
    pub fn role(self) -> text::Role {
        self.0
    }
}

/// **A hand-drawn label's type: the `TextFont` it is drawn with *and* the role to redraw it at.**
///
/// The pair is one bundle because they are one fact — a label's size — stated twice on purpose: the
/// `TextFont` is what `bevy_text` lays out, and the [`TypeRole`] is what [`resize_the_labels`] reads
/// when the person's text size changes. Every `chrome::font` call site in this crate spawns through
/// this, so no label can carry the one without the other.
pub fn typed(role: text::Role) -> impl Bundle {
    (font(role), TypeRole(role))
}

/// **A label already on screen is redrawn when the text size changes** (UI-73, #251).
///
/// `chrome::font` reads [`text_scale`] when it builds a `TextFont`, so a label spawned after the
/// setting changed is already right; a label spawned before it is not, and a person who chose 175%
/// would have half an editor at 175% and the chrome they read most at 100%. This rewrites every
/// [`TypeRole`]'s `TextFont` from its role the frame the resolved setting differs from the one last
/// applied, and does nothing at all on a settled editor (one float comparison).
///
/// **It reads the resolved `EditorSettings`, not the atomic**: the atomic exists so a spawn closure
/// with no system parameters can read the scale, while this needs the value the settings file
/// resolved to and a place to remember what it last drew.
///
/// **A `TextSpan` child is not rewritten.** No `TextSpan` in this editor is spawned through
/// [`typed`]; a span carries its own `TextFont` and would need its own marker, and nothing asks for
/// one today.
pub fn resize_the_labels(
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut applied: Local<Option<f32>>,
    mut labels: Query<(&TypeRole, &mut TextFont)>,
    mut rooms: Query<(&LinesRoom, &mut Node), Without<TypeRole>>,
) {
    let Some(settings) = settings else { return };
    if applied.is_some_and(|last| (last - settings.text_scale).abs() < f32::EPSILON) {
        return;
    }
    *applied = Some(settings.text_scale);
    let scale = settings.text_scale;
    for (role, mut font) in &mut labels {
        let want = FontSize::Px(role.0.px() * scale);
        if font.font_size != want {
            font.font_size = want;
        }
    }
    // **The room the labels are drawn in follows them**, or the label that grew is clipped by the
    // box that did not. Same one comparison, same frame.
    for (room, mut node) in &mut rooms {
        let want = Val::Px(room.at(scale));
        if room.fixed {
            if node.height != want {
                node.height = want;
            }
        } else if node.min_height != want {
            node.min_height = want;
        }
    }
}

/// **The person's text size, as every `chrome::font` call sees it** (UI-73, #251).
///
/// `chrome::font` has 117 call sites, every one of them a label the editor draws by hand, and none of
/// them has a system parameter to spare: a role's size is a fact of the type scale, and the scale a
/// person chose is a fact about the editor. So the running scale is kept here, in one atomic, and
/// [`set_text_scale`] is the only writer — a system turns the setting into it each frame
/// ([`crate::ui::follow_the_text_size`]).
///
/// **`u32` thousandths rather than an `f32`**, because an `AtomicU32` is lock-free on every target
/// this editor ships and an `f32` atomic does not exist; 1.0 is [`TEXT_SCALE_ONE`], a text size is
/// never negative, and a thousandth of a step is finer than any type the scale can draw.
///
/// **A frame that has not set it reads 1.0**, so a label built before any settings were read — a
/// harness, a build step — is drawn at the size the design was made at, not at zero.
static TEXT_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(TEXT_SCALE_ONE);

/// 1.0 as [`TEXT_SCALE`] holds it: the shipped size, in thousandths.
const TEXT_SCALE_ONE: u32 = 1_000;

/// **How large the type is against the size the editor was designed at.** 1.0 is as designed; see
/// [`crate::settings::SCALE_RANGE`] for the ends a file may ask for.
pub fn text_scale() -> f32 {
    TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed) as f32 / TEXT_SCALE_ONE as f32
}

/// **Set the type's size** (UI-73, #251). The only writer, and the value is clamped to the range a
/// settings file may hold so a hand-edited file cannot draw a label of no height.
pub fn set_text_scale(scale: f32) {
    let clamped = scale.clamp(crate::settings::SCALE_RANGE.0, crate::settings::SCALE_RANGE.1);
    let thousandths = (clamped * TEXT_SCALE_ONE as f32).round() as u32;
    TEXT_SCALE.store(thousandths, std::sync::atomic::Ordering::Relaxed);
}

/// **A role's size times a person's scale** (#324): the terminal's text size setting multiplies the
/// role, so the size still comes from the type scale and a setting cannot name a bare number.
pub fn font_scaled(role: text::Role, scale: f32) -> TextFont {
    TextFont::from_font_size(role.px() * scale)
}

/// **One glyph's advance at [`text::BODY`], logical pixels.** The shipped face is
/// `FiraMono-Regular.ttf` — monospace — so `chars * BODY_CHAR_W` is exact, not an estimate, and a
/// string can be measured without a text-layout round trip. Stated once, beside the scale it
/// belongs to (and outside it — this is a metric of the face, not a type role, and the census that
/// keeps the scale short counts the module): it was measured independently in `badges` (chord
/// columns) and `compose` (centred slot labels), and two hand-copied 6.6s drift the day the face or
/// the size changes. If the face ever goes proportional, layouts drift; they do not break.
pub const BODY_CHAR_W: f32 = 6.6;

/// **One line of [`text::BODY`], logical pixels**: Bevy 0.20's default `LineHeight`, which is
/// `RelativeToFont(1.2)` (`bevy_text` `text.rs`, `impl Default for LineHeight`). For reserving room
/// for text that comes and goes, so its arrival does not move what sits below it. A metric, beside
/// [`BODY_CHAR_W`], for the same reason that one is not a type role.
pub const BODY_LINE_H: f32 = text::BODY.px() * 1.2;

/// **One line of [`text::BODY`] at the person's text size** — [`BODY_LINE_H`] times [`text_scale`].
///
/// A box that reserves room for text has to reserve it at the size the text is drawn, or a person
/// at 200% gets a row one line tall holding a two-line-tall label: the text is clipped, and
/// `every_label_fits_its_panel_at_two_hundred_percent_text` measures exactly that. Spawn sites use
/// this; a box that has to follow a *live* change carries [`LinesRoom`] and [`resize_the_labels`]
/// rewrites it.
pub fn body_line_h() -> f32 {
    BODY_LINE_H * text_scale()
}

/// **A box that reserves room for `lines` lines of [`text::BODY`]** (UI-73, #251).
///
/// [`body_line_h`] is read when the box is spawned; a person who changes the text size afterwards
/// would leave the box the height it was built at, and the label inside it — which *does* resize,
/// because it carries a [`TypeRole`] — would be clipped by its own room. So the reserve is a
/// component too, and [`resize_the_labels`] rewrites it from the role-like numbers it holds.
///
/// **The padding is a number of its own**, because spacing does not follow the text size: a row
/// inset is the interface's rhythm (`interface_scale`'s job), and scaling it with the type would
/// double-thicken a row a person only asked to read better.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LinesRoom {
    /// How many lines of [`text::BODY`] the box reserves.
    pub lines: f32,
    /// Vertical padding added to the reserve, in logical pixels — not scaled with the text.
    pub pad: f32,
    /// Fill `Node::height` (a clipped row that keeps one height) rather than `Node::min_height`
    /// (a box that may grow past its reserve).
    pub fixed: bool,
}

impl LinesRoom {
    /// The height this room is worth at `scale`.
    pub fn at(self, scale: f32) -> f32 {
        (self.lines * BODY_LINE_H * scale + self.pad).ceil()
    }
}

// ── spacing ──────────────────────────────────────────────────────────────────────────────────────
//
// **A scale, used as a set.** van den Berg, Cornelissen & Roerdink 2009 (`10.1167/9.4.24`) find that
// clutter is *crowding*, not element count: what makes a group read as a group is that its members
// sit closer to each other than to anything else. So what matters here is the RATIO between these,
// not their absolute values — a panel where every gap is 4 px has no groups, however few things are
// in it, and that is what the tiles detail block was.
//
// `docs/ui.md` §1.2 supplies the other half, and it cuts against reflexive minimalism: the test is
// Vicente & Rasmussen's *"does this force interpretation?"*, not "how many things are on screen",
// and Yang et al. 2017 measured more information **improving** performance. So the answer to a
// crowded panel is spacing and grouping, not deletion.

/// Between items that belong to each other — chips in one axis, cells in one row.
/// **One step of a gooey layout's gap**, as `bevy_gooey::row(1.0)` / `column(1.0)` ask for: a
/// multiple of the running skin's `gap` (5 px in `editor_skin.yaml`), so an interface-size change
/// moves every row's rhythm together (UI-58/#244, UI-59/#245).
pub const GOOEY_GAP_ONE: f32 = 1.0;

/// **The skin's `gap`**, from `editor_skin.yaml`: the pixel unit [`GOOEY_GAP_ONE`] is a multiple of,
/// and the divisor a hand-built pixel gap is turned into one (`GAP_TIGHT / SKIN_GAP_PX` is 0.6).
pub const SKIN_GAP_PX: f32 = 5.0;

pub const GAP_TIGHT: f32 = 3.0;
/// Between the rows of one block.
pub const GAP_ROW: f32 = 5.0;
/// Between blocks. Several times [`GAP_TIGHT`], which is the whole point.
pub const GAP_GROUP: f32 = 16.0;
/// [`GAP_TIGHT`] in steps of the skin's gap, which is what a gooey row's or column's `gap` takes (0.6).
pub const GAP_TIGHT_STEPS: f32 = GAP_TIGHT / SKIN_GAP_PX;
/// [`GAP_GROUP`] in steps of the skin's gap (3.2).
pub const GAP_GROUP_STEPS: f32 = GAP_GROUP / SKIN_GAP_PX;
/// [`GAP_ROW`] in steps of the skin's gap (1.0: the editor skin's gap is the row gap).
pub const GAP_ROW_STEPS: f32 = GAP_ROW / SKIN_GAP_PX;

/// **A chip's (and a compact list row's) inset.** The 6 × 3 pair was an undeclared constant at
/// twelve sites in five files before it had this name — the exact drift this module's header
/// describes. The vertical 3 is [`GAP_TIGHT`] on purpose: a chip's own padding and the gap to its
/// neighbour read as one rhythm. Do not shrink it — the padding is most of a chip's click target
/// (Fitts: selection time grows as the target narrows), and 1 px of vertical padding made a row of
/// chips "a solid bar of text rather than a row of things".
pub const CHIP_PAD: UiRect = UiRect::axes(Val::Px(6.0), Val::Px(GAP_TIGHT));
/// A text field's inner inset — tighter than [`CHIP_PAD`] because a field's box is already drawn
/// by its fill, and the caret needs the width more than the text needs air.
pub const FIELD_PAD: UiRect = UiRect::axes(Val::Px(4.0), Val::Px(2.0));
/// A text field's floor. An unstated height lays out at 7 px while empty — this fact was restated
/// at five sites as a bare `18.0` before it had a name.
pub const MIN_FIELD_H: f32 = 18.0;

/// **The box a number is typed into**, and the field's third dimension beside [`MIN_FIELD_H`] and
/// [`FIELD_PAD`].
///
/// One idea with two values before 2026-09-03 — 62 in `tiles.rs` for a size and a mount height, 56
/// in `editor.rs` for a map extent — which is a fact stated twice, one file apart. 62 wins on the
/// measurement rather than by being the larger: the widest thing typed into one of these is
/// `12.34` plus the caret, which does not fit in 56 at [`text::BODY`], while the Map's `48` fits
/// comfortably in 62.
pub const NUM_FIELD_W: f32 = 62.0;

/// **A command button's inset**, and deliberately larger than [`CHIP_PAD`].
///
/// This is half of *shape carries kind* (the 2026-09-03 decision): a **toggle** — a tag, an axis
/// value, a thing that is on or off — is a [`chip`], and a **command** — `rescan mesh`, `clear this
/// cell`, a thing that happens once — is a [`button`]. Before that they were the same grey box in
/// the same row, told apart only by an ink that also meant three other things, so the way to learn
/// which was which was to press one.
pub const BUTTON_PAD: UiRect = UiRect::axes(Val::Px(10.0), Val::Px(GAP_ROW));

/// **The label column of a `LABEL  value` row.**
///
/// It was six unnamed literals — 76, 62, 56, 48, 40 and 14 — one per author, which is why no two
/// panels in this editor lined their values up with each other. Three widths, chosen by what the
/// column has to hold rather than by which file it is in:
///
/// - [`COL_TIGHT`] a single glyph or an axis letter,
/// - [`COL_LABEL`] an ordinary word (`size`, `mount`, `pieces`),
/// - [`COL_WIDE`] a phrase (`new work lands here`, `not imported`).
///
/// A value that will not fit its column is a [`COL_WIDE`] row, not a seventh number — and since
/// 2026-09-03 a column is a **floor** rather than a cap, so a label that outgrows even `COL_WIDE`
/// pushes its own value right instead of drawing on top of it.
///
/// **The KIT INFO overlap (`docs/ui_audit.md` F5) lived here**, and measuring it is what showed a
/// wider constant could not fix it: `new work lands here` plus the inspector's selection mark is 21
/// characters, which at `text::LABEL` on this monospace face is 126 px of glyphs — over `COL_WIDE`
/// as well as over the 76 px column it actually had. See [`row_label`].
pub const COL_TIGHT: f32 = 16.0;
/// See [`COL_TIGHT`].
pub const COL_LABEL: f32 = 52.0;
/// See [`COL_TIGHT`].
pub const COL_WIDE: f32 = 96.0;
/// A block heading: quiet, and separated from what came before it.
///
/// The separation is the work — a heading with the same gap above it as below is a label that could
/// belong to either side.
pub fn section<'a>(parent: &'a mut ChildSpawnerCommands, label: &str) -> EntityCommands<'a> {
    parent.spawn((
        Text::new(label.to_owned()),
        TextColor(ink().label),
        typed(text::HEADING),
        spaced(bevy_gooey::Sides { top: Some(STEP_GROUP), bottom: Some(STEP_TIGHT), ..NO_MARGIN }),
    ))
}

/// Which edge a panel is pinned to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

/// **A panel.** A child of one of the frame's docks, opaque, and as tall as the frame lets it be.
///
/// It used to be `PositionType::Absolute` at `top: TAB_STRIP_BOTTOM`, pinned to a window edge and
/// floating over the world — which is why nothing on screen filled the window and why a panel's
/// height was a number rather than a consequence. [`Frame`] owns position now; this owns width,
/// which side, and whether the panel wants the dock's whole height.
///
/// `full_height` becomes `flex_grow` rather than a pinned `bottom`, and the reason to keep the
/// distinction is unchanged: a list inside needs a real height to scroll within, and a `max_height`
/// inside a content-sized panel is never reached and does nothing — the map palette shipped that way
/// and rendered two rows. **`min_height: 0` comes with it**, because a flex item's automatic minimum
/// size is its content, so without it the panel grows to fit every row and `overflow` has nothing
/// left to clip.
///
/// **Every panel carries `Hovered`.** `view::drive` and `place_on_click` both ask "is the pointer
/// over UI" by looking for any true `Hovered`, and when only the *rows* carried one the gaps between
/// them counted as open map — a wheel turn over a list zoomed the world, and a click that missed a
/// row by a pixel dropped a piece behind the panel. `Hovered` is true for an entity **or any
/// descendant** (`bevy_picking-0.19.0/src/hover.rs:322`), so one on the root answers for the whole
/// surface. The frame above it deliberately carries none — see [`Frame`].
/// **What width a panel would like, so the frame can answer how much it may have.**
///
/// The pair is the whole of B-014's fix: `preferred` is what the panel's own constant asks for and
/// stays the width on any window that can afford it, while [`fit_docks_to_the_window`] lowers the
/// *applied* width toward [`DOCK_MIN_W`] on a window that cannot. Stored as data rather than
/// recomputed from `Node` so the system never has to guess which of `Node`'s widths was the wish.
#[derive(Component, Debug, Clone, Copy)]
pub struct PanelWidth {
    pub preferred: f32,
    /// Which dock it lives in. Carried rather than inferred from the node's margin: the margin is a
    /// layout detail that could be anything, and the question this answers is which side of the row.
    pub side: Side,
}

pub fn panel_root<'a>(
    commands: &'a mut Commands,
    frame: &Frame,
    side: Side,
    width: f32,
    full_height: bool,
    hidden: bool,
    // What a screen reader calls this panel. Every spawner has one — it is the heading the panel
    // draws — so this is never a second copy of a name (see `headless::the_tab_strip_describes_itself`).
    label: &str,
) -> EntityCommands<'a> {
    let mut node = Node {
        width: Val::Px(width),
        flex_direction: FlexDirection::Column,
        // `GAP_ROW`, not the bare `6.0` that stood here until 2026-09-03 — the one gap every panel in
        // the editor uses was the one gap that was not on the scale.
        row_gap: Val::Px(GAP_ROW),
        padding: UiRect::all(Val::Px(PAD)),
        margin: UiRect::all(Val::Px(MARGIN)),
        border: UiRect::all(Val::Px(EDGE_W)),
        border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
        ..default()
    };
    if full_height {
        node.flex_grow = 1.0;
        node.min_height = Val::Px(0.0);
    }
    if hidden {
        // `Display::None`, never `Visibility`: a visibility-hidden UI node still occupies layout and
        // still answers hover, which would leave a hidden tab's rows eating clicks aimed at the
        // world — and in a dock it would also hold the dock open at its width.
        node.display = Display::None;
    }
    let dock = match side {
        Side::Left => frame.left,
        Side::Right => frame.right,
    };
    // **The panel's fill is the skin's, repainted by `dim_the_ground`** (UI-02, #210 slice 4b).
    // `panel_root` is called from six spawn systems that hold no `Res<GooeySkin>`, so reading the skin
    // here would mean threading it through all six and would still leave every panel built *before* a
    // theme edit on the old colour. The marker says "this ground is a panel's"; the system that already
    // walks every ground is what makes it the skin's, once per frame, comparing before it writes.
    let panel = commands
        .spawn((
            // **A raw node, and why** (GAP7): the panel's width is written by three systems (`fit_docks_to_the_window` and the left pane's two), which a skin-written `Sizing` width
            // would fight; the gooey form broke the snip and library layout tests at a doubled scale (a split clamp panicked), found by the whole run.
            node,
            PanelWidth { preferred: width, side },
            // **The panel's name, for something that cannot see it** (UI-30, #224). A `Role::Panel` is a
            // box in the accessibility tree with no name on it, so a reader meets the editor as a set of
            // unlabelled regions. `label` is what the panel calls itself on screen, passed in by its
            // spawner; `AccessibleLabel` is what a reader reads out.
            bevy::prelude::AccessibleLabel::new(label),
            BackgroundColor(ink().panel_bg),
            Ground(ink().panel_bg),
            PanelGround,
            // The edge and the corner are what make this read as a surface rather than as a
            // rectangle of slightly different paint — see the elevation-ladder header.
            BorderColor::all(ink().panel_edge),
            // Which panel the keyboard is in is drawn on this border; nothing lights it until a
            // `Focused` lands, so a panel nobody has focused keeps `PANEL_EDGE`.
            PanelEdge,
            Hovered::default(),
        ))
        .id();
    commands.entity(dock).add_child(panel);
    commands.entity(panel)
}

/// **The window's own layout, and the one thing on screen that belongs to no panel.**
///
/// # Why there is a frame at all
///
/// There were two layouts and neither filled the window. The editor drew `PositionType::Absolute`
/// panels of fixed pixel width, anchored to the left and right edges and floating over a 3-D camera
/// that owned the whole window; the menu was a fixed-pixel flex grid whose own code said it *"is
/// fixed-size and simply sits in whatever window there is"*. On a 2560x1406 window that left about
/// two fifths of the screen as ground nothing could use, panels sized for twenty rows holding two,
/// and rows wrapping into their own value column — reported as *"it's bad"*, and visible in the
/// capture that started this.
///
/// So position stops being something a panel decides. A panel says how wide it wants to be and which
/// side it lives on; **where it goes is the frame's answer**, and the frame is the window.
///
/// ```text
/// +--------------------------------------------------+
/// |  < kits & maps                 KIT . furniture   |  chrome bar
/// +--------------------------------------------------+
/// |  KIT | MAP | RIGS      1 MESHES 2 TILES 3 COMPOSE |  door strip
/// +----------+---------------------------+-----------+
/// | left     |                           | right     |
/// | dock     |        viewport           | dock      |  body: flex_grow 1
/// |          |                           |           |
/// +----------+---------------------------+-----------+
/// |  stamped 4 pieces        Hold K . Cmd+O back      |  status band
/// +--------------------------------------------------+
/// ```
///
/// # Flex, deliberately, and not `Val::Percent`
///
/// [`EDITOR_UI_SCALE`] multiplies every `Val::Px` and every font size and leaves `Val::Percent`
/// alone (`docs/ui.md` §5). A layout mixing percentage widths with pixel minimums would therefore
/// scale two ways at once on a 2x display, which is a bug that only appears on somebody else's
/// monitor. `flex_grow` on the centre and pixel widths on the docks is responsive *and*
/// density-correct, because the centre is defined as whatever is left.
///
/// # What the frame must not carry
///
/// **No `Hovered`, and `Pickable::IGNORE` throughout: on the frame and on every node inside it.**
/// `view::over_ui` and `view::drive` both ask "is the pointer on the interface" by looking for a
/// true `Hovered`, and `Hovered` is true for an entity
/// *or any descendant*. So one pickable child inside a non-hoverable row (a label, a value, a
/// glyph) turns that row's `Hovered` on, and the frame's with it, and the answer is yes for the
/// entire window: the map would stop taking clicks and the wheel would stop zooming, everywhere,
/// with nothing to point at. `Hovered` belongs on the panels, which is where [`panel_root`] puts
/// it, and on the one node a row exposes as a button (`Pickable::default()` with `Hovered`).
#[derive(Resource)]
pub struct Frame {
    pub root: Entity,
    /// Above everything, and the same on every door — see `docs/2026-08-17-one-application.md` §4.1.
    pub chrome_bar: Entity,
    /// The door's own strip of panels.
    pub door_strip: Entity,
    /// Everything between the two bars. The editor fills its three slots; the **menu** puts its own
    /// columns straight in here, because a menu has no docks and no viewport — the frame is the
    /// window's shape, not the editor's.
    pub body: Entity,
    pub left: Entity,
    /// The hole the world is drawn through. Carries [`ViewportSlot`].
    pub viewport: Entity,
    pub right: Entity,
    /// **The bottom dock** (UI-61, #247): a gooey `Dock` across the foot of the workbench, under the
    /// body and above the status band. It holds the panels that say what became of the work —
    /// `docs/ui/vision.md`'s "bottom dock: history · problems · notes · log", of which the reload
    /// panel is the first to move here.
    pub bottom: Entity,
    /// One band at the foot of the window: what happened, what went wrong, what the keys are.
    pub status: Entity,
}

/// **The node the world shows through, and the only one with no background.**
///
/// `crate::surface` reads its rect and gives it to the map camera as a viewport, which is what makes
/// the viewport a region rather than the whole window with panels floating over it. Reflected so a
/// BRP caller (`tools/ux/first_run_walk.py`) can find the map on screen and click it.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ViewportSlot;

#[derive(Component)]
pub struct ShellSplit;

/// **The shell's split is still sized by its left panel, not by a person** — see
/// [`size_the_shell_split`]. Present from spawn until the first time the fraction is written by anyone else.
#[derive(Component, Default)]
struct SplitUnsized {
    /// The fraction this system last wrote, so a different one is known to be somebody else's.
    wrote: Option<f32>,
}

/// Everything that spawns into a dock runs after this.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameSystems;

/// **The census's name for this control** — what makes a key badge land on the thing it acts through.
///
/// Attached by the panel that spawns the node, and joined by id in `crate::badges`. Symbolic on
/// purpose: `keys.rs` declares a [`keys::ControlId`] for a verb's home and never learns what a
/// palette *is*, and a badge system querying fourteen domain markers (`PaletteRow`, `TagChip`,
/// `CellButton`, …) would be the second census `keys.rs` exists to delete.
///
/// **One per node, and one visible node per id.** `crate::badges::anchor` resolves an id to the
/// unique node carrying it with a non-zero [`bevy::ui::ComputedNode`] — which is also the visibility
/// test, since [`panel_root`]'s hidden form is `Display::None` and lays out to nothing. Two visible
/// at once is a bug rather than a tie to break, and
/// `every_control_the_census_homes_a_verb_at_is_on_screen` is what says so.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Control(pub keys::ControlId);

/// **This node's fill is ground, not ink — and this is the colour it rests at.**
///
/// A panel's `Ink::panel_bg` and a band's `Ink::bar_bg` are what their rows and words *stand on*. They
/// carry no information of their own, so a badge parked on bare ground covers nothing a reader
/// needs — which is the whole difference between an overlay that has somewhere to go and one that
/// hugs a dock edge and lands on the map. `badges::ink` reads this to subtract a container from the
/// occupancy census while keeping every child in it.
///
/// **The colour is carried rather than recomputed**, the way `badges::BadgeRest` is: [`dim_the_ground`]
/// restores exactly what was spawned, so it is not a second place that decides what a panel is
/// coloured, and a panel that changes its fill cannot drift out of sync with the system that dims it.
#[derive(Component)]
pub struct Ground(pub Color);

/// **This ground is a panel's, so its colour is the running skin's `surface`** (UI-02, #210 slice 4b).
///
/// `panel_root` is called from six spawn systems that hold no `Res<GooeySkin>`, so the spawn cannot
/// read the skin without threading a parameter through all of them. [`dim_the_ground`] can: it already
/// runs every frame over every [`Ground`], already compares before writing, and is therefore the one
/// place where "what a panel is coloured" can be one expression rather than six.
///
/// The marker is what tells a *panel's* ground (the skin's `surface`) from a *band's* (`Ink::bar_bg`,
/// which gooey has no role for). Without it the repaint would repaint the status bar as a panel.
#[derive(Component)]
pub struct PanelGround;

/// **This ground is a band's** — the chrome bar and the status bar. Painted from
/// [`PanelFills::band`], which is the skin's `"bar_bg"` tone, through the same system that paints a
/// panel's, so a project's skin restyles both and the editor keeps one place that decides a ground.
#[derive(Component)]
pub struct BandGround;

/// **The fills a panel and a band take from the running skin** (UI-02, #210, slice 4b).
///
/// `panel_root` used to spawn `Ink::panel_bg` and `Ink::bar_bg` — constants — so every panel in the editor was
/// painted once, at spawn, and never asked the skin again. A project's `editor_skin.yaml`, or a
/// `family/bevy_gooey.yaml` edit at run time, therefore reached the list rows and the migrated ink but
/// **not the largest surface on screen**.
///
/// One function, called by the spawn and by [`dim_the_ground`] alike, the way [`RowFills::of`] is
/// called by [`style_list_rows`] and its test. An assertion here is an assertion about what a panel
/// paints, and it needs no window and no pointer.
///
/// **The fallback is the shipped table**, never a second palette: a headless harness that never ran
/// `EditorUiPlugin` has no `GooeySkin`, and it must get exactly the colours the editor has always had.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelFills {
    /// A panel's ground — [`GooeySkin::surface`](bevy_gooey::GooeySkin::surface).
    pub panel: Color,
    /// A band's ground — the status bar and the chrome bar. A skin that carries a `"bar_bg"` tone paints
    /// the bands with it; one that does not (including the editor's own, whose exact tone set is pinned
    /// by `the_skin_carries_the_editors_other_colours.rs`) falls back to `Ink::bar_bg`, which is what the
    /// editor has always painted. So a *project's* skin can restyle the bands today without a protected
    /// assertion moving, and the editor's own look is unchanged by construction.
    pub band: Color,
}

impl PanelFills {
    /// From the running skin when there is one, the constants when there is not.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> PanelFills {
        match skin {
            Some(s) => PanelFills {
                panel: s.surface,
                band: s.tone("bar_bg").unwrap_or(ink().bar_bg),
            },
            None => PanelFills { panel: ink().panel_bg, band: ink().bar_bg },
        }
    }
}

/// [`PanelFills`] for the world as it stands.
pub fn panel_fills(world: &World) -> PanelFills {
    PanelFills::of(world.get_resource::<bevy_gooey::GooeySkin>())
}

/// **A surface whose border says whether the keyboard is in it.** Carried by every [`panel_root`].
#[derive(Component)]
pub struct PanelEdge;

/// **Put this on a [`PanelEdge`] to light its border.** The owner of the focus decides — the chooser
/// moves it with its column, an editor door with its `Context` — because *where the keyboard is* is
/// a fact those modules already hold and a second definition of it here would be a second answer.
#[derive(Component)]
pub struct Focused;

/// Paint [`PanelEdge`] from [`Focused`]. Compares before writing — `BorderColor` is change-detected
/// and there are thirteen panels, so an unconditional write would dirty the whole interface sixty
/// times a second for a border that moves twice a session (`tests/no_system_writes_every_frame.rs`).
fn light_the_focused_panel(mut panels: Query<(&mut BorderColor, Has<Focused>), With<PanelEdge>>) {
    for (mut border, focused) in &mut panels {
        let want = focus_edge(focused);
        if border.top != want {
            *border = BorderColor::all(want);
        }
    }
}

/// **How wide each panel should be, given the window** — B-014.
///
/// The frame's contract is that *a panel says how wide it wants to be, and the frame answers how much
/// room there is*. The answer was missing: every panel was `width: Val::Px(preferred)` and nothing
/// could take it back, so a 900 px window gave the two docks 485 + 346 and the map **69 px**.
///
/// **The arithmetic is over every visible panel, not the docks.** A dock is as wide as its widest
/// visible child, so the row's demand is the sum of the widest panel on each side, and that is what
/// has to fit. One panel per door is visible at a time (`Display::None` is how the rest hide, which
/// is also why they lay out to nothing and are skipped here).
///
/// **Both sides are squeezed by the same fraction**, so the pair keeps the proportion its two
/// constants chose rather than one side winning a negotiation it started with a wider number. When
/// the fraction is 1 nothing is written at all, which is the common case and the reason this costs
/// nothing on an ordinary window.
///
/// The floor is [`DOCK_MIN_W`] per side. A window narrower than the two floors can afford does not
/// get a smaller interface, it gets the viewport it has left — the docks cannot lie about their
/// content by becoming narrower than a row.
fn fit_docks_to_the_window(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut panels: Query<(&PanelWidth, &mut Node, &ComputedNode), Without<ViewportSlot>>,
    owned: Query<(), (With<ShellSplit>, Without<SplitUnsized>)>,
    collapsed: Query<(), (With<ShellSplit>, With<bevy_gooey::SplitSaved>)>,
    bars: Query<&ComputedNode, With<crate::activity_bar::ActivityBar>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let scale = ui_scale.0.max(f32::EPSILON);
    // **The activity bar (NV-03, #380) is paid by the docks, not by the map**: it stands in the body's row
    // beside them, so its width comes out of what the two docks may take while the map's share stays what
    // it was. Measured, because the bar's width is gooey's icon tab and its edge; absent, or not laid out
    // yet, it is nothing.
    let bar: f32 = bars.iter().map(|n| n.size().x * n.inverse_scale_factor()).sum();
    // The row's room in `Val::Px` units — the units the constants above are written in. `Window`'s
    // width is already logical, and `UiScale` multiplies every `Val::Px`, so dividing takes the
    // logical width back to the units the panel widths are expressed in.
    let room = window.resolution.width() / scale;

    // **The row's demand is the widest *visible* panel on each side** — a dock is as wide as its
    // widest visible child, which is the dock's own definition, and the rest are `Display::None` and
    // lay out to nothing. Two per door are visible on no door, so the two slots below are exactly
    // the two docks.
    let mut widest = [0.0f32; 2];
    // **A collapsed left pane still has the panel's room counted** (UI-R06, #336): the pane slides to nothing and a peek floats it
    // back over the viewport, and a right dock that was re-fitted to the room the hidden panel gave up would jump with the
    // collapse and move the viewport again when a peek showed the panel. The panel is out of the layout (its dock is
    // `Display::None`) and is what the right dock's share was worked out against, so that is still the answer.
    let left_stands = !collapsed.is_empty();
    for (want, node, computed) in &panels {
        let counted = node.display != Display::None && (computed.size().x > 0.0 || (left_stands && want.side == Side::Left));
        if !counted {
            continue;
        }
        let side = usize::from(want.side == Side::Right);
        widest[side] = widest[side].max(want.preferred);
    }

    // Each dock is its panel plus the margin that panel carries, **measured rather than derived**:
    // `(380 + 2 * 12) * 1.2 = 484.8`, against a real left dock of 485, and `(264 + 24) * 1.2 = 345.6`
    // against a right dock of 346. The border is inside that width, so it is not added here.
    let overhead: f32 = 2.0 * MARGIN;
    let sides: Vec<f32> = widest.iter().copied().filter(|w| *w > 0.0).collect();
    if sides.is_empty() {
        return;
    }
    let sum_pref: f32 = sides.iter().sum();
    let sum_overhead = overhead * sides.len() as f32;

    // **The viewport is paid first.** The docks were not overflowing a 900 px window — measured, they
    // took 835 of it and left the map 65 — so the question is not "do they fit" but "what is the map
    // owed". `squeeze` is the fraction of its preferred width each panel keeps.
    //
    // **Owed: the floor, or its share of the window, and the divider comes out of the room first**
    // (#282). The shell's split costs a divider's width that the two fixed docks never paid, and a
    // pane that is as wide as its panel (see [`size_the_shell_split`]) leaves the map what is left:
    // measured 741 px at a 1600 px window, 28 short of the 769 the docks alone left, and 504 at 1280
    // where the frame's own test asks for half the window. So the map is owed [`VIEWPORT_SHARE`] of
    // the window once that is more than the floor, and the panels give the difference back.
    let room = room - DIVIDER_W;
    let owed = VIEWPORT_MIN_W.max(VIEWPORT_SHARE * room);
    // (The map is owed its share of the room *before* the bar comes out, below.)
    let room = room - bar;
    let squeeze = if room - (sum_pref + sum_overhead) >= owed {
        1.0
    } else {
        let affordable = room - owed - sum_overhead;
        (affordable / sum_pref).clamp(0.0, 1.0)
    };
    // **No panel narrower than the floor**, which binds hardest on the *narrowest* side: the two
    // sides scale by one fraction, so the fraction that keeps the smaller panel at `DOCK_MIN_W` is
    // the one that keeps both usable.
    let floor_fraction = DOCK_MIN_W / sides.iter().copied().fold(f32::INFINITY, f32::min);
    let squeeze = squeeze.max(floor_fraction.min(1.0));

    let person_owns_the_split = !owned.is_empty();
    for (want, mut node, _) in &mut panels {
        // **The left panel is the pane's once a person has dragged the divider**
        // ([`the_left_panel_follows_its_pane`]); writing its width here too would be two systems
        // disagreeing about one number every frame.
        if person_owns_the_split && want.side == Side::Left {
            continue;
        }
        let width = want.preferred * squeeze;
        if node.width != Val::Px(width) {
            node.width = Val::Px(width);
        }
    }
}

/// Height of the two bars. Fixed, because a bar that changed height as its content changed would
/// move the viewport under the author's hands.
const BAR_H: f32 = 26.0;

/// **Build the frame and remember its slots.**
///
/// The `Frame` resource is how [`panel_root`] finds the dock to put a panel in; BSN-style name
/// lookup does not exist here and a marker query would need a flush between the frame and the first
/// panel. Ordering is [`FrameSystems`], which every panel spawner runs `after`.
pub fn spawn_frame(
    mut commands: Commands,
    settings: Option<Res<crate::settings::EditorSettings>>,
    workspace: Option<Res<crate::workspace::Workspace>>,
) {
    // **The workspace's own arrangement, else its default** (UI-33, #227; per workspace since
    // NV-02, #379). `sync_settings` has read the files by the time a frame is built, so this is the
    // layout the editor opens on — and it is keyed by the workspace, so Map's inspector being wide
    // says nothing about Kit's.
    let workspace = workspace.as_deref().copied().unwrap_or_default();
    let layout = settings
        .as_deref()
        .and_then(|s| s.workspace_layout.get(workspace.id()).cloned())
        .unwrap_or_else(|| crate::ui::shell_layout(workspace));
    // **A bar** (G-87): a gooey row `BAR_H` tall and the window wide, its parts centred and `GAP_GROUP` px apart, `MARGIN` in from each end, that never shrinks, with a 1 px edge along
    // the one side that faces the rest of the screen. Its colours are the editor's (`InkFollow`): `Ground`/`BandGround` paint the fill and the theme moves them.
    let bar = |border: bevy_gooey::Sides<f32>| {
        (
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), shrink: Some(0.0), ..default() }),
            bevy_gooey::GapPx(GAP_GROUP),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Px(BAR_H)), ..default() },
            bevy_gooey::Spacing { padding: bevy_gooey::Sides::x(STEP_PAD), ..default() },
            bevy_gooey::BorderPx(border),
            bevy_gooey::InkFollow,
        )
    };

    let chrome_bar = commands
        .spawn((
            bar(bevy_gooey::Sides { bottom: Some(1.0), ..default() }),
            BackgroundColor(ink().bar_bg),
            Ground(ink().bar_bg),
            BandGround,
            BorderColor::all(ink().panel_edge),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    // The strip sets its own padding — a tab is a box, not a word on a bar — so this one only says
    // how tall the band is and where it starts.
    let door_strip = commands
        .spawn((
            // The door strip: a row the window wide with its tabs 2 px apart (`GapPx`), `MARGIN` in from the left, stretched to the strip's height, never shrinking.
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Stretch), shrink: Some(0.0), ..default() }),
            bevy_gooey::GapPx(2.0),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            bevy_gooey::Spacing { padding: bevy_gooey::Sides { left: Some(STEP_PAD), ..default() }, ..default() },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    // **A dock is a column with no width of its own.** Its width is its widest visible panel, which
    // is what lets four tabs' panels share one dock and each keep the width it argued for
    // (`CONTROLS_W` vs `TILES_CONTROLS_W`) without a dock-level table saying it twice.
    let dock = || {
        (
            // A column of its panels to the start that never shrinks. A `sized` box, not a gooey column: `Role::Column` gives a node a least width of 0, and a dock's width is its widest
            // panel (its automatic minimum), which is what keeps a panel from being squeezed by the pane it stands in.
            bevy_gooey::sized(bevy_gooey::Sizing::default()),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), align: Some(AlignItems::FlexStart), shrink: Some(0.0), ..default() },
            bevy::picking::Pickable::IGNORE,
        )
    };
    // **Each side dock is a focus region too** (UI-35, #229). A `Host` viewport with no other region is
    // a keyboard trap: F6 would cycle to itself and Tab would be the only way out. The docks navigate
    // normally — `ArrowPolicy::Navigate` — so F6 walks viewport → left dock → right dock and back, and
    // the arrows in a dock move between its fields, which is what a person expects there.
    // **Each region says its name** (UI-30, #224). The frame was reachable only as an entity id in
    // the `Frame` resource: nothing a reader or a test could search for, so `record.py` guessed
    // pixels and a screen reader met three unnamed boxes. These four names are the shell's own
    // vocabulary — the same words `docs/ui/vision.md` uses for the regions.
    let left = commands
        .spawn((
            dock(),
            bevy_gooey::FocusRegion::navigate(),
            AccessibleLabel::new("Left dock"),
        ))
        .id();
    let right = commands
        .spawn((
            dock(),
            bevy_gooey::FocusRegion::navigate(),
            AccessibleLabel::new("Right dock"),
        ))
        .id();

    let viewport = commands
        .spawn((
            ViewportSlot,
            AccessibleLabel::new("Viewport"),
            // **A focus region that keeps the arrows for the map** (UI-35, #229). The viewport pans and
            // orbits on the arrow keys, so it asks gooey for `ArrowPolicy::Host` — with focus on or
            // inside it, the arrows move no focus and stay readable, while Tab and F6 still leave
            // (WCAG 2.1.2: a region never traps the keyboard). `TabIndex(0)` makes the viewport its own
            // first widget, so a click or F6 puts focus here.
            bevy_gooey::FocusRegion::host(),
            bevy::input_focus::tab_navigation::TabIndex(0),
            // **The centre is what is left**, which is the whole idea: a gooey fill column (it grows and never has a least width of its own, so a wide panel narrows it instead of pushing it
            // off the window), still a row of what it holds.
            bevy_gooey::fill_column(0.0),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), align: Some(AlignItems::Stretch), ..default() },
            bevy_gooey::Sizing { min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
            // No `BackgroundColor` at all: this is the hole the world shows through.
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    let body = commands
        .spawn((
            // The body: what the bars leave, a row of the docks and the viewport.
            bevy_gooey::fill_region(),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), align: Some(AlignItems::Stretch), ..default() },
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // **A gooey split between the left dock and the viewport** (UI-30, #224).
    let split = commands
        .spawn((
            bevy_gooey::split(bevy_gooey::SplitAxis::Horizontal, (), ()),
            bevy_gooey::SplitFraction(0.22),
            // **Both panes keep a floor, so a drag cannot squash the world** (UI-30, #224).
            // Without it `SplitMin` is `(0, 0)` and `SplitFraction` 0.95 left the viewport 47 px of
            // a 1280-px body — a dock covering the world in the only way that matters. gooey's own
            // dock uses the same 96 px for both panes. The left pane's floor is a panel's own
            // (`DOCK_MIN_W` plus its margins), because a dragged-narrow pane makes the panel
            // narrow with it ([`the_left_panel_follows_its_pane`]) and a panel under that floor
            // stops being a panel.
            bevy_gooey::SplitMin::new(DOCK_MIN_W + 2.0 * MARGIN, bevy_gooey::DOCK_PANE_MIN_PX),
            // **`Pickable::default()`, not `IGNORE`** (UI-30, #224). `bevy_picking` skips an
            // entity *and everything below it* when the entity is `IGNORE`, so an ignoring split
            // root made its divider — the one grabbable node in the frame — unhittable: the
            // pointer reached the divider's cursor, the drag press landed on nothing, and the
            // fraction stayed at 0.22 through every drag. The frame's rule is about `Hovered`
            // propagating (`spawn_frame`'s header), and a pickable node without `Hovered` sets
            // none: the panes keep `IGNORE` via `adopt_the_shell_split`, and the divider carries
            // no `Hovered` in this gooey revision. Measured over BRP after the fix: a 105-px drag
            // takes `SplitFraction` from 0.22 to 0.32.
            bevy::picking::Pickable::default(),
            ShellSplit,
            SplitUnsized::default(),
        ))
        .id();
    // **What makes the split's panes fit is the regions being parented into them**
    // (`adopt_the_shell_split`): at HEAD, with no panes occupied, the viewport measured 504 px of a
    // 1280-px body; with the adoption it measures 724. The zero basis below is the same shape every
    // other centre in this frame uses — a flex item's basis is its content when it is `Auto`, so the
    // split would otherwise take a width of its own rather than the room the docks leave.
    // A filling split (G-88): as tall as the body, as wide as what the right dock leaves (`Extent::Auto` + `grow`), with no least size of its own.
    commands.entity(split).insert((
        bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Auto), height: Some(bevy_gooey::Extent::Percent(100.0)), min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
        bevy_gooey::TextFlow { grow: Some(1.0), min_width: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
    ));
    commands.entity(body).add_children(&[split, right]);
    let _ = (left, viewport);

    let status = commands
        .spawn((
            bar(bevy_gooey::Sides { top: Some(1.0), ..default() }),
            AccessibleLabel::new("Status bar"),
            BackgroundColor(ink().bar_bg),
            Ground(ink().bar_bg),
            BandGround,
            BorderColor::all(ink().panel_edge),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    let root = commands
        .spawn((
            // **`Vh`, not `Percent`.** A UI root is a grid item of Bevy's implicit viewport node
                // whose single row is `auto`, so a percentage height is cyclic there and, under
                // Bevy 0.20's taffy, resolves to the content's height: the root grew to its
                // content and no `ScrollArea` below it ever had anything to scroll. A viewport
                // unit is a definite length and does not depend on the track.
            bevy_gooey::sized(bevy_gooey::Sizing {
                width: Some(bevy_gooey::Extent::Percent(100.0)),
                height: Some(bevy_gooey::Extent::Vh(100.0)),
                ..default()
            }),
            // See the type doc: no `Hovered`, and nothing here eats a click meant for the map.
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // **And the arrangement itself, as a gooey dock** (UI-30, #224). The hand-built `left`/`viewport`/
    // `right` row above is still what the frame *names*, because twelve systems read those entities to
    // place a control or a badge; the dock is what now holds the viewport's pane, so its divider can be
    // dragged. Moving the two docks onto gooey's `Dock` as well is the rest of this issue.
    let dock = commands
        .spawn((
            ArrangementDock,
            bevy_gooey::Dock(layout),
            // **The map is where a library piece may be let go** (UI-52, #238): the dock covers the viewport, so it is
            // the drop target, and `editor::drop_a_piece_on_the_map` places what lands on it.
            bevy_gooey::DropTarget::<crate::editor::PieceDrag>::new(),
            bevy_gooey::sized(bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Percent(100.0)), ..default() }),
        ))
        .id();
    // **The dock's own body is the host's, and it is not laid out** (bevy-coven/bevy_gooey#85).
    //
    // The dock here is the arrangement's *data*: what `ui::save_the_layout` saves, what
    // `ui::follow_the_saved_layout` restores, what `stage_panels::arrange` selects a tab in, and the node a
    // library piece is let go on. The panels themselves are still drawn by this file's own spawners into
    // `Frame`'s nodes — what `ui::register_the_shell_panels` registers is a placeholder that carries the
    // panel's id, not its contents.
    //
    // **That was silently true until gooey #85.** `update_docks` used to return early when a dock the host
    // had spawned had no `Role::DockBody` child, which is what this bare `Dock` was: it built nothing. Since
    // #85 a host-placed dock grows its body, its landing and its ghost, and builds every page its layout
    // names — so the shell grew three `panel()` cards (INSPECTOR at 22%, VIEWPORT, PLACE) filled with the
    // skin's surface, stacked over the map between the author and the level. Measured before this: the
    // viewport's own node was under a card, and `crate::view::OverUi` answered *yes* at its centre.
    //
    // gooey has no opt-out, and the path it does document is this one — *"a host that made one of them
    // itself keeps it"* (`docs/guide/docks.md`) — so the body is the frame's, a `Display::None` node the
    // layout is built into. A `Display::None` node has a zero rect, so nothing under it is laid out, drawn
    // or picked: the pages exist and are inert. **When the panels themselves move into this dock** (the rest
    // of UI-30, #224) this node becomes the real body and this paragraph goes with the change.
    // A gooey box with the dock body's role put over its own (`dock_body()` is not public), hidden through `Shown`, which writes both `Display::None` and `Visibility::Hidden`.
    let silent_body = commands
        .spawn((bevy_gooey::sized(bevy_gooey::Sizing::default()), bevy_gooey::Shown(false)))
        .insert(bevy_gooey::Role::DockBody)
        .id();
    commands.entity(dock).add_child(silent_body);
    commands.entity(viewport).add_child(dock);

    // **And the bottom dock** (UI-61, #247): a gooey `dock()` — the bundle, which brings its own real
    // body, so its panels are built and laid out — across the foot of the workbench. A row of the root's
    // own column between the body and the status band, so it takes its height from the window and every
    // panel above it keeps the rest. It starts on the reload panel: what became of each file's last edit
    // is the thing an author looks away from the map to read.
    //
    // **The `Node` is inserted after the spawn, not beside the bundle.** `bevy_gooey::dock()` brings a
    // `Node` of its own (`dock()` is `Node { 100% x 100% }, Role::Dock, Dock(layout), children![..]`),
    // and a bundle carrying the same component twice panics at spawn — every system that reads
    // `Frame` then failed with "Resource does not exist", because `spawn_frame` never finished.
    let bottom = commands
        .spawn((
            BottomDock,
            bevy_gooey::dock(bevy_gooey::DockLayout::group(["reload"])),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // The window wide and `BOTTOM_DOCK_H` tall, never shrinking, with a 1 px edge along its top, in the editor's colours (`InkFollow`). **Not shown until there is room**
    // ([`crate::shell_view::give_the_bottom_dock_its_room`] opens it, writing `display` itself: `ShownFollow`): a dock that is never laid out has no text for a label census to
    // find drawn outside it, and takes nothing from the workbench.
    commands.entity(bottom).insert((
        bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Px(BOTTOM_DOCK_H)), ..default() },
        bevy_gooey::TextFlow { shrink: Some(0.0), ..default() },
        bevy_gooey::BorderPx(bevy_gooey::Sides { top: Some(EDGE_W), ..default() }),
        bevy_gooey::InkFollow,
        bevy_gooey::ShownFollow::closed(),
        BorderColor::all(ink().panel_edge),
    ));

    commands
        .entity(root)
        .add_children(&[chrome_bar, door_strip, body, bottom, status]);

    commands.insert_resource(Frame {
        root,
        chrome_bar,
        door_strip,
        body,
        left,
        viewport,
        right,
        bottom,
        status,
    });
}

/// **The height of the bottom dock** (UI-61, #247), logical px: a panel's body at a comfortable
/// reading size — a heading and six lines of the reload state.
pub const BOTTOM_DOCK_H: f32 = 148.0;

/// **The least height the workbench keeps while the bottom dock is shown**, logical px: the row of
/// the left dock, the viewport and the inspector. The dock is the room the workbench can spare, so it
/// is shown only when the workbench would still have this much beside it. Set from the editor's
/// reference window (1280 x 720, which the headless harness's surface reproduces): with the terminal
/// open the Map workbench measures 326 px there, and a 148-px dock took nearly half of that and left
/// the inspector 24 px of visible height — a note being typed was off the panel — and in the Kit tabs
/// it covered their badge legends.
pub const WORKBENCH_FLOOR_H: f32 = 420.0;

/// **The dock that holds the workspace's arrangement** (UI-30, #224; a second dock since #247).
///
/// `ui::save_the_layout` and `ui::follow_the_saved_layout` write and read the *workspace's* layout, so
/// they must reach this dock and no other: with a bottom dock in the frame, an unfiltered query
/// would write the arrangement into the bottom dock and read the bottom dock's layout back as the
/// workspace's.
#[derive(Component)]
pub struct ArrangementDock;

/// **The bottom dock** (UI-61, #247): the panels that say what became of the work.
#[derive(Component)]
pub struct BottomDock;

/// **A panel's heading.**
///
/// `Ink::text`, not `Ink::accent`. The amber used to head every left dock, and that was one of the five jobs
/// the 2026-09-03 decision took off it: amber now means *a value you are changing right now*, and a
/// panel's name never changes. What separates a title from the rows under it is that it is the
/// largest thing on the panel ([`text::TITLE`]) and has a group gap below it — size and space, which
/// is what a heading is supposed to be made of.
pub fn title(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextColor(ink().text),
        typed(text::TITLE),
        spaced(margin_bottom(STEP_ROW)),
    ));
}

// ── what a tab has to say ────────────────────────────────────────────────────────────────────────

/// **A tab's voice: the running commentary, and the thing that went wrong.**
///
/// # The renderer was guessing at severity, and each tab guessed differently
///
/// Every tab used to hold one `status: String` written from ~215 places, and decided at *render* time
/// whether it was bad news. The Map tab sniffed `status.starts_with("NOT SAVED")`; the Anim tab
/// coloured by whether `rigs.yaml` had loaded, which is a fact about a file rather than about the
/// sentence; the Compose tab painted everything `Ink::accent`; the Tiles tab had no colour rule at all,
/// so a refusal was byte-identical to a receipt. `stamp refused:`, `is not a number of metres` and
/// `NOT WRITTEN:` all rendered in the same grey as `stamped 4 piece(s)`.
///
/// **Severity is known at the write site and nowhere else.** So the write site states it, and no
/// renderer is allowed an opinion — the same move `crate::keys` makes for bindings and this module
/// makes for panels, for the third time and the same reason: a fact stated in two places drifts.
///
/// # Two slots, because one cannot both stick and stay current
///
/// A problem **survives every note**. That is the whole point — a refusal used to vanish the moment
/// the cursor moved, because half of these messages fire on ordinary hovering. But a single slot that
/// refused to be overwritten would swallow the receipts too: an author who fixes the problem and saves
/// would watch the old error sit there and never learn the save worked.
///
/// So a note and a problem are different things with different lifetimes, and the panel shows both.
/// `editor.rs`'s `Field::Edges` had already reached this conclusion for the one readout it owns —
/// *"a fault is a state the map is in — it does not happen once and stop being true, so putting it on
/// `Last` would let the next action erase it"*. This generalises that line to every tab.
///
/// # The banner and the log are one list seen twice
///
/// A problem does not replace the last one — it joins it. The banner shows the newest, because that
/// is what just happened; the log down the bottom shows the run, because *"what has gone wrong on
/// this tab"* is a different question from *"what went wrong just now"*, and the second one was
/// unanswerable: each new refusal erased the one before it, so a session that raised five could only
/// ever show the fifth.
///
/// One list means one clearing rule. [`Status::dismiss`] — `Esc` — takes down the banner **and** the
/// log, because they are two views of the same thing and two clearing rules for one datum is the
/// drift this module exists to prevent.
///
/// Not a `Resource`: each tab holds one inside the state it already owns, so there is nothing to
/// register and no question about which tab a bare `Res<Status>` would have meant.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Status {
    note: String,
    problems: Vec<Problem>,
    /// **Everything this status has ever been handed, repeats included** — and never reset, not
    /// even by [`Status::dismiss`].
    ///
    /// It is the [`Journal`]'s clock. The journal is session-wide and a `Status` is a tab's working
    /// list capped at [`MAX_PROBLEMS`] and cleared by `Esc`, so the journal cannot be derived by
    /// looking at what a status currently holds — by the time it looks, the evidence may be gone.
    /// A number that only goes up says *"there have been three more since you last looked"* without
    /// either side keeping a copy of the other's list.
    raised: u64,
    /// How many fell off the front of [`Self::problems`] at [`MAX_PROBLEMS`].
    ///
    /// Counted rather than forgotten. This crate's caps *"refuse and name rather than truncate"*,
    /// and a log cannot refuse — the newest problem is the one most worth having — so it names what
    /// it dropped instead. A log that silently forgot its first entries would be a log that reads
    /// complete and is not.
    dropped: usize,
}

/// One problem, and how many times it has been raised back to back.
///
/// **Consecutive repeats collapse.** A refusal fires per gesture, and gestures repeat: an author
/// clicking four times at a blocked cell would otherwise push four identical lines and bury
/// everything else on the tab. Only *consecutive* ones fold, so the order stays honest — the same
/// rule `keys::rows` uses to collapse adjacent bindings that share a description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub text: String,
    pub count: usize,
    /// **What `text` was printed from** (UI-05): the problem's `what`, `where`, `why` and `fix`, kept
    /// unprefixed so the log, the journal and an agent's answer carry the same parts. `None` for a
    /// journal row written from a bare string ([`Journal::record`]).
    pub problem: Option<bevy_grimoire_core::problem::Problem>,
}

impl Problem {
    /// How it reads in the log: the text, and the tally only when there is one worth showing.
    pub fn line(&self) -> String {
        if self.count > 1 {
            format!("{}  (x{})", self.text, self.count)
        } else {
            self.text.clone()
        }
    }

    /// **What this refusal is about**, when it names one thing: the entity id in its location, which
    /// the toast's Show button selects.
    pub fn cause(&self) -> Option<&str> {
        self.problem.as_ref()?.where_.as_ref()?.entity.as_deref()
    }
}

/// How many problems a tab keeps. Past this the oldest is dropped and counted — see
/// [`Status::dropped`].
///
/// Sized to a panel rather than to memory: this is a list somebody reads down, and past a dozen or
/// so a list stops being read and starts being scrolled, which is the same argument
/// `keys::no_context_carries_more_than_a_learnable_vocabulary` makes one panel over.
pub const MAX_PROBLEMS: usize = 12;

impl Status {
    /// **What just happened, and went as asked.** Replaces the last note; never touches the problem.
    ///
    /// This is the right call for a refusal the author is not owed an alarm about — clicking bare
    /// ground with the remove tool armed, dragging a box round nothing. Those are answers, not
    /// failures, and a red block for each would teach an author to stop reading red blocks.
    ///
    /// # The line between an answer and a refusal
    ///
    /// **The test is whether the editor did what was asked.**
    ///
    /// | | the slot |
    /// |---|---|
    /// | the author asked for nothing in particular — clicked bare ground, dragged a box round nothing | **a note.** The editor answered, and the answer is "there was nothing here" |
    /// | the author asked for something and the editor **could not do it** | **a problem.** A piece that mounts on a surface cannot be flood filled; a map that cannot be written is not saved |
    /// | the author asked for something and the editor **did only part of it** | **a problem.** A fill that stopped at the cap laid some of what was asked |
    ///
    /// **Three refusals were in this slot and in the wrong one** — the fill's three (`editor.rs`,
    /// the box fill, the riders and the flood), whose own comments said *"a refusal is the answer,
    /// not a failure"*. **That sentence read the rule from the wrong half of this contract**: the
    /// half that says *an answer* is about a no-op the author can see was a no-op, and **a fill that
    /// could not be laid is not a no-op** — the author asked for a floor and did not get one.
    ///
    /// **And the difference is not cosmetic.** A `note` is one dim row, and it is erased by the next
    /// receipt and by a tab switch ([`Status::clear_note`]). A `problem` reaches the toast and the
    /// journal, and survives both. NN/g's Error-Message Guidelines: *"the very worst error messages
    /// are those that don't exist."* **Measured:** a fill that was refused said nothing anywhere the
    /// author looks, while the ghost's refusal for the same piece was plainly visible.
    pub fn note(&mut self, text: impl Into<String>) {
        self.note = text.into();
    }

    /// **The editor could not do what was asked, or did only part of it.**
    ///
    /// The line, in practice: a write that did not reach disk, a refusal handed back by `bevy_grimoire_core`,
    /// input that would not parse, a piece that changed underneath the author, or an edit that
    /// succeeded and then failed to redraw. Anything a person would want to still be on screen a
    /// minute later.
    pub fn problem(&mut self, text: impl Into<String>) {
        let text = text.into();
        let problem = bevy_grimoire_core::problem::Problem::new(text.clone());
        self.push_problem(text, problem);
    }

    /// **A [`bevy_grimoire_core::problem::Problem`], raised with its parts.** The line the toast and
    /// the log show is `prefix` and the problem's own [`line`](bevy_grimoire_core::problem::Problem::line),
    /// and the problem itself is kept on the row, unprefixed, so the journal and an agent's answer
    /// read the same `what`, `where` and `fix` from it that the line was printed from.
    pub fn raise(&mut self, prefix: &str, problem: bevy_grimoire_core::problem::Problem) {
        let text = format!("{prefix}{}", problem.line());
        self.push_problem(text, problem);
    }

    /// **A refusal about one named thing**: the line is exactly `text`, and the id (a library piece or
    /// a placement) is kept as where it happened, so the toast offers Show for it ([`Problem::cause`]).
    pub fn problem_about(&mut self, text: impl Into<String>, entity: &str) {
        use bevy_grimoire_core::problem::{Detail, Location, Problem as Core};
        let text = text.into();
        let problem = Core::with(text.clone(), Detail::default().at(Location::entity(entity)));
        self.push_problem(text, problem);
    }

    fn push_problem(&mut self, text: String, problem: bevy_grimoire_core::problem::Problem) {
        // Counted before the fold, because a repeat is a thing that happened — see [`Self::raised`].
        self.raised += 1;
        if let Some(last) = self.problems.last_mut()
            && last.text == text
        {
            last.count += 1;
            return;
        }
        self.problems.push(Problem { text, count: 1, problem: Some(problem) });
        if self.problems.len() > MAX_PROBLEMS {
            self.problems.remove(0);
            self.dropped += 1;
        }
    }

    /// **A call that either worked or refused**, routed by which it did.
    ///
    /// For the helpers that return `Result<String, String>` — `Ok` is the receipt, `Err` is the
    /// refusal. Written once here so no call site can talk itself into stringifying an error into a
    /// note, which is exactly how `NOT WRITTEN:` ended up rendering grey.
    pub fn say(&mut self, outcome: Result<String, String>) {
        match outcome {
            Ok(receipt) => self.note(receipt),
            Err(refusal) => self.problem(refusal),
        }
    }

    /// **Drop the running commentary**, leaving the problems alone.
    ///
    /// A note is a receipt for something that just happened *here*; a problem is a state the editor
    /// is in. So changing what "here" means ends a note's relevance and none of a problem's — which
    /// is why this is not [`Self::dismiss`].
    ///
    /// Reported at the keyboard, 2026-08-18, and the report was of a different bug entirely: a
    /// rotate receipt — `lamp_tall 270,270,180 deg` — sat on the panel across every tab switch,
    /// because `note` is one `String` that is only ever overwritten and Meshes and Tiles share one
    /// `ImportState`. The author read a message announcing a rotation, on a tab they had just
    /// arrived at, beside a piece that was lying on its side, and concluded the tab switch had
    /// turned it. Measured over BRP: the rotation quaternion is identical on both tabs and
    /// `library.yaml` is not written. **The stale receipt was the whole of the bug.**
    pub fn clear_note(&mut self) {
        self.note.clear();
    }

    /// **Take the notices down** — the banner and the log together, because they are one list.
    ///
    /// `Esc`'s last layer, and the only thing that clears a problem without another replacing it.
    pub fn dismiss(&mut self) {
        self.problems.clear();
        self.dropped = 0;
    }

    /// See [`Self::raised`] — the journal's clock, not a count of what is currently held.
    pub fn raised(&self) -> u64 {
        self.raised
    }

    pub fn note_text(&self) -> &str {
        &self.note
    }

    /// **The newest problem** — what the banner shows. Empty when there is none.
    pub fn problem_text(&self) -> &str {
        self.problems.last().map_or("", |p| p.text.as_str())
    }

    /// The whole run, oldest first — what the log shows and what a copy carries.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    /// How many fell off the front at [`MAX_PROBLEMS`].
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn has_problem(&self) -> bool {
        !self.problems.is_empty()
    }

    /// **The one thing to say, when only one thing fits** — the problem if there is one, else the
    /// note.
    ///
    /// For a log line and for the sentinel driver, whose whole job is to make a refusal visible in a
    /// captured frame. Printing the note there would report `stamped 4 piece(s)` from an earlier verb
    /// while the verb being logged had just refused.
    pub fn line(&self) -> &str {
        if self.has_problem() {
            self.problem_text()
        } else {
            &self.note
        }
    }

    /// Nothing to say at all — neither a receipt nor a refusal.
    pub fn is_empty(&self) -> bool {
        self.note.is_empty() && self.problems.is_empty()
    }
}

/// **Every refusal since the application started**, across tabs and doors.
///
/// # Why this is not four `Status`es
///
/// A [`Status`] is a tab's working list: capped at [`MAX_PROBLEMS`], cleared by `Esc`, and thrown
/// away with the door that owned it. That is right for *"what has gone wrong here"* and useless for
/// *"what has gone wrong at all"* — asked for at the keyboard: *"a hidden log that pops up if you
/// press the key… and that log shows every error message that's happened since the beginning of the
/// application."*
///
/// So this outlives both: `Ownership::Session`, which is the only class in `screen::OWNERSHIP` that
/// survives a door change, and exactly what a session log means.
///
/// # Written by watching, not by being told
///
/// `Status::problem` is a method on a plain struct four tabs own — it has no `Commands` and no way
/// to reach a resource, and giving it one would mean routing every refusal in the editor through an
/// event just so a log could exist. `notice::record_problems` watches [`Status::raised`] instead: a
/// counter that only goes up, so the journal can tell *"three more since I looked"* without either
/// side holding a copy of the other's list — which is what makes `Esc` clearing a tab harmless here.
#[derive(Resource, Default)]
pub struct Journal {
    entries: Vec<Problem>,
    dropped: usize,
    /// **The agent's steps for this session (E-068)**, oldest first, capped by
    /// [`crate::steps::CAP`]. Read from the operation log's own `result` lines rather than written
    /// here, so a row cannot claim more than the log says.
    ///
    /// **Behind a toggle.** The journal is opened with `Cmd+E` and read by scrolling; the steps are
    /// one more list in it rather than a second panel, because a person watching an agent wants the
    /// steps and the refusals in one column — a refusal is a step that did not happen.
    pub steps: Vec<crate::steps::Step>,
    /// How many steps fell off the front at the cap, so the list can say so rather than looking
    /// complete.
    pub steps_dropped: usize,
    /// How many log lines are not entries — a log torn by a crash, or a file edited by hand.
    pub steps_unreadable: usize,
}

/// How many the journal keeps. Ten times a panel's [`MAX_PROBLEMS`], because this one is read by
/// scrolling rather than at a glance — and capped at all for the reason that cap exists: a batch
/// that raises one refusal per frame is a real thing this editor does.
pub const JOURNAL_CAP: usize = 200;

impl Journal {
    /// **Record `times` occurrences of one refusal.** Consecutive repeats fold, exactly as they do
    /// in a [`Status`] — the same rule, so the journal reads like the toast that raised it.
    pub fn record(&mut self, text: &str, times: usize) {
        self.record_problem(text, times, None);
    }

    /// **[`Self::record`], with the structured [`bevy_grimoire_core::problem::Problem`] the text was
    /// printed from**, so a row in the journal carries the same `what`, `where` and `fix` as the toast
    /// that raised it and the answer an agent got.
    pub fn record_problem(
        &mut self,
        text: &str,
        times: usize,
        problem: Option<&bevy_grimoire_core::problem::Problem>,
    ) {
        if times == 0 {
            return;
        }
        if let Some(last) = self.entries.last_mut()
            && last.text == text
        {
            last.count += times;
            return;
        }
        self.entries.push(Problem { text: text.to_owned(), count: times, problem: problem.cloned() });
        if self.entries.len() > JOURNAL_CAP {
            self.entries.remove(0);
            self.dropped += 1;
        }
    }

    /// Oldest first, the order they happened in.
    pub fn entries(&self) -> &[Problem] {
        &self.entries
    }

    /// Named rather than silently forgotten — [`Status::dropped`]'s argument, one scope up.
    pub fn dropped(&self) -> usize {
        self.dropped
    }
}

/// **The text of the toast that is up: the one `Text` a refusal is read from.**
///
/// A marker, put by `notice.rs` on the text of the gooey toast it raised for the live tab's newest
/// refusal, so that there is exactly one such text on screen and `Cmd+C` harvests the right sentence.
/// It carries no field: which tab is live is `Mode`'s answer, asked once by
/// [`crate::notice::paint_notices`].
#[derive(Component, Clone, Copy)]
pub struct ProblemBanner;

/// **The problem, as a toast over the viewport.**
///
/// # It was a block in the status band, and that is what went wrong
///
/// The band is twenty-six pixels of chrome. A padded, wrapping block of the longest text this editor
/// renders was a flow child of it, so it stood proud of the band it was supposed to be inside.
/// Reported from the keyboard: *"when there's an error, it appears over the status bar at the bottom
/// and isn't quite in alignment, but it's pretty close. So it looks really bad."* Close-but-not is
/// worse than either — a block that plainly floats reads as deliberate, and a block a few pixels out
/// reads as broken.
///
/// # Sticky, because a refusal is a state and the journal is its record
///
/// A problem in this editor is **sticky by design** — it is a state, not an event. The toast stands
/// until it is dismissed (its own Dismiss, `Esc`, or the next refusal replacing it), and the session
/// [`Journal`] behind `Cmd+E` keeps every refusal, across tabs, out of `Esc`'s reach:
/// `notice::record_problems` has already written the journal's copy before the toast is up.
///
/// # Where it goes
///
/// Centred at the top of the **viewport**, which is the one region that belongs to nothing else:
/// the docks own both sides, `compass` owns the bottom-left, and `badges` puts its legend in the
/// bottom-right. Asked for as *"a place that isn't going to occlude much"*.
///
/// # It is `bevy_gooey`'s toast now (UI-39, #233)
///
/// The card this spawned was hand-built: a `Text` on a coloured node, faded by a timer in `notice.rs`.
/// What is spawned here now is the **host** for gooey's own toast: the strip below, and in it the one
/// [`bevy_gooey::ToastStack`] every gooey toast stands in (gooey would create its own at the window's
/// root on the first toast; made here, it hangs under the viewport and shows and hides with the strip).
/// `notice.rs` spawns the toast itself, an alert that stands until it is dismissed, with a Show button
/// when the refusal names what it is about.
pub fn problem_toast(mut commands: Commands, frame: Res<Frame>) {
    let stack = commands
        .spawn((
            Name::new("ToastStack"),
            // The stack is gooey's: a window-sized box (`sized`) with the toast stack's role put over its own.
            bevy_gooey::sized(bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Percent(100.0)), ..default() }),
            bevy_gooey::ToastStack,
            bevy::picking::Pickable::IGNORE,
            GlobalZIndex(TOAST_Z),
        ))
        .insert(bevy_gooey::Role::ToastStack)
        .id();
    commands
        .spawn((
            // A toast is not a panel. It is over the viewport rather than in the layout, which is the whole point: `Frame` owns where panels go and this is deliberately not
            // one of them. Placed `GAP_GROUP` down and the viewport wide with its stack centred, and hidden (through `Shown`) until a toast is raised.
            bevy_gooey::placed(UiRect { left: Val::Px(0.0), top: Val::Px(GAP_GROUP), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            bevy_gooey::RowLayout { justify: Some(JustifyContent::Center), ..default() },
            bevy_gooey::Shown(false),
            ToastLayer,
        ))
        .add_child(stack)
        .insert(ChildOf(frame.viewport));
}

/// Above a dialog's scrim (100), as `bevy_gooey` stands its own toast stack: a refusal about what a
/// dialog's button did must be readable over it.
const TOAST_Z: i32 = 150;

/// **Gooey's toast stack keeps clear of the menu bar** (#305).
///
/// A toast is `TopCenter` at the window's top, and the bar is the first 28 px of it, so the tour's
/// offer covered the Compose/Rigs/Tools/Help titles — photographed by the first-run walk at
/// 1088x640 and 1024x640. The editor's **own** host ([`problem_toast`]) hangs inside the viewport,
/// which already starts under the bar, so a refusal was fine; gooey spawns its own stack at the
/// window's root whenever a toast is raised with no host (the tour's offer, raised as a project
/// opens), and nothing told that stack about the bar.
///
/// The height is read from the bar's own laid-out node rather than from a constant: `menu_bar`'s
/// height comes from the skin's type size and padding, so a bigger text scale or a skin change
/// moves it, and `ToastInset` is logical px scaled by `UiScale` — the same units the bar's
/// `ComputedNode` reports through `inverse_scale_factor`. Written only when it differs, so a
/// settled editor costs a comparison.
///
/// **Ungated by screen**: the tour's offer is raised on `OnEnter(Screen::Editor)` before this pass's
/// first `Update`, and a toast can be raised on any screen.
pub fn keep_toasts_under_the_menu_bar(
    mut settings: ResMut<bevy_gooey::ToastSettings>,
    bars: Query<&ComputedNode, With<crate::menu_bar::EditorMenuBar>>,
) {
    let Some(bar) = bars.iter().next().filter(|bar| bar.size().y > 0.0) else { return };
    // Physical in, logical out: `ComputedNode::size` is surface pixels and `UiScale` multiplies
    // every `Val::Px`, so this is the height the bar's own `Val::Px` adds up to.
    let want = bevy_gooey::ToastInset::top(bar.size().y * bar.inverse_scale_factor());
    if settings.inset != want {
        settings.inset = want;
    }
}

/// **The strip the toast stands in.** Its `Display` is what shows and hides the toasts; gooey's stack
/// inside carries them. See [`problem_toast`].
#[derive(Component)]
pub struct ToastLayer;

/// One line of it. Rebuilt wholesale, so it carries nothing — `compose::ComposeLine`'s argument.
#[derive(Component)]
pub struct ProblemLogLine;

/// **Everything that has gone wrong on this tab, at the bottom of its panel.**
///
/// The banner answers *"what just happened"*; this answers *"what has gone wrong here"*, which was
/// unanswerable — each refusal replaced the last, so a session that raised five could only show the
/// fifth. Bulleted and one line each, because it is a list to scan rather than prose to read.
///
/// **A centred overlay, not a panel in the frame's flow.** It used to be pinned to the bottom of
/// whatever panel held it with `margin-top: auto`; it is absolutely positioned now — 8% down, 10% in,
/// 80% wide — so a journal of five refusals is read at a width that fits them rather than in a dock
/// column. Nothing here needs to know which panels are `full_height`.
pub fn journal_panel(mut commands: Commands, frame: Res<Frame>) {
    let panel = commands
        .spawn((
            // An overlay, not a panel in the frame's flow: it is asked for, read, and dismissed, and `Frame` owns the docks. Placed 8% down and 10% in, 80% wide and at most 76% tall, its lines
            // `GAP_ROW` apart and `PAD` in, a 1 px edge, in the editor's own colours (`InkFollow`); closed until `Cmd+E`, and `display` is the notices' to write (`ShownFollow`).
            bevy_gooey::placed(UiRect { left: Val::Percent(10.0), top: Val::Percent(8.0), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(80.0)), max_height: Some(bevy_gooey::Extent::Percent(76.0)), ..default() },
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
            bevy_gooey::GapPx(GAP_ROW),
            bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(STEP_PAD), ..default() },
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(1.0)),
            bevy_gooey::InkFollow,
            bevy_gooey::ShownFollow::closed(),
            BackgroundColor(ink().panel_bg),
            BorderColor::all(ink().key),
            JournalPanel,
            // No `CopyPane` marker: `notice::copy_out` harvests the TEXT of every visible node by
            // walking the roots, so an open journal is carried by `Cmd+C` already — the marker
            // decides nothing there any more, and stamping it here would read as a second path.
            //
            // **`Hovered`, and deliberately not `Pickable::IGNORE`** — the rule [`panel_root`]
            // states, for a panel that is not in a dock. This was an opaque 80%x76% box that no
            // "is the pointer on UI" gate could see: neither `view::over_ui`, which filters to the
            // nodes carrying `Hovered`, nor the mouse verbs, which read a true one. So a click on
            // the open journal stamped or deleted a placement on the map behind it — invisibly,
            // under a solid panel — and the wheel zoomed the world instead of scrolling this list.
            // Exactly the pair the name box's inner box carries a `Hovered` to close, and the same
            // pair `panel_root`'s own note records from when only the rows had one.
            //
            // Nothing changes while it is away: a `Display::None` node has a zero rect, which both
            // the picking backend and `over_ui` skip for the same reason.
            Hovered::default(),
        ))
        // `placed` is `Pickable::IGNORE`; the journal is the opposite (see above), so it is put back over it.
        .insert(Pickable::default())
        .id();
    commands.entity(panel).with_children(|p| {
        title(p, "EVERY ERROR THIS SESSION");
        p.spawn((
            Text::new(format!(
                "{} closes  ·  {} copies this list",
                keys::chord(keys::Action::Cancel),
                keys::chord(keys::Action::CopyInfo)
            )),
            TextColor(ink().dim),
            typed(text::LABEL),
        ));
        // Asks the project's after-edit checks about the picture on screen now. A person's click only:
        // nothing runs while they are simply editing.
        button(p, crate::after_edit::CheckViewButton, "Check view", Severity::Plain);
        // A list to scroll: a session's worth of refusals is longer than a screen by design.
        //
        // FOLLOW-OK: prose, not a selection. Nothing walks this — there is no highlight for the
        // arrows to move and no row to keep on screen, so the scroll exists for HEIGHT alone. The
        // detail pane makes the same declaration for the same reason.
        scroll_list(p, JournalList)
            .entry::<Node>()
            // **Room for the bar, because here the bar lands on words.** `scroll_list` overlays its
            // scrollbar rather than reserving a gutter — right for a list of short rows, and wrong
            // for wrapped prose, which runs under it. Measured in a frame: `…choose a piece in the
            // list|` with the thumb through the last word. Stated at this call site rather than in
            // the builder, because it is a fact about what THIS list holds.
            .and_modify(|mut n| n.padding.right = Val::Px(BAR_W + BAR_INSET * 2.0));
    });
    commands.entity(panel).insert(ChildOf(frame.viewport));
}

/// **The session journal's panel**, hidden until `Cmd+E`. See [`Journal`] for why it is not four
/// panels, and [`journal_panel`] for where it sits.
#[derive(Component)]
pub struct JournalPanel;

/// The scrolling list inside it, rebuilt from [`Journal`] by `notice::paint_journal`.
#[derive(Component)]
pub struct JournalList;

/// One bullet. `•` and not `-`: the shipped face has U+2022 (checked, as U+26A0 was not), and a
/// bullet reads as a list where a hyphen reads as a range.
///
/// # The bullet and the text are siblings, so it can wrap
///
/// This was one `Text` reading `"• {text}"` with `LineBreak::NoWrap`, on the argument that a wrapped
/// continuation restarts at column zero and breaks the bullet column. The argument was right and the
/// remedy was wrong: refusing to wrap does not keep a long refusal inside the panel, it runs it out
/// through the side of the box, which is what an author reported. This tab's messages name
/// descriptors, compositions and counts, so they are routinely longer than 380 px.
///
/// A row with the bullet as its own child fixes both at once: the text wraps, and its continuations
/// align under the text rather than under the bullet, because the bullet is not in that column.
///
/// `min_width: 0` is load-bearing. A flex item will not shrink below its min-content width by
/// default, and for text that is the longest word — so without it the row grows to fit and the
/// wrapping never happens. Same trick as `min_height: 0` on the scroll areas above.
pub fn problem_log_line(parent: &mut ChildSpawnerCommands, text: &str, colour: Color) {
    parent
        .spawn((
            // A bullet and the line, 4 px apart (0.8 of the skin's 5 px gap), as wide as the list.
            bevy_gooey::row(PROBLEM_LOG_GAP_STEPS),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            ProblemLogLine,
        ))
        .with_children(|row| {
            row.spawn((
                Text::new("•"),
                TextColor(colour),
                typed(text::LABEL),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
            row.spawn((
                // `flex_grow` hands the text the row's remaining width as a DEFINITE size —
                // without it a wrapping text item's flex base can resolve to zero width and lay
                // out one glyph per line, clipped to nothing (measured live in the Compose pane,
                // 2026-08-17). `min_width: 0` is what then lets it shrink below its longest word.
                text_narrows(),
                Text::new(text.to_owned()),
                TextColor(colour),
                typed(text::LABEL),
            ));
        });
}

/// The gap between a problem line's bullet and its words, in steps of the skin's gap: 4 px of the editor skin's 5.
const PROBLEM_LOG_GAP_STEPS: f32 = 0.8;


/// **A scrolling list that fills what is left of its panel.**
///
/// `flex_grow` with `min_height: 0`, not `max_height`: a flex item's automatic minimum size is its
/// content, which would grow the node to fit every row and leave `overflow` with nothing to clip. The
/// panel must be `full_height` for this to bound anything.
/// **Where a scroll must move so a row is on screen** — the one arithmetic every list-follow
/// system shares (`tiles::keep_selection_on_screen`, `editor::keep_palette_selection_on_screen`).
///
/// Extracted when the palette gained the same correction the candidates list had (F-9,
/// **Arms when the SELECTION changes — not when the resource holding it does.**
///
/// Every list that follows its highlight needs the same two-step: notice the selection moved, then
/// scroll *next* frame, because the rows are rebuilt on that same change and this frame's
/// `ComputedNode` still describes the previous layout.
///
/// # Why this is a type and not two lines in each system
///
/// The two lines were `if state.is_changed() { pending = true; return; }`, written twice, and both
/// were quietly broken: `is_changed` is true whenever **anything** touches the resource, and both
/// `EditorState` and `ImportState` are written most frames — a status line, a hover, a preview
/// watchdog. So the flag was re-armed every frame and the correction never ran.
///
/// Reported from the keyboard twice. First on 2026-08-14 — *"if I arrow down and the scroll view, it
/// just goes off the screen. The scroll doesn't actually happen."* — and again on 2026-08-16, *"I
/// still have the same bug."* Both times the headless test passed, because in a test nothing else
/// writes the resource, so `is_changed` goes false on the next frame and the correction fires. The
/// test was measuring a world that only exists in tests.
///
/// Keyed on the selection itself, this cannot happen: unrelated writes are invisible to it, and the
/// only thing that arms it is the thing it exists to follow.
pub struct Follow<K> {
    last: Option<K>,
    pending: bool,
}

// A manual impl, because `#[derive(Default)]` would demand `K: Default` and a selection has no
// meaningful zero — `None` is "nothing selected", which is what this starts at.
impl<K> Default for Follow<K> {
    fn default() -> Self {
        Follow { last: None, pending: false }
    }
}

impl<K: PartialEq> Follow<K> {
    /// Give it this frame's selection; it answers whether to scroll **now**.
    ///
    /// `false` on the frame the selection moves — the layout is still last frame's — and `true` on
    /// the one after, so the scroll position is written once per move rather than sixty times a
    /// second.
    ///
    /// # Except on a fast walk, where it fires one move stale rather than never
    ///
    /// The version above returned `pending` **only on a frame where the selection had not moved**,
    /// and that is a third way for this to be starved — the one the 2026-09-03 review found after
    /// the two in the header. A held arrow accelerates to `keys::REPEAT_FAST_SECS` (30 ms) and the
    /// steps land back-to-back on some frames, each pair silently dropping one correction; at any
    /// sustained frame time above 30 ms the selection changes every frame and this fires **never**.
    /// That is the reported symptom exactly: *"I don't see a list of meshes as I press up and down
    /// arrows, but I see the mesh change."*
    ///
    /// So a move that arrives while a correction is already armed **fires anyway**, using a layout
    /// that is one move behind. That is the right trade and it is only right because of the margin
    /// [`scroll_to_reveal`] now keeps: one row stale inside a two-row margin is still on screen,
    /// and the alternative is a list that never scrolls at all.
    pub fn should_scroll(&mut self, now: Option<K>) -> bool {
        if self.last != now {
            self.last = now;
            return std::mem::replace(&mut self.pending, true);
        }
        std::mem::take(&mut self.pending)
    }
}

/// 2026-08-14): two hand-copied versions of fold geometry is how the two lists drift a half-pixel
/// apart, and the arithmetic is the testable part — fold detection, the physical→logical
/// conversion, the clamp, and the dead-band all hold in a unit test, where the pixel scroll itself
/// needs a window.
///
/// Inputs are **physical** pixels — `ComputedNode` and `UiGlobalTransform`, centre and half-size —
/// and the answer is the new **logical** `ScrollPosition::y`, or `None` when the row is already
/// comfortably visible or the correction is under half a pixel (the dead-band that keeps a
/// change-detected write from re-firing layout every frame). A row taller than the list scrolls to
/// its **top**, the half you read first.
///
/// # It reveals with context, and that is the seed bug
///
/// Until 2026-09-03 the correction was exactly `row_bottom - bottom`: enough to put the row's edge
/// on the viewport's edge and not one pixel more. A test pinned that as correct — *"Touching the
/// edges exactly is still inside — flush is not off-screen"* — and it is the wrong rule for a list
/// you walk with the keyboard. Measured live on the Meshes list: after forty presses the selection
/// is glued to the bottom pixel with twenty-six rows of already-passed context above it and **zero
/// rows of what is coming below**. Reported as *"I don't see a list of meshes as I press up and down
/// arrows, but I see the mesh change"* — the list is there, and the part of it that would tell you
/// where you are going is not.
///
/// So the row is revealed with [`CONTEXT_ROWS`] of list either side of it, in units of the row's own
/// height — no new parameter, and it scales with whatever the caller's rows measure. The margin is
/// clamped so a viewport too short to hold row-plus-margins asks for the largest margin that still
/// fits rather than thrashing, which also preserves the over-tall row's scroll-to-top.
///
/// **A jump of more than one viewport re-centres instead of creeping.** A `Shift`×5 step, a click,
/// or a filter that rebuilds the list under the cursor are not walks, and edging such a move to the
/// margin puts the author at the boundary of a region they have not seen; centring says *here* .
pub fn scroll_to_reveal(
    row: (f32, f32),
    list: (f32, f32),
    scroll_y: f32,
    max_scroll_y: f32,
    inverse_scale: f32,
) -> Option<f32> {
    let (row_top, row_bottom) = (row.0 - row.1, row.0 + row.1);
    let (top, bottom) = (list.0 - list.1, list.0 + list.1);
    let (row_h, list_h) = (row.1 * 2.0, list.1 * 2.0);
    // The largest margin that still leaves the row itself room, so a short viewport degrades to
    // flush rather than to a fight between two margins it cannot satisfy.
    let margin = (CONTEXT_ROWS * row_h).min(((list_h - row_h) * 0.5).max(0.0));
    // Above the fold, or below it. Never both — the above check winning is what sends an
    // over-tall row to its top.
    let delta = if row_top - margin < top {
        row_top - margin - top
    } else if row_bottom + margin > bottom {
        row_bottom + margin - bottom
    } else {
        return None;
    };
    // Further than a viewport is a jump, not a step: put the row in the middle.
    let delta = if delta.abs() > list_h { row.0 - list.0 } else { delta };
    // **Clamped at BOTH ends.** The floor was always here; the ceiling is what the two-row margin
    // made necessary — a reveal at the end of a list asks for `max + margin`, Bevy clamps only
    // `ComputedNode::scroll_position` (`bevy_ui-0.19.0/src/layout/mod.rs:364-369`, through
    // `bypass_change_detection`), and the component every caller reads back keeps the surplus. The
    // list then stands still while the highlight walks off it — the 2026-09-03 seed, reintroduced
    // by its own fix.
    let want = (scroll_y + delta * inverse_scale).clamp(0.0, max_scroll_y.max(0.0));
    ((scroll_y - want).abs() > 0.5).then_some(want)
}

/// **Where a scroll viewport actually is, and how far it can go** — both in LOGICAL pixels, both
/// read off the node's own `ComputedNode`.
///
/// The position is the **effective** one: Bevy writes the clamped, floored value into
/// `ComputedNode::scroll_position` and leaves the `ScrollPosition` component alone, so the
/// component can hold a position the list is not at. Row geometry is laid out at the effective
/// one, so that is the only honest basis for a delta measured off the screen.
///
/// The maximum is upstream's own: `content - layout + scrollbar`, floored at zero
/// (`bevy_ui-0.19.0/src/layout/mod.rs:364-367`). `scroll_list` overlays its bar rather than
/// reserving a gutter, so `scrollbar_size` is zero here today; it is in the expression because
/// the day it is not, a reveal that ignored it would be short by exactly the bar.
pub fn scroll_bounds(list: &ComputedNode) -> (f32, f32) {
    let inv = list.inverse_scale_factor;
    let max = (list.content_size.y - list.size().y + list.scrollbar_size.y).max(0.0);
    (list.scroll_position.y * inv, max * inv)
}

/// **How much list stays visible past the selection.** Two rows: one is enough to prove the list
/// continues and not enough to read, and the cost of a third is a viewport that scrolls sooner than
/// the eye asks it to.
const CONTEXT_ROWS: f32 = 2.0;

/// The scrollbar's track. Marked so [`hide_idle_scrollbars`] can find it and its own width is
/// stated once.
#[derive(Component)]
pub struct ScrollTrack;

/// Width of the bar, and how far it is inset from the panel's inner edge.
const BAR_W: f32 = 5.0;
const BAR_INSET: f32 = 1.0;
/// The thumb never shrinks below this, or a long list makes it a dot nobody can grab (Fitts, and
/// upstream's own reason for the parameter).
const MIN_THUMB: f32 = 18.0;
/// **What the bar reserves.** Derived from the bar rather than picked, so the lane and the thing it
/// is a lane for cannot drift apart — the failure this module exists to prevent, one more time.
pub const SCROLL_GUTTER: f32 = BAR_W + BAR_INSET;

/// **A list that scrolls, and now says so.**
///
/// # It scrolled and nothing on screen admitted it
///
/// This spawned one node with `overflow: scroll_y` and a `ScrollArea`, and **no bar of any kind** —
/// so a list longer than its panel clipped, answered the wheel, and gave the author no indication
/// that there was more. It is visible in the 2026-08-18 captures: the Kit door's candidate list
/// opens part-way down, scrolled to the selection, with nothing saying what is above it.
///
/// # Shape, and why there is a wrapper
///
/// `bevy_ui_widgets::Scrollbar` goes on a **track** entity pointing at the scrolled one, and
/// `ScrollbarThumb` is its child. The track has to be positioned against the list's own box, so the
/// list gains a wrapper and the two are siblings inside it. The **marker stays on the scrolling
/// node** and that is load-bearing: every caller's queries, `chrome::Follow`'s reveal arithmetic and
/// `tests/every_list_follows_its_selection.rs` all key on it, and this returns the viewport rather
/// than the wrapper so not one call site changes.
///
/// **The thumb deliberately has no `Node`.** Upstream lays it out itself in `PostUpdate` after
/// `ui_layout_system`; giving it one looks like it works and then fights `update_scrollbar_thumb`.
///
/// **The bar overlays rather than reserving a gutter.** `Node.scrollbar_width` would reserve space,
/// and it also turns on a shipped disagreement: `ScrollArea`'s wheel handler computes its maximum
/// from `size()` while the scrollbar's own code subtracts `scrollbar_size`
/// (`scrollarea.rs:27` vs `scrollbar.rs:173`), so the two disagree by exactly the gutter width.
/// Overlaying sidesteps it.
///
/// # It bounds itself with `flex_grow`, so it needs a bounded parent
///
/// Right for a pane that owns the rest of its dock, and useless **nested inside another scroll**:
/// there the parent is content-sized, `flex_grow` resolves to nothing, and the whole thing collapses
/// to zero — measured, when the tag block vanished outright. A `scroll_box` taking a stated height
/// was written for that case and then deleted with its one caller: the answer was that a scroll
/// inside a scroll is two bars a hand has to choose between, and the pane already scrolled.
pub fn scroll_list<'a>(
    parent: &'a mut ChildSpawnerCommands,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    let viewport = parent
        .commands_mut()
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GAP_TIGHT),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                // **The bar's own lane, so nothing is drawn under it.** The scrollbar is an overlay
                // at `BAR_W` inset by `BAR_INSET`, and until 2026-09-03 the content ran straight
                // beneath it: on the Meshes tab the last chip of a wrapped tag row rendered half
                // behind the bar (the audit's F7). Padding rather than margin, because a scroll
                // viewport's padding is *inside* the clip — so the content stops short of the bar
                // and `content_size` still counts it, which is what `hide_idle_scrollbars` reads.
                padding: UiRect::right(Val::Px(SCROLL_GUTTER)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollArea,
            // **Glide and fade, from gooey (UI-R05, #335).** A scroll the crate makes (a key, the pad, Tab onto
            // a card) travels there instead of jumping, and an edge that hides content fades: two overlays beside
            // this viewport, in its wrapper, each as opaque as the content under its edge, and none at an edge
            // with nothing hidden. The wheel, a trackpad and the bar still move it 1:1.
            bevy_gooey::ScrollGlide::default(),
            bevy_gooey::ScrollEdgeFade::default(),
            marker,
        ))
        .id();

    let thumb = parent
        .commands_mut()
        .spawn((
            bevy::ui_widgets::ScrollbarThumb {
                border_radius: BorderRadius::all(Val::Px(BAR_W / 2.0)),
                border: UiRect::ZERO,
            },
            BackgroundColor(scaled(ink().row_selected, 0.9)),
        ))
        .id();

    let track = parent
        .commands_mut()
        .spawn((
            ScrollTrack,
            bevy::ui_widgets::Scrollbar::new(
                viewport,
                bevy::ui_widgets::ControlOrientation::Vertical,
                MIN_THUMB,
            ),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(BAR_INSET),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(BAR_W),
                // Hidden until there is somewhere to scroll — see [`hide_idle_scrollbars`].
                display: Display::None,
                ..default()
            },
            // The track is a drag target; the content under it is not. The panel root is
            // `Pickable::IGNORE`, so this narrows that for the one node here that is grabbable.
            bevy::picking::Pickable::default(),
        ))
        .id();
    parent.commands_mut().entity(track).add_child(thumb);

    let wrapper = parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            ..default()
        })
        .id();
    parent
        .commands_mut()
        .entity(wrapper)
        .add_children(&[viewport, track]);

    parent.commands_mut().entity(viewport)
}

/// **A bar only while there is somewhere to scroll** — the browser default, and the reason a panel
/// that fits its content shows no furniture at all.
///
/// `Display::None` rather than alpha, so a bar nobody needs costs no draw and no hit test.
///
/// **Compares before writing**, per the standing rule this crate keeps: `chrome::Follow`'s doc
/// records what unconditional per-frame writes cost, and `Node` is change-detected by the layout
/// system. Reading the target's `ComputedNode` from the track side is deliberate — the link runs
/// track → viewport and only that way, so a `Changed<ComputedNode>` filter here would be watching
/// the wrong entity.
pub fn hide_idle_scrollbars(
    mut tracks: Query<(&mut Node, &bevy::ui_widgets::Scrollbar), With<ScrollTrack>>,
    viewports: Query<&bevy::ui::ComputedNode>,
) {
    for (mut node, bar) in &mut tracks {
        let Ok(view) = viewports.get(bar.target) else {
            continue;
        };
        // The same clamp upstream uses, so the bar appears exactly when the wheel has somewhere to
        // go rather than one pixel before or after.
        let visible = view.size().y - view.scrollbar_size.y;
        let want = if view.content_size().y > visible + 0.5 {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
}

/// **A wheel turned over a scrollbar scrolls what the bar is for** (#283).
///
/// The track is a pickable node of its own, a sibling of the scroll area rather than inside it, so a
/// wheel over its five pixels targeted the track, bubbled up through the panel and reached nothing:
/// measured on big, 2026-11-03, a wheel at the inspector's right edge did nothing while the same
/// wheel 12 px to the left scrolled it. `bevy_ui_widgets`' own handler (`scrollarea.rs`) only listens
/// on the area, so this forwards the notch to the area the bar's `Scrollbar::target` names, with the
/// same arithmetic: the pixels the event is worth, held to `[0, content - visible]`.
fn forward_a_wheel_over_a_scrollbar(
    mut scroll: On<bevy::picking::events::PointerScroll>,
    tracks: Query<&bevy::ui_widgets::Scrollbar, With<ScrollTrack>>,
    mut areas: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<ScrollArea>>,
    ratio: Res<bevy::input::mouse::MouseScrollPixelsPerLine>,
) {
    let Ok(bar) = tracks.get(scroll.entity) else {
        return;
    };
    scroll.propagate(false);
    let Ok((node, computed, mut position)) = areas.get_mut(bar.target) else {
        return;
    };
    let visible = computed.size() * computed.inverse_scale_factor();
    let content = computed.content_size() * computed.inverse_scale_factor();
    let delta = scroll.to_pixels(&ratio);
    let range = (content - visible).max(Vec2::ZERO);
    if node.overflow.y == bevy::ui::OverflowAxis::Scroll {
        position.y = (position.y - delta.y).clamp(0.0, range.y);
    }
}

// ── the row vocabulary ───────────────────────────────────────────────────────────────────────────
//
// The builders below were each written AFTER their shape had been hand-rolled at least three times
// (the 2026-08-17 audit counted 8 label/value rows, 7 list rows, 5 chip variants, 6 text fields, 3
// list headings and 2 severity-rail dialects). Each one returns or takes just enough for the call
// sites that exist — a parameter with no caller is a stub, which is why several return
// `EntityCommands` for the caller to finish rather than trying to hold every variation.

/// **A list panel's heading** — "PLACE", "RIGS": the word at the top of a whole panel, over its
/// filter and list. One step louder than [`section`] (10 vs 9) because it heads the panel, not a
/// block within one — the two roles the 2026-08-17 type-role decision named.
pub fn list_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    list_heading_in(parent, text, ListHeadingInk::shipped());
}

/// **The ink a list heading draws in** (UI-02, #210): the shipped `Ink::label` where there is no skin, the
/// skin's own label tone where there is one.
///
/// A separate value rather than a `Res<GooeySkin>` parameter because every caller builds its heading
/// inside a `with_children` closure, where a system parameter cannot reach — the caller reads the skin
/// once, beside the panel it is spawning, and passes the answer in. That is the same shape
/// [`RowFills::of`] and [`BenchInk::of`] use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListHeadingInk(pub Color);

impl ListHeadingInk {
    /// The shipped palette, for a build with no skin.
    pub fn shipped() -> ListHeadingInk {
        ListHeadingInk(ink().label)
    }

    /// The skin's ink where it has one, the shipped constant where it does not. `label` is a **tone**
    /// rather than a field (gooey has none for a list's quiet heading), so a project's skin that names
    /// it gets its own; the editor's own skin does not carry one, which keeps it byte-identical.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> ListHeadingInk {
        match skin {
            Some(skin) => ListHeadingInk(skin.tone("label").unwrap_or(ink().label)),
            None => ListHeadingInk::shipped(),
        }
    }
}

/// **A list heading in a given ink.** The builder every panel uses; see [`ListHeadingInk`].
pub fn list_heading_in(parent: &mut ChildSpawnerCommands, text: &str, ink: ListHeadingInk) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextColor(ink.0),
        typed(text::HEADING),
    ));
}

/// **The label column of a `LABEL  value` row** — the shape the audit found hand-rolled eight times,
/// differing only in this width. 10 px `Ink::label`, never wrapping, never shrinking.
///
/// # The column is a floor, not a cap, and that is the F5 fix
///
/// It used to be `width: Val::Px(width)`. A `Text` does not shrink to its box and UI overflow is
/// visible by default, so a label longer than its column simply **drew over the value beside it** —
/// which is what the menu's KIT INFO row had been doing in every capture: `new work lands here` is
/// 19 characters, plus the selection mark the inspector prefixes, is 126 px of glyphs starting in a
/// 76 px column, so `yes` rendered underneath the word `lands`.
///
/// Measured by `ChooserSlice` on 2026-09-03, and the measurement is the argument: widening the
/// constant to [`COL_WIDE`] takes the overlap from ~74 px to ~30 px. It does not remove it, because
/// no fixed number can — a caller can always be handed a longer string.
///
/// So `min_width` holds the alignment that the constant exists for, and `Val::Auto` lets a label
/// that does not fit push its own value right instead of painting on it. Short labels line up at
/// exactly the column, which is the whole point, and no call site changes.
pub fn row_label(row: &mut ChildSpawnerCommands, width: f32, label: &str) {
    row_label_with(row, width, label, ());
}

/// **A [`row_label`] that keeps its column and says a long phrase on two lines** (#302).
///
/// `row_label` lets a label wider than its column push the value right, which keeps the words apart and
/// moves that one row's value out of line with every other row's. Here the column is the label's width
/// and a phrase of two words wraps inside it, so `UNDER CURSOR` stands as `UNDER` over `CURSOR` and its
/// value starts where the others do. A word still wider than `width` is the caller's to size the column
/// for: it is never cut.
pub fn row_label_wrapping(row: &mut ChildSpawnerCommands, width: f32, label: &str) {
    let id = row_label_with(row, width, label, ());
    row.commands()
        .entity(id)
        .insert((
            TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(width)), ..default() },
        ));
}

/// [`row_label`] with a marker on its label, for a label that changes while the row is on screen.
pub fn row_label_with(
    row: &mut ChildSpawnerCommands,
    width: f32,
    label: &str,
    marker: impl Bundle,
) -> Entity {
    row.spawn((
        // **The value never starts at the label's last glyph**: the label keeps its words' width and is at least the column wide, and a margin of its own
        // (`GAP_ROW`, the design system's row gap, as its spacing step `STEP_ROW`)
        // (outside its box) stands the value off however wide a long label turned out, because the row's `column_gap` only applies between boxes, and a label
        // longer than its column pushes its own box wider (measured on the KIT INFO panel: 5 px against 133 px).
        (
            bevy_gooey::Role::Sized,
            bevy_gooey::TextFlow { grow: None, shrink: Some(0.0), min_width: Some(bevy_gooey::Extent::Px(width)), align: None },
            spacing(margin_right(STEP_ROW)),
        ),
        Text::new(label.to_owned()),
        TextColor(ink().label),
        typed(text::LABEL),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
        marker,
    ))
    .id()
}

/// **The value beside a [`row_label`]** — 11 px, one step louder than its label, which is the
/// role map's one value size (the audit found 10/11 and 10/10 coexisting; 10/11 won).
pub fn row_value(row: &mut ChildSpawnerCommands, text: impl Into<String>, colour: Color, marker: impl Bundle) {
    row.spawn((
        Text::new(text.into()),
        TextColor(colour),
        typed(text::BODY),
        marker,
    ));
}

/// **The editor skin's spacing steps, named** (#260): `editor_skin.yaml`'s `spacing` scale is `[3, 5, 12, 16, 20, 24]` px, so the editor's own gaps are
/// steps of it. [`bevy_gooey::Spacing`] takes a step, and `the_editor_spacing_steps_are_its_gap_constants` holds each to the px constant it replaces.
pub const STEP_TIGHT: bevy_gooey::Space = bevy_gooey::Space::Step(0);
/// [`GAP_ROW`] (5 px) as a step of the skin's spacing scale.
pub const STEP_ROW: bevy_gooey::Space = bevy_gooey::Space::Step(1);
/// [`PAD`] (12 px) as a step of the skin's spacing scale.
pub const STEP_PAD: bevy_gooey::Space = bevy_gooey::Space::Step(2);
/// [`GAP_GROUP`] (16 px) as a step of the skin's spacing scale.
pub const STEP_GROUP: bevy_gooey::Space = bevy_gooey::Space::Step(3);

/// No margin on any side.
pub const NO_MARGIN: bevy_gooey::Sides<bevy_gooey::Space> = bevy_gooey::Sides { top: None, right: None, bottom: None, left: None };

/// A margin above only.
pub const fn margin_top(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { top: Some(space), ..NO_MARGIN }
}
/// A margin below only.
pub const fn margin_bottom(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { bottom: Some(space), ..NO_MARGIN }
}
/// A margin to the left only.
pub const fn margin_left(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { left: Some(space), ..NO_MARGIN }
}
/// A margin to the right only.
pub const fn margin_right(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { right: Some(space), ..NO_MARGIN }
}

/// **A quiet status item's words are a control's, not a button's** (#260): gooey's `status_item_quiet` puts `Role::ButtonText` on its text, and the editor's tests count `ButtonText`
/// over the world to say what a dialog's answers are (`the_bands_item_does_not_call_itself_a_button`), so the item's words become `Role::ControlText` (and take `marker`) when the item is
/// spawned. `item` is the quiet item's entity, spawned in the same command flush.
pub fn the_words_of_a_quiet_item_are_a_control(commands: &mut Commands, item: Entity, marker: impl Bundle) {
    commands.queue(move |world: &mut World| {
        let kids: Vec<Entity> = world.get::<Children>(item).map(|c| c.iter().collect()).unwrap_or_default();
        if let Some(words) = kids.into_iter().find(|k| world.get::<Text>(*k).is_some()) {
            world.entity_mut(words).insert((bevy_gooey::Role::ControlText, Pickable::IGNORE, marker));
        }
    });
}

/// **The shape of a small box that holds one edge colour** (#260): a sized box laid out as a row, with a row radius and a `px` border in `edge`, which is passed as `InkOverride` because the skin
/// paints any `BorderPx` edge in its outline unless the host's colour is pinned (so a box whose edge the host changes by state cannot be this: see the chip's GAP4). A row writes only what it is
/// asked to, so a caller's `entry::<Node>()` stands; `shrink` is `Some(0.0)` for a box that never gives way to its neighbours.
pub fn small_box(spacing: bevy_gooey::Spacing, border: bevy_gooey::Sides<f32>, edge: Color, shrink: Option<f32>) -> impl Bundle {
    (
        bevy_gooey::sized(bevy_gooey::Sizing::default()),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), shrink, ..default() },
        spacing,
        bevy_gooey::BorderPx(border),
        bevy_gooey::InkOverride { edge: Some(edge), ..default() },
        bevy_gooey::Shape { radius: Some(bevy_gooey::Radius::Step(0)), ..default() },
    )
}

/// **The shape of a small box whose colours the editor paints** (#260, G-87): a sized box laid out as a row, with a row radius and a `px` border, marked `InkFollow`, so the skin writes neither its fill nor
/// its edge and the editor's `RowRest`/`style_list_rows`, tag painters and `theme_ink::follow_the_skin` stay the one thing that colours it (by state: rest, hover, press, held, selected). A row writes
/// only what it is asked to, so a caller's `entry::<Node>()` (a margin, a centred `align_items`) stands; `shrink` is `Some(0.0)` for a box that never gives way to its neighbours.
pub fn held_box(spacing: bevy_gooey::Spacing, border: bevy_gooey::Sides<f32>, shrink: Option<f32>, sizing: bevy_gooey::Sizing) -> impl Bundle {
    (
        // A gooey `sized` box that is laid out as a row. Not a gooey row: `Role::Row` gives a node a least width of 0, and a chip, a button or a list row keeps its automatic least
        // width (its content), so a row in a narrow lane is as wide as what it holds rather than squeezed under it.
        bevy_gooey::sized(sizing),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), shrink, ..default() },
        spacing,
        bevy_gooey::BorderPx(border),
        bevy_gooey::InkFollow,
        bevy_gooey::Shape { radius: Some(bevy_gooey::Radius::Step(0)), ..default() },
    )
}

/// **A text field's inset as gooey's `Spacing`** (#260): 4 px across and 2 px down, the same box as [`FIELD_PAD`] (neither is a step of the skin's scale).
pub const FIELD_SPACING: bevy_gooey::Spacing = bevy_gooey::Spacing {
    margin: NO_MARGIN,
    padding: bevy_gooey::Sides {
        top: Some(bevy_gooey::Space::Px(2.0)),
        right: Some(bevy_gooey::Space::Px(4.0)),
        bottom: Some(bevy_gooey::Space::Px(2.0)),
        left: Some(bevy_gooey::Space::Px(4.0)),
    },
};

/// **A button's inset as gooey's `Spacing`** (#260): 10 px across and a row step down, the same box as [`BUTTON_PAD`].
pub const BUTTON_SPACING: bevy_gooey::Spacing = bevy_gooey::Spacing {
    margin: NO_MARGIN,
    padding: bevy_gooey::Sides {
        top: Some(STEP_ROW),
        right: Some(bevy_gooey::Space::Px(10.0)),
        bottom: Some(STEP_ROW),
        left: Some(bevy_gooey::Space::Px(10.0)),
    },
};

/// **A chip's inset as gooey's `Spacing`** (#260): 6 px a side and a tight step above and below, the same box as [`CHIP_PAD`]. The 6 px is not a step of the skin's scale, so it is a literal `Space::Px`.
pub const CHIP_SPACING: bevy_gooey::Spacing = bevy_gooey::Spacing {
    margin: NO_MARGIN,
    padding: bevy_gooey::Sides {
        top: Some(STEP_TIGHT),
        right: Some(bevy_gooey::Space::Px(6.0)),
        bottom: Some(STEP_TIGHT),
        left: Some(bevy_gooey::Space::Px(6.0)),
    },
};

/// **A margin for a text or a cell to stand off its neighbours by** (UI-59, #245; #260): gooey's [`bevy_gooey::Spacing`], in steps of the skin's scale.
///
/// A text carries a `Node` of its own, and a margin is the one thing about it a panel decides: the gap above a block, the half-step below a
/// heading. `Spacing` is written on a node with a `Role`, and a bare `Text` has none, so this carries gooey's `Role::Sized`, a layout role that
/// draws nothing and leaves the text's own ink and font as the host set them (`the_section_headings_keep_their_ink_and_their_margins`).
pub fn spaced(margin: bevy_gooey::Sides<bevy_gooey::Space>) -> impl Bundle {
    (bevy_gooey::Role::Sized, bevy_gooey::Spacing { margin, padding: default() })
}

/// **A text that takes the room its row leaves and may narrow to nothing** (#260): gooey's `TextFlow` on a bare text, with the layout role `TextFlow` is written on
/// (`Role::Sized`, which draws nothing and leaves the text's ink and font as the host set them). A long name wraps in its own column and never pushes its row wider.
pub fn text_narrows() -> impl Bundle {
    (
        bevy_gooey::Role::Sized,
        bevy_gooey::TextFlow { grow: Some(1.0), shrink: None, min_width: Some(bevy_gooey::Extent::Px(0.0)), align: None },
    )
}

/// **A text that keeps its words' width** (#260): `TextFlow { shrink: 0 }` on a bare text, so a tally or a mark keeps a column of its own when its row is squeezed.
pub fn text_keeps_its_width() -> impl Bundle {
    (bevy_gooey::Role::Sized, bevy_gooey::TextFlow { grow: None, shrink: Some(0.0), min_width: None, align: None })
}

/// **A margin for a gooey widget** (it has a `Role` already, which `Spacing` needs): the same margin as [`spaced`], without the layout role a bare text needs.
pub fn spacing(margin: bevy_gooey::Sides<bevy_gooey::Space>) -> bevy_gooey::Spacing {
    bevy_gooey::Spacing { margin, padding: default() }
}

/// **A gooey row with room around it** (UI-59, #245): [`bevy_gooey::row_with`] with no gap of its own, the margin outside it and
/// its cells on `align` across. `.insert(..)` a marker on the returned commands, or `.with_children(..)` the row's cells.
pub fn spaced_row<'a>(parent: &'a mut ChildSpawnerCommands, margin: bevy_gooey::Sides<bevy_gooey::Space>, align: AlignItems) -> EntityCommands<'a> {
    gapped_row(parent, 0.0, margin, align)
}

/// **A [`spaced_row`] whose cells stand `gap` steps of the skin's gap apart** (a gooey row's own rhythm: 1.0 is the skin's gap).
pub fn gapped_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: f32, margin: bevy_gooey::Sides<bevy_gooey::Space>, align: AlignItems) -> EntityCommands<'a> {
    parent.spawn((
        bevy_gooey::row_with(gap, bevy_gooey::RowLayout { align: Some(align), ..default() }),
        bevy_gooey::Spacing { margin, padding: default() },
    ))
}

/// **A gooey row that wraps its cells onto a second line instead of running off the panel's edge** (UI-59, #245): the cells
/// stand `gap` steps of the skin's gap apart, and so do the lines (gooey's wrapping container puts the skin's gap between its
/// lines as well), and the row may narrow below its widest run, which a gooey row always may, so a last chip is never clipped
/// by the panel (the 2026-09-03 audit's F7).
pub fn wrapping_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: f32, margin: bevy_gooey::Sides<bevy_gooey::Space>, align: AlignItems) -> EntityCommands<'a> {
    parent.spawn((
        bevy_gooey::row_with(gap, bevy_gooey::RowLayout { align: Some(align), wrap: Some(FlexWrap::Wrap), ..default() }),
        bevy_gooey::Spacing { margin, padding: default() },
    ))
}

/// **A [`gapped_row`] that is never squeezed** (#260): the cells stand `gap` steps apart on `align` across, and the row keeps its height when
/// the panel is shorter than its rows (a flex item that may shrink is made shorter than its text, which draws one line over the next).
pub fn rigid_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: f32, align: AlignItems) -> EntityCommands<'a> {
    parent.spawn(bevy_gooey::row_with(gap, bevy_gooey::RowLayout { align: Some(align), shrink: Some(0.0), ..default() }))
}

/// **A [`gapped_row`] whose cells gather at the right edge** (#260): a dialog's answers.
pub fn end_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: f32) -> EntityCommands<'a> {
    parent.spawn(bevy_gooey::row_with(gap, bevy_gooey::RowLayout { justify: Some(JustifyContent::FlexEnd), ..default() }))
}

/// **A text that takes the rest of its row and wraps inside it** (UI-59, #245): a label column that grows with the row and
/// may narrow below its words, so a long name wraps in its own column instead of widening the row past the panel.
///
/// A flex item's automatic minimum size is its content, so without the zero floor the column refuses to narrow and the
/// row's last cell is clipped by the panel's edge (the 2026-09-03 audit's F7). Returns the commands so a caller can mark it.
pub fn fill_text<'a>(row: &'a mut ChildSpawnerCommands, text: impl Into<String>, ink: Color, role: text::Role) -> EntityCommands<'a> {
    row.spawn((
        text_narrows(),
        Text::new(text.into()),
        TextColor(ink),
        typed(role),
    ))
}

/// **A fixed-width column holding one text** (UI-59, #245): gooey's `cell`, whose width is in body text sizes, so the px it stands for is
/// divided by the body type size and the column grows with the person's text size instead of falling short of it. A glyph column
/// ([`COL_TIGHT`]: a severity mark, a fold chevron) is the case. The cell cuts what does not fit rather than running onto its neighbour.
pub fn text_cell(row: &mut ChildSpawnerCommands, width_px: f32, text: impl Into<String>, ink: Color, role: text::Role) {
    row.spawn(bevy_gooey::cell(width_px / text::BODY.px())).with_children(|cell| {
        cell.spawn((Text::new(text.into()), TextColor(ink), typed(role)));
    });
}

/// **An empty fixed-width gap between two cells of a row** (UI-59, #245): a gooey `cell` holding nothing, sized like [`text_cell`].
pub fn gap_cell(row: &mut ChildSpawnerCommands, width_px: f32) {
    row.spawn(bevy_gooey::cell(width_px / text::BODY.px()));
}

/// **A chip** — one small clickable word. [`CHIP_PAD`] and a permanent 1 px border whose COLOUR
/// asks the questions (a ghost proposal lights it `Ink::suggest`; everything else runs [`Color::NONE`]),
/// so a chip never changes size when its state does.
///
/// **`marker` must not contain a `Node`, and the failure is a panic rather than a warning.** This
/// builder spawns one, and a bundle carrying two of the same component panics at spawn in Bevy 0.19
/// naming the component and not the call site — the trap the repo's `CLAUDE.md` already records for
/// `button_visual()`. To adjust the node, `.insert(Node { .. })` on the returned `EntityCommands`,
/// which overrides rather than duplicates. The same holds for [`list_row`], [`quiet_row`] and
/// [`text_field`], which each carry a `Node` of their own.
pub fn chip<'a>(
    parent: &'a mut ChildSpawnerCommands,
    marker: impl Bundle,
    label: &str,
    role: text::Role,
    ink: Color,
    fill: Color,
    border: Color,
) -> EntityCommands<'a> {
    let mut c = parent.spawn((
        bevy::ui_widgets::Button,
        Hovered::default(),
        // **A chip answers the pointer, like every row does.** It carried `Hovered` from the day it
        // was written — but only as a hit-test, for the "is the pointer over UI" question, and
        // nothing ever painted it. The 2026-08-17 audit found that as its seventh defect: *"hover
        // exists as a hit-test everywhere and as feedback almost nowhere"*, and the tag block is 55
        // clickable things that gave no sign of being clickable. `RowRest` is the whole fix —
        // `style_list_rows` already keys on it and has never needed to know what it is painting.
        RowRest(fill),
        marker,
        held_box(CHIP_SPACING, bevy_gooey::Sides::all(EDGE_W), Some(0.0), bevy_gooey::Sizing::default()),
        BorderColor::all(border),
        BackgroundColor(fill),
    ));
    c.with_children(|chip| {
        chip.spawn((
            Text::new(label.to_owned()),
            TextColor(ink),
            typed(role),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
    });
    c
}

/// The fill a list row returns to when the pointer leaves it. Carried on every [`list_row`] so ONE
/// system can give the whole editor's lists their hover state — the rebuilt lists are painted
/// inside change-gated `rebuild_*` systems that never see mouse motion, which is why hover was
/// deferred until the shared builder existed (the 2026-08-17 hover-scope decision).
#[derive(Component, Clone, Copy)]
pub struct RowRest(pub Color);

/// **A selectable list row** — full width, [`CHIP_PAD`] inset, `Ink::row_selected` when it is the one
/// being acted on, `Ink::row_hover` under the pointer. Returns the row for the caller to fill; what a
/// row holds is the tab's business, that it looks like every other row is not.
pub fn list_row<'a>(
    parent: &'a mut ChildSpawnerCommands,
    selected: bool,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    // `Button` is what puts a row on the editor's `Activate` bus — see [`quiet_row`] for who is
    // deliberately not on it.
    parent.spawn((bevy::ui_widgets::Button, row_shape(selected), marker))
}

/// **The same row, not on the `Activate` bus.**
///
/// # Why this exists, which is worth reading before merging the two back together
///
/// This editor has **twenty-four global `Activate` observers**, and most take a Map-door resource:
/// `on_row_click` wants `Res<Project>`, `on_tag_chip` wants `ResMut<Project>`. A global observer
/// fires for *any* `Activate` anywhere in the application, and in Bevy 0.19 a missing `Res<T>`
/// **panics** rather than skipping — so the first click on a `Button` outside the editor takes the
/// whole application down.
///
/// That was invisible while `list_row` was only ever called by editor panels. The moment the menu
/// adopted the shared row vocabulary, its first click panicked: `Res<Project>` does not exist on
/// `Screen::Project`. Two observers were fixed to take `Option`; twenty-two were not, and each is the
/// same landmine for the next caller.
///
/// So a row that brings its own click handling says so, and stays off the bus. It keeps everything
/// that makes a row *look* like a row — [`RowRest`], `Hovered`, the fill, the padding — and
/// `style_list_rows` repaints it exactly the same, because that system keys on `RowRest` and has
/// never needed `Button`.
pub fn quiet_row<'a>(
    parent: &'a mut ChildSpawnerCommands,
    selected: bool,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    parent.spawn((row_shape(selected), marker))
}

/// **Whether a row is the chosen one.** Carried rather than baked into the fill alone, because since
/// 2026-09-03 selection is a fill *and* a rail and the two must not be able to disagree.
#[derive(Component, Clone, Copy)]
pub struct RowSelected(pub bool);

/// What a list row looks like, shared by [`list_row`] and [`quiet_row`] so the two cannot drift into
/// two row shapes — which is the drift this whole module exists to stop.
///
/// **The rail is a left border, not a child node**, and the width is reserved whether or not the row
/// is selected — so selecting a row cannot move its text sideways by two pixels, which is what an
/// added child or a conditional border would do to a list the author is reading down.
fn row_shape(selected: bool) -> impl Bundle {
    let rest = if selected { ink().row_selected } else { ink().row_bg };
    (
        Hovered::default(),
        RowRest(rest),
        RowSelected(selected),
        held_box(CHIP_SPACING, bevy_gooey::Sides { left: Some(SELECT_RAIL_W), ..default() }, None, bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() }),
        BorderColor::all(if selected { ink().accent } else { Color::NONE }),
        BackgroundColor(rest),
    )
}

/// The row's non-background parts, as [`style_list_rows`] reads them.
type RowState = (
    &'static RowRest,
    &'static Hovered,
    Has<bevy::ui::Pressed>,
    Has<bevy::ui::InteractionDisabled>,
);

/// **The five states, once, for every row / chip / button in the editor.**
///
/// Disabled beats pressed beats selected beats hover beats rest. Before 2026-09-03 the middle two
/// did not exist: rest, hover and selected were the whole machine, so a control that ran a slow verb
/// looked identical while it ran and a control that could not be used looked exactly like one that
/// could.
///
/// **Why the mouse button is read here rather than trusting `Pressed`.** `bevy_ui::Pressed` is
/// maintained by `bevy_ui_widgets::ButtonPlugin`'s observers, which match `With<Button>`
/// (`bevy_ui_widgets-0.19.0/src/button.rs:59`) — and [`quiet_row`] deliberately has no `Button`,
/// because a global `Activate` observer taking a door's resource panics on the menu. So a quiet row
/// would have been the one shape in the editor that never acknowledged a click. Hover plus a held
/// primary button is the same fact one layer down, and it is what the author actually did.
/// **The colours a list row takes from the skin**, in one place, so a test can ask what the editor
/// would paint without a pointer it cannot place (UI-02, #210).
///
/// `Hovered` is bevy's `Immutable` picking component: only the picker may write it, and a headless app
/// has none, so a row's hover colour cannot be observed by hovering. What *can* be observed is the
/// expression [`style_list_rows`] paints from, which is this — one function, called by the system and
/// by the test alike, so an assertion here is an assertion about the row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowFills {
    pub rest_panel: Color,
    pub hover: Color,
    pub pressed: Color,
}

impl RowFills {
    /// From the running skin when there is one, the constants when there is not (a headless harness
    /// that never ran `EditorUiPlugin`).
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> RowFills {
        match skin {
            Some(s) => RowFills {
                rest_panel: s.surface,
                hover: s.hover.unwrap_or(ink().row_hover),
                pressed: s.pressed.unwrap_or(ink().row_pressed),
            },
            None => RowFills { rest_panel: ink().panel_bg, hover: ink().row_hover, pressed: ink().row_pressed },
        }
    }
}

/// [`RowFills`] for the world as it stands.
pub fn row_fills(world: &World) -> RowFills {
    RowFills::of(world.get_resource::<bevy_gooey::GooeySkin>())
}

fn style_list_rows(
    mouse: Res<ButtonInput<MouseButton>>,
    // **The rows' states come from the skin** (UI-02, #210). This is the one chokepoint every list
    // row's fill passes through — `style_list_rows` keys on `RowRest`, so a row spawned by any tab is
    // painted here — which makes it the first place the editor's own chrome can read `GooeySkin`
    // rather than the constants beside it. The constants stay for the callers that have not moved
    // yet (131 sites outside this module); what changes is that a *theme edit* now reaches every row
    // in the editor, which is the thing #210 exists for.
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut rows: Query<(RowState, &mut BackgroundColor)>,
) {
    let fills = RowFills::of(skin.as_deref());
    let (panel, pressed_col, hover_col) = (fills.rest_panel, fills.pressed, fills.hover);
    // gooey has no `selected` role, so a selected row keeps the editor's own colour — the honest
    // half-measure until that constant group moves.
    let selected_col = ink().row_selected;
    let held = mouse.pressed(MouseButton::Left);
    for ((rest, hovered, pressed, disabled), mut bg) in &mut rows {
        let want = if disabled {
            // Not a fill of its own: a disabled control drops back to the panel it sits on, so it
            // reads as part of the surface rather than as a thing that is merely quiet today.
            panel
        } else if pressed || (hovered.0 && held) {
            pressed_col
        } else if rest.0 == ink().row_selected {
            selected_col
        } else if hovered.0 {
            hover_col
        } else {
            rest.0
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// **What a control does, which is the half its shape cannot say.**
///
/// Shape says *kind* — a [`chip`] toggles, a [`button`] happens — and this says *severity*, which is
/// the 2026-09-03 split. It has exactly three values on purpose: before it, `clear this cell` was
/// red, `rescan mesh` was amber and every other verb was grey, with no rule between them, so red
/// meant "destructive or expensive or the author felt strongly" and therefore meant nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The ordinary case. Most verbs.
    Plain,
    /// The one obvious thing to do here — a single button per block at most.
    Primary,
    /// **Throws work away.** The only thing in this editor that is red.
    Destructive,
}

impl Severity {
    /// The word's ink. The *fill* is `Ink::row_bg` in every case: a filled red button in a panel of
    /// grey ones is a stop sign, and most destructive verbs here are ordinary editing.
    /// Public because `confirm`'s card is built once and repainted per question: which button is
    /// destructive depends on which question is up, so the mapping has to be readable from outside
    /// without being restated outside.
    pub fn ink(self) -> Color {
        match self {
            Severity::Plain => ink().text,
            Severity::Primary => ink().accent,
            Severity::Destructive => ink().danger,
        }
    }
}

/// **A command button** — a control that makes something happen, as opposed to a [`chip`], which is
/// a control that is on or off.
///
/// The two were the same box until 2026-09-03, in the same row, in the same panel: on the Meshes tab
/// the tag chips (toggles), `solid`/`edge`/`clear` (cell commands) and `rescan mesh` (an expensive
/// action) all rendered as one grey rectangle. The only way to learn which was which was to press
/// one. So a button is now visibly heavier — [`BUTTON_PAD`] rather than [`CHIP_PAD`], and a lit
/// `Ink::panel_edge` border where a chip's border is `Color::NONE` unless it has something to say.
///
/// **`marker` must not contain a `Node`** — see [`chip`] for why that is a panic rather than a
/// warning.
pub fn button<'a>(
    parent: &'a mut ChildSpawnerCommands,
    marker: impl Bundle,
    label: &str,
    severity: Severity,
) -> EntityCommands<'a> {
    let mut b = parent.spawn((
        bevy::ui_widgets::Button,
        Hovered::default(),
        RowRest(ink().row_bg),
        marker,
        held_box(BUTTON_SPACING, bevy_gooey::Sides::all(EDGE_W), Some(0.0), bevy_gooey::Sizing::default()),
        BorderColor::all(ink().panel_edge),
        BackgroundColor(ink().row_bg),
    ));
    b.with_children(|b| {
        b.spawn((
            Text::new(label.to_owned()),
            TextColor(severity.ink()),
            typed(text::CONTROL),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
    });
    b
}

// ── the overlay stack, in one place ──────────────────────────────────────────────────────────────
//
// Eight overlays shipped with five z-values and **two with none at all** — the session journal and
// the problem toast, which therefore stacked by spawn order against siblings that had opinions.
// Written as numbers at five call sites in four files, there was nothing to read to find out what
// was in front of what; written here, the list *is* the answer and `every_overlay_declares_its_z`
// fails a build that adds a sixth number somewhere else.
//
// Read it top-down: a question you must answer is in front of the keys that teach you the
// application, which are in front of a readout you glance at.

/// **A modal question.** In front of everything, because nothing else on screen is actionable while
/// one is up.
pub const MODAL_Z: i32 = 900;

/// A speech balloon over a piece (#106): in front of the viewport, behind every panel and modal.
pub const BALLOON_Z: i32 = 50;
/// **The key badges**, which you hold a key to see. Below a modal — a question that has taken the
/// keyboard is exactly the moment the shortcut layer must not cover its own answer.
pub const BADGE_Z: i32 = 500;
/// **The session journal and the problem toast.** Above the panels they are reporting on and below
/// the badges that name the key which dismisses them. These two carried **no `GlobalZIndex` at all**
/// until 2026-09-03, which is not "behind everything" but "wherever the spawn order left them".
pub const NOTICE_Z: i32 = 300;
/// **The compass**, a persistent readout in the corner of the viewport. Lowest, because it is the
/// one overlay that is never the thing you are looking at.
pub const COMPASS_Z: i32 = 100;
/// **A text field's box** — [`MIN_FIELD_H`] floor, [`FIELD_PAD`] inset, `Ink::row_bg` fill, with its
/// readout text spawned inside. The audit found this shape six times with three paddings.
pub fn text_field<'a>(
    parent: &'a mut ChildSpawnerCommands,
    width: Val,
    field: impl Bundle,
    role: text::Role,
    initial: (String, Color),
    readout: impl Bundle,
) -> EntityCommands<'a> {
    let mut boxed = parent.spawn((
        bevy::ui_widgets::Button,
        Hovered::default(),
        field,
        // The field's box (G-87): a gooey row that never shrinks, as wide as asked (`Auto` hugs its readout), `MIN_FIELD_H` tall at least, `FIELD_PAD` in from the edge (4 x 2 px, not
        // a step of the scale). Its fill is the editor's (`InkFollow`), `Ink::row_bg`.
        bevy_gooey::sized(bevy_gooey::Sizing {
            width: match width {
                Val::Px(px) => Some(bevy_gooey::Extent::Px(px)),
                Val::Percent(percent) => Some(bevy_gooey::Extent::Percent(percent)),
                _ => None,
            },
            min_height: Some(bevy_gooey::Extent::Px(MIN_FIELD_H)),
            ..default()
        }),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), shrink: Some(0.0), ..default() },
        FIELD_SPACING,
        bevy_gooey::InkFollow,
        BackgroundColor(ink().row_bg),
    ));
    boxed.with_children(|f| {
        f.spawn((
            Text::new(initial.0),
            TextColor(initial.1),
            typed(role),
            readout,
        ));
    });
    boxed
}

/// **What a field's readout shows**: the live keystrokes with a caret while focused, the committed
/// value otherwise. The `{raw}_` + `Ink::accent` idiom was written five times before it had one home;
/// the caret is what makes "empty because you cleared it" distinguishable from "empty".
pub fn field_text(editing: Option<&str>, idle: (String, Color)) -> (String, Color) {
    match editing {
        Some(raw) => (format!("{raw}_"), ink().accent),
        None => idle,
    }
}

/// **One severity's tint and word, for every rail in the editor.** Two tabs printed the same words
/// under forked maps (tiles: Warn→ACCENT, Note→DIM; anim: everything-not-blocking→LABEL) — same
/// vocabulary, three colours, which is the drift this module exists to stop. The word and the hue
/// travel together so no site can pair "worth checking" with the wrong ink.
pub fn severity_style(severity: bevy_grimoire_core::import::Severity) -> (Color, &'static str) {
    match severity {
        bevy_grimoire_core::import::Severity::Blocking => (ink().danger, "blocking"),
        bevy_grimoire_core::import::Severity::Warn => (ink().accent, "worth checking"),
        bevy_grimoire_core::import::Severity::Note => (ink().dim, "note"),
    }
}

/// **A severity rail** — the tinted left border that makes a finding's weight visible before it is
/// read. 2 px (the tiles dialect; the decision of 2026-08-17), [`GAP_TIGHT`] breathing room, a
/// [`GAP_ROW`] gap to the next. Returns the block for the caller to fill — the severity word first,
/// by convention, in the same tint. A clickable rail (anim's jump rows) adds `Button`/`Hovered`
/// and a fill through `marker`.
pub fn severity_rail<'a>(
    parent: &'a mut ChildSpawnerCommands,
    tint: Color,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    parent.spawn((
        // A column that does not give way to its neighbours, with a 2 px rail along its left, 7 px (not a step of the scale) in from it, a tight step above and below and a row step under.
        bevy_gooey::column_with(0.0, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        bevy_gooey::BorderPx(bevy_gooey::Sides { left: Some(2.0), ..default() }),
        bevy_gooey::InkOverride { edge: Some(tint), ..default() },
        bevy_gooey::Spacing {
            padding: bevy_gooey::Sides { top: Some(STEP_TIGHT), bottom: Some(STEP_TIGHT), left: Some(bevy_gooey::Space::Px(7.0)), ..default() },
            margin: margin_bottom(STEP_ROW),
        },
        BorderColor::all(tint),
        marker,
    ))
}

/// **The name box** — the centred prompt for naming a new composition.
///
/// `M` on the Map keeps the set in hand as one composition, and this is where it is named. It used to
/// be a field in the Map's status readout, which put the question in the corner of the screen while
/// the whole screen waited for it.
///
/// Two tabs asked it when Compose could also make a composition. That is why this is a shared widget
/// painted by [`paint_name_box`] rather than a Map-local one: authoring collapsed onto the Map, and a
/// widget that survives losing one of its two callers is cheaper than one that has to be rebuilt if a
/// second ever returns.
#[derive(Component)]
pub struct NameBox;

#[derive(Component)]
struct NameBoxTitle;

#[derive(Component)]
struct NameBoxValue;

#[derive(Component)]
struct NameBoxHint;

fn spawn_name_box(mut commands: Commands) {
    commands
        .spawn((
            NameBox,
            // **Per `docs/ui.md` §5 trap 7.** A full-screen container without this eats every world click: gooey's `cover` is `Pickable::IGNORE`, so a click while naming
            // falls through to the stage, which is right: the box is a prompt, not a modal that has to be dismissed before anything else works. Its card is centred in it,
            // and it is hidden through `Shown` until a tab asks a name (`paint_name_box`). The dim is the editor's own (`InkFollow`), not the skin's scrim.
            bevy_gooey::cover(),
            bevy_gooey::RowLayout { align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), ..default() },
            bevy_gooey::Shown(false),
            bevy_gooey::InkFollow,
            // Above the panels and the tab strip (101), and below the key badges (500) and the
            // confirm prompt (900) — the tier for "the screen is asking you something", which
            // outranks a panel and is outranked by a question you have to answer.
            GlobalZIndex(400),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
        ))
        .with_children(|p| {
            p.spawn((
                // The dialog card: a column 10 px between its lines (`GapPx`), 18 px in all round (neither is a step of the scale), 360 px at least, in the editor's panel colour.
                bevy_gooey::column(0.0),
                bevy_gooey::GapPx(GAP_ROW * 2.0),
                bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(bevy_gooey::Space::Px(PAD * 1.5)), ..default() },
                bevy_gooey::Sizing { min_width: Some(bevy_gooey::Extent::Px(360.0)), ..default() },
                bevy_gooey::InkFollow,
                BackgroundColor(ink().panel_bg),
                // **The dialog itself is UI; the dimmed backdrop behind it is not.**
                //
                // "Is the pointer over UI" is asked everywhere as "is any `Hovered` true"
                // (`view::drive`, `place_on_click`, `compose::pick_slot`), and this panel carried no
                // `Hovered` — so scrolling over a visible, open dialog zoomed the world behind it.
                //
                // This narrows the root's deliberate `Pickable::IGNORE` rather than undoing it. The
                // root stays click-through, because a prompt is not a modal that has to be dismissed
                // before anything else works; the 360 px box stops being, because a click landing *on*
                // the dialog and placing a piece behind it is the same bug as the scroll.
                //
                // Not `Pickable::IGNORE` here — that would make it unhoverable and reopen the hole.
                // `Hovered` is true for an entity or any descendant, so one on this panel answers for
                // the title, the value and the hint.
                Hovered::default(),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(String::new()),
                    typed(text::BODY),
                    TextColor(ink().label),
                    NameBoxTitle,
                ));
                b.spawn((
                    Text::new(String::new()),
                    typed(text::TITLE),
                    TextColor(ink().accent),
                    NameBoxValue,
                ));
                b.spawn((
                    Text::new(String::new()),
                    typed(text::BODY),
                    TextColor(ink().dim),
                    NameBoxHint,
                ));
            });
        });
}

/// One of the name box's three text lines, narrowed by the filter that tells them apart.
type NameLine<'w, 's, F> = Query<'w, 's, &'static mut Text, F>;

/// **Show the live tab's name field, or nothing.**
///
/// Reads the Map's own state rather than a shared projection, and formats the value the way the Map
/// commits it — snake_case as you type. Relocating the prompt must not quietly change what is saved.
fn paint_name_box(
    editor: Res<crate::editor::EditorState>,
    build: Res<crate::build::Build>,
    compose: Res<crate::compose::ComposeState>,
    mut roots: Query<&mut bevy_gooey::Shown, With<NameBox>>,
    mut titles: NameLine<'_, '_, (With<NameBoxTitle>, Without<NameBoxValue>, Without<NameBoxHint>)>,
    mut values: NameLine<'_, '_, (With<NameBoxValue>, Without<NameBoxTitle>, Without<NameBoxHint>)>,
    mut hints: NameLine<'_, '_, (With<NameBoxHint>, Without<NameBoxTitle>, Without<NameBoxValue>)>,
) {
    // **`grouping.is_some()` and the box being visible are the same condition**, and that is the
    // invariant rather than a description of the code.
    //
    // This used to also match on `Mode::Map`, which looked like a harmless guard and was not:
    // `EditorState::grouping` has no mode scope, so clicking the tab strip mid-name hid the box while
    // `group_name_keys` kept consuming the keyboard — every keystroke vanished until `Esc`, with
    // nothing on screen to say where they were going. Two conditions for one question is what made
    // that state reachable.
    //
    // The tab switch now clears `grouping` (see `tiles::leaving_a_tab_puts_the_name_prompt_down`), so
    // there is one owner and this reads it. The prompt is the Map's, and if a second tab ever asks
    // again the answer is another field, not another condition on this one.
    // **Two fields, one box** — the Map names a composition, the Tiles tab names a tile, and each
    // owns its own `Option<String>`. That is what this function's own note asked for when the
    // second one arrived: *"another field, not another condition on this one."* They cannot both be
    // open, because the tabs are never live together, and the title says which is being answered.
    let asking: Option<(&str, String, String)> = editor
        .grouping
        .as_ref()
        .map(|raw| {
            (
                "NAME THIS COMPOSITION",
                // Forced to snake_case as it is typed, so the naming rule teaches itself.
                format!("{}_", bevy_grimoire_core::naming::to_snake_case(raw)),
                "Enter keeps it.   Esc leaves the set in hand.".to_owned(),
            )
        })
        .or_else(|| {
            build.naming.as_ref().map(|prompt| {
                (
                    "NAME THIS TILE",
                    format!("{}_", bevy_grimoire_core::naming::to_snake_case(&prompt.raw)),
                    "Enter opens it.   Esc leaves things as they are.".to_owned(),
                )
            })
        })
        .or_else(|| {
            // **The COMPOSE tab's own question.** A third field, not a third condition on one of
            // the two above — this function's own note asked for exactly that: *"if a second tab
            // ever asks again the answer is another field, not another condition on this one."* The
            // tabs are never live together, so the chain picks the one that is asking.
            compose.renaming.as_ref().map(|rename| {
                (
                    "RENAME THIS COMPOSITION",
                    // **The same spelling the commit will write** — `to_id`, not `to_snake_case`,
                    // because a composition's id may carry a `/` namespace. A field that shows one
                    // spelling and writes another is a lie the author finds out about after the
                    // rename.
                    format!("{}_", bevy_grimoire_core::naming::to_id(&rename.raw)),
                    "Enter keeps it, and every map that stamps it follows.   Esc leaves it alone."
                        .to_owned(),
                )
            })
        });
    let shown = asking.is_some();
    for mut root in &mut roots {
        if root.0 != shown {
            root.0 = shown;
        }
    }
    let Some((title, value, hint)) = asking else {
        return;
    };
    // Guarded against the no-op write, like `editor::refresh_status`: this runs every frame and
    // `Text` is change-detected, so writing an unchanged string would re-lay the box continuously.
    // Three separate loops rather than one over a tuple: the queries have different filters, so they
    // are different types and cannot share an array.
    //
    // **It takes `&mut Mut<Text>`, not `&mut Text`.** Coercing a `Mut<Text>` down to `&mut Text` at
    // the call site runs `Mut::deref_mut`, which calls `set_changed()` — so the string was compared
    // and the component was dirtied anyway, every frame, which is exactly what the comment above
    // says this guard exists to stop. Reading `text.0` through `&mut Mut<_>` uses `Deref`; only the
    // assignment reaches `DerefMut`.
    fn set(text: &mut Mut<Text>, want: &str) {
        if text.0 != want {
            text.0 = want.to_owned();
        }
    }
    for mut t in titles.iter_mut() {
        set(&mut t, title);
    }
    for mut t in values.iter_mut() {
        set(&mut t, &value);
    }
    for mut t in hints.iter_mut() {
        set(&mut t, &hint);
    }
}


/// **The window's own furniture** — the frame, the chrome bar, the status band, the name box, and
/// the hover every list row shares.
///
/// The key list used to live here too, as a centred two-column table behind a scrim. It is
/// `crate::badges` now, and the argument for moving it is worth keeping where the deletion happened:
/// a table asks you to map a phrase back onto something on screen, and two verbs that had been bound
/// all along (`R`, `Shift+Delete`) stayed invisible for two sessions because their phrase was
/// collapsed onto one row. Drawing the chord *on the thing it acts on* removes that step.
///
/// What survives the move is the line that says the key exists. That line is not optional: Cockburn,
/// Gutwin, Scarr & Malacria 2014 (`10.1145/2659796`, already cited by `crate::keys`) document the
/// intermodal-transition failure — a fast path offered *beside* a slow one does not get adopted on
/// its own — and ExposeHK names its own weakness as exactly this, that an overlay behind a modifier
/// *"has no visual representation to aid discovery until the user … presses its trigger."* See
/// [`shortcut_hint`].
/// **Adopt the left dock and the viewport into the shell split's panes** (UI-30, #224).
///
/// **Both panes must hold a region before the split has a size at all.** Measured: with the split spawned and
/// nothing parented into it, the panes are 0×0 and the viewport gets no room. `split` spawns
/// `[pane, divider, pane]`; the panes carry a `Node` and a `Role` and no `Pickable::IGNORE`, and the divider
/// carries `Hovered` — all three forbidden under the frame, so both are corrected here.
///
/// **While the split is collapsed, the divider has its `Hovered` back, and the first pane has its own** (UI-R06, #336).
/// gooey's peek of a collapsed pane (#69) reads the pointer on the divider (the collapsed edge) and on the pane, and puts
/// `Hovered` on the first pane for as long as the split is collapsed: that is what lets the pane be pointed at, so
/// `Pickable::IGNORE` stays on a pane without it (an open split's panes must never mark the world as interface) and
/// comes off the one that has it, so a floating pane's dock can be clicked. A collapsed split's divider is a sliver on
/// the viewport's edge that is the interface, so it answers the pointer there; an open split's does not.
///
/// **Idempotent**, because the frame is spawned on each door.
fn adopt_the_shell_split(
    mut commands: Commands,
    frame: Option<Res<Frame>>,
    splits: Query<(Entity, Has<bevy_gooey::SplitSaved>), With<ShellSplit>>,
    roles: Query<&bevy_gooey::Role>,
    has_hover: Query<(), With<Hovered>>,
    pickable: Query<&bevy::picking::Pickable>,
    tree: (Query<&Children>, Query<&ChildOf>),
) {
    let (children, parents) = tree;
    let Some(frame) = frame else { return };
    for (split, collapsed) in &splits {
        let Some(kids) = children.get(split).ok() else { continue };
        for child in kids.iter() {
            match roles.get(child) {
                Ok(bevy_gooey::Role::SplitDivider) => {
                    if has_hover.get(child).is_ok() != collapsed {
                        if collapsed {
                            commands.entity(child).insert(Hovered::default());
                        } else {
                            commands.entity(child).remove::<Hovered>();
                        }
                    }
                }
                Ok(bevy_gooey::Role::SplitPane) => {
                    let want = if has_hover.get(child).is_ok() {
                        bevy::picking::Pickable::default()
                    } else {
                        bevy::picking::Pickable::IGNORE
                    };
                    if pickable.get(child).ok() != Some(&want) {
                        commands.entity(child).insert(want);
                    }
                }
                _ => {}
            }
        }
        let panes: Vec<Entity> = kids
            .iter()
            .filter(|e| matches!(roles.get(*e), Ok(bevy_gooey::Role::SplitPane)))
            .collect();
        let [first, second] = panes[..] else { continue };
        for (pane, region) in [(first, frame.left), (second, frame.viewport)] {
            if parents.get(region).map(|p| p.parent()) == Ok(pane) {
                continue;
            }
            commands.entity(region).insert(ChildOf(pane));
        }
    }
}

/// **Give the left pane the width of the panel in it** (#282, #283).
///
/// The split was spawned with `SplitFraction(0.22)`, which at a 1280 px window is a 204 px pane
/// around a panel that asks for `CONTROLS_W` 380 (456 on screen). The pane clips what overflows it
/// and the viewport is drawn over the rest, so the right half of every control in the inspector was
/// cut off or unreachable — `ENVIRONMENT`'s `18:30` drew half a button, `23:00` none, and a click at
/// `23:00`'s own reported centre landed on the viewport (measured on big, 2026-11-03: the pointer's
/// `HoverMap` entry was the viewport node).
///
/// **Follow the panel until the person moves the divider** — a window that resizes, or a second
/// dock that is laid out a frame after the first, changes the width the panel was squeezed to, and a
/// pane sized once would keep the first answer (measured: 386 px against the 349 the settled
/// widths ask for). This runs while [`SplitUnsized`] is on the split, which the frame spawns with
/// it, remembering what it wrote; a fraction that is no longer what it wrote is a drag, an arrow
/// key or a game setting, and the marker comes off for good. The width is the widest visible left
/// panel **as [`fit_docks_to_the_window`] sized it** plus the margin the panel carries
/// (`2 * MARGIN`, the same overhead that system measures), over the split's room in the same
/// `Val::Px` units, so a narrow window's squeezed panel gets a pane squeezed to match.
fn size_the_shell_split(
    mut commands: Commands,
    mut splits: Query<(Entity, &mut bevy_gooey::SplitFraction, &mut SplitUnsized, &ComputedNode, &Children)>,
    dividers: Query<&ComputedNode, With<bevy_gooey::SplitDivider>>,
    panels: Query<(&PanelWidth, &Node, &ComputedNode)>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
) {
    // **No window, no room to answer for**: [`fit_docks_to_the_window`] returns here too, so a panel's
    // width is still its preferred one and a pane sized to it would be the wrong pane.
    if windows.is_empty() {
        return;
    }
    let mut want = 0.0f32;
    for (panel, node, computed) in &panels {
        if panel.side != Side::Left || node.display == Display::None || computed.size().x <= 0.0 {
            continue;
        }
        if let Val::Px(w) = node.width {
            want = want.max(w);
        }
    }
    if want <= 0.0 {
        return;
    }
    for (split, mut fraction, mut sized, node, kids) in &mut splits {
        // **Someone else wrote the fraction since this did**: a drag, an arrow key, a game setting.
        // It is theirs from now on.
        if sized.wrote.is_some_and(|w| w != fraction.0) {
            commands.entity(split).remove::<SplitUnsized>();
            continue;
        }
        let bar = kids
            .iter()
            .find_map(|c| dividers.get(c).ok())
            .map_or(0.0, |d| d.size().x * d.inverse_scale_factor());
        let extent = node.size().x * node.inverse_scale_factor() - bar;
        if extent <= 0.0 {
            continue;
        }
        let to = ((want + 2.0 * MARGIN) / extent).clamp(0.0, 1.0);
        if fraction.0 != to {
            fraction.0 = to;
        }
        if sized.wrote != Some(to) {
            sized.wrote = Some(to);
        }
    }
}

/// The shell's split while its pane is collapsed, or sliding to or from it.
type CollapsedOrGliding = (With<ShellSplit>, Or<(With<bevy_gooey::SplitSaved>, With<bevy_gooey::SplitGlide>)>);

/// **Once a person drags the divider, the left panel is as wide as the pane they made** (#224).
///
/// Before that, [`size_the_shell_split`] sizes the pane to the panel; after it the pane is the
/// person's, and a panel that kept its own width would be cut off by a narrower pane or float in a
/// wider one — a dock that resizes without its content reflowing is not a dock. The panel's width
/// is the pane's less its margins, never under [`DOCK_MIN_W`] (the pane's own floor, `SplitMin`,
/// keeps the two from disagreeing). [`fit_docks_to_the_window`] leaves the left panel to this once
/// the split is the person's, so one system writes the number.
///
/// **Not while the split is collapsed or sliding** (UI-R06, #336): the pane is shrinking to nothing or growing back, and a panel that
/// followed it would reflow every frame of the slide and be a different width when the peek floats it over the viewport.
/// Collapsed, it keeps the width it had; a restore lands and this takes the number back from the pane, which is that width.
fn the_left_panel_follows_its_pane(
    frame: Option<Res<Frame>>,
    owned: Query<(), (With<ShellSplit>, Without<SplitUnsized>)>,
    parents: Query<&ChildOf>,
    panes: Query<&ComputedNode>,
    mut panels: Query<(&PanelWidth, &mut Node)>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    moving: Query<(), CollapsedOrGliding>,
) {
    if owned.is_empty() || windows.is_empty() || !moving.is_empty() {
        return;
    }
    let Some(frame) = frame else { return };
    let Ok(pane) = parents.get(frame.left).map(|p| p.parent()) else { return };
    let Ok(node) = panes.get(pane) else { return };
    let wide = node.size().x * node.inverse_scale_factor();
    if wide <= 0.0 {
        return;
    }
    let width = Val::Px((wide - 2.0 * MARGIN).max(DOCK_MIN_W));
    for (panel, mut n) in &mut panels {
        if panel.side == Side::Left && n.width != width {
            n.width = width;
        }
    }
}

pub struct ChromePlugin;

impl Plugin for ChromePlugin {
    fn build(&self, app: &mut App) {
        app
            // A wheel over a scrollbar scrolls its area (#283).
            .add_observer(forward_a_wheel_over_a_scrollbar)
            // **The frame first, and everything that puts a panel in it after.** `Res<Frame>` is
            // how `panel_root` finds its dock, and in Bevy 0.19 a missing `Res<T>` panics its
            // system rather than skipping it — so this is an ordering the build must state, not one
            // that happens to hold. Ordering across a set also gets a sync point inserted, which is
            // what makes the deferred `insert_resource` visible to the spawners.
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                spawn_frame.in_set(FrameSystems),
            )
            .add_systems(Update, adopt_the_shell_split.after(FrameSystems))
            // **The band's count watches the set in hand** (UI-34, #228): every frame, gated on the
            // resource, because a set is picked up and put down long after the screen is entered.
            // Registering it in the `OnEnter` tuple above would run it once, on the frame the band is
            // spawned, and never again.
            .add_systems(
                Update,
                (show_selection_count, show_problems_count)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                (
                    spawn_name_box,
                    spawn_chrome_bar,
                    spawn_status_band,
                    problem_toast,
                )
                    .after(FrameSystems),
            )
            // Every list row's hover, in one place — the rows themselves are spawned by
            // change-gated rebuilds that never see mouse motion.
            //
            // **Ungated by screen**, unlike the systems below it: the menu draws rows too, and the
            // query simply matches nothing on a screen that has none. Gating it would be a second
            // place that has to know which screens have lists.
            .add_systems(
                Update,
                (
                    style_list_rows,
                    hide_idle_scrollbars,
                    repaint_where_you_are,
                    // Ungated for the same reason: both screens have panels, and which one holds
                    // the keyboard is a question the menu asks louder than the editor does.
                    light_the_focused_panel,
                ),
            )
            // **After `Phase::Text`, not in it.** The field consumes the keystroke there; painting
            // before it would show the box one character behind what has been typed.
            .add_systems(Update,
                (paint_name_box.after(keys::Phase::Text).before(bevy_gooey::GooeySystems::Style))
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            // B-014: the docks answer to the window. Editor-only, because the menu's own columns
            // go straight into `body` and carry no `PanelWidth`.
            .add_systems(
                Update,
                fit_docks_to_the_window
                    .after(FrameSystems)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                Update,
                size_the_shell_split
                    .after(fit_docks_to_the_window)
                    .after(adopt_the_shell_split)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                Update,
                the_left_panel_follows_its_pane
                    .after(size_the_shell_split)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(Update,
                (light_the_back_button, stand_down_the_back_chord, dim_the_ground)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            // **Gooey's own toast stack stands under the menu bar** (#305). Ungated by screen: a
            // toast can be raised before the editor's frame exists (the tour's offer) and on a
            // screen with no frame at all, and gooey puts its stack at the window's root every
            // time. See `keep_toasts_under_the_menu_bar`.
            .add_systems(Update, keep_toasts_under_the_menu_bar.after(FrameSystems));
    }
}



#[cfg(test)]
mod scroll_tests {
    use super::{scroll_to_reveal, Follow};

    /// The ceiling every pre-existing case runs under: none. The arithmetic those cases pin is
    /// unchanged by the clamp; the two tests at the end of this module are the clamp's own.
    const NO_LIMIT: f32 = f32::INFINITY;

    /// **The arming rule, which is the half that was broken for two days — and then for longer.**
    ///
    /// `scroll_to_reveal`'s arithmetic was always right and always tested; nothing ever called it,
    /// because the flag that should have said "now" was re-armed every frame by an unrelated write.
    /// So this tests the flag, not the sums.
    #[test]
    fn a_follower_arms_on_the_selection_and_fires_exactly_once() {
        let mut f: Follow<usize> = Follow::default();

        // The frame the selection appears: do NOT scroll — the rows were rebuilt this frame and the
        // geometry still describes the previous layout.
        assert!(!f.should_scroll(Some(3)), "the move frame reads stale layout");
        // The next frame, with the selection unchanged: scroll, once.
        assert!(f.should_scroll(Some(3)), "the frame after is when the rows are real");
        assert!(!f.should_scroll(Some(3)), "and it does not write every frame after that");
        assert!(!f.should_scroll(Some(3)));

        // Moving again re-arms the same two-step.
        assert!(!f.should_scroll(Some(4)));
        assert!(f.should_scroll(Some(4)));

        // **The regression itself**: unrelated churn must not touch this. A resource-keyed follower
        // saw `is_changed` every frame — a status line, a hover, a preview watchdog — re-armed
        // itself, and never fired. Nothing here can observe that, which is the point.
        assert!(!f.should_scroll(Some(4)), "still quiet however busy the resource is");

        // Losing the selection arms too, so a list that had a highlight and now has none does not
        // scroll to a row that is gone.
        assert!(!f.should_scroll(None));
        assert!(f.should_scroll(None));
    }

    /// **A held arrow must not starve it, which is the third way this was broken.**
    ///
    /// The version before 2026-09-03 returned `pending` only on a frame where the selection had not
    /// moved. A held arrow reaches `keys::REPEAT_FAST_SECS` — 30 ms — and lands two steps on one
    /// frame often enough that corrections were being dropped in pairs; at any sustained frame time
    /// above 30 ms the selection changes every frame and the follow fired **never**. That is the
    /// reported symptom: the mesh changes, the list does not move.
    ///
    /// A move arriving while a correction is already armed now fires on the spot, one move stale,
    /// which the two-row margin absorbs.
    #[test]
    fn a_fast_walk_still_scrolls() {
        let mut f: Follow<usize> = Follow::default();

        // First move arms; nothing to fire yet.
        assert!(!f.should_scroll(Some(0)));
        // Now move on every single frame, as a held arrow does. Every one of these must scroll —
        // under the old rule every one of them returned false, for ever.
        for step in 1..=10 {
            assert!(
                f.should_scroll(Some(step)),
                "step {step} of a held arrow dropped its correction"
            );
        }
        // And it still settles: one more fire when the walk stops, then silence.
        assert!(f.should_scroll(Some(10)), "the last move is corrected once the walk ends");
        assert!(!f.should_scroll(Some(10)), "and then it is quiet");
    }



    /// A row with [`super::CONTEXT_ROWS`] of list on both sides asks for nothing — the common case,
    /// and the one that keeps a change-detected `ScrollPosition` from being touched sixty times a
    /// second.
    #[test]
    fn a_row_with_context_around_it_asks_for_no_scroll() {
        // List centred at 200, half 100 → fold [100, 300]. Row half 10 → margin is 2 rows = 40.
        // Comfortable band is therefore [140, 260]; a row at 200 sits in the middle of it.
        assert_eq!(scroll_to_reveal((200.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0), None);
        // Exactly two rows clear of each edge is still comfortable.
        assert_eq!(scroll_to_reveal((150.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0), None);
        assert_eq!(scroll_to_reveal((250.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0), None);
    }

    /// **Flush is not comfortable, and that reverses a decision.**
    ///
    /// The test here used to assert the opposite — *"Touching the edges exactly is still inside —
    /// flush is not off-screen"* — and it was true and beside the point. A row on the boundary has
    /// nothing after it, so a list being walked downward reads as having ended. Measured live on the
    /// Meshes list before the change: forty presses, selection on the bottom pixel, zero rows of
    /// what was coming next.
    #[test]
    fn a_flush_row_scrolls_to_earn_its_margin() {
        // Row flush against the bottom edge of fold [100, 300]: it wants 2 rows (40 px) of daylight.
        assert_eq!(
            scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0),
            Some(40.0)
        );
        // And flush against the top edge, the same the other way.
        assert_eq!(
            scroll_to_reveal((110.0, 10.0), (200.0, 100.0), 100.0, NO_LIMIT, 1.0),
            Some(60.0)
        );
    }

    /// **A viewport too short for row-plus-margins degrades to flush** rather than fighting two
    /// margins it cannot satisfy at once.
    #[test]
    fn a_short_viewport_gives_up_its_margin() {
        // Fold [180, 220] is 40 tall; a 20-tall row leaves 10 either side, not 40.
        // Row at 215 (spans [205, 225]) overhangs the bottom by 5, and wants 10 more.
        assert_eq!(
            scroll_to_reveal((215.0, 10.0), (200.0, 20.0), 0.0, NO_LIMIT, 1.0),
            Some(15.0)
        );
    }

    /// **A jump of more than a viewport re-centres instead of creeping to the margin.**
    ///
    /// A `Shift`×5 step, a click, or a filter rebuilding the list under the cursor are not walks;
    /// edging such a move to the margin leaves the author on the boundary of a region they have not
    /// seen.
    #[test]
    fn a_page_jump_centres_the_row() {
        // Fold [100, 300], 200 tall. A row centred at 900 is far past a viewport away, so the
        // correction is "put it in the middle": 900 - 200 = 700.
        assert_eq!(
            scroll_to_reveal((900.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0),
            Some(700.0)
        );
    }

    /// Walking just past the fold scrolls by the overshoot **plus the margin it is owed**.
    #[test]
    fn an_off_screen_row_scrolls_by_its_overshoot() {
        // Row bottom at 320 against a fold ending at 300: 20 px of overshoot, plus a 40 px margin.
        assert_eq!(
            scroll_to_reveal((310.0, 10.0), (200.0, 100.0), 40.0, NO_LIMIT, 1.0),
            Some(100.0)
        );
        // Row top at 80 against a fold starting at 100: 20 px back up, plus the same margin.
        assert_eq!(
            scroll_to_reveal((90.0, 10.0), (200.0, 100.0), 100.0, NO_LIMIT, 1.0),
            Some(40.0)
        );
    }

    /// The answer is logical pixels: a 2x display (inverse scale 0.5) halves the physical delta —
    /// margin included, since the margin is measured in the same physical pixels the row is.
    #[test]
    fn the_correction_converts_physical_to_logical() {
        // 20 px of overshoot plus a 40 px margin is 60 physical, which is 30 logical on top of 40.
        assert_eq!(
            scroll_to_reveal((310.0, 10.0), (200.0, 100.0), 40.0, NO_LIMIT, 0.5),
            Some(70.0)
        );
    }

    /// The scroll never goes negative — the top of the list is the top.
    #[test]
    fn the_scroll_clamps_at_zero() {
        assert_eq!(
            scroll_to_reveal((50.0, 10.0), (200.0, 100.0), 10.0, NO_LIMIT, 1.0),
            Some(0.0)
        );
    }

    /// A correction under half a pixel is noise, not a scroll — the dead-band that stops a
    /// float-jittering layout from re-marking the resource changed every frame.
    ///
    /// Measured against the **comfortable** band rather than the fold, since 2026-09-03: with a
    /// 20 px row in a 200 px fold the margin is 40, so the band ends at 260 and a row bottom at
    /// 260.3 is three tenths of a pixel of jitter, not a scroll.
    #[test]
    fn a_sub_pixel_correction_is_swallowed() {
        assert_eq!(
            scroll_to_reveal((250.3, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0),
            None
        );
    }

    /// A row taller than the whole list aligns its TOP — the half you read first — rather than
    /// oscillating between its two unsatisfiable edges.
    #[test]
    fn an_over_tall_row_aligns_its_top() {
        // Row spans [40, 560] against fold [100, 300]: top wins, scroll up by 60.
        assert_eq!(
            scroll_to_reveal((300.0, 260.0), (200.0, 100.0), 100.0, NO_LIMIT, 1.0),
            Some(40.0)
        );
    }

    /// **A reveal never asks past the end of the content.** The flush-bottom case above wants 40;
    /// with 25 of scroll left, it gets 25. Bevy clamps only `ComputedNode::scroll_position`, so
    /// an unclamped answer here would leave the `ScrollPosition` component carrying a surplus the
    /// list is not at — and every later delta computed from a place the list never went.
    #[test]
    fn a_reveal_never_asks_past_the_end_of_the_content() {
        assert_eq!(
            scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 0.0, 25.0, 1.0),
            Some(25.0)
        );
    }

    /// **A scroll already at the end asks for nothing.** No write, so no re-layout, which is what
    /// the dead-band exists for — and the frame-after-frame write at the end of a list is exactly
    /// how the seed presented.
    #[test]
    fn a_scroll_already_at_the_end_asks_for_nothing() {
        assert_eq!(
            scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 25.0, 25.0, 1.0),
            None
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    /// **The whole point.** A refusal survives every receipt that follows it — which is what the
    /// single `status: String` could not do, because half these messages fire on ordinary hovering
    /// and the next one along overwrote the last.
    #[test]
    fn a_problem_outlives_every_note() {
        let mut s = Status::default();
        s.problem("NOT SAVED: read-only file system");
        for receipt in ["placed crate@7", "removal mode off", "saved", "filled 12"] {
            s.note(receipt);
            assert_eq!(
                s.problem_text(),
                "NOT SAVED: read-only file system",
                "`{receipt}` erased the refusal"
            );
        }
        assert_eq!(
            s.note_text(),
            "filled 12",
            "the receipt line stopped being current"
        );
    }

    /// The other half, and the reason there are two slots rather than one that refuses to be
    /// overwritten: an author who fixes the problem and saves has to be able to see the save work.
    #[test]
    fn a_note_is_still_shown_while_a_problem_stands() {
        let mut s = Status::default();
        s.problem("cannot record `break_table`: no such member");
        s.note("`break_table` armed");
        assert_eq!(s.note_text(), "`break_table` armed");
        assert!(s.has_problem());
    }

    /// **A problem joins the last one; it does not replace it.** The banner shows the newest, the
    /// log shows the run — which is the question that was unanswerable when a new refusal erased
    /// the one before it.
    #[test]
    fn problems_accumulate_and_the_newest_is_the_banner() {
        let mut s = Status::default();
        s.problem("first");
        s.problem("second");
        assert_eq!(s.problem_text(), "second", "the banner shows the newest");
        assert_eq!(s.problems().len(), 2, "and the log keeps both");
        // One list, one clearing rule.
        s.dismiss();
        assert!(!s.has_problem());
        assert!(s.problems().is_empty());
        assert!(s.is_empty(), "dismissing left something behind");
    }

    /// **Consecutive repeats fold.** A refusal fires per gesture and gestures repeat; four identical
    /// lines would bury everything else on the tab.
    #[test]
    fn a_repeated_problem_folds_into_a_count() {
        let mut s = Status::default();
        for _ in 0..4 {
            s.problem("blocked: `floor@3` already covers that spot");
        }
        assert_eq!(s.problems().len(), 1);
        assert_eq!(s.problems()[0].count, 4);
        assert!(s.problems()[0].line().ends_with("(x4)"));

        // Only CONSECUTIVE ones, so the order stays honest.
        s.problem("NOT SAVED: read-only file system");
        s.problem("blocked: `floor@3` already covers that spot");
        assert_eq!(
            s.problems().len(),
            3,
            "a repeat after something else is a new entry"
        );
    }

    /// **The cap names what it dropped.** This crate's caps refuse and name rather than truncate; a
    /// log cannot refuse the newest entry, so it counts the ones it let go instead.
    #[test]
    fn the_log_caps_and_says_how_many_it_dropped() {
        let mut s = Status::default();
        for i in 0..MAX_PROBLEMS + 5 {
            s.problem(format!("problem {i}"));
        }
        assert_eq!(s.problems().len(), MAX_PROBLEMS);
        assert_eq!(s.dropped(), 5);
        assert_eq!(
            s.problem_text(),
            format!("problem {}", MAX_PROBLEMS + 4),
            "the newest must survive the cap — it is the one worth having"
        );
        s.dismiss();
        assert_eq!(s.dropped(), 0, "dismissing clears the tally with the list");
    }

    /// `say` is the one place a `Result` becomes a message, so no call site can stringify an error
    /// into the quiet slot — the bug that had `NOT WRITTEN:` rendering as a receipt at nine
    /// `tiles::persist` call sites.
    #[test]
    fn say_routes_by_the_result_and_not_by_the_wording() {
        let mut s = Status::default();
        s.say(Ok("recorded 3 member(s)".to_owned()));
        assert_eq!(s.note_text(), "recorded 3 member(s)");
        assert!(!s.has_problem());

        // Deliberately worded like a receipt: routing must not depend on how it reads.
        s.say(Err("everything went fine, honestly".to_owned()));
        assert_eq!(s.problem_text(), "everything went fine, honestly");
        assert_eq!(
            s.note_text(),
            "recorded 3 member(s)",
            "an Err overwrote the receipt"
        );
    }

    /// One line for a log or a captured frame, and it must be the bad news when there is any.
    #[test]
    fn the_one_line_form_prefers_the_problem() {
        let mut s = Status::default();
        s.note("stamped 4 piece(s)");
        assert_eq!(s.line(), "stamped 4 piece(s)");
        s.problem("stamp refused: nothing offers a surface");
        assert_eq!(s.line(), "stamp refused: nothing offers a surface");
    }
}

/// **One line where eighteen rows used to be.** See [`ChromePlugin`] for why it is not optional.
/// **What this door is open on** — the map's name, or the kit's if no map is open.
///
/// One function because two things say it: the window's title (`main::name_the_window`) and the
/// chrome bar. Which *name* is the right one is a decision, and a decision made twice is a decision
/// that drifts — the window would go on saying `furniture` while the bar said the map, and nobody
/// would notice for a week.
pub fn subject(
    open_map: Option<&crate::project::OpenMap>,
    project: Option<&crate::project::Project>,
) -> Option<String> {
    match (open_map, project) {
        (Some(m), _) => Some(m.map.name.clone()),
        (None, Some(p)) => Some(p.namespace.clone()),
        _ => None,
    }
}

/// **The way back to the chooser, and the one piece of the window that belongs to no door.**
///
/// # It failed twice as a row in a panel
///
/// Asked for at the keyboard: *"when we go into the map editor, we actually need a button to go back
/// to the main UI."* There was only `Cmd+O`, and a key nothing on screen mentions is a key nobody
/// finds. So a button was added — **inside the left panel**, under that panel's own heading, on
/// `Ink::slot_bg`, the ground this editor uses for an inspector slot. It was reported missing again:
/// *"When I enter kit editing, there's no clear way to get back to the main menu."*
///
/// `docs/2026-08-17-one-application.md` §3 diagnosed why, and it was not the contrast: the encoding
/// said *"a field of the thing you are looking at"* rather than *"a way out of it"*, because it read
/// as a row in a list of rows, and **nothing at window level was navigation at all**. This repo
/// already settled what to do when a signal fails twice — *"the encoding was not weak, it was
/// wrong"*.
///
/// So it is chrome now, above the door's own strip, the same on every door. **Four call sites
/// became one**, which is the other half of the point: a way out that each panel places is a way out
/// each panel can forget, and the Rigs door drew it in a different place from the Map door.
///
/// It still names its chord beside itself, which is `ExposeHK`'s rehearsal argument — the pointer is
/// the way in, and the label beside it is what turns a pointing habit into a typing one. The click
/// and the key both go through `editor::leave_for_menu`, so the unsaved-work refusal cannot differ
/// between them.
pub fn spawn_chrome_bar(
    mut commands: Commands,
    frame: Res<Frame>,
    door: Option<Res<crate::workspace::Workspace>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    project: Option<Res<crate::project::Project>>,
) {
    let here = door.as_deref().map(|d| {
        match subject(open_map.as_deref(), project.as_deref()) {
            Some(name) => format!("{} · {name}", d.label()),
            None => d.label().to_owned(),
        }
    });

    // **The way out names where it goes**: the project's own name, which is the page it returns to (`project_page.rs`).
    let back = back_name(project.as_deref());

    commands.entity(frame.chrome_bar).with_children(|bar| {
        bar.spawn((
            BackButton,
            Control(keys::ControlId::Back),
            // A chip-sized row: its words a row step apart, a chip's inset, never shrinking, in the editor's own fill (`InkFollow`).
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), shrink: Some(0.0), ..default() }),
            bevy_gooey::GapPx(GAP_ROW),
            CHIP_SPACING,
            bevy_gooey::InkFollow,
            BackgroundColor(ink().slot_bg),
            // The bar is `Pickable::IGNORE` so the frame answers no clicks; this narrows that for
            // the one node on it that is a button — the same move `spawn_name_box` makes.
            bevy::picking::Pickable::default(),
            Hovered::default(),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(format!("\u{2039} {back}")),
                TextColor(ink().key),
                typed(text::BODY),
                bevy::picking::Pickable::IGNORE,
            ));
            b.spawn((
                BackChord,
                Text::new(keys::chord(keys::Action::MainMenu)),
                TextColor(ink().label),
                typed(text::BODY),
                bevy::picking::Pickable::IGNORE,
            ));
        });

        // Pushes the subject to the far end. A spacer rather than `SpaceBetween` on the bar, so a
        // third thing added later lands where it is put rather than being redistributed.
        bar.spawn(bevy_gooey::spacer());

        if let Some(here) = here {
            bar.spawn((
                WhereYouAre,
                // Where you are is what `N` renames, so its badge goes on the words it changes.
                Control(keys::ControlId::Title),
                Text::new(here),
                TextColor(ink().dim),
                typed(text::BODY),
                bevy::picking::Pickable::IGNORE,
            ));
        }
    });
}

/// The bar's right-hand end: which door, on what. Marked so a test can find it — and so
/// [`repaint_where_you_are`] can keep it true.
#[derive(Component)]
pub struct WhereYouAre;

/// **The bar says what the title says, for as long as both are on screen.**
///
/// [`spawn_chrome_bar`] runs once, at `OnEnter(Editor)`, and nothing repainted this — so renaming a
/// map with `N` moved `main::name_the_window`'s title and left the bar reading the old name for the
/// rest of the session. That is exactly the drift [`subject`] was extracted to make impossible,
/// arriving through the half that was never wired: one function deciding the name is worth nothing
/// if only one of its two readers ever asks it.
///
/// Compared before writing, per the standing rule — `Text` is change-detected and this runs every
/// frame.
pub fn repaint_where_you_are(
    door: Option<Res<crate::workspace::Workspace>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    project: Option<Res<crate::project::Project>>,
    mut bar: Query<&mut Text, With<WhereYouAre>>,
) {
    let Some(door) = door else { return };
    let want = match subject(open_map.as_deref(), project.as_deref()) {
        Some(name) => format!("{} \u{00b7} {name}", door.label()),
        None => door.label().to_owned(),
    };
    for mut text in &mut bar {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// **One band at the foot of the window: what went wrong, and what the keys are.**
///
/// Both of these were spawned *inside* each panel, once per tab, which had two costs. The banner
/// carries the longest text this editor renders — refusals naming descriptors and compositions —
/// and a 300 px controls panel is the narrowest place on screen to put it. And the hint line, at
/// `CONTROLS_W`, wrapped: *"Hold K for shortcuts · Cmd+O back to kits & maps"* broke across two
/// lines on the Map door, which is visible in the capture that started this work.
///
/// Full window width fixes both by construction, and it deletes four copies of the same two calls.
///
/// **The hint goes at the far end behind a spacer, not in a `SpaceBetween` row**, so it does not
/// move when the banner appears — a hint that shifts sideways the moment something goes wrong is a
/// hint you have to re-find exactly when you are least able to.
///
/// The problem **log** deliberately stays in the panels. The banner answers *"what just happened"*
/// and belongs to the window; the log answers *"what has gone wrong here"*, is a stack of lines
/// rather than one, and a band that grew to hold it would move the viewport every time a refusal
/// landed.
pub fn spawn_status_band(mut commands: Commands, frame: Res<Frame>, project: Option<Res<crate::project::Project>>) {
    let back = back_name(project.as_deref());
    commands.entity(frame.status).with_children(|band| {
        // **The problem is not here any more.** It used to be a flow child of this band — a padded,
        // wrapping block inside twenty-six pixels of chrome, so it stood proud of the band it was
        // supposed to be in. Reported from the keyboard: *"it appears over the status bar at the
        // bottom and isn't quite in alignment… so it looks really bad."* It is a toast over the
        // viewport now; see [`problem_toast`].
        // **Which model is answering, once for the session.** It used to be printed on every
        // proposal, which is the same fact restated per piece — see `labels::Labeler`, which owns
        // both the text and the rule about when the word `connected` may be used. Empty until the
        // config has been read, so an unconfigured project shows nothing rather than a placeholder.
        // **What is in hand** (UI-34, #228): `"3 selected"` while a set is held, empty otherwise, so
        // the band answers the one question a person asks before a stamp — how many am I carrying —
        // without them counting from the ghost. Empty when nothing is held rather than `"0 selected"`:
        // a count of nothing is a row that says the same thing on every frame and teaches nobody.
        // **What the reloads say** (UI-34, #228): a count of files refused or needing a restart, in the
        // warning ink when there are any, empty when every file applied. A person driving over BRP asks
        // this in `grimoire_state.reloads`; the band answers it where they are looking.
        band.spawn((
            ProblemsCount,
            bevy_gooey::StatusKind::Info,
            Text::new(String::new()),
            TextColor(ink().dim),
            typed(text::LABEL),
            bevy::picking::Pickable::IGNORE,
        ));
        band.spawn((
            SelectionCount,
            Text::new(String::new()),
            TextColor(ink().dim),
            typed(text::LABEL),
            bevy::picking::Pickable::IGNORE,
        ));
        band.spawn((
            LabelerLine,
            Text::new(String::new()),
            TextColor(ink().dim),
            typed(text::LABEL),
            bevy::picking::Pickable::IGNORE,
        ));
        band.spawn(bevy_gooey::spacer());
        shortcut_hint(band, &back);
    });
}

/// **The status band's labeler readout.** The node is here because the band is; what it says is
/// `labels::Labeler`'s, so this module does not learn what a model is.
#[derive(Component)]
pub struct LabelerLine;

/// **How many files the last reload refused or could not take without a restart**, in the status band
/// (UI-34, #228). Written by [`show_problems_count`].
#[derive(Component)]
pub struct ProblemsCount;

/// **What the band says about the reloads, and the kind it says it as** (UI-34, #228).
///
/// The system and `tests/the_status_band_says_when_a_file_needs_attention.rs` both call this, so an
/// assertion about the band is an assertion about what is painted, not about a resource that happens
/// to hold the same value.
///
/// **Four rules, each with a reason:**
///
/// - An `applied` file is not counted. The status line already writes a receipt for every edit, and a
///   band that said "3 reloaded" on every keystroke would be a row nobody reads.
/// - **Both a refusal and a restart are the warning kind**, which is the issue's own wording ("a
///   refused reload turns the problems item to the warning kind"). A refusal keeps the previous state
///   and the editor keeps running; nothing is lost, but something a person wrote did not take, and
///   that is what the mark is for.
/// - **Both counts are named when both exist**, because a person who fixes the refusal still has a
///   restart owed, and one number in place of the other would hide half the work.
/// - **Nothing wrong says nothing**, rather than `"0 problems"` — the band is silent in the state it
///   is in almost all the time.
///
/// The ink comes from the running skin, so a theme edit reaches the band in the same frame; with no
/// skin (a hand-built harness, never the editor) it falls back to the panel colours.
pub fn reload_readout(
    kinds: &[bevy_grimoire_core::live::KindVerdict],
    skin: Option<&bevy_gooey::GooeySkin>,
) -> (String, bevy_gooey::StatusKind, Color) {
    use bevy_grimoire_core::live::Outcome;
    let refused = kinds.iter().filter(|k| k.outcome == Outcome::Refused).count();
    let waiting = kinds.iter().filter(|k| k.outcome == Outcome::NeedsRestart).count();

    let mut parts: Vec<String> = Vec::new();
    if refused > 0 {
        parts.push(if refused == 1 { "1 refused".to_owned() } else { format!("{refused} refused") });
    }
    if waiting > 0 {
        parts.push(if waiting == 1 {
            "1 needs a restart".to_owned()
        } else {
            format!("{waiting} need a restart")
        });
    }

    let kind = if refused + waiting == 0 {
        bevy_gooey::StatusKind::Info
    } else {
        bevy_gooey::StatusKind::Warning
    };
    let ink = match (kind, skin) {
        (bevy_gooey::StatusKind::Info, Some(s)) => s.text_muted,
        (bevy_gooey::StatusKind::Info, None) => ink().dim,
        (_, Some(s)) => s.accent,
        (_, None) => ink().accent,
    };
    (parts.join(" \u{b7} "), kind, ink)
}

/// **Say what became of the reloads, when the answer changes.**
///
/// Gated on `EditorReloads` (a file was decided) or `GooeySkin` (a theme edit), so a band with nothing
/// wrong writes no text on any frame. Every write is compared in place as well, which is
/// `tests/no_system_writes_every_frame.rs`'s rule: the text, the kind and the ink are change-detected,
/// and everything downstream reads that detection.
pub fn show_problems_count(
    reloads: Option<Res<crate::live::EditorReloads>>,
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut counts: Query<(&mut Text, &mut TextColor, &mut bevy_gooey::StatusKind), With<ProblemsCount>>,
) {
    let Some(reloads) = reloads else { return };
    if !reloads.is_changed() && !skin.as_ref().is_some_and(|s| s.is_changed()) {
        return;
    }
    let (said, kind, ink) = reload_readout(reloads.kinds(), skin.as_deref());
    for (mut text, mut painted, mut told) in &mut counts {
        if text.0 != said {
            text.0 = said.clone();
        }
        if painted.0 != ink {
            painted.0 = ink;
        }
        if *told != kind {
            *told = kind;
        }
    }
}

/// **How many pieces are in hand**, in the status band (UI-34, #228). Written by
/// [`show_selection_count`] from [`crate::editor::CloneDrag`], which owns the answer.
#[derive(Component)]
pub struct SelectionCount;

/// **Say how many pieces are in hand, when the answer changes.**
///
/// Gated on the resource's change detection, so a band with nothing in hand writes no text on any
/// frame: the count only moves when a set is picked up, added to or put down.
pub fn show_selection_count(
    drag: Res<crate::editor::CloneDrag>,
    mut counts: Query<&mut Text, With<SelectionCount>>,
) {
    if !drag.is_changed() {
        return;
    }
    let said = match drag.held_pieces() {
        Some(n) => format!("{n} selected"),
        None => String::new(),
    };
    for mut text in &mut counts {
        if text.0 != said {
            text.0 = said.clone();
        }
    }
}

/// The clickable way back. Marked so `editor` can hang one observer on it wherever a panel put it.
#[derive(Component)]
pub struct BackButton;

/// **The chord printed on the button**, so it can stand down while the badges are up.
#[derive(Component)]
pub struct BackChord;

/// **One `Cmd+O` on the button, never two.**
///
/// The chord is printed beside `‹ kits & maps` for the reason [`spawn_chrome_bar`] argues at
/// length — ExposeHK's rehearsal path, and an author who pressed `Esc` three times looking for the
/// exit. Holding the shortcuts key then draws that same chord *again*, as the button's badge, one
/// gap away from itself. Reported from the keyboard: the back button prints `Cmd+O` twice.
///
/// So while the overlay is up, the inline copy stands down and the badge is the one that speaks.
/// This is the move `compass::show_by_tab` already makes for the gizmo, and for the same reason: two
/// things that are each right on their own are not both wanted in the same instant.
///
/// Compares before writing — `Node` is change-detected by the layout system, and `Mut::deref_mut`
/// marks it changed whether or not the value moved.
fn stand_down_the_back_chord(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mut chords: Query<&mut Node, With<BackChord>>,
) {
    let want = if keys::pressed(&keyboard, *live, keys::Action::Shortcuts) {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut chords {
        if node.display != want {
            node.display = want;
        }
    }
}

/// **While the shortcuts key is held, the grounds step back and the badges are the nearest thing.**
///
/// This is the other half of letting a badge stand on a panel's empty middle. `badges::ink` reads
/// [`Ground`] to say that a container's fill carries nothing a reader needs, so the ground under a
/// short list is placeable — but a badge is itself an `Ink::panel_bg` box, and an opaque box on an
/// opaque box of the same colour reads as one shape with a line through it. Dropping the ground to
/// [`GROUND_HELD`] lets the `Ink::veil` and the window behind it show through, and the badge — which
/// keeps its own opacity — separates by depth rather than by its border alone.
///
/// **Fill only, and restored from the carried colour.** Every row, field, chip and word inside the
/// panel keeps its opacity, because those are exactly what the badges are pointing at.
///
/// Compares before writing: `BackgroundColor` is change-detected, and touching every panel every
/// frame would mark the whole interface dirty sixty times a second for one held key
/// (`tests/no_system_writes_every_frame.rs`).
pub fn dim_the_ground(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    // **The panel's colour is the running skin's** (UI-02, #210 slice 4b). This system already walks
    // every ground, already compares before writing, and is the only place with the skin in reach —
    // `panel_root`'s six callers have no `Res<GooeySkin>`. A `None` skin (a headless harness that
    // never ran `EditorUiPlugin`) keeps the constant, which is the shipped table.
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut grounds: Query<(&Ground, Option<&PanelGround>, Option<&BandGround>, &mut BackgroundColor)>,
) {
    let held = keys::pressed(&keyboard, *live, keys::Action::Shortcuts);
    let fills = PanelFills::of(skin.as_deref());
    for (ground, is_panel, is_band, mut bg) in &mut grounds {
        // A panel's and a band's rest colour follow the skin; anything else carries its own. Reading
        // the skin *here* rather than at spawn is what makes a theme edit reach a panel that was built
        // before it — the whole point of a skin being data.
        let rest = if is_panel.is_some() {
            fills.panel
        } else if is_band.is_some() {
            fills.band
        } else {
            ground.0
        };
        let want = if held { rest.with_alpha(rest.alpha() * GROUND_HELD) } else { rest };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// The back button's hover state, as the lift reads it.
type BackButtonRow = (&'static Hovered, &'static mut BackgroundColor);

/// Lift the button while the pointer is on it, so it reads as pressable before it is pressed.
pub fn light_the_back_button(
    mut buttons: Query<BackButtonRow, (With<BackButton>, Changed<Hovered>)>,
) {
    for (hovered, mut bg) in &mut buttons {
        bg.0 = if hovered.get() { ink().row_selected } else { ink().slot_bg };
    }
}

/// **Where the way out goes, in words**: the project's folder name, which is the page `Cmd+O` and the
/// back button return to (`project_page.rs`), and `Projects` when no project is open.
fn back_name(project: Option<&crate::project::Project>) -> String {
    project
        .and_then(|p| p.root.file_name())
        .map_or_else(|| "Projects".to_owned(), |n| n.to_string_lossy().into_owned())
}

pub fn shortcut_hint(parent: &mut ChildSpawnerCommands, back: &str) {
    parent.spawn((
        // **The way out is named here, not only on the button above it.**
        //
        // The button says `‹ kits & maps` at 11px and this line said `Hold K for shortcuts` at 10px,
        // and an author looking for the exit found neither — they pressed `Esc` three times instead,
        // which is the one key deliberately wired to mean "not that" rather than "out". Naming the
        // chord where the eye already goes for keys is ExposeHK's rehearsal argument: the novice
        // path and the expert path are the same path.
        //
        // **It says what the key does now, not what it opens.** `K` used to raise a table you read;
        // it labels the interface in place. "For shortcuts" described a document, and the whole
        // reason the document was replaced is that a reader had to carry a phrase from it back to
        // something on screen.
        Text::new(format!(
            "Hold {} — every key lands on what it does   ·   {} back to {back}",
            keys::chord(keys::Action::Shortcuts),
            keys::chord(keys::Action::MainMenu),
        )),
        TextColor(ink().label),
        typed(text::LABEL),
        // **And holding it puts that key's own badge on this line.** The one control whose verb is
        // "show me the verbs" — ExposeHK's admitted weakness is that a modifier-triggered overlay
        // *"has no visual representation to aid discovery until the user … presses its trigger"*, and
        // this is the loop closing: the line that tells you about `K` is where `K` labels itself.
        Control(keys::ControlId::Hint),
    ));
}
