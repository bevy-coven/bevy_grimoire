//! **The outliner: everything on the map as a tree you can search** (UI-51, #237).
//!
//! An OUTLINER section at the top of the Map door's INSPECTOR panel, shown by View ▸ Show outliner: a search box over
//! gooey's virtualized `tree`. The first level is what each thing *is* — a piece's group is the first
//! of its descriptor's `kind` tags (`other` for a piece with none), and the map's compositions, people,
//! triggers and notes are groups of their own — and the second is the things themselves, one row each.
//!
//! - **Searching narrows the tree to what matches and the groups that hold it.** Every word typed must
//!   appear in the row's words or its group's name, case aside, so `lamp` leaves the lamps under their
//!   group and nothing else. A search opens every group it leaves, so a match is never behind a closed
//!   chevron.
//! - **Selecting a row selects the piece.** Rows are the selection's own vocabulary: the first selected
//!   becomes the lock's piece (the primary the INSPECTOR's POSE block shows and the viewport outlines),
//!   and a Ctrl- or Shift-click selects several, exactly as Shift-clicking pieces in the viewport does
//!   ([`crate::selection::Selection`]). Clicking a group selects nothing, and neither does a row for a
//!   composition, a person, a trigger or a note: those are not placements, so there is no piece to put
//!   in the selection and nothing is invented.
//! - **The tree follows the viewport.** A piece selected there is the selected row here, when the
//!   search leaves it in view.
//! - **A row's menu carries the two toggles a row has** ([`row_menu`]): *Pin* is the piece's
//!   `placement_set_pin`, the same write the `P` key makes (one log line, one Cmd+Z), and *Hide* takes
//!   the piece out of the viewport **for this session**. A hide is view state: the map format has no
//!   "hidden" field and this does not add one, because a map file that says a wall is invisible is a map
//!   every consumer has to agree with. A row that is not a placement offers neither.
//!
//! gooey's tree row is a chevron and a label, so the toggles are gooey's own `context_menu` named on the
//! rows, the way [`crate::palette_menu`] names one on the palette's — never a control built here.
//!
//! # Keys
//!
//! A tree row's key is a `u64`. A piece's is a hash of its id with two bits clear, a group's a hash of
//! its name with the top bit set and any other thing's a hash of its section and id with the second set
//! ([`placement_key`], [`group_key`], [`thing_key`]), so the three families never meet and a key
//! survives a rebuild: the tree keeps its own selection and open groups across a map edit.

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;
use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::stack::descriptor_of;
use bevy_gooey::{Checked, SelectMode, Table, TableSelection, Tree, TreeNode, ValueChange, observe, text_input, tree};

use crate::editor::{MapControlsPane, TargetLock};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::selection::Selection;

/// The top bit of a key: set on a group's, clear on everything under one.
const GROUP_BIT: u64 = 1 << 63;

/// The second bit: set on a row for something that is not a placement (a composition, a person, a
/// trigger or a note), so it names no piece and offers no pin and no eye.
const THING_BIT: u64 = 1 << 62;

/// The group a piece with no `kind` tag stands under.
const OTHER_GROUP: &str = "other";

/// The four groups the map's other contents stand under, in the order the tree draws them. A
/// composition is a stamped group, a person a marker from the map's `people/` file, a trigger one of its
/// `scripts/`, and a note one of its `.notes.ron`.
pub const COMPOSITIONS: &str = "compositions";
pub const PEOPLE: &str = "people";
pub const TRIGGERS: &str = "triggers";
pub const NOTES: &str = "notes";

/// How much of a note's text a row shows. One line, so the row stays one line tall however long the note
/// is; the note itself is read in the NOTES panel.
const NOTE_PREVIEW_CHARS: usize = 48;

/// **What the map holds that is not a placement**, read once per rebuild and handed to
/// [`nodes_for`]: the stamps, the people, the triggers and the notes, as the rows that name them.
#[derive(Default, Clone)]
pub struct Contents {
    /// `(id, the composition it is of)` for each stamp.
    pub compositions: Vec<(String, String)>,
    /// Each person's name.
    pub people: Vec<String>,
    /// Each trigger's id.
    pub triggers: Vec<String>,
    /// `(id, the first words of its text)` for each note.
    pub notes: Vec<(String, String)>,
}

impl Contents {
    /// Whether there is nothing but the pieces.
    pub fn is_empty(&self) -> bool {
        self.compositions.is_empty() && self.people.is_empty() && self.triggers.is_empty() && self.notes.is_empty()
    }
}

/// The outliner's state, on its section: what is typed, what the tree was last built from, and which
/// piece each row means.
#[derive(Component, Default)]
pub struct Outliner {
    /// What the search box holds.
    pub query: String,
    /// The piece behind each row's key: a placement only, so a row for a person is not one.
    rows: HashMap<u64, String>,
    /// Each group's rows, in the order they are drawn, so a row can be asked what it is under.
    children: HashMap<u64, Vec<u64>>,
    /// A fingerprint of what the tree was last built from (the query and every piece's id and
    /// descriptor), so a map edit that changes no piece's identity (a turn, a nudge) is not a rebuild.
    built_from: u64,
    /// The query the tree was last built for, so a settled frame compares two short strings and nothing else.
    built_for: String,
    /// The notes' generation the tree was last built from: the notes are read from a resource that marks
    /// itself changed every frame, so the generation is what says they moved.
    built_notes: u64,
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

    /// **The rows drawn under the group `key`**, in the order they are drawn.
    pub fn rows_under(&self, key: u64) -> &[u64] {
        self.children.get(&key).map_or(&[], Vec::as_slice)
    }

    /// **What the row `key` names**: the piece it stands for, or `None` for a group and for a row for a
    /// composition, a person, a trigger or a note.
    pub fn piece_of(&self, key: u64) -> Option<&str> {
        self.rows.get(&key).map(String::as_str)
    }

    /// **Whether the row `key` offers a lock and an eye.** Both are a piece's: a group, a composition, a
    /// person, a trigger and a note are not placements, so there is nothing to pin and nothing to draw.
    pub fn toggles_offered(&self, key: u64) -> bool {
        self.rows.contains_key(&key)
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

/// **The key of the row for something that is not a placement**, hashed from its section and its id so a
/// note and a person that happen to share a name are still two rows.
pub fn thing_key(section: &str, id: &str) -> u64 {
    fnv(&format!("{section}\u{1f}{id}")) & !GROUP_BIT | THING_BIT
}

pub struct OutlinerPlugin;

impl Plugin for OutlinerPlugin {
    fn build(&self, app: &mut App) {
        // **The eye is the session's**, so it is registered here and nowhere else: closing the section is
        // not showing everything again.
        app.init_resource::<Eyes>();
        app.add_systems(
            Update,
            (rebuild_the_tree, follow_the_viewport, tick_the_menu_row).chain().run_if(in_state(Screen::Editor)),
        );
        // **The row's menu** (UI-51, #237): gooey's own `context_menu`, named on the rows that are drawn,
        // so a row offers Pin and Hide without a control being built here.
        app.add_systems(OnEnter(Screen::Editor), spawn_row_menu)
            .add_systems(Update, name_the_menu_on_rows.run_if(in_state(Screen::Editor)));
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

/// **The tree, built from the map and the search.** Returns the nodes, the piece behind each key, and the
/// rows drawn under each group.
///
/// A row is kept when every word of `query` is in its words or its group's name. A group with no row kept
/// is not drawn. A piece's groups are in name order; the map's own four (compositions, people, triggers,
/// notes) come after them in the order the issue names them, because a piece is what a map is mostly made
/// of and a reader looks for it first. Pieces are in the map's order, so a rebuild never shuffles rows
/// under the pointer. `group_open` says whether a group the tree already had is open; a group it did not
/// have starts open, and so does every group while something is searched for.
#[allow(clippy::type_complexity)]
pub fn nodes_for(
    map: &Map,
    library: &bevy_grimoire_core::library::Library,
    contents: &Contents,
    query: &str,
    group_open: impl Fn(u64) -> Option<bool>,
) -> (Vec<TreeNode>, HashMap<u64, String>, HashMap<u64, Vec<u64>>) {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let kept = |haystack: &str, group: &str| {
        let haystack = format!("{haystack} {group}").to_lowercase();
        words.iter().all(|word| haystack.contains(word.as_str()))
    };
    // **A piece's group**: a `BTreeMap`, so the groups of pieces are in name order.
    let mut piece_groups: std::collections::BTreeMap<String, Vec<(u64, String)>> = std::collections::BTreeMap::new();
    let mut rows = HashMap::new();
    for placed in &map.placements {
        let kind = descriptor_of(library, placed).and_then(|d| d.kind.first().cloned()).unwrap_or_else(|| OTHER_GROUP.to_owned());
        if !kept(&format!("{} {}", placed.id, placed.descriptor), &kind) {
            continue;
        }
        let key = placement_key(&placed.id);
        rows.insert(key, placed.id.clone());
        piece_groups.entry(kind).or_default().push((key, format!("{} \u{b7} {}", placed.id, placed.descriptor)));
    }
    // **The map's other contents, four groups of their own** (UI-51, #237), after the pieces' groups.
    let notes: Vec<(u64, String)> = contents
        .notes
        .iter()
        .filter(|(id, text)| kept(&format!("{id} {text}"), NOTES))
        .map(|(id, text)| (thing_key(NOTES, id), if text.is_empty() { id.clone() } else { format!("{id} \u{b7} {text}") }))
        .collect();
    let others: [(String, Vec<(u64, String)>); 4] = [
        (
            COMPOSITIONS.to_owned(),
            contents
                .compositions
                .iter()
                .filter(|(id, of)| kept(&format!("{id} {of}"), COMPOSITIONS))
                .map(|(id, of)| (thing_key(COMPOSITIONS, id), format!("{id} \u{b7} {of}")))
                .collect(),
        ),
        (
            PEOPLE.to_owned(),
            contents.people.iter().filter(|name| kept(name, PEOPLE)).map(|name| (thing_key(PEOPLE, name), name.clone())).collect(),
        ),
        (
            TRIGGERS.to_owned(),
            contents.triggers.iter().filter(|id| kept(id, TRIGGERS)).map(|id| (thing_key(TRIGGERS, id), id.clone())).collect(),
        ),
        (NOTES.to_owned(), notes),
    ];
    let searching = !words.is_empty();
    let mut nodes = Vec::with_capacity(piece_groups.len() + others.len());
    let mut children = HashMap::new();
    let group_of = |kind: &str, rows_here: Vec<(u64, String)>, children: &mut HashMap<u64, Vec<u64>>| -> TreeNode {
        let key = group_key(kind);
        children.insert(key, rows_here.iter().map(|(k, _)| *k).collect::<Vec<u64>>());
        let node = TreeNode::new(key, kind.to_owned()).children(rows_here.into_iter().map(|(k, label)| TreeNode::new(k, label)));
        if searching || group_open(key).unwrap_or(true) { node.expanded() } else { node }
    };
    for (kind, rows_here) in piece_groups {
        if !rows_here.is_empty() {
            nodes.push(group_of(&kind, rows_here, &mut children));
        }
    }
    for (kind, rows_here) in others {
        if !rows_here.is_empty() {
            nodes.push(group_of(&kind, rows_here, &mut children));
        }
    }
    (nodes, rows, children)
}

/// Whether the tree needs building again: a fingerprint of the query, every piece's identity, and the map's
/// other contents.
fn fingerprint(map: &Map, contents: &Contents, query: &str) -> u64 {
    let mut text = String::with_capacity(query.len() + map.placements.len() * 16);
    text.push_str(query);
    for placed in &map.placements {
        text.push('\u{1f}');
        text.push_str(&placed.id);
        text.push('\u{1e}');
        text.push_str(&placed.descriptor);
    }
    for (id, of) in &contents.compositions {
        text.push('\u{1d}');
        text.push_str(id);
        text.push_str(of);
    }
    for name in &contents.people {
        text.push('\u{1c}');
        text.push_str(name);
    }
    for id in &contents.triggers {
        text.push('\u{1b}');
        text.push_str(id);
    }
    for (id, preview) in &contents.notes {
        text.push('\u{1a}');
        text.push_str(id);
        text.push_str(preview);
    }
    fnv(&text)
}

/// **The contents the tree draws that are not placements**, read from where each already lives: the map's
/// own stamps, its `people/` and `scripts/` files, and the notes the editor holds ([`crate::notes_view`]).
///
/// Read at a rebuild and nowhere else. A file the project cannot read is left out rather than refused: the
/// outliner is a picture of the map, and the panel that owns each file says what is wrong with it
/// (`notes_view`, the problems panel) — one damaged file must not empty the tree.
fn contents_of(open: &OpenMap, project: &Project, notes: Option<&crate::notes_view::NotesView>) -> Contents {
    let map = &open.map;
    let compositions = map.stamps.iter().map(|s| (s.id.clone(), s.of.clone())).collect();
    let people = crate::ops_people::people_of(project, &map.name).people.into_iter().map(|p| p.name).collect();
    let triggers = crate::ops_scripts::scripts_of(project, &map.name).triggers.into_iter().map(|t| t.id).collect();
    let notes = notes
        .map(|view| {
            view.notes()
                .notes
                .iter()
                .map(|n| (n.id.clone(), first_words(&n.text, NOTE_PREVIEW_CHARS)))
                .collect()
        })
        .unwrap_or_default();
    Contents { compositions, people, triggers, notes }
}

/// **The first `chars` characters of `text`, one line**, for a row that must stay one line tall.
fn first_words(text: &str, chars: usize) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= chars {
        return one_line;
    }
    let mut out: String = one_line.chars().take(chars.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// **The tree is rebuilt when the map or the search moved**, and the groups a person closed stay closed.
fn rebuild_the_tree(
    open: Option<Res<OpenMap>>,
    project: Option<Res<Project>>,
    notes: Option<Res<crate::notes_view::NotesView>>,
    mut outliners: Query<&mut Outliner>,
    mut tables: Query<&mut Table, With<OutlinerTree>>,
) {
    let (Some(open), Some(project)) = (open, project) else { return };
    let (Ok(mut outliner), Ok(mut table)) = (outliners.single_mut(), tables.single_mut()) else { return };
    // **A settled frame does no work that grows with the map.** The fingerprint reads every piece, so it is taken
    // only when the map, the library or the search moved, or the tree has not been built yet: ten thousand pieces
    // at sixty frames a second was a second of string building for a tree nobody touched.
    // **The notes are watched by a generation, not `is_changed`** (`notes_view` bumps its own clock every
    // frame, so the resource is always "changed" and a settled frame would rebuild the whole tree).
    let notes_now = notes.as_ref().map(|n| n.generation()).unwrap_or(0);
    let moved = outliner.built_from == 0
        || open.is_changed()
        || project.is_changed()
        || outliner.built_notes != notes_now
        || outliner.built_for != outliner.query;
    if !moved {
        return;
    }
    outliner.looks += 1;
    let contents = contents_of(&open, &project, notes.as_deref());
    let now = fingerprint(&open.map, &contents, &outliner.query);
    if outliner.built_from == now {
        let query = outliner.query.clone();
        outliner.built_for = query;
        return;
    }
    let Some(tree) = table.source_mut::<Tree>() else { return };
    let (nodes, rows, children) = nodes_for(&open.map, &project.library, &contents, &outliner.query, |key| tree.is_open(key));
    tree.set_nodes(nodes);
    outliner.rows = rows;
    outliner.children = children;
    outliner.built_from = now;
    outliner.built_notes = notes_now;
    let query = outliner.query.clone();
    outliner.built_for = query;
    outliner.rebuilds += 1;
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

/// **The pieces a person's eye has taken out of the viewport, for this session** (UI-51, #237).
///
/// A resource and not a field of the section, because the eye outlives the section: closing the outliner is
/// not showing everything again, and the map's own drawing system ([`crate::editor::hide_carried`]) is the
/// one place a placement's `Visibility` is written — a second writer would fight it for the same component.
///
/// **Not a map field, on purpose.** A hidden piece is view state, and a map file that says a wall is
/// invisible is a map every consumer (a game, a generator, this editor) has to agree with. The eye is
/// therefore this session's, and reopening the map shows everything.
#[derive(Resource, Default, Clone, PartialEq)]
pub struct Eyes {
    hidden: BTreeSet<String>,
}

impl Eyes {
    /// Whether the piece `id` is behind the eye.
    pub fn is_hidden(&self, id: &str) -> bool {
        self.hidden.contains(id)
    }

    /// **Put the piece `id` behind the eye (`hidden` true) or in front of it.** Returns whether it moved.
    pub fn set(&mut self, id: &str, hidden: bool) -> bool {
        if hidden {
            self.hidden.insert(id.to_owned())
        } else {
            self.hidden.remove(id)
        }
    }

    /// Every piece behind the eye, in the order they were hidden.
    pub fn hidden(&self) -> Vec<String> {
        self.hidden.iter().cloned().collect()
    }

    /// Whether nothing is hidden.
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty()
    }
}

/// **The lock on a row's piece**, which is the same write the `P` key makes: `placement_set_pin`, so a
/// toggle here is one operation, one log line and one Cmd+Z.
///
/// The reason a pin carries is asked for by the keystroke path (`EditorState::pinning` is private to
/// [`crate::editor`], and the field's own prompt is the one place it is read), so the row writes the same
/// sentence the editor writes when it pins a piece from a panel: a reason a reader can act on, named for
/// where it came from. Unpinning is immediate.
pub fn toggle_the_lock(project: &Project, open: &mut OpenMap, state: &mut crate::editor::EditorState, id: &str) {
    let Some(placed) = open.map.placements.iter().find(|p| p.id == id) else {
        state.status.problem(format!("{id} is no longer on the map, so it cannot be pinned."));
        return;
    };
    let pinned = !placed.owned;
    let because = pinned.then(|| "pinned from the outliner".to_owned());
    crate::editor::set_pin(project, open, state, id, pinned, because);
}

/// **A row's lock, worked the way the menu works it**: a piece already pinned is unpinned at once, and one
/// that is not is pinned with the reason a test gives (the menu's path asks the author for it).
pub fn set_lock_for_test(world: &mut World, key: u64, on: bool) {
    use bevy::ecs::system::RunSystemOnce;
    let Some(id) = row_piece(world, key) else { return };
    // **A system, not three `resource_mut`s**: a `World` lends one mutable borrow at a time, and the pin
    // writes two resources at once. Running it as a system is how the editor's own callers do it.
    world.run_system_once(move |project: Res<Project>, mut open: ResMut<OpenMap>, mut state: ResMut<crate::editor::EditorState>| {
        let was = open.map.placements.iter().find(|p| p.id == id).is_some_and(|p| p.owned);
        if was == on {
            return;
        }
        let because = on.then(|| "pinned from the outliner".to_owned());
        crate::editor::set_pin(&project, &mut open, &mut state, &id, on, because);
    })
    .unwrap_or_else(|e| panic!("the pin could not be written: {e}"));
}

/// **A row's eye, worked the way the menu works it**: the piece goes behind the session's eye or comes back.
pub fn set_eye_for_test(world: &mut World, key: u64, on: bool) {
    let Some(id) = row_piece(world, key) else { return };
    world.resource_mut::<Eyes>().set(&id, !on);
}

/// The piece the row `key` names, or `None` when it names none.
fn row_piece(world: &mut World, key: u64) -> Option<String> {
    let mut outliners = world.query::<&Outliner>();
    let outliner = outliners.iter(world).next()?;
    outliner.piece_of(key).map(str::to_owned)
}

/// **The one context menu every outliner row names.** A component on the entity, not a resource, for the
/// reason [`crate::piece_menu::PieceMenu`] gives: it is one entity's identity, not editor state.
#[derive(Component)]
pub struct OutlinerRowMenu;

/// What one item of the row's menu does.
#[derive(Clone, Copy)]
enum RowVerb {
    /// The piece's lock: `placement_set_pin`, the same write the `P` key makes.
    Lock,
    /// The piece out of the viewport for this session.
    Eye,
}

/// What the menu holds, top to bottom. One table, so the menu and its test read the same list.
const ROW_ITEMS: [(&str, RowVerb); 2] = [("Pin", RowVerb::Lock), ("Hide", RowVerb::Eye)];

/// **The row's menu, spawned once with the screen.** gooey's `context_menu`; the items are gooey's
/// `menu_item`s, and the row they were opened for is gooey's `ContextTarget`.
fn spawn_row_menu(mut commands: Commands) {
    let menu = commands.spawn((bevy_gooey::context_menu(()), OutlinerRowMenu)).id();
    for (label, verb) in ROW_ITEMS {
        commands
            .spawn((bevy_gooey::menu_item(label), ChildOf(menu)))
            .observe(move |activate: On<bevy::ui_widgets::Activate>,
                           parents: Query<&ChildOf>,
                           targets: Query<&bevy_gooey::ContextTarget>,
                           rows: Query<&bevy_gooey::TableRow>,
                           outliners: Query<&Outliner>,
                           mut project: Option<ResMut<Project>>,
                           mut open: Option<ResMut<OpenMap>>,
                           mut state: Option<ResMut<crate::editor::EditorState>>,
                           mut eyes: ResMut<Eyes>| {
                let Some(row) = bevy_gooey::ContextTarget::of(activate.entity, &parents, &targets) else { return };
                let Ok(table_row) = rows.get(row) else { return };
                let Ok(outliner) = outliners.single() else { return };
                let Some(id) = outliner.piece_of(table_row.key).map(str::to_owned) else {
                    // A group, a composition, a person, a trigger and a note are not placements: there is
                    // nothing to pin and nothing to draw, and the menu says so by offering neither.
                    return;
                };
                match verb {
                    RowVerb::Lock => {
                        let (Some(project), Some(open), Some(state)) = (project.as_deref_mut(), open.as_deref_mut(), state.as_deref_mut()) else { return };
                        toggle_the_lock(project, open, state, &id);
                    }
                    RowVerb::Eye => {
                        let hidden = !eyes.is_hidden(&id);
                        eyes.set(&id, hidden);
                    }
                }
            });
    }
}

/// **A row is rebound as the tree scrolls, so the menu is named on the rows that do not have it yet**
/// rather than once. The filter is an archetype test: it costs nothing while every row has the menu.
///
/// **Only the outliner tree's rows.** The editor's other tables (the DATA table) draw `TableRow`s of
/// their own, and naming this menu on those would offer Pin and Hide for a data row.
fn name_the_menu_on_rows(
    menu: Query<Entity, With<OutlinerRowMenu>>,
    trees: Query<Entity, With<OutlinerTree>>,
    rows: Query<(Entity, &bevy_gooey::TableRow), Without<bevy_gooey::ContextMenu>>,
    mut commands: Commands,
) {
    let Ok(menu) = menu.single() else { return };
    let Ok(tree) = trees.single() else { return };
    for (row, table_row) in &rows {
        if table_row.table != tree {
            continue;
        }
        // `try_`: the tree rebinds its pool in the same frame and may despawn a row this has just seen.
        commands.entity(row).try_insert(bevy_gooey::ContextMenu(menu));
    }
}
