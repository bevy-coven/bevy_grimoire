//! **In-app help: the written guide, reachable from the editor** (UI-91, #257).
//!
//! `docs/guide/` is the guide, one page a subject. This module is the one list of what it holds — an id, a title and the file —
//! so three surfaces read it instead of each naming pages for itself:
//!
//! - **Help ▸ a page**: every page is a row of the Help menu ([`crate::menu_bar`], `AppVerb::GuidePage`).
//! - **A panel header's "?"**: the page that covers what the panel holds ([`page_for_panel`]), so the question a person has
//!   while looking at the panel is one click from its answer instead of a browser search.
//! - **The guide's own index** (`docs/guide/README.md`), which `the_panel_headers_link_their_guide_page` keeps in step.
//!
//! A page that is not in [`PAGES`] cannot be opened, and a heading no page names gets no "?"; the test fails on both, so the
//! catalog cannot go stale in either direction.
//!
//! # Why the button is gooey's `icon_button`
//!
//! The interface's own tests count every `Role::ButtonText` in the world (a question's two answers are the whole list), so a
//! panel header cannot hold a worded `button`. `icon_button` is a button with a picture and a tooltip and no `ButtonText`, which
//! is what a "?" is, and the glyph is in the editor's own atlas as `panel.help`.

use bevy::prelude::*;
use bevy_gooey::{Activate, IconSource, Tooltip, icon_button};

use crate::app_verbs::{AppVerb, open_link};
use crate::chrome::PanelGround;

/// One page of the written guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuidePage {
    /// The page's id, as a settings file or a link names it (`building-maps`).
    pub id: &'static str,
    /// What the menu row and the tooltip say.
    pub title: &'static str,
    /// The file under `docs/guide/`.
    pub file: &'static str,
}

/// **The guide, in the order it reads.** `docs/guide/README.md` lists the same pages, in the same order, and
/// `the_panel_headers_link_their_guide_page` fails when the two disagree.
pub const PAGES: &[GuidePage] = &[
    GuidePage { id: "getting-started", title: "Getting started", file: "getting-started.md" },
    GuidePage { id: "building-maps", title: "Building maps", file: "building-maps.md" },
    GuidePage { id: "kits-and-labels", title: "Kits and labels", file: "kits-and-labels.md" },
    GuidePage { id: "agents", title: "Agents", file: "agents.md" },
    GuidePage { id: "your-game", title: "Your game", file: "your-game.md" },
    GuidePage { id: "keys", title: "Keys", file: "keys.md" },
    GuidePage { id: "data-files", title: "Data files", file: "data-files.md" },
    GuidePage { id: "reload-contract", title: "The reload contract", file: "reload-contract.md" },
    GuidePage { id: "loop", title: "The development loop", file: "loop.md" },
];

/// Where a page is published: the repository's copy, so the link always names the page the editor was built from.
const PUBLISHED_AT: &str = "https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/";

/// The page with this id, if the guide has one.
pub fn page(id: &str) -> Option<&'static GuidePage> {
    PAGES.iter().find(|page| page.id == id)
}

/// The address a page opens at.
pub fn url(page: &GuidePage) -> String {
    format!("{PUBLISHED_AT}{}", page.file)
}

/// **The page that covers what a panel holds**, by the heading the panel draws (`INSPECTOR`, `PLACE`, `PIECES`, `RIGS`,
/// `COMPOSE`).
///
/// Three headings name a job rather than a door (`INSPECTOR` is the Map's, the Kit's and the Anim door's), and the job is what
/// the page is about, so the same answer serves all three.
pub fn page_for_panel(heading: &str) -> Option<&'static GuidePage> {
    let id = match heading {
        "INSPECTOR" | "PLACE" => "building-maps",
        "PIECES" | "TILES" | "CONTROLS" => "kits-and-labels",
        "RIGS" | "COMPOSE" => "kits-and-labels",
        "AGENT" => "agents",
        _ => return None,
    };
    page(id)
}

/// **The "?" a panel header carries**: the page it opens.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelHelp {
    /// The page's id.
    pub page: &'static str,
}

/// The row a panel's heading and its "?" share.
#[derive(Component)]
pub struct PanelHeading;

pub struct HelpPlugin;

impl Plugin for HelpPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_panel_help_activate).add_systems(Update, mark_the_panel_headings);
    }
}

fn on_panel_help_activate(activate: On<Activate>, helps: Query<&PanelHelp>, mut commands: Commands) {
    let Ok(help) = helps.get(activate.entity) else { return };
    let Some(page) = page(help.page) else { return };
    let url = url(page);
    commands.queue(move |world: &mut World| open_link(world, &url));
}

/// **A heading the editor draws at the top of a panel is that panel's title row**, and gets the "?" beside it.
///
/// The heading is a `Text` whose words are the panel's own accessible label — `chrome::panel_root` was handed the same string —
/// so the panel is found by what it says rather than by a second list of ids in this module. A heading with no page is left
/// alone (`page_for_panel` is the one answer), and a row already marked is not marked twice.
fn mark_the_panel_headings(
    mut commands: Commands,
    atlas: Option<Res<crate::toolbar::ToolbarAtlas>>,
    panels: Query<(Entity, &AccessibleLabel, &Children), With<PanelGround>>,
    texts: Query<&Text>,
    parents: Query<&ChildOf>,
    rowed: Query<(), With<PanelHeading>>,
) {
    let Some(atlas) = atlas else { return };
    let Some(index) = crate::icons::index(HELP_ICON) else { return };
    for (panel, name, children) in &panels {
        let Some(page) = page_for_panel(&name.0) else { continue };
        // The heading is the panel's child drawn in the panel's own words: `chrome::title` and
        // `chrome::list_heading_in` put it there, and `panel_root` was handed the same string as its
        // accessible label, so a panel is found by what it says rather than by a list of ids here.
        let Some((at, heading)) = children
            .iter()
            .enumerate()
            .find(|(_, child)| texts.get(*child).is_ok_and(|text| text.0 == name.0))
        else {
            continue;
        };
        if parents.get(heading).is_ok_and(|parent| rowed.get(parent.parent()).is_ok()) {
            continue;
        }
        // **The heading and its "?" are one row**, so the button sits beside the words rather than
        // inside the text run. The row takes the heading's place in the panel, which keeps the title
        // at the top of it.
        let row = commands
            .spawn((
                PanelHeading,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(crate::chrome::GAP_TIGHT),
                    ..default()
                },
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        commands.entity(panel).insert_children(at, &[row]);
        commands.entity(row).add_child(heading);
        commands.entity(row).with_child((
            PanelHelp { page: page.id },
            icon_button(
                IconSource::Atlas { image: atlas.image.clone(), layout: atlas.layout.clone(), index },
                Tooltip::new(format!("Open the {} guide", page.title)),
            ),
        ));
    }
}

/// The glyph a panel's "?" draws, from the editor's own atlas.
pub const HELP_ICON: &str = "panel.help";

/// The Help menu's rows: the guide's pages first, then what the editor's own verbs offer.
pub fn menu_rows() -> Vec<AppVerb> {
    PAGES.iter().map(|page| AppVerb::GuidePage(page.id)).collect()
}
