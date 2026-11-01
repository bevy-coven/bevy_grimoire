//! **The menu of a library piece** (UI-36, #230): right-click a row of the Map's palette and that
//! piece's verbs open at the pointer: place it, mount it, send it back out of the kit.
//!
//! Like [`crate::piece_menu`] this builds nothing of its own: the menu is `bevy_gooey`'s `context_menu`,
//! spawned once and named on every palette row, and each item is a verb the editor already has, by the
//! piece's id and through the operation registry, so a menu choice is the key's or the click's one log
//! line and the same Cmd+Z:
//!
//! - **Place** is the row's own click (`brush_arm`).
//! - **Mount on floor / wall / ceiling** is `M`'s write (`piece_set_mount`), the mount the piece has now
//!   carrying its authored wall height.
//! - **Remove from the kit** is `Delete` on the Meshes shelf (`piece_remove`): the same refusal while a
//!   map places the piece, the same trip back to NOT IMPORTED.
//!
//! A palette row is a `Button`, and gooey's guide says a right-click on a button also activates it, so a
//! right-click arms the piece too. That is what a person means by choosing a row and asking what can be
//! done with it, and "Place" stays in the menu for the one who opened it by the keyboard.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_gooey::{ContextMenu, ContextTarget, context_menu, menu_item, separator};
use bevy_grimoire_core::descriptor::{Mount, mount_height};

use crate::editor::{EditorState, PaletteRow, arm_piece, set_mount};
use crate::project::{OpenMap, Project};
use crate::tiles::ImportState;

/// On the one menu every palette row names. A component and not a resource, for the reason
/// [`crate::piece_menu::PieceMenu`] gives.
#[derive(Component)]
pub struct PaletteMenu;

/// What a menu item does to the piece.
#[derive(Clone, Copy)]
enum Verb {
    Place,
    Mount(MountKind),
    SendBack,
}

/// The mounts the menu offers: the three that need no further answer. A wall mount keeps the height the
/// piece has, or the list's default when it had none (the same carry `M` makes).
#[derive(Clone, Copy)]
enum MountKind {
    Floor,
    Wall,
    Ceiling,
}

/// The height `M` gives a piece that moves onto a wall with none of its own.
const DEFAULT_WALL_HEIGHT: f32 = 1.8;

/// What is in the menu, top to bottom; a gap is a separator. One table, so the menu and its test read
/// the same list.
const ITEMS: [Option<(&str, Verb)>; 6] = [
    Some(("Place", Verb::Place)),
    None,
    Some(("Mount on floor", Verb::Mount(MountKind::Floor))),
    Some(("Mount on wall", Verb::Mount(MountKind::Wall))),
    Some(("Mount on ceiling", Verb::Mount(MountKind::Ceiling))),
    Some(("Remove from the kit", Verb::SendBack)),
];

/// Spawns the menu and names it on every palette row.
pub struct PaletteMenuPlugin;

impl Plugin for PaletteMenuPlugin {
    fn build(&self, app: &mut App) {
        // On entering the editor, for the reason `PieceMenuPlugin` gives: the door change despawns what
        // a Startup system spawned.
        app.add_systems(OnEnter(crate::screen::Screen::Editor), spawn_menu)
            .add_systems(Update, name_on_rows);
    }
}

/// A row is rebuilt whenever the library or the folds change, so the menu is named on the rows that do
/// not have it yet rather than once. The filter is an archetype test: it costs nothing while every row
/// has the menu.
fn name_on_rows(
    menu: Query<Entity, With<PaletteMenu>>,
    rows: Query<Entity, (With<PaletteRow>, Without<ContextMenu>)>,
    mut commands: Commands,
) {
    let Ok(menu) = menu.single() else {
        return;
    };
    for row in &rows {
        // `try_`: the palette rebuild runs in the same frame and may despawn a row this has just seen.
        commands.entity(row).try_insert(ContextMenu(menu));
    }
}

fn spawn_menu(mut commands: Commands) {
    let menu = commands.spawn((context_menu(()), PaletteMenu)).id();
    for entry in ITEMS {
        let Some((label, verb)) = entry else {
            commands.spawn((separator(), ChildOf(menu)));
            continue;
        };
        commands
            .spawn((menu_item(label), ChildOf(menu)))
            .observe(on_item(verb));
    }
}

#[allow(clippy::type_complexity)]
fn on_item(
    verb: Verb,
) -> impl FnMut(
    On<Activate>,
    Query<&ChildOf>,
    Query<&ContextTarget>,
    Query<&PaletteRow>,
    Option<ResMut<Project>>,
    Option<ResMut<ImportState>>,
    Option<ResMut<EditorState>>,
    Option<ResMut<crate::labels::Suggestions>>,
    Option<ResMut<crate::labels::LabelGeneration>>,
    Option<Res<OpenMap>>,
) {
    move |activate, parents, targets, rows, project, import, state, suggestions, generation, open| {
        let (Some(mut project), Some(mut import), Some(mut state)) = (project, import, state) else {
            return;
        };
        let Some(row) = ContextTarget::of(activate.entity, &parents, &targets) else {
            return;
        };
        let Ok(PaletteRow(ix)) = rows.get(row) else {
            return;
        };
        let Some(id) = project.library.descriptors.get(*ix).map(|d| d.id.clone()) else {
            return;
        };
        let raised = import.status.raised();
        let done = match verb {
            Verb::Place => {
                arm_piece(&mut state, &project, open.as_deref(), &id);
                return;
            }
            Verb::Mount(kind) => {
                let had = project
                    .measured
                    .descriptors
                    .iter()
                    .find(|d| d.id == id)
                    .and_then(|d| d.mount.as_ref())
                    .and_then(mount_height);
                let want = match kind {
                    MountKind::Floor => Mount::OnFloor,
                    MountKind::Wall => Mount::OnWall {
                        height: had.unwrap_or(DEFAULT_WALL_HEIGHT),
                    },
                    MountKind::Ceiling => Mount::OnCeiling,
                };
                // Taken before the write, the only moment the old value still exists (`M` does the same).
                let before = import.snapshot(&project);
                let landed = set_mount(&mut import, &mut project, &id, &want);
                if landed {
                    import.record(before);
                    import
                        .status
                        .note(format!("mount: {}", bevy_grimoire_core::descriptor::mount_label(Some(&want))));
                }
                landed
            }
            Verb::SendBack => {
                let (Some(mut suggestions), Some(mut generation)) = (suggestions, generation) else {
                    return;
                };
                crate::tiles::send_back_out_of_library(
                    &mut project,
                    &mut import,
                    &mut suggestions,
                    &mut generation,
                    &id,
                )
            }
        };
        // The Map's status row is the editor's, not the Meshes shelf's: hand the answer across.
        if import.status.raised() > raised {
            state.status.problem(import.status.problem_text().to_owned());
        } else if done {
            state.status.note(import.status.note_text().to_owned());
        }
    }
}
