//! **The LIBRARY browser: every piece the project has, as a grid of portraits you can search, filter by tag
//! and drag onto the map** (UI-52, #238).
//!
//! The issue's What is "a grid of piece thumbnails with search and tag filters. Dragging a piece onto the
//! viewport places it." The PLACE palette is a *list* of names with 30 px thumbnails, which is the right
//! shape for "the piece I always use" and the wrong shape for "the piece that looks like this": this is the
//! second view of the same library, drawn with gooey's `card_grid` — a portrait above a name, walked by the
//! arrows, one tab stop — because a portrait is the thing being chosen.
//!
//! - **Its own search box.** The palette's filter is `filter::Pane::Palette` and narrows the list; the
//!   browser's search is its own, so narrowing one does not narrow the other. Every word typed must appear
//!   in a piece's id or one of its `kind` tags, case aside.
//! - **A tag row narrows by `kind`.** The library's own kind vocabulary as gooey `toggle`s, so "only the
//!   lights" is one press. The search and the tags fold together: a card is drawn only when its piece
//!   passes both.
//! - **A card is a drag source carrying `editor::PieceDrag`**, the same payload the palette's rows carry, so
//!   a card let go over the viewport is placed by the same code — `place_one`, the click's own commit, one
//!   undo step. Nothing here places anything itself.
//! - **Choosing a card arms the brush**, through `editor::arm_piece` — the same operation a palette row's
//!   click dispatches — so a card is usable with the mouse alone.
//!
//! # Where it is
//!
//! The Kit workspace's LIBRARY panel (NV-08, #385), in the right dock, shown by View ▸ Show library. **It was a section of the Map's
//! INSPECTOR, and that route is gone**: the Map holds no library browser, and the row is offered only while the Kit workspace is open
//! (it is disabled, with its reason, anywhere else), so the library is reached one way. **Not there until it is asked for**, for the
//! reason the outliner records: a text box and a grid of cards are entities, and the screens that count every button or look for "the"
//! empty text input in the world (the settings search, the dialogs' answers, the Kit's own screens) would find them.
//!
//! # The grid is respawned, and that is gooey's rule
//!
//! `card_grid` spawns a card per entry and says so: "the cards are spawned once; respawn the grid to change
//! them". So a redraw despawns the grid and spawns another where it stood, and the payload each card carries
//! is put on it by [`name_the_payloads_on_cards`] right after — one system, because a card's own bundle has
//! no room for a payload the crate knows nothing about.

use std::collections::BTreeSet;

use bevy::prelude::*;
use bevy_gooey::{Card, CardRef, ValueChange, observe, text_input};

use crate::chrome::{self, Frame, Side};
use crate::editor::PieceDrag;
use crate::project::Project;
use crate::screen::Screen;
use crate::workspace::Workspace;

/// **The browser's state**, on its section: the search, what it keeps, and the cards it drew.
#[derive(Component, Default)]
pub struct LibraryBrowser {
    /// What the search box holds.
    pub query: String,
    /// The `kind` tags kept. Empty means no tag narrows anything.
    kept: BTreeSet<String>,
    /// The descriptor each card's index stands for, in the order the cards were drawn.
    cards: Vec<String>,
    /// A fingerprint of what the cards were last drawn from (the search, the tags and every descriptor's
    /// id and kinds), so a settled frame draws nothing and an edit that changes no piece is no redraw.
    built_from: u64,
    /// The `kind`s the tag row was last built from, so a redraw does not replace the toggle a person just pressed.
    drawn_tags: Vec<String>,
    /// The search and the kept tags the cards were last drawn for, so a settled frame is one string comparison and
    /// **does not walk the library**.
    drawn_for: (String, Vec<String>),
    /// How many times the library has been walked to decide whether to draw: the cost that grows with the library,
    /// and the number the budget is written against. A settled frame adds none.
    looks: u32,
    /// How many times the cards have been drawn, for the UI budget (#259): a settled frame costs none.
    draws: u32,
}

impl LibraryBrowser {
    /// The descriptor the card at `index` stands for.
    pub fn descriptor(&self, index: usize) -> Option<&str> {
        self.cards.get(index).map(String::as_str)
    }

    /// How many cards are drawn.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether nothing passes the search and the tags.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// The tags this browser keeps, in name order.
    pub fn kept(&self) -> Vec<String> {
        self.kept.iter().cloned().collect()
    }

    /// How many times the cards have been drawn since the section opened.
    pub fn draws(&self) -> u32 {
        self.draws
    }

    /// **How many times the library has been walked to decide whether to draw**, since the section opened. A settled
    /// frame adds none: what moves the cards is compared as two strings first.
    pub fn looks(&self) -> u32 {
        self.looks
    }

    /// **Whether a piece belongs in the browser**: every word of the search is in its id or a `kind` tag,
    /// and it carries every kept tag.
    pub fn keeps(&self, id: &str, kinds: &[String]) -> bool {
        let words: Vec<String> = self.query.split_whitespace().map(str::to_lowercase).collect();
        let haystack = format!("{id} {}", kinds.join(" ")).to_lowercase();
        words.iter().all(|word| haystack.contains(word.as_str()))
            && self.kept.iter().all(|tag| kinds.iter().any(|kind| kind == tag))
    }

    /// **Keep the tag `kind`, or stop keeping it**, which narrows the grid or widens it.
    fn keep(&mut self, kind: &str, kept: bool) {
        if kept {
            self.kept.insert(kind.to_owned());
        } else {
            self.kept.remove(kind);
        }
    }
}

/// The grid of cards.
#[derive(Component)]
pub struct LibraryGrid;

/// The search box.
#[derive(Component)]
pub struct LibrarySearch;

/// The row of tag toggles.
#[derive(Component)]
pub struct LibraryTags;

/// One tag's toggle: the `kind` it narrows by.
#[derive(Component)]
pub struct LibraryTag(pub String);

/// The View ▸ Show library row.
#[derive(Component)]
pub struct LibraryItem;

/// The root of the Kit workspace's library panel, so a test and the layout can name it.
#[derive(Component)]
pub struct LibraryRoot;

pub struct LibraryBrowserPlugin;

impl Plugin for LibraryBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_the_cards, name_the_payloads_on_cards, tick_the_menu_row).chain().run_if(in_state(Screen::Editor)),
        );
        // **A card chosen arms its piece**, through the operation a palette row's click dispatches.
        app.add_observer(choose_from_a_card);
        // **A tag toggle narrows the grid.**
        app.add_observer(toggle_a_tag);
    }
}

/// **View ▸ Show library puts the LIBRARY panel into the Kit workspace's right dock, and takes it out again** (NV-08, #385). Not there
/// until it is asked for, for the reason the module docs give. Asked outside the Kit workspace it does nothing: the row is disabled there
/// ([`tick_the_menu_row`]), and this is the second guard for a toggle sent by something that is not the menu.
pub fn toggle_the_library(
    change: On<ValueChange<bool>>,
    workspace: Option<Res<Workspace>>,
    frame: Option<Res<Frame>>,
    blocks: Query<Entity, With<LibraryRoot>>,
    mut commands: Commands,
) {
    if !change.value {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    }
    if !blocks.is_empty() || !workspace.is_some_and(|w| *w == Workspace::Kit) {
        return;
    }
    let Some(frame) = frame else { return };
    let root = chrome::panel_root(&mut commands, &frame, Side::Right, chrome::LIST_W, true, false, "LIBRARY").insert(LibraryRoot).id();
    commands.entity(root).with_children(|p| {
        p.spawn((
            LibraryBrowser::default(),
            Name::new("library_browser"),
            bevy_gooey::column(chrome::GAP_TIGHT_STEPS),
        ))
        .with_children(|block| {
            chrome::list_heading(block, "LIBRARY");
            block.spawn((
                text_input("Find a piece", ""),
                LibrarySearch,
                observe(|typed: On<ValueChange<String>>, mut browsers: Query<&mut LibraryBrowser>| {
                    if let Ok(mut browser) = browsers.single_mut()
                        && browser.query != typed.value
                    {
                        browser.query = typed.value.clone();
                    }
                }),
            ));
            // **The tag row is filled by `draw_the_tags`**, from the library's own vocabulary: a project
            // with no `kind` tags has no row, and one that grows a tag gets a toggle for it.
            chrome::wrapping_row(block, chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch).insert(LibraryTags);
            block.spawn((bevy_gooey::row(0.0), LibraryGridMarker));
        });
    });
}

/// **Where the grid goes**, so a redraw can put one in the same place without knowing the section's shape.
#[derive(Component)]
struct LibraryGridMarker;

/// **The tick on the menu row says whether the panel is there**, whichever way it came or went (a workspace change respawns the frame and
/// takes it with it), and **the row is offered only in the Kit workspace**: anywhere else it is disabled and says why.
fn tick_the_menu_row(
    items: Query<(Entity, Has<bevy_gooey::Checked>, Has<bevy_gooey::Disabled>), With<LibraryItem>>,
    blocks: Query<(), With<LibraryRoot>>,
    workspace: Option<Res<Workspace>>,
    mut commands: Commands,
) {
    let present = !blocks.is_empty();
    let offered = workspace.is_some_and(|w| *w == Workspace::Kit);
    for (item, ticked, refused) in &items {
        match (present, ticked) {
            (true, false) => {
                commands.entity(item).insert(bevy_gooey::Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<bevy_gooey::Checked>();
            }
            _ => {}
        }
        match (offered, refused) {
            (false, false) => {
                commands.entity(item).insert(bevy_gooey::Disabled::new(NOT_OFFERED));
            }
            (true, true) => {
                commands.entity(item).remove::<bevy_gooey::Disabled>();
            }
            _ => {}
        }
    }
}

/// What the View ▸ Show library row says outside the Kit workspace, where it is disabled.
pub const NOT_OFFERED: &str = "The library browser is in the Kit workspace";

/// **Whether the cards need drawing again**, as a fingerprint of the search, the kept tags and every descriptor's
/// identity and kinds.
///
/// **Hashed as it is read, with no string built** (the rule `outliner::fingerprint` records): the first version
/// built a `String` of about 24 bytes per descriptor plus a `join` per descriptor on every frame, which for a
/// library of hundreds is kilobytes allocated per frame to decide nothing.
fn fingerprint(browser: &LibraryBrowser, project: &Project) -> u64 {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    fn fold(mut hash: u64, bytes: &[u8]) -> u64 {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(PRIME);
        }
        hash
    }
    let mut hash = fold(BASIS, browser.query.as_bytes());
    for tag in &browser.kept {
        hash = fold(hash, b"\x1e");
        hash = fold(hash, tag.as_bytes());
    }
    for descriptor in &project.library.descriptors {
        hash = fold(hash, b"\x1f");
        hash = fold(hash, descriptor.id.as_bytes());
        for kind in &descriptor.kind {
            hash = fold(hash, b"\x1d");
            hash = fold(hash, kind.as_bytes());
        }
    }
    hash
}

/// **Every `kind` the library uses**, in name order: the tags the row offers.
fn kinds_of(project: &Project) -> Vec<String> {
    let mut kinds: BTreeSet<String> = BTreeSet::new();
    for descriptor in &project.library.descriptors {
        kinds.extend(descriptor.kind.iter().cloned());
    }
    kinds.into_iter().collect()
}

/// **The cards and the tags are drawn when the library, the search or the tags moved** (UI-52, #238).
///
/// gooey's rule is that a `card_grid`'s cards are spawned once and the grid is respawned to change them, so a
/// redraw despawns the grid it drew last time and spawns a new one **in the same place** — hence `LibraryGrid` on
/// the grid itself: a redraw that only despawned what carried the marker would leave every earlier grid standing.
///
/// **The tag toggles are rebuilt only when the vocabulary changes**, not on every redraw: a toggle is the control
/// the person just pressed, so replacing it on the press that narrowed the grid would take the keyboard off it.
#[allow(clippy::too_many_arguments)]
fn draw_the_cards(
    project: Option<Res<Project>>,
    mut browsers: Query<&mut LibraryBrowser>,
    grids: Query<Entity, With<LibraryGrid>>,
    markers: Query<Entity, With<LibraryGridMarker>>,
    tags: Query<(Entity, Option<&Children>), With<LibraryTags>>,
    thumbs: Option<Res<crate::thumbs::Thumbnails>>,
    mut budget: ResMut<crate::ui_budget::UiFrame>,
    mut commands: Commands,
) {
    // **This system's cost, into the frame's total** (UI-93, #259): a library of hundreds of cards is content, and
    // the budget's whole point is that a settled frame pays nothing for it.
    let spent = crate::ui_budget::measure(&mut budget);
    let Some(project) = project else { return };
    let Ok(mut browser) = browsers.single_mut() else { return };
    // **A settled frame is two comparisons and nothing that grows with the library.** What a person can change
    // without touching the library — the search and the tags — is compared as it stands; a library edit marks
    // `Project` changed. Only then is the library walked, by the fingerprint, to decide the same question exactly.
    let request = (browser.query.clone(), browser.kept());
    let moved = project.is_changed() || browser.drawn_for != request || browser.built_from == 0;
    if !moved {
        return;
    }
    browser.looks += 1;
    let now = fingerprint(&browser, &project);
    if browser.built_from == now {
        browser.drawn_for = request;
        return;
    }
    browser.drawn_for = request;
    let _rebuild = spent.rebuilding();
    browser.built_from = now;
    browser.draws += 1;

    // **The tags**, one gooey toggle per `kind`, on when it narrows the grid. Rebuilt only when the vocabulary
    // itself changed, so the toggle a person pressed keeps its focus and its own state.
    let kinds = kinds_of(&project);
    for (row, children) in &tags {
        if browser.drawn_tags != kinds {
            for child in children.into_iter().flat_map(|kids| kids.iter()) {
                commands.entity(child).despawn();
            }
            let kept = browser.kept.clone();
            commands.entity(row).with_children(|row| {
                for kind in &kinds {
                    row.spawn((bevy_gooey::toggle(kind.clone(), kept.contains(kind)), LibraryTag(kind.clone())));
                }
            });
            browser.drawn_tags = kinds.clone();
        }
    }

    // **The cards.**
    let kept: Vec<&bevy_grimoire_core::descriptor::Descriptor> =
        project.library.descriptors.iter().filter(|d| browser.keeps(&d.id, &d.kind)).collect();
    browser.cards = kept.iter().map(|d| d.id.clone()).collect();
    let cards: Vec<Card> = kept
        .iter()
        .map(|d| {
            let mut card = Card::new(bevy_grimoire_core::naming::display_name(&d.id)).line(d.id.clone());
            if let Some(image) = thumbs.as_deref().and_then(|t| t.image(&d.id)) {
                card = card.image(image);
            }
            card
        })
        .collect();
    // **The grid this drew last time goes**, by its own marker: a grid names `LibraryGrid`, so exactly one is ever
    // in the section however many redraws have happened.
    for grid in &grids {
        commands.entity(grid).despawn();
    }
    for marker in &markers {
        commands.entity(marker).with_children(|marker| {
            marker.spawn((bevy_gooey::card_grid(cards.clone()), LibraryGrid));
        });
    }
}

/// **Each card of the library's grid carries its piece**, as the payload a drop on the map reads: the same
/// `PieceDrag` the palette's rows carry, so the drop is the palette's drop with nothing added.
///
/// **Only this grid's cards.** Every `CardRef` in the editor belongs to some `card_grid` (the launcher's project
/// list has cards), and a payload on one of those would let a project card be dropped on the map as a piece.
fn name_the_payloads_on_cards(
    browsers: Query<&LibraryBrowser>,
    grids: Query<Entity, With<LibraryGrid>>,
    unmarked: Query<(Entity, &CardRef), Without<bevy_gooey::DragPayload<PieceDrag>>>,
    mut commands: Commands,
) {
    let Ok(browser) = browsers.single() else { return };
    let Ok(grid) = grids.single() else { return };
    for (card, at) in &unmarked {
        if at.grid != grid {
            continue;
        }
        if let Some(id) = browser.descriptor(at.index) {
            commands.entity(card).try_insert(bevy_gooey::DragPayload(PieceDrag(id.to_owned())));
        }
    }
}

/// **A tag toggle narrows the grid, or stops narrowing it.**
fn toggle_a_tag(
    change: On<ValueChange<bool>>,
    tags: Query<&LibraryTag>,
    mut browsers: Query<&mut LibraryBrowser>,
) {
    let Ok(tag) = tags.get(change.source) else { return };
    if let Ok(mut browser) = browsers.single_mut() {
        browser.keep(&tag.0, change.value);
    }
}

/// **A card chosen arms its piece**, through `editor::arm_piece` — the operation a palette row's click
/// dispatches — so the card and the row hold the same piece and write the same log line.
fn choose_from_a_card(
    chosen: On<bevy_gooey::ValueChange<usize>>,
    grids: Query<(), With<LibraryGrid>>,
    browsers: Query<&LibraryBrowser>,
    project: Option<Res<Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    state: Option<ResMut<crate::editor::EditorState>>,
) {
    // **Only the library's own grid.** This is a global observer, so it hears every `ValueChange<usize>` in the
    // editor — a toolbar's toggle group, a settings choice, the keymap — and none of those is a card.
    // **Only the library's own grid.** This is a global observer, so it hears every `ValueChange<usize>` in the
    // editor — a toolbar's toggle group, a settings choice, the keymap — and none of those is a card.
    if !chosen.is_final || grids.get(chosen.source).is_err() {
        return;
    }
    // The event is on the grid, so a grid with no card chosen (`None`) is not a choice.
    let Ok(browser) = browsers.single() else { return };
    let (Some(project), Some(mut state)) = (project, state) else { return };
    let Some(id) = browser.descriptor(chosen.value) else { return };
    crate::editor::arm_piece(&mut state, &project, open.as_deref(), id);
}
