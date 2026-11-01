//! **The menu of a placed piece** (UI-36, #230): right-click a piece and its verbs open at the pointer.
//!
//! The menu is `bevy_gooey`'s own `context_menu`, spawned once and named on the pieces with
//! `ContextMenu`; this module builds nothing of its own. Two things are the editor's:
//!
//! - **Finding the piece.** The editor's pieces are scenes with no colliders, so no picking backend
//!   can hit one. The right-click is resolved the way the keys resolve "the piece under the cursor"
//!   (`editor::under_cursor_target`: the ray's first volume, then the `H` lock) and handed to gooey as
//!   the secondary `PointerClick` its guide says a host sends when it does its own picking.
//! - **What the items do.** Each item is the verb a key already has (`editor::piece_verb`), by the
//!   piece's id and through the operation registry, so a menu choice is one log line and one `Cmd+Z`,
//!   the same as the key. Move and Note are the two tools: they arm the move tool with this piece in hand
//!   (`editor::carry_piece`) and the Note tool with its box open on this piece (`editor::note_pick_piece`,
//!   shared with the Shift-click). The chord each item shows is the command's own, read from `commands`.
//!
//! The keyboard way in (Shift+F10 on a focused widget) is gooey's and needs nothing here.

use bevy::camera::NormalizedRenderTarget;
use bevy::input::ButtonInput;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, WindowRef};
use bevy_gooey::{ContextMenu, ContextTarget, context_menu, menu_item, separator};

use crate::commands;
use crate::editor::{
    CloneDrag, EditorState, MoveDrag, NoteBox, PieceVerb, Placement, Rung, TargetLock, carry_piece, note_pick_piece,
    piece_verb, snap_level, ui_blocks_gesture,
};
use crate::view::MainCamera;
use crate::keys::Action;
use crate::project::{OpenMap, Project};

/// On the one menu every placed piece names. A component and not a resource: `every_resource_is_classified`
/// asks what a door does to each resource, and this is one entity's identity, not editor state.
#[derive(Component)]
pub struct PieceMenu;

/// What one item does to the piece the menu is for: a one-shot verb the registry runs, or one of the two
/// tools, entered with this piece already chosen.
#[derive(Clone, Copy)]
enum Act {
    Verb(PieceVerb),
    /// The move tool, with this piece in hand: the next click on the map puts it down.
    Move,
    /// The Note tool, with the note box open on this piece.
    Note,
}

/// What is in the menu, top to bottom: the words, what it does, and the key it already has. A gap
/// is a separator. One table, so the menu and its test read the same list.
const ITEMS: [Option<(&str, Act, Option<Action>)>; 11] = [
    Some(("Turn left", Act::Verb(PieceVerb::TurnLeft), Some(Action::TurnPieceLeft))),
    Some(("Turn right", Act::Verb(PieceVerb::TurnRight), Some(Action::TurnPieceRight))),
    Some(("Tip about x", Act::Verb(PieceVerb::TipX), Some(Action::TipX))),
    Some(("Tip about z", Act::Verb(PieceVerb::TipZ), Some(Action::TipZ))),
    Some(("Lift", Act::Verb(PieceVerb::Lift), Some(Action::LiftUp))),
    Some(("Lower", Act::Verb(PieceVerb::Lower), Some(Action::LiftDown))),
    None,
    Some(("Move", Act::Move, Some(Action::MoveMode))),
    Some(("Note", Act::Note, Some(Action::NoteTool))),
    None,
    Some(("Remove", Act::Verb(PieceVerb::Remove), None)),
];

/// Spawns the menu and watches for the right-click that opens it.
pub struct PieceMenuPlugin;

impl Plugin for PieceMenuPlugin {
    fn build(&self, app: &mut App) {
        // **On entering the editor, not at startup**: leaving a door despawns everything it made
        // (`screen::close_the_door`), the menu with it, and the next door needs its own.
        app.add_systems(OnEnter(crate::screen::Screen::Editor), spawn_menu)
            .add_systems(Update, open_on_right_click.run_if(crate::editor::in_map_mode));
    }
}

fn spawn_menu(mut commands: Commands) {
    let menu = commands.spawn((context_menu(()), PieceMenu)).id();
    for entry in ITEMS {
        match entry {
            Some((label, act, action)) => {
                // The chord is the command's own, so a rebind shows here with no second table. `Remove`
                // has none: its key arms the removal *mode*, which is not what this item does.
                let item = match action {
                    Some(action) => menu_item(label).with_shortcut(commands::command(action).shortcut),
                    None => menu_item(label),
                };
                commands
                    .spawn((item, ChildOf(menu)))
                    .observe(move |activate: On<Activate>,
                                   parents: Query<&ChildOf>,
                                   targets: Query<&ContextTarget>,
                                   placements: Query<&Placement>,
                                   placed: Query<(Entity, &Placement)>,
                                   assets: Res<AssetServer>,
                                   keyboard: Res<ButtonInput<KeyCode>>,
                                   rung: Res<Rung>,
                                   mut move_drag: ResMut<MoveDrag>,
                                   mut clone_drag: ResMut<CloneDrag>,
                                   mut note_box: ResMut<NoteBox>,
                                   project: Option<Res<Project>>,
                                   open: Option<ResMut<OpenMap>>,
                                   state: Option<ResMut<EditorState>>,
                                   mut commands: Commands| {
                        let (Some(project), Some(mut open), Some(mut state)) = (project, open, state)
                        else {
                            return;
                        };
                        let Some(piece) = ContextTarget::of(activate.entity, &parents, &targets)
                        else {
                            return;
                        };
                        let Ok(Placement(id)) = placements.get(piece) else {
                            return;
                        };
                        match act {
                            Act::Verb(verb) => piece_verb(
                                &mut commands,
                                &assets,
                                &project,
                                &mut open,
                                &mut state,
                                &placed,
                                id,
                                verb,
                                snap_level(&keyboard, rung.0),
                            ),
                            Act::Move => carry_piece(&open, &mut state, &mut move_drag, &mut clone_drag, id),
                            Act::Note => note_pick_piece(&open, &mut state, &mut note_box, id.clone(), false),
                        }
                    });
            }
            None => {
                commands.spawn((separator(), ChildOf(menu)));
            }
        }
    }
}

/// A right-click over a piece names the menu on it and sends the click gooey opens a menu on.
///
/// On the release, so the press that gooey's "a press outside closes" rule reads is over before the
/// menu exists. A cursor over the interface is not a click on the map, the same rule the placing and
/// removal tools follow.
#[allow(clippy::too_many_arguments)]
fn open_on_right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    hovered_ui: Query<&bevy::picking::hover::Hovered>,
    pointer: Res<crate::view::Pointer>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    window: Option<Single<(Entity, &Window), With<PrimaryWindow>>>,
    menu: Query<Entity, With<PieceMenu>>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut lock: Option<ResMut<TargetLock>>,
    placed: Query<(Entity, &Placement)>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Right) {
        return;
    }
    let (Ok(menu), Some(project), Some(open), Some(lock), Some(window)) =
        (menu.single(), project, open, lock.as_deref_mut(), window)
    else {
        return;
    };
    if ui_blocks_gesture(hovered_ui.iter().any(|h| h.0), false) {
        return;
    }
    let Some(index) = crate::editor::under_cursor_target(lock, *pointer, camera, &project, &open)
    else {
        return;
    };
    let Some(id) = open.map.placements.get(index).map(|p| p.id.as_str()) else {
        return;
    };
    let Some((piece, _)) = placed.iter().find(|(_, p)| p.0 == id) else {
        return;
    };
    let (window_entity, window) = *window;
    // The window's own cursor, or the pointer the editor sensed when that is injected (an agent driving
    // the editor over BRP never moves the window's cursor).
    let Some(at) = window
        .cursor_position()
        .or_else(|| pointer.0.map(|p| p / window.scale_factor().max(1.0)))
    else {
        return;
    };
    let Some(target) = WindowRef::Primary.normalize(Some(window_entity)) else {
        return;
    };
    commands.entity(piece).insert(ContextMenu(menu));
    commands.trigger(PointerClick {
        entity: piece,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(target),
                position: at,
            },
        ),
        button: PointerButton::Secondary,
        hit: HitData::new(window_entity, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
}
