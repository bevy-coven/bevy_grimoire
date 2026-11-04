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
//! **Present only while it is wanted, not hidden.** The interface's tests count `Role::ButtonText` over
//! the whole world to say what a dialog's answers are (`a_question_is_asked_in_a_gooey_dialog`), and a
//! button that is merely `Display::None` is still counted: the first version of this prompt kept one and
//! the full workspace gate refused it. So the prompt is spawned when the map is empty and no question is
//! open (a modal covers the viewport, and a second button behind it is noise), and despawned the frame
//! either stops being true.
//!
//! The overlay is a child of the frame's viewport, so it follows the world's hole when a dock is
//! dragged, and it is `Pickable::IGNORE` itself so a click that misses the button reaches the map.

use bevy::prelude::*;

use crate::chrome::{Frame, FrameSystems};
use crate::confirm::Confirm;
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
        app.add_systems(
            Update,
            (keep_the_prompt_while_the_map_is_empty.after(FrameSystems), read_as_a_control)
                .chain()
                .run_if(in_state(Screen::Editor)),
        )
        .add_observer(on_open_library);
    }
}

/// **The prompt exists exactly while the open map has no placements and no question is open.**
/// Reads before it writes: a map that stays empty (or stays full) spawns and despawns nothing.
fn keep_the_prompt_while_the_map_is_empty(
    mut commands: Commands,
    frame: Option<Res<Frame>>,
    open: Option<Res<OpenMap>>,
    confirm: Option<Res<Confirm>>,
    dialogs: Query<(), With<bevy_gooey::Dialog>>,
    prompt: Query<Entity, With<EmptyMapPrompt>>,
) {
    let wanted = open.is_some_and(|o| o.map.placements.is_empty())
        && !confirm.is_some_and(|c| c.is_open())
        && dialogs.is_empty();
    let standing = prompt.iter().next();
    match (wanted, standing) {
        (true, None) => {
            let Some(frame) = frame else { return };
            spawn_prompt(&mut commands, frame.viewport);
        }
        (false, Some(_)) => {
            for e in &prompt {
                commands.entity(e).despawn();
            }
        }
        _ => {}
    }
}

fn spawn_prompt(commands: &mut Commands, viewport: Entity) {
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
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            ChildOf(viewport),
        ))
        .with_children(|p| {
            p.spawn(bevy_gooey::label(SENTENCE));
            p.spawn((bevy_gooey::button(ACTION), OpenLibraryButton));
        });
}

/// **A standing control reads as `ControlText`, not `ButtonText`.** The interface's tests count
/// `Role::ButtonText` over the whole world to say what a dialog's answers are, and
/// `the_bands_item_does_not_call_itself_a_button` requires that no words outside a dialog carry it: the
/// band's job item has the same shape. The button is still gooey's (`Role::Button`, `Activate`, focus,
/// the skin's button ground); only the role of its words is the control's.
fn read_as_a_control(
    mut commands: Commands,
    buttons: Query<&Children, Added<OpenLibraryButton>>,
    words: Query<(), With<Text>>,
) {
    for kids in &buttons {
        for kid in kids.iter().filter(|k| words.contains(*k)) {
            commands.entity(kid).insert(bevy_gooey::Role::ControlText);
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
