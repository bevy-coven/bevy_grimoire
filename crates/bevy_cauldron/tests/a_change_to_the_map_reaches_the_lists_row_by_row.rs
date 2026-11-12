//! **A change reaches the outliner and the history as the rows it touched, and the lists read exactly as a whole build
//! would draw them** (UI-93, #259).
//!
//! The frame a change lands on used to rebuild both lists whole: 10,000 outliner nodes and 5,000 history lines, 3.8 ms on
//! `bmb` and 6.6 ms on `big`, against the 2 ms the issue asks. The lists now put the one row that came, went or was
//! relabelled into gooey's tree (`Tree::insert`, `Tree::remove`, `Tree::relabel`) and leave the rest alone. A cache that
//! drifts from what it caches is the way that goes wrong, so this file holds the one property that matters:
//!
//! - **after any sequence of changes, each list reads the same as a build from scratch** — the outliner's rows (their order,
//!   their labels, the pieces behind them and the rows under each group) are [`nodes_for`]'s, and the history's lines are
//!   the session's own. The changes are random, from fixed seeds so a failure repeats: a piece placed (a new group, a new
//!   layer, a piece put over another and replacing it), an undo, a redo, a jump to any line, a piece hidden or locked, a
//!   layer hidden.
//! - **the cheap path is the one taken for the changes it is for**: a piece placed or taken back and a mark set are patches,
//!   not builds.

use bevy::prelude::*;
use bevy_cauldron::editor::{self, EditorState, HistoryTree};
use bevy_cauldron::outliner::{
    Contents, Eyes, Locks, Outliner, OutlinerItem, OutlinerSearch, OutlinerTree, nodes_for, placement_key,
};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_gooey::{RowSource, Table, Tree, ValueChange};

use crate::fixtures;

/// The seeds the random sequences start from: fixed, so a failure is the same failure on the next run.
const SEEDS: [u64; 4] = [0x9e37_79b9_7f4a_7c15, 0x2545_f491_4f6c_dd1d, 7, 0xdead_beef];
/// How many changes one seed makes.
const STEPS: usize = 120;
/// The side of the grid pieces are placed on, in cells: small, so a placement lands on another and replaces it now and then.
const CELLS: u32 = 5;
/// A cell's width in metres: wide enough that two cells never overlap.
const CELL_M: f32 = 3.0;
/// How many frames a change is given to reach the lists.
const FRAMES_PER_CHANGE: usize = 3;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **The project**: a floor piece, a floor piece of another kind, a ceiling piece, and a piece that is already placed so
/// the lists have a row before the first change.
fn rig(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(CELLS as f32 * CELL_M + 1.0)
        .kind_tokens(&["lamp"])
        .descriptor("crate", "alpha")
        .descriptor("lamp", "alpha")
        .descriptor("beam", "alpha")
        .place("crate", (1.0, 1.0))
        .build("m");
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.ron");
    let text = std::fs::read_to_string(&library).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    let text = rewrite(&text, "lamp", "kind: [\"prop\"]", "kind: [\"lamp\"]");
    let text = rewrite(&text, "beam", "mount: Some(OnFloor)", "mount: Some(OnCeiling)");
    std::fs::write(&library, text).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("the View menu carries one Show outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 8);
    app
}

/// **Rewrite the first `was` after the descriptor `id` to `becomes`**, which is how a real kit says where a piece goes.
fn rewrite(text: &str, id: &str, was: &str, becomes: &str) -> String {
    let at = text.find(&format!("id: \"{id}\"")).unwrap_or_else(|| panic!("no `{id}` in the library"));
    let found = text[at..].find(was).unwrap_or_else(|| panic!("`{id}` does not have `{was}`")) + at;
    let mut text = text.to_owned();
    text.replace_range(found..found + was.len(), becomes);
    text
}

/// A small, fast random source: the same seed makes the same sequence on every machine.
struct Dice(u64);

impl Dice {
    fn roll(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.roll() % n.max(1) as u64) as usize
    }
}

fn the_outliners_tree(app: &mut App) -> Tree {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<OutlinerTree>>().single(world).expect("one outliner tree");
    world.get::<Table>(entity).and_then(|t| t.source::<Tree>()).cloned().expect("the outliner is a tree")
}

fn the_historys_tree(app: &mut App) -> Tree {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<HistoryTree>>().single(world).expect("one history list");
    world.get::<Table>(entity).and_then(|t| t.source::<Tree>()).cloned().expect("the history is a tree")
}

/// Every row a tree shows, top to bottom: its key and its words.
fn rows_of(tree: &Tree) -> Vec<(u64, String)> {
    (0..tree.len()).map(|row| (tree.key(row), tree.cell(row, 0).into_owned())).collect()
}

/// **The outliner as a build from scratch would draw it**, against what it draws.
fn the_outliner_reads_as_a_whole_build(app: &mut App, said: &str) {
    let held = the_outliners_tree(app);
    let (map, library, eyes, locks) = {
        let world = app.world();
        (
            world.resource::<OpenMap>().map.clone(),
            world.resource::<Project>().library.clone(),
            world.resource::<Eyes>().clone(),
            world.resource::<Locks>().clone(),
        )
    };
    let (nodes, rows, children) = nodes_for(&map, &library, &Contents::default(), "", (&eyes, &locks), |key| held.is_open(key));
    let whole = Tree::new(nodes);
    assert_eq!(rows_of(&held), rows_of(&whole), "the outliner's rows are a whole build's after {said}");
    let world = app.world_mut();
    let outliner = world.query::<&Outliner>().single(world).expect("the outliner is open");
    for (group, under) in &children {
        assert_eq!(outliner.rows_under(*group), under.as_slice(), "the rows under group {group:x} are a whole build's after {said}");
    }
    for placed in &map.placements {
        let key = placement_key(&placed.id);
        assert_eq!(outliner.piece_of(key), rows.get(&key).map(String::as_str), "the piece behind `{}`'s row after {said}", placed.id);
    }
}

/// **The history as the session's own lines say it**: line 0 is the map as it was, then every entry oldest first, and the
/// line the editor stands on says `now:`.
fn the_history_reads_as_the_sessions_lines(app: &mut App, said: &str) {
    let held = the_historys_tree(app);
    let state = app.world().resource::<EditorState>();
    let depth = state.undo_depth();
    let rows = state.history();
    assert_eq!(held.len(), rows.len() + 1, "the history holds line 0 and one line per entry after {said}");
    let now = |position: usize| if position == depth { "now: " } else { "" };
    let first = held.label(0).unwrap_or_default().trim_start_matches("now: ").to_owned();
    assert_eq!(held.label(0), Some(format!("{}{first}", now(0)).as_str()), "line 0 after {said}");
    for row in rows {
        let who = if row.agent { "the Familiar" } else { "you" };
        let want = format!("{}{}  \u{b7} {who}", now(row.position), row.label);
        assert_eq!(held.label(row.position as u64), Some(want.as_str()), "history line {} after {said}", row.position);
    }
}

/// **One random change**, and its name for the failure message.
fn change(app: &mut App, dice: &mut Dice) -> String {
    let descriptors = ["crate", "lamp", "beam"];
    let depth = app.world().resource::<EditorState>().undo_depth();
    let redo = app.world().resource::<EditorState>().redo_depth();
    let ids: Vec<(String, (f32, f32))> =
        app.world().resource::<OpenMap>().map.placements.iter().map(|p| (p.id.clone(), p.at)).collect();
    match dice.below(11) {
        0..=3 => {
            let (x, z) = (dice.below(CELLS as usize), dice.below(CELLS as usize));
            let at = (x as f32 * CELL_M + 1.0, z as f32 * CELL_M + 1.0);
            let descriptor = descriptors[dice.below(descriptors.len())];
            editor::place_for_test(app.world_mut(), descriptor, at);
            format!("a `{descriptor}` placed at {at:?}")
        }
        4 if !ids.is_empty() => {
            // **A piece put where another stands replaces it**: a piece leaves from the middle of the map's list and another
            // is added at its end.
            let (id, at) = ids[dice.below(ids.len())].clone();
            let descriptor = descriptors[dice.below(descriptors.len())];
            editor::place_for_test(app.world_mut(), descriptor, at);
            format!("a `{descriptor}` placed over `{id}` at {at:?}")
        }
        5 if depth > 0 => {
            editor::undo_for_test(app.world_mut());
            "an undo".to_owned()
        }
        6 if redo > 0 => {
            editor::redo_for_test(app.world_mut());
            "a redo".to_owned()
        }
        7 => {
            let position = dice.below(depth + redo + 1);
            editor::jump_for_test(app.world_mut(), position);
            format!("a jump to line {position}")
        }
        8 if !ids.is_empty() => {
            let (id, _) = &ids[dice.below(ids.len())];
            let hidden = !app.world().resource::<Eyes>().is_hidden(id);
            app.world_mut().resource_mut::<Eyes>().set(id, hidden);
            format!("`{id}` hidden {hidden}")
        }
        9 if !ids.is_empty() => {
            let (id, _) = &ids[dice.below(ids.len())];
            let locked = !app.world().resource::<Locks>().is_locked(id);
            app.world_mut().resource_mut::<Locks>().set(id, locked);
            format!("`{id}` locked {locked}")
        }
        10 => {
            let hidden = !app.world().resource::<Eyes>().layer_hidden("ceiling");
            app.world_mut().resource_mut::<Eyes>().set_layer("ceiling", hidden);
            format!("the ceiling layer hidden {hidden}")
        }
        _ => "nothing".to_owned(),
    }
}

/// **Any run of changes leaves both lists as a build from scratch would draw them.**
#[test]
fn after_any_run_of_changes_the_lists_read_as_a_whole_build() {
    for seed in SEEDS {
        let mut app = rig(&format!("a_change_reaches_the_lists_row_by_row_{seed:x}"));
        let mut dice = Dice(seed);
        let mut said = format!("seed {seed:x}, before any change");
        the_outliner_reads_as_a_whole_build(&mut app, &said);
        the_history_reads_as_the_sessions_lines(&mut app, &said);
        for step in 0..STEPS {
            let what = change(&mut app, &mut dice);
            said = format!("seed {seed:x}, step {step} ({what})");
            updates(&mut app, FRAMES_PER_CHANGE);
            the_outliner_reads_as_a_whole_build(&mut app, &said);
            the_history_reads_as_the_sessions_lines(&mut app, &said);
        }
    }
}

/// **A piece placed, a piece taken back and a mark set are patches**: the tree was told about the rows, not built again.
#[test]
fn a_piece_placed_taken_back_or_marked_is_a_patch_not_a_build() {
    let mut app = rig("a_change_reaches_the_lists_row_by_row_patches");
    let counts = |app: &mut App| {
        let world = app.world_mut();
        let outliner = world.query::<&Outliner>().single(world).expect("the outliner is open");
        (outliner.rebuilds(), outliner.patches())
    };
    let (built, patched) = counts(&mut app);
    assert_eq!(built, 1, "the tree was built once, when the section opened");
    assert_eq!(patched, 0, "and that was a whole build");

    editor::place_for_test(app.world_mut(), "crate", (4.0, 4.0));
    updates(&mut app, FRAMES_PER_CHANGE);
    assert_eq!(counts(&mut app), (built + 1, patched + 1), "a piece placed is one change, and a patch");

    let id = app.world().resource::<OpenMap>().map.placements.last().map(|p| p.id.clone()).expect("the piece is on the map");
    app.world_mut().resource_mut::<Eyes>().set(&id, true);
    updates(&mut app, FRAMES_PER_CHANGE);
    assert_eq!(counts(&mut app), (built + 2, patched + 2), "a piece hidden is one relabelled row, and a patch");
    let held = the_outliners_tree(&mut app);
    assert!(held.label(placement_key(&id)).is_some_and(|label| label.ends_with("(hidden)")), "the row says so: {:?}", held.label(placement_key(&id)));

    editor::undo_for_test(app.world_mut());
    updates(&mut app, FRAMES_PER_CHANGE);
    assert_eq!(counts(&mut app), (built + 3, patched + 3), "a placement taken back is one change, and a patch");
    assert!(!the_outliners_tree(&mut app).contains(placement_key(&id)), "its row is gone");
}

/// **A nudge under a held search is not a rebuild**: nothing a row shows moved, so the snapshot says so and the tree is left
/// as it is, search or no search.
#[test]
fn a_nudge_under_a_held_search_rebuilds_nothing() {
    let mut app = rig("a_change_reaches_the_lists_row_by_row_search_nudge");
    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerSearch>>().single(world).expect("one search box")
    };
    app.world_mut().commands().trigger(ValueChange { source: search, value: "crate".to_owned(), is_final: false });
    updates(&mut app, FRAMES_PER_CHANGE);
    let rebuilds = |app: &mut App| {
        let world = app.world_mut();
        world.query::<&Outliner>().single(world).expect("the outliner is open").rebuilds()
    };
    let built = rebuilds(&mut app);
    let held = rows_of(&the_outliners_tree(&mut app));
    app.world_mut().resource_mut::<OpenMap>().map.placements[0].at.0 += 0.5;
    updates(&mut app, FRAMES_PER_CHANGE);
    assert_eq!(rebuilds(&mut app), built, "a piece nudged under a held search is no change to any row");
    assert_eq!(rows_of(&the_outliners_tree(&mut app)), held, "and the tree stands as it did");
}

/// **Entries recorded after an undo of two, in the frame that follows it, leave the history reading as the session's lines**:
/// the stacks end with the same number of lines and nothing to redo, which a redo of everything also does, so the list is not
/// patched from the two stack lengths alone.
#[test]
fn entries_recorded_after_an_undo_of_two_leave_the_history_as_the_sessions_lines() {
    let mut app = rig("a_change_reaches_the_lists_row_by_row_history_drift");
    for n in 0..3 {
        editor::place_for_test(app.world_mut(), "crate", (4.0 + n as f32 * CELL_M, 4.0));
        updates(&mut app, FRAMES_PER_CHANGE);
    }
    editor::undo_for_test(app.world_mut());
    editor::undo_for_test(app.world_mut());
    updates(&mut app, FRAMES_PER_CHANGE);
    for n in 0..2 {
        editor::place_for_test(app.world_mut(), "lamp", (4.0 + n as f32 * CELL_M, 10.0));
    }
    updates(&mut app, FRAMES_PER_CHANGE);
    the_history_reads_as_the_sessions_lines(&mut app, "an undo of two and two new entries");
    the_outliner_reads_as_a_whole_build(&mut app, "an undo of two and two new entries");
}
