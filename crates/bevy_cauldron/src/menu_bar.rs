//! **The menu bar: every command, in exactly one menu** (UI-31, #225).
//!
//! `docs/ui/vision.md` §4 puts a menu bar above the toolbar and §5 says what it holds: the categories
//! the registry already names (`file`, `edit`, `view`, `map`, `meshes`, `tiles`, `compose`, `rigs`,
//! `tools`, `help`), every command in exactly one of them. Nothing here declares a menu's contents: a
//! menu is a category, and its rows are the commands whose id starts with it. That is the one rule
//! that keeps the bar honest — a command added to the registry appears, and no command appears twice.
//!
//! **The bar is gooey's** (`menu_bar`, `menu_title`, `menu_item`), so Down, Enter, Space, a pad's south
//! button, Left and Right, F10/Alt and Escape are the crate's own, and the shortcut each row shows is
//! [`Command::shortcut_in`] — the chord the **settings** gave it, not the default, so a rebound key is
//! what the menu says.
//!
//! **A command that cannot run is `Disabled` with the registry's own reason** ([`Command::enabled`]).
//! The reason is the tooltip, which is also what a person sees walking onto the row by keyboard: a
//! greyed row that says nothing is the failure this guards. It is judged every frame the stance
//! changes and written only on a difference, because a menu is closed almost all the time.
//!
//! **Activating a row presses its chord**, through the same [`crate::command_run::Chords`] queue the
//! toolbar and the empty-map prompt use, so a menu item and its key are one dispatcher and one undo
//! step.

use bevy::prelude::*;

use crate::chrome::{Frame, FrameSystems};
use crate::command_run::Chords;
use crate::commands::{self, Command};
use crate::keys::Live;
use crate::screen::Screen;

/// **The order the menus read in** — `docs/ui/vision.md` §5's list, plus the three categories the
/// registry has that the vision's sentence does not name: `meshes` and `tiles` are the Kit door's
/// panels and `compose` is its composition tool. A command in no menu would be a command with no door
/// a person can find, so every category the registry declares is here, and
/// `every_category_has_a_menu` fails until a new one is.
///
/// It is a list rather than a sort so the bar does not move when a category is added: a new one is
/// appended.
pub const MENUS: [(&str, &str); 10] = [
    ("file", "File"),
    ("edit", "Edit"),
    ("view", "View"),
    ("map", "Map"),
    ("meshes", "Meshes"),
    ("tiles", "Tiles"),
    ("compose", "Compose"),
    ("rigs", "Rigs"),
    ("tools", "Tools"),
    ("help", "Help"),
];

/// One row of a menu: the command it stands for, so a click resolves back to it and a test finds the
/// row by its words.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuCommand(pub crate::keys::Action);

/// The bar itself, so a test can find it and nothing else claims to be one.
#[derive(Component)]
pub struct EditorMenuBar;

pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::command_run::CommandRunPlugin>() {
            app.add_plugins(crate::command_run::CommandRunPlugin);
        }
        app.add_systems(
            Update,
            (keep_the_menu_bar, read_the_titles_as_controls, refresh_the_rows)
                .chain()
                .after(FrameSystems)
                .run_if(in_state(Screen::Editor)),
        )
            .add_observer(on_menu_item_activate);
    }
}

/// **The bar is there while the workbench is, and not while a question is.**
///
/// A modal dialog is drawn over a scrim that takes the pointer and the keys, so a menu bar showing
/// through it is not a control a person can reach — and the interface's own tests count
/// `Role::ButtonText` over the whole world to say what a dialog's answers are, which a bar of ten
/// buttons breaks (`a_question_is_asked_in_a_gooey_dialog`). Hiding it is not enough: a `Display::None`
/// button is still counted (the empty-map prompt learned that the hard way), so the bar is despawned
/// and built again when the question closes.
///
/// It is a system rather than `OnEnter` for one more reason: the bar is a child of the frame's root,
/// which the frame respawns on every door, so "there is a frame and no bar" is the condition that
/// covers both a new screen and a new door.
fn keep_the_menu_bar(
    mut commands: Commands,
    frame: Option<Res<Frame>>,
    live: Option<Res<Live>>,
    question: Option<Res<crate::confirm::Confirm>>,
    bars: Query<Entity, With<EditorMenuBar>>,
) {
    let Some(frame) = frame else { return };
    let standing = bars.iter().next();
    // **A question, not any dialog.** A menu opens as a gooey modal popover, which is a `Dialog`, so
    // "no dialog" would despawn the bar the moment a menu opens — measured here, the first take of
    // this slice's clip showed it. What must stand down is the editor's own question, which is drawn
    // over a scrim that covers the bar and is what the dialog tests count.
    let wanted = !question.is_some_and(|q| q.is_open());
    match (wanted, standing) {
        (true, None) => spawn_menu_bar(&mut commands, &frame, live.as_deref().copied()),
        (false, Some(bar)) => {
            commands.entity(bar).despawn();
        }
        _ => {}
    }
}

/// **A bar title's words are `ControlText`, not `ButtonText`** — the same retag
/// `empty_state::read_as_a_control` makes, for the same reason: this editor's tests count
/// `Role::ButtonText` over the whole world to say what a **dialog's answers** are, and
/// `the_bands_item_does_not_call_itself_a_button` requires that nothing outside a dialog reads as one.
/// A menu title is a bar control, not an answer to a question. The title is still gooey's (its button,
/// its `Activate`, its focus, its skin), and `bevy_gooey` has a request for a role of its own:
/// `bevy-coven/bevy_gooey#39` (G-35).
fn read_the_titles_as_controls(
    mut commands: Commands,
    titles: Query<&Children, Added<bevy_gooey::MenuTitle>>,
    words: Query<(), With<Text>>,
) {
    for kids in &titles {
        for kid in kids.iter().filter(|k| words.contains(*k)) {
            commands.entity(kid).insert(bevy_gooey::Role::ControlText);
        }
    }
}

/// **The bar itself, at the top of the window.** The chrome bar was index 0 of the frame's root, so the
/// menu bar goes above it: index 0, and the chrome bar moves to 1.
fn spawn_menu_bar(commands: &mut Commands, frame: &Frame, live: Option<Live>) {
    let live = live.unwrap_or_default();
    // One `Vec` per menu, built here rather than inside the spawner: the spawners are closures the
    // relationship machinery calls later, and a registry read inside one would be the second place
    // that decides what a menu holds.
    let registry = commands::all();
    let menus: Vec<(&'static str, Vec<Command>)> = MENUS
        .iter()
        .map(|(category, title)| {
            let rows = registry.iter().copied().filter(|c| c.category() == *category).collect();
            (*title, rows)
        })
        .collect();
    let bar = commands
        .spawn((
            EditorMenuBar,
            bevy_gooey::menu_bar(Children::spawn(SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                for (title, rows) in menus {
                    let rows = rows.clone();
                    parent.spawn(bevy_gooey::menu_title(
                        title,
                        Children::spawn(SpawnWith(move |menu: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                            for command in &rows {
                                menu.spawn((
                                    MenuCommand(command.action),
                                    bevy_gooey::menu_item(command.title)
                                        .with_shortcut(command.shortcut_in(live)),
                                ));
                            }
                        })),
                    ));
                }
            }))),
        ))
        .id();
    commands.entity(frame.root).insert_children(0, &[bar]);
}

/// **A row says the settings' chord, and why it cannot run when it cannot.**
///
/// The comparison is against the `Disabled` component the row carries **now**, not against a record
/// kept beside it: the first version kept one and was wrong in the one way that matters — the spawn
/// filled the record and never put `Disabled` on the row, so every frame agreed there was nothing to
/// do and no row was ever greyed. Reading the component back is the only check that cannot disagree
/// with what the menu shows.
///
/// Nothing is written unless the answer changed: a menu is closed almost all the time, and an
/// unconditional write every frame would dirty the whole cascade sixty times a second
/// (`no_system_writes_every_frame.rs`; `light_the_focused_panel` and `hide_idle_scrollbars` are the
/// same shape). `Disabled`'s own hooks read the reason, so a row that already says the right thing
/// costs one comparison.
fn refresh_the_rows(
    mut commands: Commands,
    live: Option<Res<Live>>,
    rows: Query<(Entity, &MenuCommand, Option<&bevy_gooey::Disabled>)>,
) {
    let Some(live) = live else { return };
    for (entity, row, disabled) in &rows {
        let Some(command) = commands::by_id(commands::identity(row.0).0) else {
            continue;
        };
        let want = command.enabled(*live).err();
        if disabled.map(|d| d.0.as_str()) == want {
            continue;
        }
        match want {
            Some(reason) => {
                commands.entity(entity).insert(bevy_gooey::Disabled::new(reason));
            }
            None => {
                commands.entity(entity).remove::<bevy_gooey::Disabled>();
            }
        }
    }
}

/// **A row chosen is its command's chord pressed**, the same move the toolbar makes: one dispatcher,
/// one judge, one undo step.
fn on_menu_item_activate(
    activate: On<bevy_gooey::Activate>,
    rows: Query<&MenuCommand>,
    mut chords: ResMut<Chords>,
    live: Option<Res<Live>>,
) {
    let Ok(row) = rows.get(activate.entity) else {
        return;
    };
    let Some(command) = commands::by_id(commands::identity(row.0).0) else {
        return;
    };
    let live = live.map(|l| *l).unwrap_or_default();
    chords.queue(&command, live);
}
