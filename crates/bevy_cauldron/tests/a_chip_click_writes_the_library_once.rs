//! **One chip click writes `library.ron` once, not twice** — E-049.
//!
//! # Why this file exists
//!
//! `tiles::toggle_tag` sent a library label change through `editor::set_label`, which dispatches
//! `label_set` — and `label_set`'s commit already writes the library, because that is what
//! `commit_measured` is: the one door to the file. The caller then ran `persist(project, ..)`, whose
//! `Persist::Library` arm calls `write_library`, which serializes the same `measured` again.
//!
//! **Two writes, and the second is byte-identical to the first**, which is why neither a diff of the
//! file nor any existing test could see it: the file on disk is right either way. What was wrong is
//! that the click cost two `to_ron` → `save_atomic` round trips — a full library serialized and
//! fsynced twice, plus two file-stamp changes the watcher then has to dismiss. Every other library
//! mutator in this file pays for one.
//!
//! The measure is a count, not the bytes: [`library_writes_for_test`] is incremented exactly where
//! `commit_measured` writes, so nothing here can pass for the wrong reason.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::tiles;

use crate::fixtures;
use fixtures::Fixture;

/// A project holding one piece, opened on the Meshes tab with it selected, so its `kind` chips are
/// on screen and a click reaches the real observer.
fn editor_with_the_piece_in_hand(tag: &str) -> App {
    let root = Fixture::new(tag)
        .descriptor("crate", "alpha")
        .kind_tokens(&["container", "decor"])
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, tiles::Mode::Meshes)
            .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let id = app
        .world()
        .resource::<bevy_cauldron::project::Project>()
        .library
        .descriptors
        .first()
        .map(|d| d.id.clone())
        .unwrap_or_else(|| panic!("the fixture must carry a descriptor"));
    app.world_mut()
        .resource_mut::<tiles::ImportState>()
        .selected_library_id = Some(id);
    for _ in 0..4 {
        app.update();
    }
    app
}

#[test]
fn a_chip_click_writes_the_library_once() {
    let mut app = editor_with_the_piece_in_hand("e049-chip-one-write");
    let before = tiles::library_writes_for_test();

    tiles::click_tag_chip_for_test(app.world_mut(), "kind", "decor");
    for _ in 0..4 {
        app.update();
    }

    let after = tiles::library_writes_for_test();
    // **Exactly one.** Two means the click serialized and wrote the same library twice; zero means
    // the click never reached the file at all, which would be a worse bug wearing this one's shape.
    assert_eq!(
        after - before,
        1,
        "a chip click on a library piece must write `library.ron` once, got {}",
        after - before
    );

    // And the write it did make is the change the click asked for — the count alone would also be
    // satisfied by one write of the unchanged library.
    let kinds = app
        .world()
        .resource::<bevy_cauldron::project::Project>()
        .measured
        .descriptors
        .first()
        .map(|d| d.kind.clone())
        .unwrap_or_default();
    assert!(
        kinds.iter().any(|k| k == "decor"),
        "the single write carries the clicked token, kinds are {kinds:?}"
    );
}

#[test]
fn a_refused_chip_click_writes_nothing() {
    // A token the vocabulary does not hold is refused by `label_set` before the file is touched. This
    // pins the other side of the count: the one write above has to be the edit's, not the door's.
    let mut app = editor_with_the_piece_in_hand("e049-chip-refused");
    let before = tiles::library_writes_for_test();
    // **Both resources at once, through `resource_scope`**, because `set_label` takes them mutably
    // together and two `resource_mut` calls are two borrows of the world.
    let mut refused = true;
    app.world_mut()
        .resource_scope(|world, mut import: Mut<tiles::ImportState>| {
            let mut project = world.resource_mut::<bevy_cauldron::project::Project>();
            refused = bevy_cauldron::editor::set_label(
                &mut import,
                &mut project,
                "crate",
                "kind",
                &["not-a-kind".to_owned()],
            );
        });
    assert!(!refused, "a token outside the vocabulary must be refused");
    assert_eq!(
        tiles::library_writes_for_test(),
        before,
        "a refused label write must not reach the file"
    );
}
