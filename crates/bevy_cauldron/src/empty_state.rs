//! **An empty map says what to do next** (UI-81, #255).
//!
//! A new map is a black rectangle with a grid in it, and the only thing on screen that says what to do
//! with it is the PLACE list in the other dock, which a person who has not found it yet is not looking
//! at. `docs/ui/vision.md` §4 asks that an empty state "says what to do next and offers the action":
//! one sentence and one primary button, over the viewport, there while the map has no placements and
//! gone the frame it has one.
//!
//! **The sentence and the button are `bevy_gooey`'s** (`label`, `button`): nothing here draws a widget.
//! The button is a command like any other. It queues the chord of `view.focus_filter` ("Filter the
//! list"), the same press `Cmd+F` is, so the library's filter box takes the keyboard and the person can
//! type what they are looking for. That is the whole of "open the library": the PLACE list is always
//! on screen, and what a newcomer lacks is the place to start typing.
//!
//! The overlay is a child of the frame's viewport, so it follows the world's hole when a dock is
//! dragged, and it is `Pickable::IGNORE` itself so a click that misses the button reaches the map.

use bevy::prelude::*;

use crate::chrome::{Frame, FrameSystems};
use crate::project::OpenMap;
use crate::screen::Screen;

/// What the prompt says. One sentence, no full stop, as the writing style has every label.
pub const SENTENCE: &str = "Place a piece from the library";

/// The primary action's label.
pub const ACTION: &str = "Open library";

/// The overlay: the sentence and the action, centred over the viewport.
#[derive(Component)]
pub struct EmptyMapPrompt;

/// The prompt's button.
#[derive(Component)]
pub struct OpenLibraryButton;

pub struct EmptyStatePlugin;

impl Plugin for EmptyStatePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::command_run::CommandRunPlugin>() {
            app.add_plugins(crate::command_run::CommandRunPlugin);
        }
        app.add_systems(OnEnter(Screen::Editor), spawn_prompt.after(FrameSystems))
            .add_systems(Update, show_while_the_map_is_empty.run_if(in_state(Screen::Editor)))
            .add_observer(on_open_library);
    }
}

fn spawn_prompt(mut commands: Commands, frame: Res<Frame>) {
    commands
        .spawn((
            Name::new("EmptyMapPrompt"),
            EmptyMapPrompt,
            Node {
                // PLACES-ITSELF-OK: an overlay on the viewport, not a panel in the frame's flow.
                // `Frame` owns where docks go; this sits over the hole the world is drawn through.
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(crate::chrome::GAP_GROUP),
                // Hidden until the map is known to be empty: a map that opens with pieces never
                // flashes the prompt.
                display: Display::None,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            ChildOf(frame.viewport),
        ))
        .with_children(|p| {
            p.spawn(bevy_gooey::label(SENTENCE));
            p.spawn((bevy_gooey::button(ACTION), OpenLibraryButton));
        });
}

/// **Shown while the open map has no placements.** Compares before it writes, so a map that stays
/// empty (or stays full) dirties nothing.
fn show_while_the_map_is_empty(open: Option<Res<OpenMap>>, mut prompt: Query<&mut Node, With<EmptyMapPrompt>>) {
    let want = if open.is_some_and(|o| o.map.placements.is_empty()) { Display::Flex } else { Display::None };
    for mut node in &mut prompt {
        if node.display != want {
            node.display = want;
        }
    }
}

/// **The action: the library's filter takes the keyboard**, by the chord `Cmd+F` is.
fn on_open_library(
    activate: On<bevy_gooey::Activate>,
    buttons: Query<(), With<OpenLibraryButton>>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    if let Some(command) = crate::commands::by_id("view.focus_filter") {
        chords.queue(&command, *live);
    }
}
