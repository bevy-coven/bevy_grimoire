//! **The Game UI preview panel** (UI-03, #211).
//!
//! A game ships `family/bevy_gooey.yaml` so its own UI matches its art. This panel is the one place the
//! editor shows that palette, and it shows it the way `bevy_gooey` means a skin to be shown: a
//! [`SkinScope`] on this subtree, so the widgets below it are drawn from the game's skin while the rest
//! of the window keeps the editor's own.
//!
//! # Why the chrome never takes it
//!
//! The editor is not the game. A game's light skin put under the editor's chrome would be unreadable
//! (the editor's own ink is measured for its own darks — UI-02, #210), and a game's palette is a fact
//! about the game. So the project's file lands in [`crate::ui_skin::GameSkin`], a resource of its own,
//! and this panel is the only reader that puts it on the screen.
//!
//! # Not there until it is asked for
//!
//! View ▸ Game UI preview spawns the panel, and asking again takes it away — the pattern the outliner
//! and the library browser use. It is spawned on demand rather than kept hidden because the editor's
//! own screens count every button in the world and take the first empty text field as "the" search box;
//! a hidden panel of gooey widgets would be found by them.
//!
//! # What it shows
//!
//! A small screen of the widgets a game actually has — a heading, body words, a primary button, a plain
//! button, a toggle, a meter — drawn under the project's skin. Editing the project's `accent` moves the
//! primary button's fill and its label's ink, and no chrome button anywhere, which is exactly what
//! `tests/a_project_skin_previews_only_in_the_panel.rs` asserts.

use bevy::prelude::*;
use bevy_gooey::{button, heading, label, meter, primary_button, toggle, ValueChange};

use crate::editor::MapControlsPane;
use crate::screen::Screen;
use crate::ui_skin::{GameSkin, ProjectSkinScope};

/// **The Game UI preview panel**, marked so the menu row can find it and the scope system can reach it.
///
/// It carries [`ProjectSkinScope`], which is what puts the project's [`bevy_gooey::SkinScope`] on it.
#[derive(Component)]
pub struct GameUiPreview;

/// The panel's body: the words above the sample widgets, so a person reads which skin they are seeing.
#[derive(Component)]
pub struct GameUiPreviewNote;

/// The View ▸ Game UI preview row.
#[derive(Component)]
pub struct GameUiPreviewItem;

/// The primary button in the sample screen, so a test can read the fill it is drawn with.
#[derive(Component)]
pub struct PreviewPrimary;

/// A plain button in the sample screen.
#[derive(Component)]
pub struct PreviewPlain;

/// The toggle in the sample screen.
#[derive(Component)]
pub struct PreviewToggle;

pub struct GameUiPreviewPanelPlugin;

impl Plugin for GameUiPreviewPanelPlugin {
    fn build(&self, app: &mut App) {
        // **No global `ValueChange` observer here.** The menu row carries the press observer on
        // itself (`observe(toggle_the_preview)` in `menu_bar`), the way the outliner's and the library
        // browser's rows do; an `add_observer` beside it would answer every `ValueChange` in the
        // editor and spawn a second panel for one press.
        app.add_systems(Update, (tick_the_menu_row, note_the_skin).chain().run_if(in_state(Screen::Editor)));
    }
}

/// **View ▸ Game UI preview puts the panel into the Map door's INSPECTOR and takes it out again.**
///
/// Spawned on demand, for the reason the module docs give. The panel is inserted first among the
/// pane's rows so it stands where a person asked for it rather than at the foot of a long list.
pub fn toggle_the_preview(
    change: On<ValueChange<bool>>,
    panes: Query<Entity, With<MapControlsPane>>,
    panels: Query<Entity, With<GameUiPreview>>,
    mut commands: Commands,
) {
    if !change.value {
        for panel in &panels {
            commands.entity(panel).despawn();
        }
        return;
    }
    if !panels.is_empty() {
        return;
    }
    let Ok(pane) = panes.single() else { return };
    let panel = commands
        .spawn((
            GameUiPreview,
            // **The scope's home.** `ui_skin::skin_scope` puts the project's skin here the frame the
            // file lands, and takes it off when the project stops shipping one.
            ProjectSkinScope,
            Name::new("game_ui_preview"),
            bevy_gooey::column(crate::chrome::GAP_TIGHT_STEPS),
        ))
        .with_children(|panel| {
            crate::chrome::list_heading(panel, "GAME UI PREVIEW");
            panel.spawn((
                GameUiPreviewNote,
                Text::new(""),
                crate::chrome::typed(crate::chrome::text::HINT),
                TextColor(crate::chrome::ink().label),
            ));
            // **What a game's screen is made of**, on gooey's own widgets: a heading, body words, the
            // one main action, an ordinary button, a switch and a meter. Each takes its colour from the
            // skin it stands under — the project's, here — so an edit to `accent` moves the primary
            // button and nothing in the editor's chrome.
            // A gooey panel: its padding, gap, frame and radius are the skin's (the raw node's own 12 px and 3 px were overwritten by the skin's panel).
            panel.spawn(bevy_gooey::panel())
            .with_children(|sample| {
                sample.spawn(heading("A game's own screen"));
                sample.spawn(label("Words a player reads, in the game's own ink."));
                sample.spawn((primary_button("Play"), PreviewPrimary));
                sample.spawn((button("Settings"), PreviewPlain));
                sample.spawn((toggle("Sound", true), PreviewToggle));
                sample.spawn(meter(0.6, 1.0));
            });
        })
        .id();
    commands.entity(pane).insert_children(0, &[panel]);
}

/// **The panel's own line says which skin it is drawing** — the project's, or the editor's where the
/// project ships none. A preview that did not say so would leave a person guessing whether the palette
/// on screen is their game's or the editor's.
fn note_the_skin(game: Res<GameSkin>, mut notes: Query<&mut Text, With<GameUiPreviewNote>>) {
    let text = if game.0.is_some() {
        "Drawn from this project's family/bevy_gooey.yaml. The editor's own panels keep their palette."
    } else {
        "This project ships no family/bevy_gooey.yaml, so this is the editor's own palette. Edit a game skin's accent and the buttons here follow it."
    };
    for mut note in &mut notes {
        if note.0 != text {
            note.0 = text.to_owned();
        }
    }
}

/// **The tick on the menu row says whether the panel is there**, whichever way it came or went (a door
/// change respawns the pane and takes the panel with it).
fn tick_the_menu_row(
    items: Query<(Entity, Has<bevy_gooey::Checked>), With<GameUiPreviewItem>>,
    panels: Query<(), With<GameUiPreview>>,
    mut commands: Commands,
) {
    let present = !panels.is_empty();
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
