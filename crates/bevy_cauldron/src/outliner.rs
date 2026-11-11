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
//! - **The map's other contents are four groups of their own** below the pieces': its stamps
//!   (`compositions`), its people (`people/<map>.ron`), its triggers (`scripts/<map>.ron`) and its notes
//!   (`.notes.ron`). A row in one of them names the thing and nothing else: it is not a placement, so
//!   clicking it selects nothing and its menu offers no Pin and no Hide.
//!
//! # A row's menu
//!
//! gooey's tree row is a chevron and a label, so a row's two toggles are gooey's own `context_menu`
//! ([`crate::outliner::OutlinerRowMenu`]) named on **the piece rows only**, the way [`crate::palette_menu`]
//! names one on the palette's rows: never a control built here, and never on a row that has nothing to
//! toggle. The rows are a pool rebound as the tree scrolls, so the menu is removed again from a row that is
//! rebound to a group or to something that is not a piece.
//!
//! - **Pin** is the piece's `placement_set_pin`, the write the `O` key makes (`Action::OwnToggle`), through
//!   [`crate::editor::toggle_pin`] — the same function the key's prompt goes through, so a pin always asks
//!   for its reason the way `EditorState::pinning` says it must. One operation, one log line, one Cmd+Z.
//! - **Hide** takes the piece out of the viewport **for this session** ([`Eyes`]). A hide is view state:
//!   the map format has no "hidden" field and this does not add one, because a map file that says a wall
//!   is invisible is a map every consumer has to agree with.
//!
//! # Keys
//!
//! A tree row's key is a `u64`. A piece's is a hash of its id with the two high bits clear, a group's a
//! hash of its name with the top bit set and any other thing's a hash of its section and id with the second
//! set ([`placement_key`], [`group_key`], [`thing_key`]), so the three families never meet and a key
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

/// The top bit of a key: set on a group's, clear on a piece's.
const GROUP_BIT: u64 = 1 << 63;

/// The group a piece with no `kind` tag stands under.
const OTHER_GROUP: &str = "other";

/// **The layer a piece stands in, as a person reads it** (UI-51, #237), from the one field that already answers it:
/// the descriptor's [`Mount`](bevy_grimoire_core::descriptor::Mount).
///
/// **No new map field, and this is the smallest data change there is.** A `Placed` has no layer because it does not
/// need one: what decides where a piece sits is its descriptor's mount — the map's own note on [`Placed::at`]
/// says the height "comes from the descriptor's Mount, never from the author" — and the one case a mount cannot
/// settle alone, [`Mount::OnSurfaceOrFloor`](bevy_grimoire_core::descriptor::Mount::OnSurfaceOrFloor), is recorded
/// on the placement by [`Placed::on`], which is `Some(host)` when it took the surface. Adding a `layer` field would
/// be a second statement of a fact the file already holds, and the day the two disagreed the map would be wrong in
/// a way no reader could see.
///
/// The names are the six things a person would say: floor, wall, ceiling, surface (on another piece), opening (a
/// doorway) and decal.
pub fn layer_of(library: &bevy_grimoire_core::library::Library, placed: &bevy_grimoire_core::map::Placed) -> &'static str {
    layer_in(descriptor_of(library, placed).as_deref(), placed)
}

/// **[`layer_of`] for a caller that has already looked the descriptor up**: a rebuild asks about every piece of the map, and
/// a lookup per question is three lookups per piece (UI-93, #259).
fn layer_in(descriptor: Option<&bevy_grimoire_core::descriptor::Descriptor>, placed: &bevy_grimoire_core::map::Placed) -> &'static str {
    use bevy_grimoire_core::descriptor::Mount;
    match descriptor.and_then(|d| d.mount.as_ref()) {
        // **The two-way mount is settled by the placement's own record**: `on` is `Some` when it took a surface.
        Some(Mount::OnSurfaceOrFloor { .. }) => {
            if placed.on.is_some() {
                LAYER_SURFACE
            } else {
                LAYER_FLOOR
            }
        }
        Some(Mount::OnWall { .. }) | Some(Mount::OnFace { .. }) => LAYER_WALL,
        Some(Mount::OnCeiling) => LAYER_CEILING,
        Some(Mount::OnSurface { .. }) => LAYER_SURFACE,
        Some(Mount::InOpening { .. }) => LAYER_OPENING,
        Some(Mount::Decal { .. }) => LAYER_DECAL,
        Some(Mount::OnFloor) | Some(Mount::Tiled) | None => LAYER_FLOOR,
    }
}

/// A piece standing on the floor.
pub const LAYER_FLOOR: &str = "floor";
/// A piece on a wall: a sconce, a shelf, a sign.
pub const LAYER_WALL: &str = "wall";
/// A piece hanging from the ceiling.
pub const LAYER_CEILING: &str = "ceiling";
/// A piece resting on another piece.
pub const LAYER_SURFACE: &str = "surface";
/// A piece filling a doorway.
pub const LAYER_OPENING: &str = "opening";
/// A piece painted onto a surface.
pub const LAYER_DECAL: &str = "decal";

/// The outliner's state, on its section: what is typed, what the tree was last built from, and which
/// piece each row means.
#[derive(Component, Default)]
pub struct Outliner {
    /// What the search box holds.
    pub query: String,
    /// The piece behind each row's key: a placement only, so a row for a person is not one.
    rows: HashMap<u64, String>,
    /// The rows drawn under each group, in the order they are drawn, so a row can be asked what it is under.
    children: HashMap<u64, Vec<u64>>,
    /// The notes' generation the tree was last built from. `NotesView` marks itself changed every frame (it
    /// keeps its own clock), so `is_changed` says nothing about whether the notes moved.
    built_notes: u64,
    /// The stat of the two side files the tree was last built from, so a write to `people/` or `scripts/` is
    /// noticed even though no resource in the editor changed.
    built_files: Vec<FileStat>,
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

    /// **The rows drawn under the group `key`**, in the order they are drawn.
    pub fn rows_under(&self, key: u64) -> &[u64] {
            self.children.get(&key).map_or(&[], Vec::as_slice)
        }

        /// **What the row `key` names**: the piece it stands for, or `None` for a group and for a row for a
        /// composition, a person, a trigger or a note.
        pub fn piece_of(&self, key: u64) -> Option<&str> {
            self.rows.get(&key).map(String::as_str)
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

/// The second bit: set on a row for something that is not a placement (a composition, a person, a trigger
/// or a note), so it names no piece and offers no pin and no eye.
pub const THING_BIT: u64 = 1 << 62;

/// **The key of the row for the piece `id`**, with both high bits clear so no family can collide with it.
pub fn placement_key(id: &str) -> u64 {
    fnv(id) & !(GROUP_BIT | THING_BIT)
}

/// The key of the row for the group `kind`.
pub fn group_key(kind: &str) -> u64 {
    fnv(kind) | GROUP_BIT
}

/// **The key of the row for `kind` under `layer`** (UI-51, #237).
///
/// A kind is not unique in the tree any more: `prop` stands under `floor` and under `ceiling`, and a tree drops a
/// second node that repeats a key. So the kind's key is scoped by its layer — `floor/prop` — and the layer's own
/// row keeps [`group_key`]'s plain name, which is what a person reads.
pub fn layer_kind_key(layer: &str, kind: &str) -> u64 {
    group_key(&format!("{layer}/{kind}"))
}

/// **The key of the row for something that is not a placement**, hashed from its section and its id so a
/// note and a person that happen to share a name are still two rows.
pub fn thing_key(section: &str, id: &str) -> u64 {
    (fnv(&format!("{section}\u{1f}{id}")) & !GROUP_BIT) | THING_BIT
}

pub struct OutlinerPlugin;

impl Plugin for OutlinerPlugin {
    fn build(&self, app: &mut App) {
        // **The eye is registered here and reset by the door** (`screen::door_state!`): it holds ids of the
        // open map's rows, so it cannot outlive the map.
        app.init_resource::<Eyes>();
        app.add_systems(
            Update,
            (rebuild_the_tree, follow_the_viewport, tick_the_menu_row).chain().run_if(in_state(Screen::Editor)),
        );
        // **A row's menu** (UI-51, #237): gooey's own `context_menu`, named on the piece rows and taken off the
        // others — never a control built here.
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

/// **What the map holds that is not a placement**, read at a rebuild and handed to [`nodes_for`]: the stamps,
/// the people, the triggers and the notes, as the rows that name them.
#[derive(Default, Clone, PartialEq)]
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

/// The group a stamp stands under.
pub const COMPOSITIONS: &str = "compositions";
/// The group a person stands under.
pub const PEOPLE: &str = "people";
/// The group a trigger stands under.
pub const TRIGGERS: &str = "triggers";
/// The group a note stands under.
pub const NOTES: &str = "notes";

/// How much of a note's text a row shows. One line, so the row stays one line tall however long the note is.
const NOTE_PREVIEW_CHARS: usize = 48;

/// **The tree, built from the map, the map's other contents and the search.** Returns the nodes, the piece
/// behind each key, and the rows drawn under each group.
///
/// A row is kept when every word of `query` is in its words or its group's name. A group with no row kept is
/// not drawn. A piece's groups are in name order; the map's own four (compositions, people, triggers, notes)
/// come after them in the order the issue names them, because a piece is what a map is mostly made of and a
/// reader looks for it first. Pieces are in the map's order, so a rebuild never shuffles rows under the
/// pointer. `group_open` says whether a group the tree already had is open; a group it did not have starts
/// open, and so does every group while something is searched for.
#[allow(clippy::type_complexity)]
pub fn nodes_for(
    map: &Map,
    library: &bevy_grimoire_core::library::Library,
    contents: &Contents,
    query: &str,
    group_open: impl Fn(u64) -> Option<bool>,
) -> (Vec<TreeNode>, HashMap<u64, String>, HashMap<u64, Vec<u64>>) {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let searching = !words.is_empty();
    let kept = |haystack: &str, group: &str| {
        if !searching {
            return true;
        }
        let haystack = format!("{haystack} {group}").to_lowercase();
        words.iter().all(|word| haystack.contains(word.as_str()))
    };
    // **A piece's group is its layer first, then what it is** (UI-51, #237): a `BTreeMap` of layers, each holding a
    // `BTreeMap` of kinds, so both levels are in name order and a rebuild never shuffles a row under the pointer.
    // **The layer comes from the piece's own mount**, [`layer_of`]'s rule, so a wall sconce and the floor it hangs
    // above are never in the same group.
    let mut piece_groups: std::collections::BTreeMap<&'static str, std::collections::BTreeMap<String, Vec<(u64, String)>>> =
        std::collections::BTreeMap::new();
    let mut rows = HashMap::with_capacity(map.placements.len());
    // **Every layer the map has, kept or not**, because the tree's shape is decided from the whole map and not from what a
    // search left (see `nested` below).
    let mut layers_seen: BTreeSet<&'static str> = BTreeSet::new();
    for placed in &map.placements {
        // **One lookup per piece**: the layer and the kind both come from the same descriptor.
        let descriptor = descriptor_of(library, placed);
        let layer = layer_in(descriptor.as_deref(), placed);
        layers_seen.insert(layer);
        let kind: &str = descriptor.as_deref().and_then(|d| d.kind.first()).map_or(OTHER_GROUP, String::as_str);
        // **A search may name the layer**, so `wall` finds what hangs on a wall, which is the same rule the kinds and
        // the map's other sections follow. **Nothing is built for a piece when nothing is typed**: the haystack is a
        // string per piece, and ten thousand of them are most of what an unfiltered rebuild would cost.
        if searching && !kept(&format!("{} {} {layer}", placed.id, placed.descriptor), kind) {
            continue;
        }
        let key = placement_key(&placed.id);
        rows.insert(key, placed.id.clone());
        let kinds = piece_groups.entry(layer).or_default();
        // The kind's name is copied once per kind, not once per piece.
        let group = match kinds.get_mut(kind) {
            Some(group) => group,
            None => kinds.entry(kind.to_owned()).or_default(),
        };
        group.push((key, format!("{} \u{b7} {}", placed.id, placed.descriptor)));
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
    let mut nodes = Vec::with_capacity(piece_groups.len() + others.len());
    let mut children = HashMap::new();
    let leaf_group = |key: u64, kind: &str, rows_here: Vec<(u64, String)>, children: &mut HashMap<u64, Vec<u64>>| -> TreeNode {
        children.insert(key, rows_here.iter().map(|(k, _)| *k).collect::<Vec<u64>>());
        let node = TreeNode::new(key, kind.to_owned()).children(rows_here.into_iter().map(|(k, label)| TreeNode::new(k, label)));
        if searching || group_open(key).unwrap_or(true) { node.expanded() } else { node }
    };
    // **A layer is a group whose children are its kinds** (UI-51, #237): `floor` holds `light-source` and `prop`, so a
    // person reads what is on the ground separately from what hangs on a wall.
    //
    // **A map with one layer's pieces grows no level**, because there is nothing to tell apart: the kind rows stand
    // at the top as they did, and the tree a reader already knew is the tree they get. A second layer arriving is
    // the one thing that adds the level, and it adds it for every layer at once so the reader never sees the shape
    // change under them twice.
    let layers: Vec<(&'static str, std::collections::BTreeMap<String, Vec<(u64, String)>>)> =
        piece_groups.into_iter().filter(|(_, kinds)| !kinds.is_empty()).collect();
    // **Decided from the whole map, not from what a search left**, so typing does not re-shape the tree under the
    // reader's hand: a search narrows the rows and leaves the levels where they were.
    let nested = layers_seen.len() > 1;
    for (layer, kinds) in layers {
        let mut kind_keys: Vec<u64> = Vec::new();
        let mut kind_nodes: Vec<TreeNode> = Vec::new();
        for (kind, rows_here) in kinds {
            if rows_here.is_empty() {
                continue;
            }
            let key = if nested { layer_kind_key(layer, &kind) } else { group_key(&kind) };
            kind_keys.push(key);
            kind_nodes.push(leaf_group(key, &kind, rows_here, &mut children));
        }
        if kind_nodes.is_empty() {
            continue;
        }
        if !nested {
            nodes.extend(kind_nodes);
            continue;
        }
        let layer_key = group_key(layer);
        children.insert(layer_key, kind_keys);
        let node = TreeNode::new(layer_key, layer.to_owned()).children(kind_nodes);
        nodes.push(if searching || group_open(layer_key).unwrap_or(true) { node.expanded() } else { node });
    }
    for (kind, rows_here) in others {
        if !rows_here.is_empty() {
            nodes.push(leaf_group(group_key(&kind), &kind, rows_here, &mut children));
        }
    }
    (nodes, rows, children)
}

/// Whether the tree needs building again: a fingerprint of the query, every piece's identity, the map's stamps, the
/// notes' generation, and **the stat of each side file** the map's other contents live in.
///
/// **Hashed as it is read, with no string built.** It used to append every piece's id and descriptor to a `String` — a few
/// hundred kilobytes for ten thousand pieces, allocated and filled on the frame a change lands on — and hash that. The hash
/// is the same one ([`fnv`]), taken over the pieces' own bytes as they are walked, so a build allocates nothing. The two
/// separator bytes are the ones the old text used, so a piece that gained or lost a character still moves the fingerprint.
///
/// **Two stats, and no file's contents.** `people/<map>.ron` and `scripts/<map>.ron` are the two facts the map cannot
/// show: `person_set` and `trigger_set` write them and change no pixel of the map, and a hand edit is the same. Each
/// contributes its path, its modification time and its length — one `metadata` call each — so a write is a rebuild
/// while a settled frame still opens nothing. Parsing them is [`contents_of`]'s work, after this has said a rebuild
/// is due.
fn fingerprint(
    map: &Map,
    library: &bevy_grimoire_core::library::Library,
    notes: u64,
    files: &[FileStat],
    query: &str,
) -> u64 {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    fn fold(mut hash: u64, bytes: &[u8]) -> u64 {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(PRIME);
        }
        hash
    }
    let mut hash = fold(BASIS, query.as_bytes());
    for placed in &map.placements {
        hash = fold(hash, b"\x1f");
        hash = fold(hash, placed.id.as_bytes());
        hash = fold(hash, b"\x1e");
        hash = fold(hash, placed.descriptor.as_bytes());
        // **The layer and the kind the row is drawn under** (UI-51, #237): both are facts about the descriptor, so a mount
        // or a kind edited in the library, or a piece moved onto a table, is a rebuild. They are the static names the
        // tree groups by, so nothing is allocated; `\x1c` and `\x1b` are bytes the other fields never carry, so the
        // fields cannot be read into one another.
        let descriptor = descriptor_of(library, placed);
        hash = fold(hash, b"\x1c");
        hash = fold(hash, layer_in(descriptor.as_deref(), placed).as_bytes());
        hash = fold(hash, b"\x1b");
        hash = fold(hash, descriptor.as_deref().and_then(|d| d.kind.first()).map_or(OTHER_GROUP, String::as_str).as_bytes());
    }
    // **The map's own stamps**, which are rows of the tree too.
    for stamped in &map.stamps {
        hash = fold(hash, b"\x1d");
        hash = fold(hash, stamped.id.as_bytes());
        hash = fold(hash, stamped.of.as_bytes());
    }
    // **The notes' generation**, because the notes view marks itself changed every frame and its own count of
    // replacements is what says they moved.
    hash = fold(hash, &notes.to_le_bytes());
    // **Each side file's identity**, so a write to `people/` or `scripts/` is a rebuild even though the map did not
    // move. The path is folded too, so opening another map is a different fingerprint whatever its stats are.
    for FileStat { path, modified, len } in files {
        hash = fold(hash, b"\x19");
        hash = fold(hash, path.as_bytes());
        hash = fold(hash, &modified.unwrap_or(0).to_le_bytes());
        hash = fold(hash, &len.unwrap_or(0).to_le_bytes());
    }
    hash
}

/// **A side file as cheaply as it can be told apart**: its path, and its modification time and length (both `None`
/// when it is not there). One `metadata` call, no read.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct FileStat {
    path: String,
    modified: Option<u128>,
    len: Option<u64>,
}

impl FileStat {
    /// **The stat of `path`, in NANOSECONDS since the epoch.**
    ///
    /// Whole seconds were the first version and they lost a write: two saves inside one second are the same stat,
    /// so `person_set` followed by a hand edit in the same tick rebuilt nothing and the tree showed the first. A
    /// filesystem that keeps only seconds still answers in seconds here, which is no worse than it was; the one
    /// that keeps nanoseconds — every Mac and Linux — now tells the two writes apart.
    fn of(path: &std::path::Path) -> FileStat {
        let shown = path.to_string_lossy().into_owned();
        let Ok(meta) = std::fs::metadata(path) else {
            return FileStat { path: shown, modified: None, len: None };
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_nanos());
        let len = meta.len();
        FileStat { path: shown, modified, len: Some(len) }
    }
}

/// **The two side files a map's other contents live in**: its people and its triggers. Statted on every look,
/// because a write to either is a change the map itself cannot show.
///
/// **The paths come from the operations that own them** ([`crate::ops_people::path_of`] and
/// [`crate::ops_scripts::path_of`]), not from a second copy of `people/<map>.ron` written here: a stat of a path
/// nothing writes is a rebuild nothing asks for, and the two spellings would drift the first time either moved. A
/// map name the operations refuse is no side file at all, which is the same answer as a missing one.
fn side_files(project: &Project, map: &Map) -> Vec<FileStat> {
    [
        crate::ops_people::path_of(project, &map.name).ok(),
        crate::ops_scripts::path_of(project, &map.name).ok(),
    ]
    .iter()
    .map(|path| path.as_deref().map(FileStat::of).unwrap_or_default())
    .collect()
}

/// **The contents the tree draws that are not placements**, each read from where it already lives: the map's
/// own stamps, its `people/` and `scripts/` files, and the notes the editor holds ([`crate::notes_view`]).
///
/// **Read only when a rebuild is really going to happen.** Two of the four are files, so reading them on every
/// `OpenMap` change would be disk work on a frame that turns out to change no row; the cheap fingerprint is
/// taken first and the files are opened only when it says the tree must be rebuilt.
fn contents_of(open: &OpenMap, project: &Project, notes: Option<&crate::notes_view::NotesView>) -> Contents {
    let map = &open.map;
    let compositions = map.stamps.iter().map(|s| (s.id.clone(), s.of.clone())).collect();
    let people = crate::ops_people::people_of(project, &map.name).people.into_iter().map(|p| p.name).collect();
    let triggers = crate::ops_scripts::scripts_of(project, &map.name).triggers.into_iter().map(|t| t.id).collect();
    let notes = notes
        .map(|view| view.notes().notes.iter().map(|n| (n.id.clone(), first_words(&n.text, NOTE_PREVIEW_CHARS))).collect())
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
    mut budget: ResMut<crate::ui_budget::UiFrame>,
) {
    // **This system's cost, into the frame's total** (UI-93, #259): the tree is the list that grows with the map, and the
    // budget's whole point is that a settled frame pays nothing for its depth. The guard writes on drop, so every `return`
    // below is measured without a call of its own.
    let spent = crate::ui_budget::measure(&mut budget);
    let (Some(open), Some(project)) = (open, project) else { return };
    let (Ok(mut outliner), Ok(mut table)) = (outliners.single_mut(), tables.single_mut()) else { return };
    // **Two stats, not two parses.** `people/<map>.ron` and `scripts/<map>.ron` are the two facts no resource in the
    // editor can show: `person_set` writes one and a hand edit the other, and the map does not change at all. A
    // `metadata` call each is the whole cost of noticing, and they are taken **before** the gate below, because a
    // change in either is precisely a reason the gate has to open.
    let files = side_files(&project, &open.map);
    // **A settled frame does no work that grows with the map.** The fingerprint reads every piece, so it is taken
    // only when the map, the library, the notes, the search or one of those two files moved, or the tree has not
    // been built yet: ten thousand pieces at sixty frames a second was a second of string building for a tree
    // nobody touched.
    let notes_now = notes.as_ref().map(|n| n.generation()).unwrap_or(0);
    let moved = outliner.built_from == 0
        || open.is_changed()
        || project.is_changed()
        || outliner.built_for != outliner.query
        || outliner.built_notes != notes_now
        || outliner.built_files != files;
    if !moved {
        return;
    }
    outliner.looks += 1;
    let now = fingerprint(&open.map, &project.library, notes_now, &files, &outliner.query);
    if outliner.built_from == now && outliner.built_notes == notes_now {
        let query = outliner.query.clone();
        outliner.built_for = query;
        return;
    }
    // **From here it is a rebuild**, which the budget reports apart from a settled frame.
    let _rebuild = spent.rebuilding();
    // **The two files are read here and nowhere earlier**: the cheap fingerprint has already said a rebuild is due,
    // so opening `people/` and `scripts/` is work that reaches the screen in this frame.
    let contents = contents_of(&open, &project, notes.as_deref());
    let Some(tree) = table.source_mut::<Tree>() else { return };
    let (nodes, rows, children) = nodes_for(&open.map, &project.library, &contents, &outliner.query, |key| tree.is_open(key));
    tree.set_nodes(nodes);
    outliner.rows = rows;
    outliner.children = children;
    outliner.built_from = now;
    outliner.built_notes = notes_now;
    outliner.built_files = files;
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
/// A resource and not a field of the section, because the eye outlives the section: closing the outliner is not
/// showing everything again, and the map's own drawing system ([`crate::editor::hide_carried`]) is the one place
/// a placement's `Visibility` is written — a second writer would fight it for the same component.
///
/// **Not a map field, on purpose.** A hidden piece is view state, and a map file that says a wall is invisible is
/// a map every consumer (a game, a generator, this editor) has to agree with. The eye is therefore this session's
/// and is **reset at every door** (`screen::door_state!`): opening another map shows everything, because the ids
/// it holds name rows of the map that was open.
#[derive(Resource, Default)]
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

    /// Every piece behind the eye, in name order.
    pub fn hidden(&self) -> Vec<String> {
        self.hidden.iter().cloned().collect()
    }

    /// Whether nothing is hidden.
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty()
    }
}

/// **The one context menu an outliner row names.** A component on the entity, not a resource, for the reason
/// [`crate::piece_menu::PieceMenu`] gives: it is one entity's identity, not editor state.
#[derive(Component)]
pub struct OutlinerRowMenu;

/// **What one item of a row's menu does.** Public because [`ROW_ITEMS`] is: a table of labels decoded by an index
/// would be two lists that can drift, and the menu and a test both read this one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowVerb {
    /// The piece's lock: `placement_set_pin`, through the `O` key's own [`crate::editor::pin_or_unpin`].
    Pin,
    /// The piece out of the viewport for this session.
    Hide,
}

/// What the menu holds, top to bottom: the words and what each one does. One table, so the menu and its test read
/// the same list, and there is no index to decode.
pub const ROW_ITEMS: [(&str, RowVerb); 2] = [("Pin", RowVerb::Pin), ("Hide", RowVerb::Hide)];

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
                // A row rebound to a group or to something that is not a piece offers nothing: the menu is taken
                // off such a row below, and this is the same answer if a click races that.
                let Some(id) = outliner.piece_of(table_row.key).map(str::to_owned) else { return };
                match verb {
                    RowVerb::Pin => {
                        let (Some(project), Some(open), Some(state)) = (project.as_deref_mut(), open.as_deref_mut(), state.as_deref_mut()) else { return };
                        crate::editor::pin_or_unpin(project, open, state, &id);
                    }
                    RowVerb::Hide => {
                        let hidden = !eyes.is_hidden(&id);
                        eyes.set(&id, hidden);
                    }
                }
            });
    }
}

/// **The menu sits on the piece rows and on nothing else**, and a row that is rebound to something else loses it.
///
/// The tree's rows are a **pool**: the same entity shows row 3 now and row 40 after a scroll, and a group, a
/// person, a trigger and a note are rows too. So this both puts the menu on a row whose key names a piece and
/// **takes it off** a row whose key does not — otherwise a group row would offer Pin and Hide, and choosing one
/// would do nothing and say nothing.
fn name_the_menu_on_rows(
    menu: Query<Entity, With<OutlinerRowMenu>>,
    trees: Query<Entity, With<OutlinerTree>>,
    outliners: Query<&Outliner>,
    rows: Query<(Entity, &bevy_gooey::TableRow, Has<bevy_gooey::ContextMenu>)>,
    mut commands: Commands,
) {
    let Ok(menu) = menu.single() else { return };
    let Ok(tree) = trees.single() else { return };
    let Ok(outliner) = outliners.single() else { return };
    for (row, table_row, named) in &rows {
        if table_row.table != tree {
            continue;
        }
        match (outliner.piece_of(table_row.key).is_some(), named) {
            (true, false) => {
                // `try_`: the tree rebinds its pool in the same frame and may despawn a row this has just seen.
                commands.entity(row).try_insert(bevy_gooey::ContextMenu(menu));
            }
            (false, true) => {
                commands.entity(row).try_remove::<bevy_gooey::ContextMenu>();
            }
            _ => {}
        }
    }
}
