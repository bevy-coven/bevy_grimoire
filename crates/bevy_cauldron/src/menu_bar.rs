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

use std::path::PathBuf;

use bevy::ecs::spawn::{SpawnIter, SpawnWith};
use bevy::prelude::*;

use crate::app_verbs::AppVerb;
use crate::chrome::{Frame, FrameSystems};
use crate::command_run::Chords;
use crate::commands::{self, Command};
use crate::keys::{Action, Live};
use crate::screen::Screen;

/// **The order the menus read in** — `docs/ui/vision.md` §5's list, plus the three categories the
/// registry has that the vision's sentence does not name: `meshes` and `tiles` are the Kit door's
/// panels and `compose` is its composition tool. A command in no menu would be a command with no door
/// a person can find, so every category the registry declares is here, and
/// `every_category_has_a_menu` fails until a new one is.
///
/// It is a list rather than a sort so the bar does not move when a category is added: a new one is
/// appended.
pub const MENUS: [(&str, &str); 11] = [
    ("file", "File"),
    ("edit", "Edit"),
    ("view", "View"),
    ("workspace", "Workspace"),
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

/// **View → Show statistics** (#302): the one row of the bar that is a setting and not a command. It is a
/// tick, and choosing it writes `show_statistics` to the user's settings file, which the editor reads back
/// as it reads an edit made by hand; [`follow_the_statistics_setting`] keeps the tick on what the settings say.
#[derive(Component)]
pub struct StatisticsItem;

/// **View → Show activity bar** (NV-03, #380): a tick on the `show_activity_bar` setting, written to the user's settings file
/// like Show statistics, so the bar can be collapsed away and the choice is kept. [`follow_the_activity_bar_setting`] keeps the
/// tick on what the settings say.
#[derive(Component)]
pub struct ActivityBarItem;

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
            (keep_the_menu_bar, read_the_titles_as_controls, refresh_the_rows, follow_the_statistics_setting, follow_the_activity_bar_setting)
                .chain()
                .after(FrameSystems)
                .run_if(in_state(Screen::Editor)),
        )
        .add_systems(
            Update,
            take_the_escape_that_closed_a_menu
                .after(bevy_gooey::GooeySystems::Navigate)
                .before(crate::keys::Phase::Act)
                .run_if(in_state(Screen::Editor)),
        )
        .add_observer(on_menu_item_activate);
    }
}

/// **`Esc` that closes a menu is the menu's, not also the editor's back-out** (#287).
///
/// gooey closes an open menu on `Esc` in its `Navigate` set, and `Esc` is the editor's own `Cancel`,
/// which peels the armed piece: one press put the menu away *and* the brush down. gooey does not say
/// that it took the key, but it does take `MenuOpen` off the title when it closes the menu, so a press
/// of `Esc` in the frame a menu closed is the press that closed it, and it is taken ([`take`]) before
/// the verbs run. A menu closed by choosing a row or by a click elsewhere has no `Esc` in its frame, so
/// nothing is taken from it.
///
/// [`take`]: crate::confirm::take
fn take_the_escape_that_closed_a_menu(
    mut closed: RemovedComponents<bevy_gooey::MenuOpen>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
) {
    // Drained every frame, so a menu closed by a click does not leave its removal waiting for a later
    // `Esc`.
    let a_menu_closed = closed.read().count() > 0;
    if a_menu_closed {
        crate::confirm::take(&mut keyboard, KeyCode::Escape);
    }
}

/// **The bar is there while the workbench is, a question or not** (#299).
///
/// It used to be taken down while the editor's own question was up, because the interface's tests count `Role::ButtonText`
/// over the whole world to say what a dialog's answers are. That made the layout jump by the bar's height (34 px) every
/// time a question opened and closed. The bar's titles are retagged ([`read_the_titles_as_controls`]) and a menu's rows are
/// not buttons, so the count is the dialog's alone, and the dialog's scrim covers the bar: nothing is reachable through it.
///
/// It is a system rather than `OnEnter` for one more reason: the bar is a child of the frame's root, which the frame
/// respawns on every door, so "there is a frame and no bar" is the condition that covers both a new screen and a new door.
fn keep_the_menu_bar(
    mut commands: Commands,
    frame: Option<Res<Frame>>,
    live: Option<Res<Live>>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    recents: Option<Res<crate::recent::RecentProjects>>,
    bars: Query<Entity, With<EditorMenuBar>>,
) {
    let Some(frame) = frame else { return };
    if bars.iter().next().is_some() {
        return;
    }
    spawn_menu_bar(
        &mut commands,
        &frame,
        live.as_deref().copied(),
        settings.as_deref().is_some_and(|s| s.show_statistics),
        settings.as_deref().is_none_or(|s| s.show_activity_bar),
        recent_projects(recents.as_deref()),
    );
}

/// **The last [`RECENT_SHOWN`] projects that are still there**, as File → Open recent lists them.
///
/// [`RECENT_SHOWN`]: crate::app_verbs::RECENT_SHOWN
fn recent_projects(recents: Option<&crate::recent::RecentProjects>) -> Vec<PathBuf> {
    let Some(Ok(recent)) = recents.map(|r| crate::recent::Recent::load(&r.0)) else { return Vec::new() };
    recent
        .listing()
        .into_iter()
        .filter(|row| !row.missing)
        .map(|row| row.project.path)
        .take(crate::app_verbs::RECENT_SHOWN)
        .collect()
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

/// **One row of a menu**: a command of the registry, a verb of the application, the recent projects as a submenu, a line
/// between two groups, or the one setting the bar carries.
#[derive(Clone, Debug)]
enum Slot {
    Command(Action),
    Verb(AppVerb),
    /// **Every page of the written guide, in the guide's own order** (UI-91, #257): one row each, so
    /// Help finds every page and no page is named twice.
    Guide,
    Recent(Vec<PathBuf>),
    Separator,
    Statistics,
    /// View ▸ Show activity bar (NV-03, #380).
    ActivityBar,
    /// View ▸ Show outliner (UI-51, #237).
    Outliner,
    /// View ▸ Show outline (SG-04, #403): the open Markdown page's headings, in the Files panel.
    FileOutline,
    /// View ▸ Show library (UI-52, #238): the same library as a grid of portraits.
    Library,
    /// View ▸ Game UI preview (UI-03, #211): the project's game skin, drawn only inside that panel.
    GameUi,
    /// View ▸ Show agent (UI-60, #246).
    Familiar,
    /// View ▸ Pause agent writes (UI-60, #246).
    PauseAgent,
    /// View ▸ Follow the agent (AF-04, #360).
    FollowAgent,
    /// View ▸ Show left panel (feedback-1794146831022): the tick is the panel shown.
    LeftPanel,
    /// View ▸ Zen mode (feedback-1794146831022).
    Zen,
    /// View ▸ Show reload panel (UI-61, #247): the bottom dock, shown whatever the window's room.
    ReloadPanel,
    /// Window ▸ Reset layout (UI-33, #227).
    ResetLayout,
    /// View ▸ Light map (UI-80, #254): a tick that shows or hides the viewport overlay. Only built in a
    /// build that has the overlay; `layout` adds it under the same `cfg` the row's spawn is under.
    #[cfg(feature = "bevy_light_grid")]
    LightMap,
    /// View ▸ Show style areas (SG-06, #405): outlines each area of the open map, with its name.
    StyleAreas,
}

/// **The Map menu, in the groups its keys fall into**: choosing and tools, turning and tipping, filling, and the map itself.
/// A map command in none of them is appended after the last, and `the_map_menu_names_every_map_command` fails until it is
/// placed, so the groups cannot go stale.
pub const MAP_GROUPS: [&[Action]; 4] = [
    // Place: what is in hand, and the tools that act on the pieces already down.
    &[
        Action::CycleTarget,
        Action::PalettePrev,
        Action::PaletteNext,
        Action::PickComposition,
        Action::CompositionPrev,
        Action::CompositionNext,
        Action::ArmComposition,
        Action::Remove,
        Action::MoveMode,
        Action::CloneMode,
        Action::RunMode,
    ],
    // Turn and tip: the pose of the piece in hand or under the cursor.
    &[Action::TurnPieceLeft, Action::TurnPieceRight, Action::TipX, Action::TipZ, Action::Straighten, Action::LiftDown, Action::LiftUp],
    // Fill: what the solvers lay down, and keeping or leaving it.
    &[Action::Fill, Action::Generate, Action::GenerateDeclared, Action::GenerateComposed, Action::AcceptProposal],
    // Map: the map's own settings and notes.
    &[Action::GroupFromSet, Action::RenameMap, Action::NoteTool, Action::SetRule, Action::OwnToggle, Action::CycleGrid],
];

/// What `category`'s menu holds, in order. Every command of the category is in it exactly once: the ones the layout names where
/// it names them, and any it does not after them.
fn layout(category: &str, registry: &[Command], recent: Vec<PathBuf>) -> Vec<Slot> {
    let named: Vec<Slot> = match category {
        "file" => vec![
            Slot::Verb(AppVerb::NewProject),
            Slot::Verb(AppVerb::OpenProject),
            Slot::Recent(recent),
            Slot::Separator,
            Slot::Verb(AppVerb::AddMeshes),
            Slot::Separator,
            Slot::Command(Action::Save),
            Slot::Command(Action::MainMenu),
            Slot::Separator,
            Slot::Verb(AppVerb::Quit),
        ],
        "map" => MAP_GROUPS
            .iter()
            .enumerate()
            .flat_map(|(i, group)| {
                let rows = group.iter().copied().map(Slot::Command);
                let line = (i > 0).then_some(Slot::Separator);
                line.into_iter().chain(rows)
            })
            .collect(),
        "help" => vec![
            // The tour is the agent's guide channel, which the `debugger` feature builds (it is on by default, and in the app).
            #[cfg(feature = "debugger")]
            Slot::Verb(AppVerb::TakeTour),
            Slot::Verb(AppVerb::KeyboardShortcuts),
            Slot::Command(Action::Shortcuts),
            Slot::Separator,
            // The written guide, page by page (UI-91, #257). The rows are the catalog's, so a page
            // added to `docs/guide` appears here the frame it is listed, and one removed goes.
            Slot::Guide,
            Slot::Separator,
            Slot::Verb(AppVerb::GettingStarted),
            Slot::Verb(AppVerb::WhatsNew),
            Slot::Separator,
            Slot::Verb(AppVerb::ReportProblem),
            Slot::Verb(AppVerb::About),
        ],
        _ => Vec::new(),
    };
    let left_over: Vec<Slot> = registry
        .iter()
        .filter(|c| c.category() == category)
        .filter(|c| !named.iter().any(|s| matches!(s, Slot::Command(a) if *a == c.action)))
        .map(|c| Slot::Command(c.action))
        .collect();
    let mut slots: Vec<Slot> = named.into_iter().chain(left_over).collect();
    if category == "view" {
        slots.push(Slot::Separator);
        slots.push(Slot::Statistics);
        slots.push(Slot::ActivityBar);
        slots.push(Slot::LeftPanel);
        slots.push(Slot::Zen);
        slots.push(Slot::Verb(AppVerb::ShowProblems));
        slots.push(Slot::Verb(AppVerb::MaximiseTerminal));
        slots.push(Slot::ReloadPanel);
        slots.push(Slot::Outliner);
        slots.push(Slot::FileOutline);
        slots.push(Slot::Library);
        slots.push(Slot::GameUi);
        #[cfg(feature = "bevy_light_grid")]
        slots.push(Slot::LightMap);
        slots.push(Slot::StyleAreas);
        slots.push(Slot::Familiar);
        slots.push(Slot::PauseAgent);
        slots.push(Slot::FollowAgent);
        slots.push(Slot::Separator);
        slots.push(Slot::ResetLayout);
    }
    slots
}

/// **The bar itself, at the top of the window.** The chrome bar was index 0 of the frame's root, so the
/// menu bar goes above it: index 0, and the chrome bar moves to 1.
fn spawn_menu_bar(commands: &mut Commands, frame: &Frame, live: Option<Live>, show_statistics: bool, show_activity_bar: bool, recent: Vec<PathBuf>) {
    let live = live.unwrap_or_default();
    // One `Vec` per menu, built here rather than inside the spawner: the spawners are closures the
    // relationship machinery calls later, and a registry read inside one would be the second place
    // that decides what a menu holds.
    let registry = commands::all();
    let menus: Vec<(&'static str, Vec<Slot>)> =
        MENUS.iter().map(|(category, title)| (*title, layout(category, &registry, recent.clone()))).collect();
    let bar = commands
        .spawn((
            EditorMenuBar,
            bevy_gooey::menu_bar(Children::spawn(SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                for (title, slots) in menus {
                    parent.spawn(bevy_gooey::menu_title(
                        title,
                        Children::spawn(SpawnWith(move |menu: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                            for slot in slots {
                                spawn_slot(menu, slot, live, show_statistics, show_activity_bar);
                            }
                        })),
                    ));
                }
            }))),
        ))
        .id();
    commands.entity(frame.root).insert_children(0, &[bar]);
}

/// One row of a menu.
fn spawn_slot(
    menu: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>,
    slot: Slot,
    live: Live,
    show_statistics: bool,
    show_activity_bar: bool,
) {
    match slot {
        Slot::Command(action) => {
            let command = commands::command(action);
            menu.spawn((MenuCommand(action), bevy_gooey::menu_item(command.title).with_shortcut(command.shortcut_in(live))));
        }
        Slot::Guide => {
            for page in crate::help::PAGES {
                menu.spawn((AppVerb::GuidePage(page.id), bevy_gooey::menu_item(page.title)));
            }
        }
        Slot::Verb(verb) => {
            let mut item = bevy_gooey::menu_item(verb.title());
            if let Some(shortcut) = verb.shortcut() {
                item = item.with_shortcut(shortcut);
            }
            menu.spawn((verb, item));
        }
        Slot::Recent(projects) => {
            let rows: Vec<_> = projects
                .into_iter()
                .map(|path| {
                    let verb = AppVerb::OpenRecent(path);
                    let item = bevy_gooey::menu_item(verb.title());
                    (verb, item)
                })
                .collect();
            menu.spawn(bevy_gooey::submenu("Open recent", Children::spawn(SpawnIter(rows.into_iter()))));
        }
        Slot::Separator => {
            menu.spawn(bevy_gooey::separator());
        }
        Slot::ResetLayout => {
            menu.spawn((AppVerb::ResetLayout, bevy_gooey::menu_item(AppVerb::ResetLayout.title())));
        }
        Slot::Familiar => {
            menu.spawn((
                crate::familiar::FamiliarItem,
                bevy_gooey::check_item("Show agent", false),
                bevy_gooey::observe(crate::familiar::toggle_the_familiar),
            ));
        }
        Slot::PauseAgent => {
            menu.spawn((
                crate::familiar::PauseItem,
                bevy_gooey::check_item("Pause agent writes", false),
                bevy_gooey::observe(crate::familiar::toggle_the_pause),
            ));
        }
        Slot::FollowAgent => {
            menu.spawn((
                crate::familiar::FollowItem,
                bevy_gooey::check_item("Follow the agent", true),
                bevy_gooey::observe(crate::familiar::toggle_the_follow),
            ));
        }
        Slot::LeftPanel => {
            menu.spawn((
                crate::shell_view::LeftPanelItem,
                bevy_gooey::check_item("Show left panel", true),
                bevy_gooey::observe(crate::shell_view::toggle_the_left_panel),
            ));
        }
        Slot::Zen => {
            menu.spawn((
                crate::shell_view::ZenItem,
                bevy_gooey::check_item("Zen mode", false),
                bevy_gooey::observe(crate::shell_view::toggle_zen),
            ));
        }
        Slot::ReloadPanel => {
            menu.spawn((
                crate::shell_view::ReloadPanelItem,
                bevy_gooey::check_item("Show reload panel", false),
                bevy_gooey::observe(crate::shell_view::toggle_the_reload_panel),
            ));
        }
        Slot::Outliner => {
            menu.spawn((
                crate::outliner::OutlinerItem,
                bevy_gooey::check_item("Show outliner", false),
                bevy_gooey::observe(crate::outliner::toggle_the_outliner),
            ));
        }
        // **View ▸ Show outline** (SG-04, #403): spawned on demand, for the outliner's reason.
        Slot::FileOutline => {
            menu.spawn((
                crate::file_outline::OutlineItem,
                bevy_gooey::check_item("Show outline", false),
                bevy_gooey::observe(crate::file_outline::toggle_the_outline),
            ));
        }
        // **View ▸ Show library** (UI-52, #238): the library as a grid of portraits, with its own search and
        // a tag row. Asked for here for the outliner's reason: a text box and a grid of cards are entities
        // that the screens counting every button in the world would find.
        Slot::Library => {
            menu.spawn((
                crate::library_browser::LibraryItem,
                bevy_gooey::check_item("Show library", false),
                bevy_gooey::observe(crate::library_browser::toggle_the_library),
            ));
        }
        // **View ▸ Game UI preview** (UI-03, #211): the one place a project's game skin is drawn. Asked
        // for here for the outliner's reason — the sample screen's buttons are entities the screens
        // counting every button in the world would find — so the panel is spawned on demand.
        Slot::GameUi => {
            menu.spawn((
                crate::game_ui_preview::GameUiPreviewItem,
                bevy_gooey::check_item("Game UI preview", false),
                bevy_gooey::observe(crate::game_ui_preview::toggle_the_preview),
            ));
        }
        // **View ▸ Light map** (UI-80, #254). Gated with the slot: a build without the overlay has no
        // row that looks live and does nothing, which is what `commands::enabled` exists to prevent.
        #[cfg(feature = "bevy_light_grid")]
        Slot::LightMap => {
            menu.spawn((
                crate::light_overlay::LightMapItem,
                bevy_gooey::check_item("Light map", false),
                bevy_gooey::observe(crate::light_overlay::on_light_map_item),
            ));
        }
        // **View ▸ Show style areas** (SG-06, #405): a tick for the overlay that outlines the open map's areas. Off by default.
        Slot::StyleAreas => {
            menu.spawn((
                crate::design_areas::StyleAreasItem,
                bevy_gooey::check_item("Show style areas", false),
                bevy_gooey::observe(crate::design_areas::on_style_areas_item),
            ));
        }
        Slot::ActivityBar => {
            menu.spawn((
                ActivityBarItem,
                bevy_gooey::check_item("Show activity bar", show_activity_bar),
                bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<bool>>, file: Option<Res<crate::settings::UserSettingsFile>>| {
                    if let Some(file) = file
                        && let Err(why) = crate::setting_catalogue::write_key(&file.0, "show_activity_bar", Some(crate::setting_catalogue::Value::On(change.value)))
                    {
                        bevy::log::warn!("could not save Show activity bar: {why}");
                    }
                }),
            ));
        }
        Slot::Statistics => {
            menu.spawn((
                StatisticsItem,
                bevy_gooey::check_item("Show statistics", show_statistics),
                bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<bool>>, file: Option<Res<crate::settings::UserSettingsFile>>| {
                    if let Some(file) = file
                        && let Err(why) = crate::setting_catalogue::write_key(&file.0, "show_statistics", Some(crate::setting_catalogue::Value::On(change.value)))
                    {
                        bevy::log::warn!("could not save Show statistics: {why}");
                    }
                }),
            ));
        }
    }
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

/// The tick on View → Show activity bar says what the settings say, whichever way they were changed.
fn follow_the_activity_bar_setting(
    settings: Option<Res<crate::settings::EditorSettings>>,
    items: Query<(Entity, Has<bevy_gooey::Checked>), With<ActivityBarItem>>,
    mut commands: Commands,
) {
    let Some(settings) = settings else { return };
    for (item, ticked) in &items {
        match (settings.show_activity_bar, ticked) {
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

/// The tick on View → Show statistics says what the settings say, whichever way they were changed.
fn follow_the_statistics_setting(
    settings: Option<Res<crate::settings::EditorSettings>>,
    items: Query<(Entity, Has<bevy_gooey::Checked>), With<StatisticsItem>>,
    mut commands: Commands,
) {
    let Some(settings) = settings else { return };
    for (item, ticked) in &items {
        match (settings.show_statistics, ticked) {
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
