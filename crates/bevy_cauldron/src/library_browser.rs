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
//! A LIBRARY section at the top of the Map door's INSPECTOR, shown by View ▸ Show library. **Not there until
//! it is asked for**, for the reason the outliner records: a text box and a grid of cards are entities, and
//! the screens that count every button or look for "the" empty text input in the world (the settings search,
//! the dialogs' answers) would find them.
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

use crate::editor::{MapControlsPane, PieceDrag};
use crate::project::Project;
use crate::screen::Screen;

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

/// FNV-1a over `text`, which is stable across runs and platforms (a `HashMap`'s hasher is not).
fn fnv(text: &str) -> u64 {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    text.bytes().fold(BASIS, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME))
}

/// **View ▸ Show library puts the section into the Map door's INSPECTOR, first among its rows, and takes it
/// out again.** Not there until it is asked for, for the reason the module docs give.
pub fn toggle_the_library(
    change: On<ValueChange<bool>>,
    panes: Query<Entity, With<MapControlsPane>>,
    blocks: Query<Entity, With<LibraryBrowser>>,
    mut commands: Commands,
) {
    if !change.value {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    }
    if !blocks.is_empty() {
        return;
    }
    let Ok(pane) = panes.single() else { return };
    let block = commands
        .spawn((
            LibraryBrowser::default(),
            Name::new("library_browser"),
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() },
        ))
        .with_children(|block| {
            crate::chrome::list_heading(block, "LIBRARY");
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
            block.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(crate::chrome::GAP_TIGHT),
                    ..default()
                },
                LibraryTags,
            ));
            block.spawn((Node { ..default() }, LibraryGridMarker));
        })
        .id();
    commands.entity(pane).insert_children(0, &[block]);
}

/// **Where the grid goes**, so a redraw can put one in the same place without knowing the section's shape.
#[derive(Component)]
struct LibraryGridMarker;

/// **The tick on the menu row says whether the section is there**, whichever way it came or went (a door
/// change respawns the panel and takes it with it).
fn tick_the_menu_row(
    items: Query<(Entity, Has<bevy_gooey::Checked>), With<LibraryItem>>,
    blocks: Query<(), With<LibraryBrowser>>,
    mut commands: Commands,
) {
    let present = !blocks.is_empty();
    for (item, ticked) in &items {
        match (present, ticked) {
            (true, false) => {
                commands.entity(item).insert(bevy_gooey::Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<bevy_gooey::Checked>();
            }
            _ => {}
        }
    }
}

/// The fingerprint of everything the cards are drawn from: the search, the kept tags and every piece's
/// identity and kinds.
fn fingerprint(browser: &LibraryBrowser, project: &Project) -> u64 {
    let mut text = String::with_capacity(browser.query.len() + project.library.descriptors.len() * 24);
    text.push_str(&browser.query);
    for tag in &browser.kept {
        text.push('\u{1e}');
        text.push_str(tag);
    }
    for descriptor in &project.library.descriptors {
        text.push('\u{1f}');
        text.push_str(&descriptor.id);
        text.push('\u{1d}');
        text.push_str(&descriptor.kind.join(" "));
    }
    fnv(&text)
}

/// **Every `kind` the library uses**, in name order: the tags the row offers.
fn kinds_of(project: &Project) -> Vec<String> {
    let mut kinds: BTreeSet<String> = BTreeSet::new();
    for descriptor in &project.library.descriptors {
        kinds.extend(descriptor.kind.iter().cloned());
    }
    kinds.into_iter().collect()
}

/// **The cards and the tags are drawn when the library, the search or the tags moved.**
///
/// gooey's rule is that a `card_grid`'s cards are spawned once and the grid is respawned to change them, so
/// a redraw is a despawn of the old grid and a new one where it stood — never a pool written in place.
#[allow(clippy::type_complexity)]
fn draw_the_cards(
    project: Option<Res<Project>>,
    mut browsers: Query<&mut LibraryBrowser>,
    grids: Query<Entity, With<LibraryGrid>>,
    markers: Query<Entity, With<LibraryGridMarker>>,
    tags: Query<(Entity, Option<&Children>), With<LibraryTags>>,
    thumbs: Option<Res<crate::thumbs::Thumbnails>>,
    mut commands: Commands,
) {
    let Some(project) = project else { return };
    let Ok(mut browser) = browsers.single_mut() else { return };
    let now = fingerprint(&browser, &project);
    if browser.built_from == now {
        return;
    }
    browser.built_from = now;
    browser.draws += 1;

    // **The tags**: one gooey toggle per `kind`, on when it narrows the grid. A toggle keeps its own state
    // in `StartsOn`, and the observer below reads the change back into the browser.
    let kinds = kinds_of(&project);
    for (row, children) in &tags {
        for child in children.into_iter().flat_map(|kids| kids.iter()) {
            commands.entity(child).despawn();
        }
        commands.entity(row).with_children(|row| {
            for kind in &kinds {
                row.spawn((bevy_gooey::toggle(kind.clone(), browser.kept.contains(kind)), LibraryTag(kind.clone())));
            }
        });
    }

    // **The cards.**
    let kept: Vec<&bevy_grimoire_core::descriptor::Descriptor> = project
        .library
        .descriptors
        .iter()
        .filter(|d| browser.keeps(&d.id, &d.kind))
        .collect();
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
    for grid in &grids {
        commands.entity(grid).despawn();
    }
    // **A redrawn grid goes where the one before it stood**; the first draw, in `toggle_the_library`, has
    // already put it there.
    for marker in &markers {
        commands.entity(marker).with_children(|marker| {
            marker.spawn(bevy_gooey::card_grid(cards.clone()));
        });
    }
}

/// **Each card carries its piece**, as the payload a drop on the map reads: the same `PieceDrag` the
/// palette's rows carry, so the drop is the palette's drop with nothing added.
///
/// gooey's card bundle has no room for a payload the crate knows nothing about, so this puts one on the
/// cards that do not have it yet. The filter is an archetype test: it costs nothing once every card has one.
fn name_the_payloads_on_cards(
    browsers: Query<&LibraryBrowser>,
    unmarked: Query<(Entity, &CardRef), Without<bevy_gooey::DragPayload<PieceDrag>>>,
    mut commands: Commands,
) {
    let Ok(browser) = browsers.single() else { return };
    for (card, at) in &unmarked {
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
    browsers: Query<&LibraryBrowser>,
    project: Option<Res<Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    state: Option<ResMut<crate::editor::EditorState>>,
) {
    if !chosen.is_final {
        return;
    }
    // The event is on the grid, so a grid with no card chosen (`None`) is not a choice.
    let Ok(browser) = browsers.single() else { return };
    let (Some(project), Some(mut state)) = (project, state) else { return };
    let Some(id) = browser.descriptor(chosen.value) else { return };
    crate::editor::arm_piece(&mut state, &project, open.as_deref(), id);
}

/// **Keep the tag `kind` for a test**, through the same state a toggle writes.
pub fn keep_tag_for_test(world: &mut World, kind: &str, kept: bool) {
    let mut browsers = world.query::<&mut LibraryBrowser>();
    for mut browser in browsers.iter_mut(world) {
        browser.keep(kind, kept);
    }
}
