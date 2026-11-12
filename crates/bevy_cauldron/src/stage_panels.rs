//! **Every panel can be staged for a capture** (UI-92, #258).
//!
//! A changelog clip of a panel has to start on that panel. The only way to get one there was a person's keys and clicks —
//! a storyboard guessing pixels at a tab's place — so the panels most worth recording (the launcher, the loader, the
//! settings screen, each dock panel) were the ones no clip could be made of.
//!
//! This module is the catalog and the one function that honours a request: a name in, the shell arranged to show that
//! panel, or a refusal saying why not. `tools/demo/record.py` sends the request through `grimoire/stage_panel` before a
//! sequence starts, and so can any BRP caller, so a capture and an agent's own look go through one door.
//!
//! # What a panel is, here
//!
//! Three of the names are *screens* — `launcher`, `loader` and `settings` — and the rest are the workbench's panels, by the
//! id [`bevy_gooey::PanelRegistry`] registers them under (`inspector`, `viewport`, `place`). A dock panel is
//! staged by making its group's tab the selected one, which is what a click on that tab does, so a clip shows the panel a
//! person would be looking at rather than a layout built for the camera.

use bevy::prelude::*;
use bevy_gooey::{Dock, DockLayout, PanelRegistry};

use crate::screen::Screen;

/// One thing a capture can start on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage {
    /// The name a storyboard or an agent asks for.
    pub id: &'static str,
}

/// **Every panel a capture can be staged on**, in the order a person meets them: the three screens before and around the
/// work, then the dock panels in the shell's own left-to-right order.
///
/// **The status band is not here, on purpose** ([`NOT_STAGED`]): the frame draws it under every workbench panel rather
/// than by a tab, so a stage for it would either lie or build a layout for the camera.
pub const STAGES: &[Stage] = &[
    Stage { id: "launcher" },
    Stage { id: "loader" },
    Stage { id: "settings" },
    Stage { id: "inspector" },
    Stage { id: "viewport" },
    Stage { id: "place" },
];

/// **How long the loader is held on each stage when it is staged**, seconds: two frames at the slowest rate a capture
/// sequence may ask for (`sequencer::MIN_FPS`), so a staged loader always has a frame to give.
pub const STAGE_HOLD_S: f32 = 1.0;

/// The stage with this name.
pub fn stage(id: &str) -> Option<&'static Stage> {
    STAGES.iter().find(|stage| stage.id == id)
}

/// Every stage's name, in the catalog's order, for a refusal that says what would have worked.
pub fn names() -> String {
    STAGES.iter().map(|stage| stage.id).collect::<Vec<_>>().join(", ")
}

/// **Whether a name is a screen rather than a dock panel**: a screen is a `Screen` state or a dialog over the workbench,
/// never a `PanelRegistry` entry.
pub fn is_screen(id: &str) -> bool {
    matches!(id, "launcher" | "loader" | "settings")
}

/// **Panels the shell registers and a capture cannot stage**, with the reason, so the catalog can be checked against the
/// registry in both directions. The status band is drawn by the frame under every workbench panel rather than by a tab, so
/// it has no group to select and is already in every workbench clip.
pub const NOT_STAGED: &[(&str, &str)] = &[(
    "status",
    "the status band is drawn by the frame under every workbench panel, not by a tab, so it is in every workbench clip",
)];

/// **The order a run stages them in**: the workbench's own panels first, then the settings dialog over them, then the
/// loader, then the launcher last — because leaving the workbench is what the last two do, and only a stage that moves
/// the screen can come after it. A run in the catalog's reading order would refuse `inspector` after `launcher`.
pub fn capture_order() -> Vec<&'static str> {
    let mut order: Vec<&'static str> = dock_panels();
    order.extend(["settings", "loader", "launcher"]);
    order
}

/// The dock panels, in the catalog's order.
pub fn dock_panels() -> Vec<&'static str> {
    STAGES.iter().map(|stage| stage.id).filter(|id| !is_screen(id)).collect()
}

/// **What staging did**, so a caller has to say what it got rather than assume it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    /// The panel that was staged.
    pub id: &'static str,
    /// The screen the editor is on, or is moving to this frame.
    pub screen: Screen,
}

/// **Arrange the shell so `id` is what a person is looking at**, and say what it arranged — or refuse, in words that name
/// the panel, what the shell has, and what would have worked.
///
/// The one function: `grimoire/stage_panel` calls it, and nothing else does, so a capture and an agent cannot disagree
/// about what staging a panel means.
pub fn arrange(world: &mut World, id: &str) -> Result<Staged, String> {
    let Some(stage) = stage(id) else {
        return Err(format!("`{id}` is not a panel a capture can start on. The panels are: {}.", names()));
    };
    // **No resource is assumed to exist.** `resource::<..>()` panics in Bevy when a resource is absent, and a BRP
    // command is reachable from a screen whose plugins have not put it there — so a missing one is a refusal that
    // names it, not a panic that takes the editor down.
    let Some(now) = world.get_resource::<State<Screen>>().map(|state| *state.get()) else {
        return Err("the editor has no screen state, so there is nothing here to arrange. Open a project first.".to_owned());
    };
    // **What the last stage put up comes down first**, or a clip of the panel after the settings screen would film the
    // settings screen. The dialog is gooey's own, so the way down is the `Leave` a person's Escape writes.
    if stage.id != "settings" {
        let mut open: Vec<Entity> = world.query_filtered::<Entity, With<crate::settings_screen::SettingsDialog>>().iter(world).collect();
        for dialog in open.drain(..) {
            world.entity_mut(dialog).insert(bevy_gooey::Leave);
        }
    }
    match stage.id {
        // **The launcher is a state**, reached the way File ▸ Close project reaches it.
        "launcher" => {
            let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() else {
                return Err("the editor has no screen state to move to, so the launcher cannot be staged".to_owned());
            };
            next.set(Screen::Launcher);
            Ok(Staged { id: stage.id, screen: Screen::Launcher })
        }
        // **The loader is a state with work in it**, so staging it asks for the open it shows: the project this editor
        // has, through the same argv the command line takes, so the clip is of a project actually opening.
        "loader" => {
            let Some(argv) = re_open_argv(world) else {
                return Err("`loader` shows a project opening, and no project is open to open again".to_owned());
            };
            let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() else {
                return Err("the editor has no screen state to move to, so the loader cannot be staged".to_owned());
            };
            next.set(Screen::Loading);
            world.insert_resource(crate::loading::LoadRequest(argv));
            world.insert_resource(crate::loading::StageDelay(std::time::Duration::from_secs_f32(STAGE_HOLD_S)));
            Ok(Staged { id: stage.id, screen: Screen::Loading })
        }
        // **The settings screen is a dialog over the workbench**, so it goes up where the person is.
        "settings" => {
            world.write_message(crate::settings_screen::OpenSettings);
            Ok(Staged { id: stage.id, screen: now })
        }
        panel => stage_a_dock_panel(world, panel, now),
    }
}

/// **The argv that opens the project this editor has**, door, map and kit included, so the loader shows the same project
/// a person opened rather than a default one. The words are the ones [`crate::args::split`] reads: the root and the map as
/// positionals, then `--door` and `--kit`.
fn re_open_argv(world: &World) -> Option<Vec<String>> {
    let project = world.get_resource::<crate::project::Project>()?;
    let door = world.get_resource::<crate::workspace::Workspace>().copied().unwrap_or_default();
    let mut argv = vec![project.root.to_string_lossy().into_owned()];
    if let Some(map) = world.get_resource::<crate::project::OpenMap>().map(|open| open.map.name.clone()).or_else(|| project.open_map_name.clone()) {
        argv.push(map);
    }
    argv.push("--door".to_owned());
    argv.push(door.label().to_lowercase());
    // A kit is the directory the library was read from, when it is not the project's own default.
    let default = project.root.join("assets").join("grimoire");
    if let Ok(rest) = project.grimoire_dir.strip_prefix(&default)
        && let Some(name) = rest.file_name()
    {
        argv.push("--kit".to_owned());
        argv.push(name.to_string_lossy().into_owned());
    }
    Some(argv)
}

/// **A dock panel is staged by selecting its tab**, in every dock that holds it: the shell's arrangement is data, so this
/// is a write to that data and the dock shows it the same frame. A layout already showing the panel is left alone, so a
/// request for what is on screen does not reach `ui::save_the_layout` as a change.
fn stage_a_dock_panel(world: &mut World, panel: &str, now: Screen) -> Result<Staged, String> {
    let registered: Vec<&'static str> = match world.get_resource::<PanelRegistry>() {
        Some(registry) => dock_panels().into_iter().filter(|id| registry.contains(id)).collect(),
        None => Vec::new(),
    };
    if !registered.contains(&panel) {
        return Err(format!(
            "`{panel}` is not a panel this shell registers, so there is nothing to show. Registered: {}.",
            registered.join(", ")
        ));
    }
    if now != Screen::Editor {
        return Err(format!("`{panel}` is a workbench panel and the editor is not open (the screen is {now:?})"));
    }
    let mut docked = false;
    // **The arrangement's dock, and only it** (review of #247): a tab is selected in the dock that
    // arranges the shell, never in the bottom dock, whose one panel (`reload`) no capture stages.
    let mut docks = world.query_filtered::<&mut Dock, With<crate::chrome::ArrangementDock>>();
    for mut dock in docks.iter_mut(world) {
        if let Some(next) = select(&dock.0, panel) {
            dock.0 = next;
            docked = true;
        } else if dock.0.contains(panel) {
            docked = true;
        }
    }
    if !docked {
        return Err(format!(
            "`{panel}` is registered and no dock shows it, so there is no tab to select: the arrangement in force does not name it"
        ));
    }
    // The catalog's own `&'static str`, so a `Staged` never carries a caller's string.
    let id = stage(panel).map_or("unknown", |stage| stage.id);
    Ok(Staged { id, screen: now })
}

/// **The layout with `panel`'s tab selected**, or `None` when it is already the front one.
pub fn select(layout: &DockLayout, panel: &str) -> Option<DockLayout> {
    fn walk(node: &DockLayout, panel: &str) -> Option<DockLayout> {
        match node {
            DockLayout::Split { axis, fraction, first, second } => {
                let new_first = walk(first, panel);
                let new_second = walk(second, panel);
                (new_first.is_some() || new_second.is_some()).then(|| {
                    DockLayout::split(
                        *axis,
                        *fraction,
                        new_first.unwrap_or_else(|| (**first).clone()),
                        new_second.unwrap_or_else(|| (**second).clone()),
                    )
                })
            }
            DockLayout::Group { panels, selected } => {
                let at = panels.iter().position(|id| id == panel)?;
                (*selected != at).then(|| DockLayout::Group { panels: panels.clone(), selected: at })
            }
        }
    }
    walk(layout, panel)
}
