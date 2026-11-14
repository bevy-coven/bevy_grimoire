//! **The style areas of the open map, drawn on it and named where the cursor is** (SG-06, #405).
//!
//! An area is a rectangle on its level's map (`bevy_grimoire_core::design`), and its `area.yaml` is the design tree's data, not
//! the map's. This module is the editor's view of them:
//!
//! - **View ▸ Show style areas** outlines each area of the open map on its floor and writes its name beside it. The outline is an
//!   overlay like the light map and is **off by default**; showing it writes no file and leaves no undo step.
//! - **The status band's UNDER CURSOR line names the area the cursor is in** (`editor::sense_under_cursor` asks
//!   [`StyleAreas::area_at`]), shown or not.
//! - **The inspector's Style link** opens the page of the area the cursor was last over on the map, and the level's own page when
//!   it was over none. It is spawned only while there is a page to open, so a project with no style has no extra button in the
//!   panel, and a click goes through the one reveal every other way of arriving at a file uses.
//!
//! **The areas are read, not held**: [`refresh`] walks the level's `areas/` folder every [`REFRESH_S`] and writes the resource only
//! when the answer differs, so an area an agent drew with `design_area_set`, or one edited by hand, is on screen within that
//! interval and a settled editor costs a comparison.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_grimoire_core::design::{self, AreaOnMap};

use crate::editor::{EditorState, MapControlsPane};
use crate::project::{OpenMap, Project};
use crate::reveal::Revealing;

/// **How often the level's areas are read again, in seconds.** Half a second is "at once" to a person and one small folder walk.
pub const REFRESH_S: f32 = 0.5;

/// **How far above the floor the outline is drawn, in metres**, over the floor fill and the grid so neither hides it.
const OUTLINE_LIFT_M: f32 = 0.04;

/// The outline's colour and the label's: amber, apart from the light map's blue and the crowd heat.
// CHROME-OK: world ink, the colour of an overlay drawn on the map's floor, not of a panel: it must read against any floor.
const OUTLINE: Color = Color::srgb(0.95, 0.72, 0.18);

/// **The reveal's name for the Style link**, which a refusal of the reveal quotes.
const STYLE_LINK_OPERATION: &str = "style_link";

/// **The level's areas, whether they are drawn, and where the cursor was last on the map.**
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct StyleAreas {
    /// Whether the outlines are drawn. A view of this door's map, off by default.
    pub on: bool,
    /// The open map's level folder, relative to the project root.
    pub level: Option<String>,
    /// Its areas, by name.
    pub areas: Vec<AreaOnMap>,
    /// **The name of the area the cursor was last over on the map**, `None` when it was over none. It is not cleared by the
    /// cursor going onto a panel, because the Style link is in one and a click on it is the question this answers.
    pub cursor_area: Option<String>,
}

impl StyleAreas {
    /// **The area a map point is in**, the smallest that holds it.
    pub fn area_at(&self, at: (f32, f32)) -> Option<&AreaOnMap> {
        design::area_at(&self.areas, at)
    }

    /// **Take a reading of the cursor, and say which area it is in.** `ground` is the cursor's point on the map's ground in map metres, or
    /// `None` when the cursor is not on the map (on a panel, or off the window): that names no area and **keeps** the one it was last in,
    /// because the Style link is on a panel and a click on it asks which. A point inside an area sets it; a point outside every area clears it.
    pub fn observe_cursor(&mut self, ground: Option<(f32, f32)>) -> Option<String> {
        let at = ground?;
        let found = self.area_at(at).map(|a| a.name.clone());
        if self.cursor_area != found {
            self.cursor_area = found.clone();
        }
        found
    }

    /// **The page the Style link opens, and what to call it**: the area the cursor was last in when it has a `style.md`, else the
    /// level's own.
    pub fn link(&self, root: &std::path::Path) -> Option<(String, String)> {
        if let Some(area) = self.cursor_area.as_deref().and_then(|name| self.areas.iter().find(|a| a.name == name)).filter(|a| a.has_style) {
            return Some((format!("area {}", area.name), format!("{}/{}.{}", area.path, design::STYLE_TOPIC, design::MARKDOWN_EXT)));
        }
        let level = self.level.as_deref()?;
        let page = format!("{level}/{}.{}", design::STYLE_TOPIC, design::MARKDOWN_EXT);
        root.join(&page).is_file().then(|| ("this level".to_owned(), page))
    }
}

/// **The UNDER CURSOR line with the area added**: the piece's own line, then `in the area <name>`, or the area alone over bare floor.
///
/// Pure, for `editor::under_readout`'s reason: the pick needs a viewport and a headless test has none.
pub fn under_phrase(line: String, area: Option<&str>) -> String {
    match area {
        None => line,
        Some(name) if line.is_empty() => format!("in the area {name}"),
        Some(name) => format!("{line}  — in the area {name}"),
    }
}

pub struct StyleAreasPlugin;

impl Plugin for StyleAreasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StyleAreas>().add_observer(on_style_link).add_systems(
            Update,
            (
                refresh,
                follow_the_tick,
                mirror_the_link.after(refresh).run_if(on_the_map),
                draw_outlines
                    .after(refresh)
                    .run_if(on_the_map)
                    .run_if(resource_exists::<OpenMap>)
                    .run_if(resource_exists::<Project>),
                name_the_areas.after(refresh),
            ),
        );
    }
}

/// **Whether the Map or World tab is the one showing**, where the map's floor is on screen.
fn on_the_map(mode: Option<Res<crate::tiles::Mode>>) -> bool {
    mode.is_some_and(|m| matches!(*m, crate::tiles::Mode::Map | crate::tiles::Mode::World))
}

/// The View ▸ Show style areas row, so its observer knows the change came from it.
#[derive(Component)]
pub struct StyleAreasItem;

/// **The View row's observer**: the tick is what the overlay should be, so choosing it twice is not a reversal.
pub fn on_style_areas_item(
    change: On<bevy::ui_widgets::ValueChange<bool>>,
    items: Query<(), With<StyleAreasItem>>,
    areas: Option<ResMut<StyleAreas>>,
    state: Option<ResMut<EditorState>>,
) {
    let (Some(mut areas), Some(mut state)) = (areas, state) else { return };
    if items.get(change.source).is_err() {
        return;
    }
    areas.on = change.value;
    state.status.note(if change.value { "style areas shown".to_owned() } else { "style areas hidden".to_owned() });
}

/// **The row's tick is the overlay's state**, whichever door moved it. Compared before writing.
fn follow_the_tick(areas: Option<Res<StyleAreas>>, items: Query<(Entity, Has<bevy_gooey::Checked>), With<StyleAreasItem>>, mut commands: Commands) {
    let Some(areas) = areas else { return };
    for (item, ticked) in &items {
        match (areas.on, ticked) {
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

/// **Read the open map's level's areas** every [`REFRESH_S`], and write the resource only when they differ.
fn refresh(mut areas: ResMut<StyleAreas>, project: Option<Res<Project>>, open: Option<Res<OpenMap>>, time: Res<Time>, mut since: Local<f32>) {
    *since += time.delta_secs();
    // **Asked only when the interval is up or the map or project changed**: the level's folder is a stat and maybe a directory read.
    let moved = open.as_ref().is_some_and(|o| o.is_changed()) || project.as_ref().is_some_and(|p| p.is_changed());
    if *since < REFRESH_S && !moved {
        return;
    }
    *since = 0.0;
    let level = match (project.as_deref(), open.as_deref()) {
        (Some(project), Some(open)) => Some((project.root.clone(), bevy_grimoire_core::board::level_place(&project.root, &open.map.name))),
        _ => None,
    };
    let (want_level, want_areas) = match &level {
        Some((root, level)) => (Some(level.clone()), design::areas_of_level(root, level)),
        None => (None, Vec::new()),
    };
    if areas.level != want_level {
        areas.level = want_level;
        areas.cursor_area = None;
    }
    if areas.areas != want_areas {
        areas.areas = want_areas;
    }
}

/// **Outline each area on the floor**, in map metres turned into the world's by the map's origin.
fn draw_outlines(areas: Res<StyleAreas>, open: Res<OpenMap>, project: Res<Project>, mut gizmos: Gizmos) {
    if !areas.on {
        return;
    }
    let y = open.map.origin.1 - crate::editor::ground_drop(&project) + OUTLINE_LIFT_M;
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for area in &areas.areas {
        let [x0, z0, x1, z1] = area.rect;
        let centre = Vec3::new(open.map.origin.0 + (x0 + x1) * 0.5, y, open.map.origin.2 + (z0 + z1) * 0.5);
        gizmos.rect(Isometry3d::new(centre, flat), Vec2::new(x1 - x0, z1 - z0), OUTLINE);
    }
}

/// The name written beside an area's outline.
#[derive(Component)]
pub struct AreaLabel(pub String);

/// **Write each area's name at its corner**, projected through the camera, while the outline is shown. Labels exist only while
/// they are shown: nothing is spawned for an editor that never turns the overlay on.
#[allow(clippy::too_many_arguments)]
fn name_the_areas(
    areas: Res<StyleAreas>,
    open: Option<Res<OpenMap>>,
    mode: Option<Res<crate::tiles::Mode>>,
    ui_scale: Res<UiScale>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<crate::view::MainCamera>>>,
    mut labels: Query<(Entity, &AreaLabel, &mut Node)>,
    mut commands: Commands,
) {
    let shown = areas.on && on_the_map(mode);
    let (Some(open), Some(camera), true) = (open, camera, shown) else {
        for (label, ..) in &labels {
            commands.entity(label).despawn();
        }
        return;
    };
    let (cam, cam_tf) = *camera;
    for (label, AreaLabel(name), _) in &labels {
        if !areas.areas.iter().any(|a| &a.name == name) {
            commands.entity(label).despawn();
        }
    }
    for area in &areas.areas {
        let [x0, z0, ..] = area.rect;
        let world = Vec3::new(open.map.origin.0 + x0, open.map.origin.1, open.map.origin.2 + z0);
        let at = cam.world_to_viewport(cam_tf, world).ok().map(|p| p / ui_scale.0);
        let existing = labels.iter_mut().find(|(_, AreaLabel(name), _)| name == &area.name);
        match (existing, at) {
            (Some((_, _, mut node)), Some(at)) => {
                if node.left != Val::Px(at.x) {
                    node.left = Val::Px(at.x);
                }
                if node.top != Val::Px(at.y) {
                    node.top = Val::Px(at.y);
                }
            }
            (Some((label, ..)), None) => {
                commands.entity(label).despawn();
            }
            (None, Some(at)) => {
                commands.spawn((
                    AreaLabel(area.name.clone()),
                    Name::new(format!("style area {}", area.name)),
                    Text::new(area.name.clone()),
                    TextColor(OUTLINE),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                    Node {
                        // PLACES-ITSELF-OK: an area's name is anchored to the area's corner projected through the camera, not a panel.
                        position_type: PositionType::Absolute,
                        left: Val::Px(at.x),
                        top: Val::Px(at.y),
                        ..default()
                    },
                    GlobalZIndex(crate::chrome::BALLOON_Z),
                ));
            }
            (None, None) => {}
        }
    }
}

/// The Style link in the inspector, with the file it opens.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct StyleLink(pub String);

/// The block that holds the link, so it can be taken out whole.
#[derive(Component)]
pub struct StyleBlock;

/// **The link is there while there is a page to open**, naming the area or the level it is for, and is taken out when there is
/// none. It is not kept hidden: a button that is not drawn is still a button to every screen that counts them.
fn mirror_the_link(
    areas: Res<StyleAreas>,
    project: Option<Res<Project>>,
    panes: Query<Entity, With<MapControlsPane>>,
    blocks: Query<Entity, With<StyleBlock>>,
    mut links: Query<(&mut StyleLink, &Children)>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
) {
    let want = project.as_ref().and_then(|p| areas.link(&p.root));
    let Some((what, page)) = want else {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    };
    let label = format!("Open style of {what}");
    if blocks.is_empty() {
        let Ok(pane) = panes.single() else { return };
        let block = commands
            .spawn((StyleBlock, Name::new("style block"), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() }))
            .with_children(|block| {
                crate::chrome::list_heading(block, "STYLE");
                crate::chrome::button(block, StyleLink(page), &label, crate::chrome::Severity::Plain);
            })
            .id();
        commands.entity(pane).insert_children(0, &[block]);
        return;
    }
    for (mut link, children) in &mut links {
        if link.0 != page {
            link.0 = page.clone();
        }
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != label
            {
                text.0 = label.clone();
            }
        }
    }
}

/// **A click on the Style link opens the page**, through the reveal Go to anything and a cross-link use.
fn on_style_link(activate: On<Activate>, links: Query<&StyleLink>, revealing: Option<ResMut<Revealing>>) {
    let (Ok(link), Some(mut revealing)) = (links.get(activate.entity), revealing) else { return };
    revealing.request_by_person(STYLE_LINK_OPERATION, &crate::ops::Shown::File { path: link.0.clone(), lines: None });
}
