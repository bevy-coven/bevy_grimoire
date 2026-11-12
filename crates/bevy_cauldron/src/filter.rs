//! **Narrowing a list by typing at it.**
//!
//! The map palette is 41 rows and the tiles candidate list is 318. Grouping made the first one
//! navigable — `docs/ui.md` §3.5, fixed positions, never reordered by recency — but grouping does not
//! scale to three hundred, where the only way to reach a row is to remember which pack it came from
//! and scroll. A filter is the escape hatch that does not disturb the ordering: the rows that survive
//! stay in exactly the order they were in, so what an author learned about where things sit is still
//! true. Nothing is re-ranked, ever.
//!
//! One box per list, and the text **persists when focus leaves it** — that is why this is its own
//! resource rather than the transient buffer the name/id/size fields share. A filter you have to
//! retype every time you click a row is a filter nobody uses twice.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;



/// Which list a box narrows.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    /// The map tab's PLACE palette.
    Palette,
    /// The tiles tab's candidate list.
    Candidates,
    /// The animation bench's rig list.
    Rigs,
    /// **The tag block in the mesh detail pane** — the project's whole vocabulary, 55 chips at the
    /// shipped kit's size, of which a piece holds three to six.
    ///
    /// The fourth pane and the only one that narrows a *grid* rather than a list, which changes two
    /// things and nothing else. Its `Enter` **takes the one match** instead of handing the keyboard
    /// back (see [`keys`]), because for a list `Enter` means "done narrowing" and here it means
    /// "that one" — the block had no keyboard path at all before, only 55 mouse targets. And its box
    /// carries no [`crate::keys::ControlId`] of its own: it sits inside a block that already is one.
    Tags,
}

impl Pane {
    /// **Every pane**, so adding one costs a decision in [`Pane::walk`] and in the ratchet that
    /// reads it.
    pub const ALL: [Pane; 4] = [Pane::Palette, Pane::Candidates, Pane::Rigs, Pane::Tags];

    /// **The list walk this box's rows answer to**, or empty for the one pane that narrows a grid.
    ///
    /// Stated here because it is a fact about the pane rather than about the census, and stated at
    /// all because forgetting it is silent: `PrevRig`/`NextRig` are [`Pane::Rigs`]' walk and shipped
    /// without `keys::Draft::also_filtered`, so clicking the Rigs box killed the rig walk — the exact
    /// defect the exemption exists to close, missed on the fourth pane and caught by nothing.
    /// `every_filtered_pane_keeps_its_own_walk` reads this.
    pub const fn walk(self) -> &'static [crate::keys::Action] {
        use crate::keys::Action::*;
        match self {
            Pane::Palette => &[PalettePrev, PaletteNext],
            // **Four, because one node serves two tabs.** `CandidateList` is the Meshes candidates
            // and the Tiles kit walk, so the box above it narrows rows both pairs step through.
            Pane::Candidates => &[PrevCandidate, NextCandidate, TileListPrev, TileListNext],
            Pane::Rigs => &[PrevRig, NextRig],
            // The tag block is a grid of chips, not a list with a cursor: its `Enter` takes the one
            // match rather than committing a walk. See the variant's own note.
            Pane::Tags => &[],
        }
    }
}

/// The filter text of each list, and which box is taking keys.
///
/// **Its own resource, and deliberately not a field on `EditorState`.** Every keystroke here has to
/// rebuild the list it filters, so this resource is named in `editor::rebuild_palette`'s run
/// condition on purpose — where `SizeEdit`, `RemovalDrag` and `EditorState` itself are deliberately
/// kept out of it, because arming a brush must not respawn a hundred and eighty rows. The
/// difference is whether the rebuild is the point.
///
/// (This paragraph used to say `rebuild_palette` ran on `resource_changed::<EditorState>`. It never
/// did — the condition is `Project` / `Folded` / `Filters` / `ThumbGeneration` — and the claim was
/// load-bearing enough that `editor::arm_palette_rows` exists precisely because it is false: the
/// palette's selection has to be written onto rows that already exist.)
/// **The four texts, for "did what the list shows change?"** (#278).
///
/// Kept beside `Filters` rather than inside it so the answer is a comparison of *text*, which is what
/// the list is narrowed by, and not of the whole resource — `focus` is written by every click in and
/// out of a box, and a row click that blurs the box must not read as "the list changed".
#[derive(Resource, Default, PartialEq, Eq, Debug)]
pub struct FilterTexts(pub [String; 4]);

/// **What the lists are narrowed to**, in one value, so a change to it can be compared.
pub fn filter_texts(filters: &Filters) -> FilterTexts {
    FilterTexts([
        filters.palette.clone(),
        filters.candidates.clone(),
        filters.rigs.clone(),
        filters.tags.clone(),
    ])
}

#[derive(Resource, Default)]
pub struct Filters {
    palette: String,
    candidates: String,
    rigs: String,
    tags: String,
    /// The box that owns the keyboard, or `None`. Read by `editor::not_typing`.
    focus: Option<Pane>,
}

impl Filters {
    /// What this list is narrowed to. Empty means everything.
    pub fn text(&self, pane: Pane) -> &str {
        match pane {
            Pane::Palette => &self.palette,
            Pane::Candidates => &self.candidates,
            Pane::Rigs => &self.rigs,
            Pane::Tags => &self.tags,
        }
    }

    /// Which box owns the keyboard, if any — the read half of [`Filters::take_focus`], so a test
    /// can assert who is typing without the field being public to every writer in the crate.
    pub fn focus_pane(&self) -> Option<Pane> {
        self.focus
    }

    /// **The box that owns the keyboard, by the name an operation's view gives it** (E-002, #129 slice 25), or `None`.
    pub(crate) fn focus_name(&self) -> Option<String> {
        self.focus.map(|pane| crate::ops_view::pane_name(pane).to_owned())
    }

    /// **The box an operation's answer asks to start fresh**, or `None` when the answer asks for nothing.
    ///
    /// **`fresh` is the operation's own flag, not a comparison** (E-002, #129 review M1). It used to be "the box the
    /// view names, when it is not the one that has the keyboard", which made `filter_focus` unable to reach the box
    /// already focused: the answer said `applied`, the clear never ran, and a person who pressed `F` twice on a
    /// half-typed search kept the text. `take_focus` is what clears the search, so the flag is what says to call it.
    /// `None` when the answer names no box or does not ask for a fresh one: a write through a `ResMut` marks the
    /// filters changed, and every system gated on that redraws.
    pub(crate) fn box_to_clear(&self, named: Option<&str>, fresh: bool) -> Option<Pane> {
        if !fresh {
            return None;
        }
        crate::ops_view::pane_from_name(named?)
    }

    /// **Put the keyboard in this box**, and start the search fresh — the same two writes
    /// `on_click` makes, so the `F` key and the mouse leave the filter in one state rather than two.
    ///
    /// A method rather than a public field: `focus` is read by `editor::not_typing` to decide
    /// whether every other key in the editor fires, and a field anything could set is a field that
    /// gets set from somewhere that has not thought about that.
    pub fn take_focus(&mut self, pane: Pane) {
        self.text_mut(pane).clear();
        self.focus = Some(pane);
    }

    /// Type one character into a box, for a headless driver — the real path is `keys`, which reads
    /// the message stream and cannot be driven without an `App`. The integration tests reach it
    /// through the public crate, so it is not gated on `cfg(test)`.
    pub fn push_for_test(&mut self, pane: Pane, c: char) {
        self.text_mut(pane).push(c);
    }

    fn text_mut(&mut self, pane: Pane) -> &mut String {
        match pane {
            Pane::Palette => &mut self.palette,
            Pane::Candidates => &mut self.candidates,
            Pane::Rigs => &mut self.rigs,
            Pane::Tags => &mut self.tags,
        }
    }

    /// **Empty one box without giving the keyboard back** — the one thing `Escape` and `blur` do not
    /// between them.
    ///
    /// `tiles::take_the_one_match` needs exactly this: it has just taken the token that was typed, and
    /// the next token wants a clear box and the cursor still in it. A public method rather than
    /// widening [`Self::text_mut`], which is private because a filter anything can write is a filter
    /// that gets written from somewhere that has not thought about the focus.
    pub fn clear(&mut self, pane: Pane) {
        self.text_mut(pane).clear();
    }

    /// Is a filter box taking keys right now?
    pub fn typing(&self) -> bool {
        self.focus.is_some()
    }

    /// **Give the keyboard back, keeping what was typed.**
    ///
    /// The text surviving is the whole point of this being its own resource (see the module note):
    /// an author who filtered to `grate`, placed one, and wants a second should not have to type it
    /// again. Only the focus goes.
    pub fn blur(&mut self) {
        self.focus = None;
    }

    /// Does this id survive the list's filter?
    ///
    /// Case-insensitive substring, not a fuzzy match: an author filtering `crt` wants the three CRTs,
    /// and a fuzzy matcher would also hand back `concrete_wall` for the same three letters. A
    /// predictable filter is one you can trust to have shown you everything.
    pub fn keeps(&self, pane: Pane, id: &str) -> bool {
        let needle = self.text(pane);
        needle.is_empty()
            || id
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase())
    }
}

/// The clickable box itself.
#[derive(Component, Clone, Copy)]
pub struct FilterBox(pub Pane);

/// The text inside it.
#[derive(Component, Clone, Copy)]
pub struct FilterText(pub Pane);

/// **A filter box's ink, from the editor's skin** (UI-02, #210).
///
/// The box has four inks and one of them is a *state*: the ground it sits on
/// (`Ink::row_bg`), the ground it turns under the pointer (`Ink::row_hover`) and the ground it
/// holds while it owns the keyboard (`Ink::focus_bg`), which is the one field gooey calls
/// `pressed` — a recessed well, not a raised card. The caret's ink is the accent, and the
/// two text inks are `text` and `text_muted`.
///
/// **Each falls back to its `chrome.rs` constant**, as [`crate::chrome::RowFills`] does, so a
/// harness with no skin draws the shipped palette and no existing test re-pins.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FieldInk {
    /// A box at rest.
    rest: Color,
    /// A box under the pointer.
    hover: Color,
    /// A box that owns the keyboard — gooey's `pressed`, which this editor draws recessed.
    focus: Color,
    /// What is typed.
    typed: Color,
    /// A placeholder, and the `_` that stands in for a caret.
    quiet: Color,
    /// The caret itself.
    accent: Color,
}

impl FieldInk {
    fn of(skin: Option<&bevy_gooey::GooeySkin>) -> FieldInk {
        let Some(skin) = skin else {
            return FieldInk { rest: crate::chrome::ink().row_bg, hover: crate::chrome::ink().row_hover, focus: crate::chrome::ink().focus_bg, typed: crate::chrome::ink().text, quiet: crate::chrome::ink().dim, accent: crate::chrome::ink().accent };
        };
        FieldInk {
            rest: skin.surface_raised,
            hover: skin.tone("hover").unwrap_or(crate::chrome::ink().row_hover),
            focus: skin.tone("pressed").unwrap_or(crate::chrome::ink().focus_bg),
            typed: skin.text,
            quiet: skin.text_muted,
            accent: skin.accent,
        }
    }
}

/// Spawn a filter box above a list.
///
/// **`chrome::text_field`, where this hand-rolled the same box.** It was one of the five modules
/// the 2026-09-03 audit found using no shared builder at all (F9), and what it had built by hand
/// was a text field: a box that owns the keyboard, with a readout inside it. Adopting the builder
/// costs the `CHIP_PAD` inset and buys `FIELD_PAD`, a [`crate::chrome::MIN_FIELD_H`] floor (an
/// unstated field height lays out at 7 px while empty) and the same shape as the four other fields
/// in the editor — which is the whole of D4.
pub fn spawn(parent: &mut ChildSpawnerCommands, pane: Pane, skin: Option<&bevy_gooey::GooeySkin>) {
    let ink = FieldInk::of(skin);
    let mut b = crate::chrome::text_field(
        parent,
        Val::Percent(100.0),
        FilterBox(pane),
        crate::chrome::text::BODY,
        (placeholder(pane).to_owned(), ink.quiet),
        FilterText(pane),
    );
    // **Which census control this box *is*, and the one pane that is not one.**
    //
    // One `ControlId::Filter` covers the three list boxes: they live in different tabs' panels, so
    // exactly one is ever laid out and the badge lands on whichever list is on screen. The tag box
    // breaks that arithmetic — it is in the mesh detail pane, which is on screen at the same moment
    // as the candidate list beside it, so two boxes would both claim `Filter` and the badge system
    // would have two answers to one question. It sits inside a block that is already
    // `ControlId::Tags`, and that is where `/`'s badge belongs anyway: one row above the box, on the
    // thing the key narrows.
    if pane != Pane::Tags {
        b.insert(crate::chrome::Control(crate::keys::ControlId::Filter));
    }
}

/// **What an empty box says it will narrow.**
///
/// Three of the four sit directly above the list they filter, where "filter" is unambiguous because
/// there is nothing else it could mean. The tag box does not: it is in the mesh detail pane, one row
/// under `mount`/`front`, **at the same moment** as the candidate list's own box across the screen —
/// two identical words, two different jobs. Measured in a frame rather than argued about: the first
/// capture after this box existed showed both, and neither said which was which.
fn placeholder(pane: Pane) -> &'static str {
    match pane {
        Pane::Tags => "filter tags",
        _ => "filter",
    }
}

/// Click to focus. Clicking the box that is already focused clears it, which is the fastest way back
/// to the whole list and needs no second control.
pub fn on_click(
    activate: On<Activate>,
    boxes: Query<&FilterBox>,
    project: Option<Res<crate::project::Project>>,
    mut filters: ResMut<Filters>,
) {
    let Ok(b) = boxes.get(activate.entity) else {
        return;
    };
    // **A click always starts a fresh search, with the cursor in the box.** Clicking a focused box
    // used to clear it *and* blur it, so narrowing a list twice meant click, type, click, click, type
    // — the second click looked like it had done nothing. Enter and Escape still both leave, so there
    // is no way to get stuck in a box you did not mean to open.
    //
    // **Through the operation the keys and an agent use** (E-002, #129 review): the click wrote the two fields itself,
    // a second path to the state `filter_focus` decides. A door with no project keeps the direct write, because there is
    // no operation to carry — the filters are the editor's own and the box is still the one clicked.
    let Some(project) = project else {
        filters.take_focus(b.0);
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        view: Some(crate::ops_view::View { filter_pane: filters.focus_name(), ..Default::default() }),
        ..crate::ops::Ctx::default()
    };
    let _ = crate::ops::dispatch(&mut ctx, "filter_focus", &serde_json::json!({ "pane": crate::ops_view::pane_name(b.0) }));
    if let Some(view) = ctx.view.take()
        && let Some(pane) = filters.box_to_clear(view.filter_pane.as_deref(), view.filter_fresh)
    {
        filters.take_focus(pane);
    }
}

/// Keystrokes into the focused box.
///
/// Ungated by `not_typing` — this system *is* the typing, so a guard against typing would stop it
/// from ever running. Same shape as `rename_keys` and `size_edit_keys`: read the buffered events,
/// ignore releases, match the logical key.
pub fn keys(mut events: MessageReader<KeyboardInput>, mut filters: ResMut<Filters>) {
    let Some(pane) = filters.focus else {
        // **Drain before leaving.** The box opens in `Phase::Act` and this system runs in
        // `Phase::Text` — so on the frame the box opens, this system has already run and the
        // opening keystroke is **still in the stream**. It was read on the next frame, when the box
        // was focused, and the author typed `Cmd+F` and got an `f` in the filter they had just
        // opened.
        //
        // The same rule every text field in this crate follows — `tiles::cell_keys`,
        // `editor::group_name_keys`, `editor::pin_reason_keys` all drain before leaving — and for
        // the same reason: the keystroke that opened a field must not become its first character.
        // The `xseam` bug, in a new place, because the filter was the one field that did not drain.
        events.clear();
        return;
    };
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            // Enter and Escape both stop typing; Escape also throws the filter away, so there is one
            // key that always gets you back to the whole list.
            //
            // **Except in the tag box, whose `Enter` belongs to the block below it.** For a list,
            // `Enter` means *done narrowing* — the rows are already on screen and the arrows walk
            // them. The tag block has no arrows and 55 targets, so there `Enter` means *take that
            // one*, and `tiles::take_the_one_match` is the single owner of it. Left alone here
            // rather than handled in both places: two systems reading the same keystroke and each
            // acting on it is the two-path failure this crate refuses everywhere else.
            //
            // `Escape` still leaves, from every box including this one, so there is no way to be
            // stuck in a filter.
            Key::Enter if pane == Pane::Tags => {}
            Key::Enter => filters.focus = None,
            Key::Escape => {
                filters.text_mut(pane).clear();
                filters.focus = None;
            }
            Key::Backspace => {
                filters.text_mut(pane).pop();
            }
            Key::Character(s) => {
                // No spaces: ids are snake_case and a space can only ever match nothing, so accepting
                // one would be accepting a keystroke that empties the list.
                let s = s.clone();
                if s.chars().all(|c| !c.is_whitespace()) {
                    filters.text_mut(pane).push_str(&s);
                }
            }
            _ => {}
        }
    }
}

/// Show what is typed, with a caret while focused and the placeholder when empty.
pub fn refresh(
    filters: Res<Filters>,
    mut texts: Query<(&FilterText, &mut Text, &mut TextColor)>,
    mut boxes: Query<(&FilterBox, &Hovered, &mut BackgroundColor)>,
    // **The box's ink, from the editor's skin** (UI-02, #210).
    skin: Option<Res<bevy_gooey::GooeySkin>>,
) {
    let ink = FieldInk::of(skin.as_deref());
    // The fill answers to the pointer as well as to focus, so it repaints every frame — hover
    // cannot wait for `Filters` to change. The `!=` guard keeps the steady state a comparison.
    // Focus beats hover; hover is `chrome::ink().row_hover`'s signifier that the box takes a click.
    for (which, hovered, mut bg) in &mut boxes {
        let want = if filters.focus == Some(which.0) {
            ink.focus
        } else if hovered.0 {
            ink.hover
        } else {
            ink.rest
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    // **Every frame, like the fill above** — and this used to be gated on `filters.is_changed()`.
    //
    // That gate was correct while every box outlived the keystrokes typed into it. The tag box does
    // not: `tiles::rebuild_detail` despawns and respawns the whole detail pane on each keystroke, so
    // the box is a **new entity** carrying a fresh placeholder, and by then the change this repaint
    // needed has already been spent. Found in a frame — the box read `filter` while `1 of 55` stood
    // under it, so the block knew what had been typed and the box denied it.
    //
    // Costs nothing in the steady state: both loops compare before writing, which is what
    // `a_drawing_system_writes_only_when_something_changed` is actually asking for.
    for (which, mut text, mut colour) in &mut texts {
        let focused = filters.focus == Some(which.0);
        let raw = filters.text(which.0);
        // **Compared without building the string.** The loop above earns its per-frame run by
        // comparing before writing, and this one used to spend a `format!` or a `to_owned` just to
        // have something to compare against — two allocations per box per frame, in the steady state
        // where nothing changed. The caret is one trailing character, so the wanted string is two
        // borrowed halves and the comparison is `strip_suffix`; `strip_suffix("")` returns the whole
        // string, so the two caretless arms reduce to a plain `!=`.
        let (head, tail, want_colour) = match (focused, raw.is_empty()) {
            (true, _) => (raw, "_", ink.accent),
            (false, true) => (placeholder(which.0), "", ink.quiet),
            (false, false) => (raw, "", ink.typed),
        };
        if text.0.strip_suffix(tail).is_none_or(|rest| rest != head) {
            text.0.clear();
            text.0.push_str(head);
            text.0.push_str(tail);
        }
        if colour.0 != want_colour {
            colour.0 = want_colour;
        }
    }
}

/// **A click in the world takes the keyboard back from a filter box.**
///
/// Enter and Escape were the only ways out, and nothing said so. Meanwhile `editor::place_on_click`
/// is gated on `not_typing`, which reads [`Filters::typing`] — so filtering the palette to find a
/// piece and then clicking to place it did **nothing at all, silently**. That is the failure this
/// repo's rules exist to prevent, and it was reachable by the most natural way to find one piece
/// among a hundred and eighty.
///
/// # Why `Phase::Sense`, and why the same click still places
///
/// [`crate::keys::Phase`] exists to decide who owns the keyboard once, before anything reads a key.
/// Running here — ahead of `editor::sense_context`, which computes `Live` from this very flag —
/// means the focus is already gone by the time `place_on_click` evaluates `not_typing` in
/// `Phase::Act`. So one click blurs and places, rather than the first click being eaten as a
/// dismissal and the author clicking again.
///
/// A click on a widget is left alone. It might be this box, another filter, or a palette row, and
/// none of those is "the author is done here" — `on_filter_click` already owns the first case.
pub fn blur_on_world_click(
    mouse: Res<ButtonInput<MouseButton>>,
    pointer: Res<crate::view::Pointer>,
    // **The boxes, not the world.** This asked *does anything carry `Hovered`* — true for the whole
    // window once the docks are hovered — so a click anywhere kept the keyboard, and the author had to
    // find the box to get out of it. The question this system actually has is narrower: did the click
    // land **outside every filter box**? A box's own rect answers it, and a panel elsewhere does not.
    boxes: Query<(&bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform), With<FilterBox>>,
    mut filters: ResMut<Filters>,
) {
    if !mouse.just_pressed(MouseButton::Left) || !filters.typing() {
        return;
    }
    let Some(at) = pointer.0 else {
        // No pointer is no click: leave the keyboard where it is.
        return;
    };
    let over_a_box = boxes
        .iter()
        .any(|(node, tf)| node.size() != Vec2::ZERO && node.contains_point(*tf, at));
    if over_a_box {
        return;
    }
    filters.blur();
}

#[cfg(test)]
mod blur_tests {
    use super::*;

    /// Blurring gives back the keyboard and keeps the search — the two halves of the module note.
    #[test]
    fn blurring_keeps_the_text_and_drops_the_focus() {
        let mut f = Filters { focus: Some(Pane::Palette), ..Default::default() };
        *f.text_mut(Pane::Palette) = "grate".to_owned();

        f.blur();

        assert!(!f.typing(), "the keyboard goes back to the editor");
        assert_eq!(
            f.text(Pane::Palette),
            "grate",
            "the search survives, so placing a second one needs no retyping"
        );
    }
}
