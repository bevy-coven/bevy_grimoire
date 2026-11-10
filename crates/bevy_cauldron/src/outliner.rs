//! **The outliner: the map's contents as a tree you can search** (UI-51, #237).
//!
//! An OUTLINER section at the top of the Map door's INSPECTOR panel, shown by View ▸ Show outliner: a search box over
//! gooey's virtualized `tree`. The first level is what each piece *is* (the first of its descriptor's
//! `kind` tags, `other` for a piece with none), the second is the pieces themselves, one row per
//! placement named `<id> · <descriptor>`.
//!
//! - **Searching narrows the tree to what matches and the groups that hold it.** Every word typed must
//!   appear in the row's id, descriptor or group, case aside, so `lamp` leaves the lamps under their
//!   group and nothing else. A search opens every group it leaves, so a match is never behind a closed
//!   chevron.
//! - **Selecting a row selects the piece.** Rows are the selection's own vocabulary: the first selected
//!   becomes the lock's piece (the primary the INSPECTOR's POSE block shows and the viewport outlines),
//!   and a Ctrl- or Shift-click selects several, exactly as Shift-clicking pieces in the viewport does
//!   ([`crate::selection::Selection`]). Clicking a group selects nothing.
//! - **The tree follows the viewport.** A piece selected there is the selected row here, when the
//!   search leaves it in view.
//!
//! Nothing here writes the map: a selection is session state, so there is no operation to dispatch and
//! nothing to undo. Visibility and lock toggles, and compositions, people, triggers and notes as
//! further groups, are the rest of the issue and are not built.
//!
//! # Keys
//!
//! A tree row's key is a `u64`. A piece's is a hash of its id with the top bit clear and a group's a
//! hash of its name with the top bit set ([`placement_key`], [`group_key`]), so the two never meet and a
//! key survives a rebuild: the tree keeps its own selection and open groups across a map edit.

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;
use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::stack::descriptor_of;
use bevy_gooey::{Checked, SelectMode, Table, TableSelection, Tree, TreeNode, ValueChange, observe, text_input, tree};

use crate::editor::{MapControlsPane, TargetLock};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::selection::Selection;

/// The top bit of a key: set on a group's, clear on a piece's.
const GROUP_BIT: u64 = 1 << 63;

/// The group a piece with no `kind` tag stands under.
const OTHER_GROUP: &str = "other";

/// The outliner's state, on its section: what is typed, what the tree was last built from, and which
/// piece each row means.
#[derive(Component, Default)]
pub struct Outliner {
    /// What the search box holds.
    pub query: String,
    /// The piece behind each row's key.
    rows: HashMap<u64, String>,
    /// A fingerprint of what the tree was last built from (the query and every piece's id and
    /// descriptor), so a map edit that changes no piece's identity (a turn, a nudge) is not a rebuild.
    built_from: u64,
    /// The query the tree was last built for, so a settled frame compares two short strings and nothing else.
    built_for: String,
    /// How many times the tree has been built from the map. A settled map and search cost none: this is the
    /// number the UI budget (UI-93, #259) is written against, and a test holds it.
    rebuilds: u32,
    /// How many times every piece has been read to decide whether to build: the cost that grows with the map. A settled
    /// frame adds none.
    looks: u32,
}

impl Outliner {
    /// How many times the tree has been built since the section opened.
    pub fn rebuilds(&self) -> u32 {
        self.rebuilds
    }

    /// How many times every piece has been read to decide whether to build, since the section opened.
    pub fn looks(&self) -> u32 {
        self.looks
    }
}

/// The tree widget.
#[derive(Component)]
pub struct OutlinerTree;

/// The search box.
#[derive(Component)]
pub struct OutlinerSearch;

/// FNV-1a over `text`, which is stable across runs and platforms (a `HashMap`'s hasher is not).
fn fnv(text: &str) -> u64 {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    text.bytes().fold(BASIS, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME))
}

/// The key of the row for the piece `id`.
pub fn placement_key(id: &str) -> u64 {
    fnv(id) & !GROUP_BIT
}

/// The key of the row for the group `kind`.
pub fn group_key(kind: &str) -> u64 {
    fnv(kind) | GROUP_BIT
}

pub struct OutlinerPlugin;

impl Plugin for OutlinerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (rebuild_the_tree, follow_the_viewport, tick_the_menu_row).chain().run_if(in_state(Screen::Editor)));
    }
}

/// The View ▸ Show outliner row.
#[derive(Component)]
pub struct OutlinerItem;

/// **View ▸ Show outliner puts the section into the Map door's INSPECTOR panel, first among its rows, and
/// takes it out again.** It is not there until it is asked for: a text box and a tree are entities, and the
/// screens that look for "the" empty text input or count every button in the world (the settings search, the
/// dialogs' answers) would find these. The row is a menu row, which the bar does not count as a button.
pub fn toggle_the_outliner(
    change: On<ValueChange<bool>>,
    panes: Query<Entity, With<MapControlsPane>>,
    blocks: Query<Entity, With<Outliner>>,
    mut commands: Commands,
) {
    if !change.value {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    }
    if !blocks.is_empty() {
        return;
    }
    let Ok(pane) = panes.single() else { return };
    let block = commands
        .spawn((
            Outliner::default(),
            Name::new("outliner"),
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() },
        ))
        .with_children(|block| {
            crate::chrome::list_heading(block, "OUTLINER");
            block.spawn((
                text_input("Find a piece", ""),
                OutlinerSearch,
                observe(|typed: On<ValueChange<String>>, mut outliners: Query<&mut Outliner>| {
                    if let Ok(mut outliner) = outliners.single_mut()
                        && outliner.query != typed.value
                    {
                        outliner.query = typed.value.clone();
                    }
                }),
            ));
            block.spawn((tree(Vec::<TreeNode>::new()), OutlinerTree, SelectMode::Multi, observe(select_the_pieces)));
        })
        .id();
    commands.entity(pane).insert_children(0, &[block]);
}

/// **The tick on the menu row says whether the section is there**, whichever way it came or went (a door
/// change respawns the panel and takes it with it).
fn tick_the_menu_row(items: Query<(Entity, Has<Checked>), With<OutlinerItem>>, blocks: Query<(), With<Outliner>>, mut commands: Commands) {
    let present = !blocks.is_empty();
    for (item, ticked) in &items {
        match (present, ticked) {
            (true, false) => {
                commands.entity(item).insert(Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// **A click on rows selects the pieces they name**, the first of them the primary. A group row names no
/// piece, so a click on one changes nothing.
fn select_the_pieces(
    picked: On<ValueChange<TableSelection>>,
    outliners: Query<&Outliner>,
    open: Option<Res<OpenMap>>,
    lock: Option<ResMut<TargetLock>>,
    selection: Option<ResMut<Selection>>,
) {
    let (Ok(outliner), Some(open), Some(mut lock), Some(mut selection)) = (outliners.single(), open, lock, selection) else { return };
    // In the map's own order, so the same rows always make the same selection with the same primary.
    let ids: Vec<String> = open
        .map
        .placements
        .iter()
        .filter(|p| picked.value.0.iter().any(|key| outliner.rows.get(key).is_some_and(|id| *id == p.id)))
        .map(|p| p.id.clone())
        .collect();
    let Some(primary) = ids.first().and_then(|id| open.map.placements.iter().find(|p| p.id == *id)) else { return };
    lock.hold(&primary.id.clone(), primary.at);
    selection.set(ids);
}

/// **The tree, built from the map and the search.** Returns the nodes and the piece behind each key.
///
/// A piece is kept when every word of `query` is in its id, its descriptor or its group's name. A group
/// with no piece kept is not drawn. Groups and pieces are in a stable order (groups by name, pieces in
/// the map's order), so a rebuild never shuffles rows under the pointer. `group_open` says whether a
/// group the tree already had is open; a group it did not have starts open, and so does every group
/// while something is searched for.
pub fn nodes_for(
    map: &Map,
    library: &bevy_grimoire_core::library::Library,
    query: &str,
    group_open: impl Fn(u64) -> Option<bool>,
) -> (Vec<TreeNode>, HashMap<u64, String>) {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut groups: std::collections::BTreeMap<String, Vec<TreeNode>> = std::collections::BTreeMap::new();
    let mut rows = HashMap::new();
    for placed in &map.placements {
        let kind = descriptor_of(library, placed).and_then(|d| d.kind.first().cloned()).unwrap_or_else(|| OTHER_GROUP.to_owned());
        let haystack = format!("{} {} {}", placed.id, placed.descriptor, kind).to_lowercase();
        if !words.iter().all(|word| haystack.contains(word.as_str())) {
            continue;
        }
        let key = placement_key(&placed.id);
        rows.insert(key, placed.id.clone());
        groups.entry(kind).or_default().push(TreeNode::new(key, format!("{} \u{b7} {}", placed.id, placed.descriptor)));
    }
    let searching = !words.is_empty();
    let nodes = groups
        .into_iter()
        .map(|(kind, pieces)| {
            let key = group_key(&kind);
            let node = TreeNode::new(key, kind).children(pieces);
            if searching || group_open(key).unwrap_or(true) { node.expanded() } else { node }
        })
        .collect();
    (nodes, rows)
}

/// Whether the tree needs building again: a fingerprint of the query and of every piece's identity.
fn fingerprint(map: &Map, query: &str) -> u64 {
    let mut text = String::with_capacity(query.len() + map.placements.len() * 16);
    text.push_str(query);
    for placed in &map.placements {
        text.push('\u{1f}');
        text.push_str(&placed.id);
        text.push('\u{1e}');
        text.push_str(&placed.descriptor);
    }
    fnv(&text)
}

/// **The tree is rebuilt when the map or the search moved**, and the groups a person closed stay closed.
fn rebuild_the_tree(
    open: Option<Res<OpenMap>>,
    project: Option<Res<Project>>,
    mut outliners: Query<&mut Outliner>,
    mut tables: Query<&mut Table, With<OutlinerTree>>,
    mut budget: ResMut<crate::ui_budget::UiFrame>,
) {
    // **This system's cost, into the frame's total** (UI-93, #259): the tree is the list that grows
    // with the map, and the budget's whole point is that a settled frame pays nothing for its depth.
    let started = crate::ui_budget::begin();
    let spent = |budget: &mut crate::ui_budget::UiFrame| crate::ui_budget::end(started, budget);
    let (Some(open), Some(project)) = (open, project) else {
        spent(&mut budget);
        return;
    };
    let (Ok(mut outliner), Ok(mut table)) = (outliners.single_mut(), tables.single_mut()) else {
        spent(&mut budget);
        return;
    };
    // **A settled frame does no work that grows with the map.** The fingerprint reads every piece, so it is taken
    // only when the map, the library or the search moved, or the tree has not been built yet: ten thousand pieces
    // at sixty frames a second was a second of string building for a tree nobody touched.
    let moved = outliner.built_from == 0 || open.is_changed() || project.is_changed() || outliner.built_for != outliner.query;
    if !moved {
        spent(&mut budget);
        return;
    }
    outliner.looks += 1;
    let now = fingerprint(&open.map, &outliner.query);
    if outliner.built_from == now {
        let query = outliner.query.clone();
        outliner.built_for = query;
        spent(&mut budget);
        return;
    }
    let Some(tree) = table.source_mut::<Tree>() else {
        spent(&mut budget);
        return;
    };
    let (nodes, rows) = nodes_for(&open.map, &project.library, &outliner.query, |key| tree.is_open(key));
    tree.set_nodes(nodes);
    outliner.rows = rows;
    outliner.built_from = now;
    let query = outliner.query.clone();
    outliner.built_for = query;
    outliner.rebuilds += 1;
    spent(&mut budget);
}

/// **A piece selected in the viewport is the selected row**, when the search leaves it in view.
fn follow_the_viewport(
    lock: Option<Res<TargetLock>>,
    selection: Option<Res<Selection>>,
    outliners: Query<&Outliner>,
    trees: Query<(Entity, Option<&TableSelection>), With<OutlinerTree>>,
    mut commands: Commands,
) {
    let (Some(lock), Some(selection), Ok(outliner), Ok((tree, now))) = (lock, selection, outliners.single(), trees.single()) else { return };
    let selected: Vec<&str> = match (lock.placement_id(), selection.ids()) {
        (Some(primary), ids) if ids.first().map(String::as_str) == Some(primary) => ids.iter().map(String::as_str).collect(),
        (Some(primary), _) => vec![primary],
        (None, _) => Vec::new(),
    };
    let want: BTreeSet<u64> = selected.iter().map(|id| placement_key(id)).filter(|key| outliner.rows.contains_key(key)).collect();
    if now.map(|n| &n.0) != Some(&want) && !(now.is_none() && want.is_empty()) {
        commands.entity(tree).insert(TableSelection(want));
    }
}
