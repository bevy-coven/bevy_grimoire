//! **The outliner groups a map's pieces by the layer they stand in, and names a stamp and a note by what they are**
//! (UI-51, #237).
//!
//! The issue's *What* asks for "placements by layer and group". The first slice grouped by kind, which is a different
//! question: a floor lamp and the sconce above it are both `light-source`, and a person standing at the map asking
//! *what is on the floor* could not answer it from the tree. This file is the layer half, plus the two row labels the
//! previous pass claimed and did not write.
//!
//! **A layer is derived, never stored.** `Placed` has no layer field — see [`bevy_cauldron::outliner::layer_of`] — and
//! the smallest data change is none: the descriptor's `Mount` already says where a piece goes, and `Placed::on` already
//! records which half of a two-way mount a piece took. A field would be a second statement of a fact the map holds, so
//! this file pins the derivation instead: a floor piece, a wall piece, a ceiling piece and a surface piece each stand
//! under their own layer, and a two-way piece lands under `floor` or `surface` by its own `on`.
//!
//! The labels are the other half. A stamp row reads `<id> · <the composition it is of>` and a note row reads
//! `<id>` or `<id> · <its first words>`, and a long note is cut to one line — the row is one line tall, so a note of
//! four hundred characters must not be drawn as four hundred.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::outliner::{
    COMPOSITIONS, NOTES, Outliner, OutlinerItem, OutlinerSearch, OutlinerTree, group_key, layer_of, placement_key,
    thing_key,
};
use bevy_cauldron::project::OpenMap;
use bevy_gooey::{RowSource, Table, Tree, ValueChange};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **Rewrite one descriptor's `mount`**, which is how a real kit says where its pieces go.
fn mount(library: &Path, id: &str, mount: &str) {
    let text = std::fs::read_to_string(library).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    let at = text.find(&format!("id: \"{id}\"")).unwrap_or_else(|| panic!("no `{id}` in the library"));
    let was = "mount: Some(OnFloor)";
    let found = text[at..].find(was).unwrap_or_else(|| panic!("`{id}` does not have the fixture's floor mount")) + at;
    let mut text = text;
    text.replace_range(found..found + was.len(), &format!("mount: Some({mount})"));
    std::fs::write(library, text).unwrap_or_else(|e| panic!("{library:?}: {e}"));
}

/// **Add a stamp to the map file**, which is how an author puts a group of props down.
fn stamp(map: &Path, id: &str, of: &str) {
    let text = std::fs::read_to_string(map).unwrap_or_else(|e| panic!("{map:?}: {e}"));
    let stamped = format!(
        "(id: \"{id}\", of: \"{of}\", at: (3.0, 3.0), yaw: 0.0, overrides: [], of_fingerprint: None, note: None)"
    );
    assert!(text.contains("stamps: []"), "the fixture's map has room for a stamp: {text}");
    std::fs::write(map, text.replacen("stamps: []", &format!("stamps: [{stamped}]"), 1))
        .unwrap_or_else(|e| panic!("{map:?}: {e}"));
}

/// A project with one piece per layer, a stamp and a note.
fn rig(tag: &str) -> App {
    let root: PathBuf = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("on_floor", "alpha")
        .descriptor("on_wall", "alpha")
        .descriptor("on_ceiling", "alpha")
        .descriptor("on_a_table", "alpha")
        .descriptor("either", "alpha")
        .descriptor("desk", "alpha")
        .descriptor("long_note", "alpha")
        .place("on_floor", (1.0, 1.0))
        .place("on_wall", (0.5, 0.5))
        .place("on_ceiling", (1.5, 1.5))
        // **The desk's row id is `desk@3`**, because `place` numbers the rows it mints; `on_a_table` rests on it.
        .place("desk", (2.0, 1.0))
        .place_on("on_a_table", "desk@3", (2.0, 1.0))
        .place("either", (3.0, 1.0))
        .build("m");
    let library = root.join("assets/grimoire").join(fixtures::DEFAULT_KIT).join("library.ron");
    mount(&library, "on_wall", "OnWall( height: 1.8 )");
    mount(&library, "on_ceiling", "OnCeiling");
    mount(&library, "on_a_table", "OnSurface( class: \"worktop\" )");
    // The desk has to *offer* the surface, or the piece on it is refused by the library's own check.
    let text = std::fs::read_to_string(&library).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    let at = text.find("id: \"desk\"").expect("the desk is in the library");
    let was = "offers: ( surfaces: [], sockets: [] )";
    let found = text[at..].find(was).expect("the desk has the fixture's empty offers") + at;
    let mut text = text;
    text.replace_range(found..found + was.len(), "offers: ( surfaces: [\"worktop\"], sockets: [] )");
    std::fs::write(&library, text).unwrap_or_else(|e| panic!("{library:?}: {e}"));
    mount(&library, "either", "OnSurfaceOrFloor( class: \"worktop\" )");
    stamp(&root.join("assets/grimoire/maps/m.map.ron"), "arch", "the harbour gate");
    // **A long note**, so its row is the one that has to be cut to a line.
    let notes = root.join("assets/grimoire/maps/m.notes.ron");
    if let Some(parent) = notes.parent() {
        std::fs::create_dir_all(parent).expect("the notes folder");
    }
    std::fs::write(
        &notes,
        format!(
            "(version: 1, notes: [(id: \"note_one\", targets: [Cell(1, 1)], text: \"{}\", state: Open)])",
            "a sentence that runs well past what one row of the tree can draw, so the row has to cut it somewhere"
        ),
    )
    .expect("a notes file");

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    show_the_outliner(&mut app, true);
    app
}

fn show_the_outliner(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerItem>>().single(world).expect("the View menu carries one Show outliner row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

fn tree(app: &mut App) -> bevy_gooey::Tree {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<OutlinerTree>>().single(world).expect("one outliner tree");
    world.get::<Table>(entity).and_then(|t| t.source::<Tree>()).cloned().expect("the outliner is a tree")
}

/// **The rows drawn under a group of the tree**, read off the outliner's own record of them.
fn under(app: &mut App, key: u64) -> Vec<u64> {
    let world = app.world_mut();
    let outliner = world.query::<&Outliner>().single(world).expect("the outliner is open");
    outliner.rows_under(key).to_vec()
}

/// **Every row under a layer**, the pieces themselves included: a layer's children are its kinds, and a kind's are
/// its pieces, so this is the walk a reader's eye makes down the tree.
fn everything_under(app: &mut App, key: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut frontier = vec![key];
    while let Some(at) = frontier.pop() {
        for row in under(app, at) {
            out.push(row);
            frontier.push(row);
        }
    }
    out
}

fn search_for(app: &mut App, words: &str) {
    let search = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<OutlinerSearch>>().single(world).expect("one search box")
    };
    app.world_mut().commands().trigger(ValueChange { source: search, value: words.to_owned(), is_final: false });
    updates(app, 3);
}

fn piece_id(app: &mut App, descriptor: &str) -> String {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.descriptor == descriptor)
        .map(|p| p.id.clone())
        .unwrap_or_else(|| panic!("no `{descriptor}` on the map"))
}

/// **Every piece stands under the layer its mount says**, and a piece with nothing else in its layer still has one
/// row there.
#[test]
fn a_piece_sits_under_the_layer_its_mount_names() {
    let mut app = rig("outliner_layers");
    let held = tree(&mut app);
    for (descriptor, layer) in [
        ("on_floor", "floor"),
        ("on_wall", "wall"),
        ("on_ceiling", "ceiling"),
        ("on_a_table", "surface"),
        // **A two-way mount takes the layer of the half it took**: this one stands on the floor.
        ("either", "floor"),
    ] {
        let id = piece_id(&mut app, descriptor);
        let layer_row = group_key(layer);
        assert!(
            held.contains(layer_row),
            "the tree has a `{layer}` group for `{descriptor}`: {descriptor} is under {:?}",
            held.label(layer_row)
        );
        let rows_here = everything_under(&mut app, layer_row);
        let seen: Vec<(u64, String)> = rows_here.iter().map(|k| (*k, held.label(*k).unwrap_or_default().to_owned())).collect();
        assert!(
            rows_here.contains(&placement_key(&id)),
            "`{descriptor}` (`{id}`, key {:x}) is under the `{layer}` group: {seen:?}",
            placement_key(&id)
        );
    }
    // **The floor group holds both floor pieces**, so the layer is a grouping and not a rename of the kind row.
    let desk = piece_id(&mut app, "desk");
    let floor: Vec<u64> = everything_under(&mut app, group_key("floor"));
    assert!(floor.contains(&placement_key(&desk)), "the desk offers a top and stands on the floor, so it is under `floor`");
}

/// **A search naming a layer finds what stands in it**, the same rule the kinds and the map's other sections follow.
#[test]
fn a_search_for_a_layer_leaves_what_stands_in_it() {
    let mut app = rig("outliner_layer_search");
    search_for(&mut app, "wall");
    let held = tree(&mut app);
    assert!(held.contains(group_key("wall")), "`wall` keeps the wall group");
    assert!(!held.contains(group_key("ceiling")), "and leaves the ceiling group, whose pieces are not on a wall");
        let wall_piece = piece_id(&mut app, "on_wall");
    assert!(held.contains(placement_key(&wall_piece)), "the sconce is what the search was for");
    let ceiling_piece = piece_id(&mut app, "on_ceiling");
    assert!(!held.contains(placement_key(&ceiling_piece)), "and the ceiling lamp is not");
}

/// **The layer is the descriptor's, and it is derived**: the same rule the editor applies, read off one function, so
/// a piece under `wall` in the tree is a piece the editor calls a wall piece.
#[test]
fn the_layer_is_the_descriptors_and_nothing_is_stored_on_the_placement() {
    let app = rig("outliner_layer_derived");
    let open = app.world().resource::<OpenMap>();
    let library = app.world().resource::<bevy_cauldron::project::Project>().library.clone();
    for placed in &open.map.placements {
        let layer = layer_of(&library, placed);
        assert!(
            ["floor", "wall", "ceiling", "surface", "opening", "decal"].contains(&layer),
            "`{}` stands in one of the six layers, not `{layer}`",
            placed.id
        );
    }
    let by_descriptor = |descriptor: &str| {
        open.map
            .placements
            .iter()
            .find(|p| p.descriptor == descriptor)
            .map(|p| layer_of(&library, p).to_owned())
            .unwrap_or_else(|| panic!("no `{descriptor}`"))
    };
    assert_eq!(by_descriptor("on_wall"), "wall");
    assert_eq!(by_descriptor("on_ceiling"), "ceiling");
    assert_eq!(by_descriptor("on_a_table"), "surface");
    assert_eq!(by_descriptor("either"), "floor", "a two-way piece with no host on the floor");
}

/// **A stamp's row says what composition it is of, and a note's says its first words, cut to one line.**
#[test]
fn a_stamp_row_and_a_note_row_say_what_they_are() {
    let mut app = rig("outliner_labels");
    let held = tree(&mut app);

    let stamp_row = thing_key(COMPOSITIONS, "arch");
    let stamp_label = held.label(stamp_row).unwrap_or_else(|| panic!("the tree has a row for the stamp"));
    assert!(
        stamp_label.contains("arch") && stamp_label.contains("the harbour gate"),
        "a stamp row names the stamp and the composition it is of: {stamp_label:?}"
    );

    // **A long note is cut**, and it ends in an ellipsis so the reader knows there is more.
    let note_row = thing_key(NOTES, "note_one");
    let note_label = held.label(note_row).unwrap_or_else(|| panic!("the tree has a row for the note"));
    assert!(note_label.starts_with("note_one"), "a note row leads with its id: {note_label:?}");
    assert!(
        note_label.chars().count() < 80,
        "the note's row is one line, so its words are cut: {} characters: {note_label:?}",
        note_label.chars().count()
    );
    assert!(note_label.contains('\u{2026}'), "and the cut says so with an ellipsis: {note_label:?}");

    // **The note's whole text is what the row cut**, so the row shows the note's own words rather than a copy.
    let notes = app.world().resource::<bevy_cauldron::notes_view::NotesView>().notes();
    let note = notes.notes.iter().find(|n| n.id == "note_one").expect("the fixture's note reached the editor");
    let first = note.text.split_whitespace().next().expect("the note has words");
    assert!(note_label.contains(first), "the row shows the note's own first words: {note_label:?} against {first:?}");
}

/// **A piece's row label names the piece and its descriptor**, and a search finds a piece by either.
#[test]
fn a_piece_row_names_the_piece() {
    let mut app = rig("outliner_piece_label");
    let held = tree(&mut app);
    let wall = piece_id(&mut app, "on_wall");
    let label = held.label(placement_key(&wall)).unwrap_or_else(|| panic!("the sconce has a row"));
    assert!(label.contains(&wall) && label.contains("on_wall"), "a piece row names its id and descriptor: {label:?}");

    search_for(&mut app, "on_ceiling");
    let held = tree(&mut app);
    let ceiling = piece_id(&mut app, "on_ceiling");
    assert_eq!(held.len(), 3, "the ceiling layer, its kind, and the lamp: {}", held.len());
    assert!(held.contains(placement_key(&ceiling)));
}

/// **How many times the tree has been built**, read off the outliner's own count.
fn rebuilds(app: &mut App) -> u32 {
    let world = app.world_mut();
    world.query::<&Outliner>().single(world).expect("the outliner is open").rebuilds()
}

fn rebuilds_now(app: &mut App) -> u32 {
    rebuilds(app)
}

/// **A write to `people/` rebuilds the tree once, and the frames after it rebuild nothing.**
///
/// The two side files are the facts no resource in the editor can show: `person_set` writes `people/<map>.ron` and
/// changes no pixel of the map, so the tree has to notice a file. It does that with two `metadata` calls per frame,
/// and the fingerprint they feed must move on the write and then hold still — a stat read twice in a row is not a
/// rebuild, or a hundred and twenty settled frames would be a hundred and twenty trees.
///
/// **Written through the path the operation owns** ([`crate::ops_people::path_of`]'s rule, mirrored here as the
/// `people/<map>.ron` under the project's own grimoire directory), which is the file the tree stats.
#[test]
fn a_write_to_the_people_file_rebuilds_once_and_then_nothing() {
    let mut app = rig("outliner_side_files");
    let built = rebuilds_now(&mut app);
    updates(&mut app, 30);
    assert_eq!(rebuilds_now(&mut app), built, "thirty settled frames rebuild nothing ({built} -> {})", rebuilds_now(&mut app));

    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let people = project.grimoire_dir.join("people").join("m.ron");
    std::fs::create_dir_all(people.parent().expect("the people folder")).expect("a people folder");
    std::fs::write(&people, "(version: 1, people: [(name: \"Mara\", at: (2.0, 2.0))])").expect("a people file");
    updates(&mut app, 4);
    assert_eq!(rebuilds_now(&mut app), built + 1, "the write is one rebuild ({built} -> {})", rebuilds_now(&mut app));
    let name = piece_id(&mut app, "on_wall");
    let _ = name;
    let held = tree(&mut app);
    assert!(
        held.node_count() > 0 && held.contains(group_key("people")),
        "the person the write added is a row of the tree"
    );

    updates(&mut app, 30);
    assert_eq!(rebuilds_now(&mut app), built + 1, "and thirty frames after it rebuild nothing ({built} -> {})", rebuilds_now(&mut app));
}
