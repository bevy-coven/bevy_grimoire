//! **A reference opens what it names** (NV-07, #384; series getting around, #377).
//!
//! Two claims, and each is measured on what a person reads or on where the editor lands:
//!
//! 1. **A piece's `Used by` names the maps that place it**, from the project's own `maps/` reader —
//!    the same one `placement_remove` asks, so a second scan cannot disagree with it.
//! 2. **A reference to a file asks the one reveal for it**, and the answer is the sentence
//!    `reveal::page_of` writes for a file: the Files workspace is GD-11 (#373), so the reference is
//!    refused **with what is missing** rather than doing nothing. That is the honest half of the
//!    issue's first acceptance item today, and the test pins the sentence rather than a destination.
//!    The cue half (Sound's `Used by`, NV-08 #385) and Cmd-click (GD-11) are not built, and nothing
//!    here pretends otherwise.

use bevy::prelude::*;

use bevy_cauldron::crosslinks;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("xlink_{tag}_{}", std::process::id()))
        .sized_descriptor("bed", "alpha", 2.0, 1.0)
        .place_as("bed@0", "bed", (2.0, 2.0))
        .side(64.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **A piece's `Used by` names the maps that place it.**
#[test]
fn a_pieces_used_by_names_the_maps_that_place_it() {
    let app = editor("used_by");
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let maps = crosslinks::used_by(&project, "bed").expect("`maps/` reads");
    assert_eq!(maps, vec!["m".to_owned()], "the open map is the one that places `bed`");
    assert_eq!(
        crosslinks::used_by_line(&project, "bed").expect("the line"),
        "placed in m",
        "the line a panel draws names the map, not a count of zero"
    );
}

/// **A piece nothing places says so**, rather than drawing an empty list: the dead-end rule.
#[test]
fn a_piece_no_map_places_says_so() {
    let app = editor("used_by_none");
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    assert_eq!(
        crosslinks::used_by_line(&project, "nothing_places_this").expect("the line"),
        "nothing places `nothing_places_this`"
    );
}

/// **A reference to a map's file is refused with the page that is missing** — GD-11 (#373) — rather than
/// doing nothing.
#[test]
fn a_file_reference_is_refused_with_the_page_that_is_missing() {
    let app = editor("open_file");
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let path = open
        .map_path
        .strip_prefix(&project.root)
        .expect("the open map is inside the project")
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(path, "assets/grimoire/maps/m.map.ron", "the file a person edits for `bed@0`");

    let shown = bevy_cauldron::ops::Shown::File { path, lines: None };
    let why = match bevy_cauldron::reveal::page_of(&shown, Some(&project)) {
        Ok(_) => panic!("the Files workspace does not exist yet, so this must have no page"),
        Err(why) => why,
    };
    assert!(
        why.contains("no page that shows one yet") && why.contains("#373"),
        "the refusal must name the missing page and its issue, and said: {why}"
    );
}
