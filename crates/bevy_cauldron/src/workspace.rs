//! **The editor's workspaces** — the two-axis model that replaced the doors (NV-02, #379).
//!
//! # What a door was, and what a workspace is
//!
//! A *door* was one entity you could work on, and it showed the panels that entity needed: the Kit
//! door showed Meshes, Tiles and Compose, the Map door showed the Map, and the Rigs door showed the
//! rig bench. That was a *place* and a *job* welded into one value, which is why there were three of
//! them and why crossing cost a reload.
//!
//! A **workspace** is only the *job*. Which panels are on screen is a second axis, and it is the
//! dock layout: [`Self::default_layout`] says where a workspace starts and the person's own
//! arrangement says where it is now. Crossing between workspaces keeps the project, the open map and
//! the selection, because the two axes no longer share a value — `docs/ui/vision.md` §3 and §4.
//!
//! # Nine, and only the built ones are registered
//!
//! [`Self::ALL`] is the canonical order the keys follow, and it is fixed for good: `Cmd`/`Ctrl`+`1`
//! is always Project, so a person's fingers keep their meaning across releases. A workspace whose
//! part is not built yet is **not registered** ([`Self::is_built`]) — a placeholder screen is a
//! promise the editor cannot keep — and its command says which issue brings it ([`Self::missing`]).
//! That is why the key exists before the workspace does: the chord is stable, and the command answers
//! *why* it does nothing, which is the rule `commands::Command::enabled` already keeps for every
//! other verb.

use bevy::prelude::*;
use bevy_gooey::{DockLayout, SplitAxis};

use crate::tiles::Mode;

/// **One axis of the editor's navigation**: the job a person is doing.
///
/// A `Resource` because exactly one workspace is on screen, and it is the value every panel, every
/// run condition and the window title read. [`Mode`] is the other axis — which panel of that job —
/// and it stays a separate value so a workspace can show three of them.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug, Hash)]
pub enum Workspace {
    /// The project page: its maps, its kits, the guide, what is new.
    Project,
    /// Build the level out of the library.
    #[default]
    Map,
    /// The world under the level: terrain, water, forest, lights.
    World,
    /// Make reusable content: meshes → tiles → compositions.
    Kit,
    /// The rig bench.
    Animation,
    /// Cue tables and music scores.
    Sound,
    /// The project's files and the editor for them.
    Files,
    /// The running game, beside its own window.
    Game,
    /// The Familiar: what it is doing, and what it is waiting on.
    Agent,
    /// **The game's style** (SG-05, #404): the design tree on the left, the selected page rendered in the middle, and that place's dream board on the right.
    Design,
}

/// **The key every workspace's default layout is made of** — the four regions of the shell.
///
/// Named here rather than in `ui.rs` because a workspace's default layout is a fact about the
/// workspace, and a second list of ids in the module that draws them is the second census this
/// crate refuses.
pub const INSPECTOR: &str = "inspector";
/// The map, or whatever the workspace shows in the middle.
pub const VIEWPORT: &str = "viewport";
/// The library the map places from.
pub const PLACE: &str = "place";

/// **The workspaces whose part is not built yet, each with the issue that brings it** (the sentence a refused chord answers with).
///
/// Empty since the Project workspace (NV-08, #385) was the last: every workspace is registered. The table stays, because the refusal it
/// feeds ([`Workspace::missing`], `Command::enabled`, `Action::is_offered`) is how a *future* workspace's chord exists before its panel
/// does, and `an_unbuilt_workspace_is_refused_with_its_reason.rs` proves that mechanism on a table that is not empty.
pub const WAITING: [(Workspace, &str); 0] = [];

/// **What `workspace` is waiting for in `table`**, or `None` when it is not listed there. The lookup [`Workspace::missing`] makes, over a
/// table a test can fill.
pub const fn waiting_reason(table: &[(Workspace, &'static str)], workspace: Workspace) -> Option<&'static str> {
    let mut at = 0;
    while at < table.len() {
        if table[at].0 as u8 == workspace as u8 {
            return Some(table[at].1);
        }
        at += 1;
    }
    None
}

impl Workspace {
    /// **Every workspace there is, in the order the keys follow.** Fixed for good: see the module
    /// note.
    pub const ALL: [Workspace; 10] = [
        Workspace::Project,
        Workspace::Map,
        Workspace::World,
        Workspace::Kit,
        Workspace::Animation,
        Workspace::Sound,
        Workspace::Files,
        Workspace::Game,
        Workspace::Agent,
        // **Appended, so no chord changes meaning** (SG-05, #404): `Cmd`/`Ctrl`+`0` is the tenth.
        Workspace::Design,
    ];

    /// **The workspace's command id**, spelled `workspace.<id>` (P1: one name for the key, the
    /// menu, the palette and an agent).
    pub const fn id(self) -> &'static str {
        match self {
            Workspace::Project => "project",
            Workspace::Map => "map",
            Workspace::World => "world",
            Workspace::Kit => "kit",
            Workspace::Animation => "animation",
            Workspace::Sound => "sound",
            Workspace::Files => "files",
            Workspace::Game => "game",
            Workspace::Agent => "agent",
            Workspace::Design => "design",
        }
    }

    /// The name in the chrome bar and on the workspace's tab: the id, in capitals.
    pub const fn label(self) -> &'static str {
        match self {
            Workspace::Project => "PROJECT",
            Workspace::Map => "MAP",
            Workspace::World => "WORLD",
            Workspace::Kit => "KIT",
            Workspace::Animation => "ANIMATION",
            Workspace::Sound => "SOUND",
            Workspace::Files => "FILES",
            Workspace::Game => "GAME",
            Workspace::Agent => "AGENT",
            Workspace::Design => "DESIGN",
        }
    }

    /// What a menu row and a receipt call it: sentence case, one word (vision §9).
    pub const fn title(self) -> &'static str {
        match self {
            Workspace::Project => "Project",
            Workspace::Map => "Map",
            Workspace::World => "World",
            Workspace::Kit => "Kit",
            Workspace::Animation => "Animation",
            Workspace::Sound => "Sound",
            Workspace::Files => "Files",
            Workspace::Game => "Game",
            Workspace::Agent => "Agent",
            Workspace::Design => "Design",
        }
    }

    /// **The icon on this workspace's tab** (`icons::ICONS`), for the activity bar NV-03 draws.
    ///
    /// Every name here is already in the atlas, so a workspace never asks the icon set for something
    /// it does not have — `icons::index` answering `None` would be a blank tab with no way to tell
    /// why.
    pub const fn icon(self) -> &'static str {
        match self {
            Workspace::Project => "panel.library",
            Workspace::Map => "panel.outliner",
            Workspace::World => "map.generate",
            Workspace::Kit => "panel.data",
            Workspace::Animation => "map.run_mode",
            Workspace::Sound => "panel.log",
            Workspace::Files => "file.save",
            Workspace::Game => "panel.reload",
            Workspace::Agent => "panel.familiar",
            Workspace::Design => "panel.notes",
        }
    }

    /// **The panels this workspace shows**, in the order its strip lists them.
    ///
    /// This is the door's `tabs()` under the new name, and it answers the same question: *which
    /// panel of the job is this workspace showing*. `Mode` did not stop being a panel when the doors
    /// went; it stopped being a *place*.
    pub const fn panels(self) -> &'static [Mode] {
        match self {
            Workspace::Map => &[Mode::Map],
            Workspace::Kit => &[Mode::Meshes, Mode::Tiles, Mode::Compose],
            Workspace::Animation => &[Mode::Anim],
            // **The world under the level, and the audio** (NV-08, #385): each is one panel of its own.
            Workspace::World => &[Mode::World],
            Workspace::Sound => &[Mode::Sound],
            Workspace::Game => &[Mode::Game],
            Workspace::Agent => &[Mode::Agent],
            // **The game's style** (SG-05, #404): one panel of its own, drawn in the three parts the issue names.
            Workspace::Design => &[Mode::Design],
            Workspace::Files => &[Mode::Files],
            // **The project's page, inside the workbench** (NV-08, #385): its maps, its kits and the agent guide. `Screen::Project` stays
            // the page of a project with no door open (Cmd+O, Close project); this is the same page as one panel of the editor.
            Workspace::Project => &[Mode::Project],
        }
    }

    /// Where this workspace opens. The first of [`Self::panels`]; a workspace with no panels opens on
    /// the default mode, which is the Map, because that is the mode the editor starts in.
    pub fn opens_on(self) -> Mode {
        self.panels().first().copied().unwrap_or_default()
    }

    /// Does this workspace show that panel? The one place `Mode` and `Workspace` are compared.
    pub fn shows(self, mode: Mode) -> bool {
        self.panels().contains(&mode)
    }

    /// **The workspace that shows this panel.** Every `Mode` belongs to exactly one workspace, by
    /// construction of [`Self::panels`] — so a caller names the panel it wants and lets the workspace
    /// follow, rather than stating both and risking a pair that disagree.
    pub fn showing(mode: Mode) -> Workspace {
        Workspace::ALL
            .into_iter()
            .find(|w| w.shows(mode))
            .unwrap_or_default()
    }

    /// **Whether this workspace's part is built**, and so whether it is registered.
    ///
    /// A workspace that is not built is refused rather than shown empty: the vision's *"no
    /// placeholder screens"*. It is built exactly when [`WAITING`] does not list it, so the one table
    /// says both whether it is registered and what it is waiting for.
    pub const fn is_built(self) -> bool {
        self.missing().is_none()
    }

    /// **What this workspace is waiting for**, in the issue series' own words, or `None` when it is
    /// built. The command's `enabled` answer is this sentence, so a person who presses the chord
    /// reads where it comes from instead of wondering whether the key works.
    pub const fn missing(self) -> Option<&'static str> {
        waiting_reason(&WAITING, self)
    }

    /// **The workspace, by its id** — the `--workspace` flag, a command id's tail, and an agent's
    /// answer all spell it the same way.
    pub fn from_id(id: &str) -> Option<Workspace> {
        Workspace::ALL.into_iter().find(|w| w.id().eq_ignore_ascii_case(id))
    }

    /// **The `--workspace` spelling**, and the three words the flag had before the workspaces
    /// existed.
    ///
    /// `rigs` is kept as a spelling of [`Workspace::Animation`] because it is what every recorded
    /// `tools/demo` take and every note in `docs/` passes, and a flag that stops reading an old
    /// script is a flag that breaks somebody's evening for no gain. It is an *alias*, not a second
    /// name: the workspace is Animation and every word the editor says is Animation.
    pub fn from_flag(flag: &str) -> Option<Workspace> {
        let flag = flag.trim();
        if flag.eq_ignore_ascii_case("rigs") {
            return Some(Workspace::Animation);
        }
        Workspace::from_id(flag)
    }

    /// **Does this workspace edit the map?** The one workspace whose middle is the level a person
    /// is changing; the others show it.
    pub const fn edits_the_map(self) -> bool {
        matches!(self, Workspace::Map)
    }

    /// **Run condition: is the workspace that edits the map open?**
    ///
    /// `Option<Res<Workspace>>`, because **every run condition is evaluated** and this one is asked
    /// in the menu too, where there is no workspace at all. It gates what *writes* the map: since
    /// #379 the map stays open in every workspace that shows the world, so this is no longer "is
    /// there a map" but "may this workspace change one".
    pub fn map_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| w.edits_the_map())
    }

    /// **Run condition: is the World workspace open?** (NV-08, #385.) What the World's panels need to keep
    /// running — the terrain line's readout and its keys — and nothing the Map does. `Option`, for the reason
    /// [`Self::map_is_open`] is.
    pub fn world_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| *w == Workspace::World)
    }

    /// **Run condition: is the Project workspace open?** (NV-08, #385.) What the project's page, drawn inside the workbench, is spawned under.
    pub fn project_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| *w == Workspace::Project)
    }

    /// **Run condition: is the Sound workspace open?** (NV-08, #385.)
    pub fn sound_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| *w == Workspace::Sound)
    }

    /// **Run condition: is the Game workspace open?** (NV-09, #386.)
    pub fn game_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| *w == Workspace::Game)
    }

    /// **Run condition: is the Agent workspace open?** (NV-10, #387.)
    pub fn agent_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| *w == Workspace::Agent)
    }

    /// **Run condition: is the Design workspace open?** (SG-05, #404.)
    pub fn design_is_open(workspace: Option<Res<Workspace>>) -> bool {
        workspace.is_some_and(|w| *w == Workspace::Design)
    }

    /// Whether this workspace's middle is the world — the workspaces that share one viewport
    /// (`docs/ui/vision.md` §4: Map, World, Kit and Animation).
    pub const fn shows_the_map(self) -> bool {
        matches!(self, Workspace::Map | Workspace::World | Workspace::Kit | Workspace::Animation)
    }

    /// **Where this workspace starts**: its dock, before a person has moved a divider.
    ///
    /// Each is a different arrangement on purpose — the Kit workspace gives the library more room
    /// because its job is choosing pieces, the Animation bench gives the inspector more — so that a
    /// person who moves a divider in one workspace and finds it where they left it in the next has
    /// proof the layouts are per workspace rather than one shared one.
    pub fn default_layout(self) -> DockLayout {
        let horizontal = SplitAxis::Horizontal;
        match self {
            // **The page beside the view** (NV-08, #385): the project's maps, kits and guide in the left dock, like the Sound workspace's table.
            Workspace::Project => DockLayout::split(horizontal, 0.34, DockLayout::group([INSPECTOR]), DockLayout::group([VIEWPORT])),
            Workspace::Map => DockLayout::split(
                horizontal,
                0.22,
                DockLayout::group([INSPECTOR]),
                DockLayout::split(horizontal, 0.78, DockLayout::group([VIEWPORT]), DockLayout::group([PLACE])),
            ),
            // **The world's panels beside the view, and the inspector under them** (NV-08 moves
            // them here).
            Workspace::World => DockLayout::split(
                SplitAxis::Vertical,
                0.70,
                DockLayout::group([VIEWPORT]),
                DockLayout::group([INSPECTOR]),
            ),
            // **More room for the library**: making a kit is choosing pieces, and the inspector is
            // read in bursts.
            Workspace::Kit => DockLayout::split(
                horizontal,
                0.30,
                DockLayout::group([INSPECTOR]),
                DockLayout::split(horizontal, 0.72, DockLayout::group([VIEWPORT]), DockLayout::group([PLACE])),
            ),
            // **A little more for the inspector**: the bench's slots and its clip list are dense.
            Workspace::Animation => DockLayout::split(
                horizontal,
                0.28,
                DockLayout::group([INSPECTOR]),
                DockLayout::split(horizontal, 0.74, DockLayout::group([VIEWPORT]), DockLayout::group([PLACE])),
            ),
            Workspace::Sound => DockLayout::split(horizontal, 0.30, DockLayout::group([INSPECTOR]), DockLayout::group([VIEWPORT])),
            Workspace::Files => DockLayout::split(horizontal, 0.24, DockLayout::group([INSPECTOR]), DockLayout::group([VIEWPORT])),
            Workspace::Game => DockLayout::split(horizontal, 0.34, DockLayout::group([VIEWPORT]), DockLayout::group([INSPECTOR])),
            Workspace::Agent => DockLayout::split(horizontal, 0.30, DockLayout::group([INSPECTOR]), DockLayout::group([VIEWPORT])),
            // **The tree, the page and the board** (SG-05, #404): the tree beside the page, and the place's board at the right.
            Workspace::Design => DockLayout::split(
                horizontal,
                0.22,
                DockLayout::group([INSPECTOR]),
                DockLayout::split(horizontal, 0.68, DockLayout::group([VIEWPORT]), DockLayout::group([PLACE])),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every workspace says one thing about itself in each table**, so a table that grew without
    /// the others is caught rather than shipped as a blank tab.
    #[test]
    fn every_workspace_names_itself_once_and_is_reachable() {
        // Owner exception (2026-11-13): workspace.rs::every_workspace_names_itself_once_and_is_reachable: the count follows the tenth workspace, Design (#404, the issue's own decision).
        assert_eq!(Workspace::ALL.len(), 10);
        for (i, workspace) in Workspace::ALL.into_iter().enumerate() {
            // The order the keys follow is the order of `ALL`, so a second workspace with the same id
            // would take somebody else's chord.
            assert_eq!(
                Workspace::ALL.iter().filter(|w| w.id() == workspace.id()).count(),
                1,
                "two workspaces share the id `{}`, so a command and a chord are ambiguous",
                workspace.id()
            );
            assert_eq!(
                Workspace::from_id(workspace.id()),
                Some(workspace),
                "`{}` does not read back as itself, so an agent's answer and the flag disagree",
                workspace.id()
            );
            assert!(
                !workspace.label().is_empty() && !workspace.title().is_empty(),
                "workspace {i} has no name"
            );
            assert!(
                crate::icons::index(workspace.icon()).is_some(),
                "the icon `{}` for the {} workspace is not in the atlas, so its tab draws blank",
                workspace.icon(),
                workspace.title()
            );
            // **A built workspace opens on a panel it shows.** An unbuilt one has none, and its
            // command says which issue brings it.
            if workspace.is_built() {
                assert!(
                    workspace.shows(workspace.opens_on()),
                    "the {} workspace opens on `{}`, which it does not show",
                    workspace.title(),
                    workspace.opens_on().label()
                );
                assert!(workspace.missing().is_none(), "a built workspace is not waiting for anything");
            } else {
                assert!(
                    workspace.missing().is_some(),
                    "the {} workspace is not built and does not say why, so its chord is a dead key \
                     with no answer",
                    workspace.title()
                );
            }
        }
    }

    /// **The Map workspace is the only one that edits the map**, which is what the run conditions
    /// ask.
    #[test]
    fn the_map_is_edited_from_one_workspace() {
        assert!(Workspace::Map.edits_the_map());
        assert!(!Workspace::Kit.edits_the_map());
        assert!(!Workspace::Agent.edits_the_map());
        // And every panel has exactly one workspace, so `showing` never has to choose.
        for mode in Mode::ALL {
            let homes: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| w.shows(mode)).collect();
            assert_eq!(homes.len(), 1, "`{}` is shown by {} workspaces", mode.label(), homes.len());
        }
    }
}
