//! **The activity bar** (NV-03, #380; series getting around, #377): a vertical bar on the window's far
//! left with one icon per *registered* workspace, in registry order.
//!
//! # What it is built from
//!
//! gooey's [`bevy_gooey::side_tabs_icons`] — a vertical `TabList` whose tabs are square targets holding
//! an icon, a tooltip naming the tab and a [`bevy_gooey::CornerBadge`] pill — so the sliding selection
//! indicator, the arrows, Enter and the roving focus are gooey's, not a second copy here. What is the
//! editor's is which tabs there are ([`Workspace::is_built`]: no placeholder screens), what the icon is
//! (`Workspace::icon`, from the editor's icon set), and what a pick does (`door_open`, the operation a key,
//! the menu, the palette and an agent already call).
//!
//! # The badges, and only the ones that have a source
//!
//! A badge is a count of what needs a person ([`badge`]); a count of zero draws nothing, so a bar that has
//! nothing to say is silent. Map and Files carry **the problems count for their files** — the same refused
//! and needs-a-restart reloads the status band's `1 refused` says. Agent carries **the wishes and lessons
//! waiting for an answer**. A workspace with no source draws no badge: never a placeholder number.
//!
//! **Agent and Files are not registered yet** (NV-10 and GD-11), so their tabs, and the Agent badge with
//! them, are not on the bar today; [`badge`] already answers for them, so registering the workspace is the
//! only change either needs. The Agent badge **replaces the status-band chip** (AW-02, #354) *when that
//! tab exists*, so there is one notice per waiting thing and not two; until then the chip is the only
//! notice there is, and it stays.
//!
//! # Spawned on demand, never hidden
//!
//! The bar is spawned while it is wanted and despawned when it is not, never `Display::None`: screens
//! count every `ButtonText` in the world and take the first empty text field as their search box, and a
//! hidden widget is still in the world. **View ▸ Show activity bar** is the `show_activity_bar` setting, so
//! the choice is saved in the user's settings file.
//!
//! A badge cannot be changed on a gooey tab once spawned, so the bar is rebuilt when what it shows changes
//! (a workspace registered, a count moved): rare, and the selection is carried across in the rebuild.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_gooey::{FocusRegion, IconSource, IconTab, SideTab, TabsValue, Tooltip, ValueChange};

use crate::chrome::{EDGE_W, Frame, ink};
use crate::keys::Action;
use crate::screen::Screen;
use crate::workspace::Workspace;

/// The name a screen reader and a test find the bar by.
pub const BAR_NAME: &str = "Activity bar";

/// The bar: the strip's container, spawned into the frame's body at its far left.
///
/// It records the frame it was spawned into and what it was drawn with, so [`keep_the_activity_bar`] can
/// tell a bar that is current from one that is stale — a new frame (the old one's root, and the bar with
/// it, are despawned), or a count that moved.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ActivityBar {
    /// The frame root this bar belongs to.
    pub frame: Entity,
    /// Each tab's workspace and the badge it was drawn with (`0` is none), in tab order.
    pub shown: Vec<(Workspace, usize)>,
}

/// The gooey `side_tabs_icons` strip inside the bar: the entity that holds [`TabsValue`] and reports a
/// pick as a `ValueChange<usize>`. Its tabs, in order, are [`ActivityBar::shown`]'s workspaces.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ActivityStrip(pub Vec<Workspace>);

pub struct ActivityBarPlugin;

impl Plugin for ActivityBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (keep_the_activity_bar, describe_the_tabs, follow_the_workspace)
                .chain()
                .after(crate::chrome::FrameSystems)
                .run_if(in_state(Screen::Editor)),
        );
    }
}

/// **The badge for a workspace**, or `0` for none: `problems` is the reloads a file's edit did not take
/// (refused, or waiting on a restart), `waiting` is the wishes and lessons nobody has answered.
///
/// `Game` answers `0` here: its badge is a state, not a count (a crash, [`crate::game_view::GameView::crashed`]), and the
/// bar adds it where it reads the game ([`keep_the_activity_bar`]) so this stays a function of its inputs.
pub fn badge(workspace: Workspace, problems: usize, waiting: usize) -> usize {
    match workspace {
        Workspace::Map => problems,
        Workspace::Agent => waiting,
        // `Files` answers `0` (GD-11, #373): the count is the reloads a file's edit did not take, which the Map tab says, and
        // a second tab saying the same number would be two places for one fact (the protected badge test pins Map alone).
        Workspace::Project | Workspace::World | Workspace::Kit | Workspace::Animation | Workspace::Sound | Workspace::Files | Workspace::Game | Workspace::Design => 0,
    }
}

/// **The workspaces the bar draws**: the registered ones, in the order the keys follow.
pub fn registered() -> Vec<Workspace> {
    registered_in(&crate::workspace::WAITING)
}

/// **[`registered`] against a table of unbuilt workspaces** — the seam a test fills (every real workspace is built), so the bar's filter is
/// proved to leave a waiting workspace out.
pub fn registered_in(waiting: &[(Workspace, &'static str)]) -> Vec<Workspace> {
    Workspace::ALL.into_iter().filter(|w| crate::workspace::waiting_reason(waiting, *w).is_none()).collect()
}

/// **The reloads a file's edit did not take**: refused ones and ones that need a restart. The one count
/// the status band's problems item and the Map tab's badge both say.
pub fn problems(reloads: Option<&crate::live::EditorReloads>) -> usize {
    use bevy_grimoire_core::live::Outcome;
    reloads.map_or(0, |r| r.kinds().iter().filter(|k| matches!(k.outcome, Outcome::Refused | Outcome::NeedsRestart)).count())
}

/// **The wishes and lessons nobody has answered** — what the status band's chip counts.
pub fn waiting(wishes: Option<&crate::present::Wishes>, lessons: Option<&crate::present::Lessons>) -> usize {
    wishes.map_or(0, crate::present::Wishes::waiting) + lessons.map_or(0, crate::present::Lessons::waiting)
}

/// **Everything waiting for the person in the AGENT block**: the wishes, the lessons and the style proposals (SG-07, #406).
pub fn waiting_on_you(wishes: Option<&crate::present::Wishes>, lessons: Option<&crate::present::Lessons>, styles: Option<&crate::style_proposals::StyleProposals>) -> usize {
    waiting(wishes, lessons) + styles.map_or(0, crate::style_proposals::StyleProposals::waiting)
}

/// What [`keep_the_activity_bar`] reads, bundled so the system stays under clippy's argument limit.
#[derive(SystemParam)]
struct Wants<'w> {
    settings: Option<Res<'w, crate::settings::EditorSettings>>,
    view: Option<Res<'w, crate::shell_view::ShellView>>,
    reloads: Option<Res<'w, crate::live::EditorReloads>>,
    wishes: Option<Res<'w, crate::present::Wishes>>,
    lessons: Option<Res<'w, crate::present::Lessons>>,
    styles: Option<Res<'w, crate::style_proposals::StyleProposals>>,
    game: Option<Res<'w, crate::game_view::GameView>>,
}

/// **Spawn the bar while it is wanted, and keep it what it should show.**
///
/// A system rather than `OnEnter` for the menu bar's reason: the bar is a child of the frame's body, which
/// the frame respawns on every entry, so "there is a frame and no bar for it" is the one condition that
/// covers a new screen, a new workspace and the setting turned back on.
fn keep_the_activity_bar(
    mut commands: Commands,
    frame: Option<Res<Frame>>,
    workspace: Option<Res<Workspace>>,
    atlas: Option<Res<crate::toolbar::ToolbarAtlas>>,
    wants: Wants,
    bars: Query<(Entity, &ActivityBar)>,
) {
    let Some(frame) = frame else { return };
    // **Zen mode takes the bar with everything else** (`shell_view::ZenParts`): it is not drawn, so it is
    // not spawned, rather than hidden.
    let wanted = wants.settings.is_none_or(|s| s.show_activity_bar) && !wants.view.is_some_and(|v| v.zen);
    let problems = problems(wants.reloads.as_deref());
    let waiting = waiting_on_you(wants.wishes.as_deref(), wants.lessons.as_deref(), wants.styles.as_deref());
    let crashed = usize::from(wants.game.as_deref().is_some_and(crate::game_view::GameView::crashed));
    let shown: Vec<(Workspace, usize)> = registered()
        .into_iter()
        .map(|w| (w, badge(w, problems, waiting) + if w == Workspace::Game { crashed } else { 0 }))
        .collect();

    let mut current = false;
    for (entity, bar) in &bars {
        if wanted && bar.frame == frame.root && bar.shown == shown {
            current = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    if !wanted || current || shown.is_empty() {
        return;
    }

    let selected = workspace.and_then(|w| shown.iter().position(|(shown, _)| *shown == *w)).unwrap_or(0);
    let tabs: Vec<SideTab> = shown
        .iter()
        .map(|(workspace, count)| {
            let tab = SideTab::new(workspace.title()).count(*count);
            match atlas.as_deref().and_then(|atlas| {
                crate::icons::index(workspace.icon())
                    .map(|index| IconSource::Atlas { image: atlas.image.clone(), layout: atlas.layout.clone(), index })
            }) {
                Some(source) => tab.icon(source),
                None => tab,
            }
        })
        .collect();

    let strip = commands
        .spawn((bevy_gooey::side_tabs_icons(tabs, selected), ActivityStrip(shown.iter().map(|(w, _)| *w).collect())))
        .observe(go_to_the_workspace)
        .id();
    let bar = commands
        .spawn((
            ActivityBar { frame: frame.root, shown },
            // A column of the tabs, as tall as the frame, that never gives way, with the edge it shares with the docks along its right.
            bevy_gooey::column_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Stretch), shrink: Some(0.0), ..default() }),
            bevy_gooey::Sizing { height: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            bevy_gooey::BorderPx(bevy_gooey::Sides { right: Some(EDGE_W), ..default() }),
            bevy_gooey::InkOverride { edge: Some(ink().panel_edge), ..default() },
            BackgroundColor(ink().bar_bg),
            BorderColor::all(ink().panel_edge),
            // **A focus region of its own, so F6 reaches it** (UI-35, #229): viewport → bar → docks and
            // back. The arrows move between its tabs, which is what `ArrowPolicy::Navigate` is.
            FocusRegion::navigate(),
            AccessibleLabel::new(BAR_NAME),
        ))
        .add_child(strip)
        .id();
    commands.entity(frame.body).insert_children(0, &[bar]);
}

/// **A pick on the bar goes to that workspace**, by `door_open` — the one operation every route takes — so
/// the question about unsaved edits, the refusal and the history are the same as the key's.
///
/// Deferred, because the switch takes the whole `World`; the `EditorState` check is for the reason
/// `tiles::cross_on_click` has it: an observer fires on every screen.
fn go_to_the_workspace(change: On<ValueChange<usize>>, strips: Query<&ActivityStrip>, mut commands: Commands) {
    let Ok(strip) = strips.get(change.source) else { return };
    let Some(workspace) = strip.0.get(change.value).copied() else { return };
    commands.queue(move |world: &mut World| {
        if world.contains_resource::<crate::editor::EditorState>() {
            crate::editor::open_door(world, workspace, None);
        }
    });
}

/// **Give each tab its tooltip: the workspace's name and its shortcut.**
///
/// gooey's tab draws the name as its tooltip already; this adds the chord on a keycap beside it, from the
/// workspace's own command (`workspace.<id>`), so the bar and the keys cannot disagree.
fn describe_the_tabs(
    mut commands: Commands,
    live: Option<Res<crate::keys::Live>>,
    tabs: Query<(Entity, &ChildOf), Added<IconTab>>,
    lists: Query<(&Children, &ChildOf)>,
    strips: Query<&ActivityStrip>,
) {
    for (tab, parent) in &tabs {
        let Ok((siblings, list_parent)) = lists.get(parent.parent()) else { continue };
        let Ok(strip) = strips.get(list_parent.parent()) else { continue };
        let Some(workspace) = siblings.iter().position(|s| s == tab).and_then(|i| strip.0.get(i)).copied() else { continue };
        let Some(action) = Action::ALL.into_iter().find(|a| a.workspace() == Some(workspace)) else { continue };
        let command = crate::commands::command(action);
        let shortcut = live.as_deref().map_or(command.shortcut, |live| command.shortcut_in(*live));
        commands.entity(tab).insert(Tooltip::new(workspace.title()).with_shortcut(shortcut));
    }
}

/// **The indicator is on the workspace that is open**, whichever way it was reached — a key, the menu, the
/// palette, an agent or a pick on the bar itself. gooey reports no change for a value the host writes, so
/// this does not loop back into [`go_to_the_workspace`].
fn follow_the_workspace(workspace: Option<Res<Workspace>>, mut strips: Query<(&ActivityStrip, &mut TabsValue)>) {
    let Some(workspace) = workspace else { return };
    for (strip, mut value) in &mut strips {
        let Some(index) = strip.0.iter().position(|w| w == workspace.as_ref()) else { continue };
        if value.0 != index {
            value.0 = index;
        }
    }
}
