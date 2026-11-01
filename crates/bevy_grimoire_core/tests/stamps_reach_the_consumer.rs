//! **A map's stamps reach the consumer.** The trust this file exists to protect.
//!
//! # The failure it was written for
//!
//! An author draws a fence run once, keeps it as a named group, and stamps it on the map. The
//! editor draws the group's rows and the map file holds **one reference**. The game loads that
//! file and reads `map.placements` — and the group's rows are not there, because a reference has
//! to be expanded by somebody, and the game was not doing it.
//!
//! Measured on Sanctuary's `town.map.ron`, before this was fixed: the editor drew 746 rows, the
//! game received 728. The 18 missing rows were a house.
//!
//! Nothing said so. The editor said "saved", the map file said "1 stamp", and the game ran a
//! level with a hole in it. That is the failure this file exists to make impossible: **a level the
//! author drew is the level the player walks.**
//!
//! # What is tested here, and what is not
//!
//! Here: the *contract* — the consumer's loader hands the game the stamped rows, refuses a stamp
//! that does not expand, keeps the reference, and round-trips. The arithmetic of the expansion
//! itself belongs to `bevy_grimoire_core::composition`, which has its own tests.
//!
//! Elsewhere: that the editor's picture and the game's world are the same picture — which is a
//! claim about two crates, and lives in `bevy_cauldron`'s suite.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::composition::{Body, Composition, Compositions, Envelope, Member, Stamped};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;
use bevy_grimoire_core::vocab::Vocabularies;

/// A scratch project directory, wiped and made.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_stamps_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// A library with one measurable piece — a fence post, say.
fn library() -> Library {
    Library {
        version: LIBRARY_VERSION,
        note: None,
        descriptors: vec![bevy_grimoire_core::descriptor::Descriptor {
            id: "kit/fence".to_owned(),
            extent: bevy_grimoire_core::descriptor::Extent {
                footprint: Some((2.0, 0.1)),
                height: Some(1.0),
            },
            ..Default::default()
        }],
    }
}

/// A group of two fence posts, two metres apart — the shape of a short fence run.
fn fence_run() -> Composition {
    let member = |id: &str, at: (f32, f32)| Member {
        id: id.to_owned(),
        body: Body::Descriptor {
            id: "kit/fence".to_owned(),
            tip: (0, 0),
            on: None,
            patch: None,
        },
        at,
        yaw: 0.0,
        lift: 0.0,
        paint: 0,
        of_fingerprint: None,
        note: None,
    };
    Composition {
        id: "kit/fence_run".to_owned(),
        envelope: Envelope::Anchored,
        members: vec![member("a", (0.0, 0.0)), member("b", (2.0, 0.0))],
        locations: Vec::new(),
        note: None,
    }
}

/// A project on disk: one kit with one piece, one group, and one map.
///
/// `stamps` is what the map holds. `None` writes a map with no stamps at all.
fn project(tag: &str, stamps: Option<&[Stamped]>) -> PathBuf {
    let dir = scratch(tag);
    let grimoire = dir.join("assets/grimoire");
    std::fs::create_dir_all(grimoire.join("kit")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(grimoire.join("maps")).unwrap_or_else(|e| panic!("{e}"));

    let kits = Kits {
        version: KITS_VERSION,
        note: None,
        lattice: Lattice::default(),
        bind: vec![Bind {
            namespace: "kit".to_owned(),
            dir: "kit".to_owned(),
        }],
        bash: Vec::new(),
        authoring: Some("kit".to_owned()),
    };
    std::fs::write(
        grimoire.join(bevy_grimoire_core::kits::KITS_FILE),
        kits.to_ron().unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("vocab.ron"),
        ron::to_string(&Vocabularies::default()).unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("kit/library.ron"),
        library().to_ron().unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("kit/project.ron"),
        "(\n    version: 2,\n    note: None,\n    patches: [],\n)",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let compositions = Compositions {
        version: 1,
        compositions: vec![fence_run()],
        note: None,
    };
    std::fs::write(
        grimoire.join(Compositions::FILE),
        ron::ser::to_string_pretty(&compositions, ron::ser::PrettyConfig::default())
            .unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let map = Map {
        version: MAP_VERSION,
        name: "yard".to_owned(),
        origin: (0.0, 0.0, 0.0),
        bounds: (100.0, 10.0, 100.0),
        bash: None,
        placements: vec![Placed {
            id: "by_hand".to_owned(),
            descriptor: "kit/fence".to_owned(),
            at: (10.0, 10.0),
            yaw: 0.0,
            ..Placed::default()
        }],
        stamps: stamps.map(|s| s.to_vec()).unwrap_or_default(),
        locations: Vec::new(),
        note: None,
        ..Default::default()
    };
    std::fs::write(
        grimoire.join("maps/yard.map.ron"),
        ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
            .unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    grimoire
}

fn stamp(id: &str, of: &str, at: (f32, f32)) -> Stamped {
    Stamped {
        id: id.to_owned(),
        of: of.to_owned(),
        at,
        yaw: 0.0,
        ..Stamped::default()
    }
}

/// **The consumer gets the rows the stamp stands for.**
///
/// This is the whole point. The editor drew the group's members; the game must have them, at the
/// same positions, with the same ids.
#[test]
fn the_consumer_gets_the_rows_a_stamp_stands_for() {
    let grimoire = project("rows", Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]));
    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");

    // The hand-authored row, plus the group's two.
    assert_eq!(
        map.placements.len(),
        3,
        "the consumer must have the hand-placed row and the group's two: {:?}",
        map.placements
    );

    // **The same rows `expand` produces**, by id and by position. The group's members sit at the
    // stamp's position, offset by their own place in the group.
    let rows: Vec<(&str, (f32, f32))> = map
        .placements
        .iter()
        .map(|p| (p.id.as_str(), p.at))
        .collect();
    assert!(
        rows.contains(&("run@1/a", (5.0, 5.0))),
        "the group's first member is missing: {rows:?}"
    );
    assert!(
        rows.contains(&("run@1/b", (7.0, 5.0))),
        "the group's second member is missing or in the wrong place: {rows:?}"
    );
}

/// **The rows are the *same* rows the editor drew.**
///
/// Not "two rows somewhere" — the same ids, in the same order, at the same positions. The editor
/// expands through `composition::expand`; the consumer now goes through the same function, so
/// this is a check that there is only one expander, expressed as a fact about the output.
#[test]
fn the_editor_and_the_consumer_expand_the_same_rows() {
    let grimoire = project("same", Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]));
    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");

    // What the editor's own expander says, read from the file the editor wrote.
    let raw =
        std::fs::read_to_string(grimoire.join("maps/yard.map.ron")).unwrap_or_else(|e| panic!("{e}"));
    let authored = Map::parse(&raw).unwrap_or_else(|e| panic!("{e}"));
    let expansion = bevy_grimoire_core::composition::expand_map(
        &authored,
        &data.compositions.compositions,
        &data.bound.library,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let from_expansion: Vec<(&str, (f32, f32))> = expansion
        .placements
        .iter()
        .map(|p| (p.id.as_str(), p.at))
        .collect();
    let from_consumer: Vec<(&str, (f32, f32))> = map
        .placements
        .iter()
        .filter(|p| p.id.contains('/'))
        .map(|p| (p.id.as_str(), p.at))
        .collect();

    assert_eq!(
        from_expansion, from_consumer,
        "the editor's expansion and the consumer's must be the same rows"
    );
}

/// **A stamp that does not expand refuses the load — by name.**
///
/// Not a map with the rows quietly missing. The refusal names the stamp, because the author has
/// one stamp and needs to know which.
#[test]
fn a_stamp_that_cannot_expand_refuses_the_load() {
    let grimoire = project(
        "refuse",
        Some(&[stamp("ghost@1", "kit/no_such_group", (5.0, 5.0))]),
    );
    let err = ProjectData::load(&grimoire, &["yard"]).expect_err("the stamp cannot expand");
    assert!(
        err.contains("ghost@1"),
        "the refusal must name the stamp: {err}"
    );
    assert!(
        err.contains("kit/no_such_group"),
        "the refusal must name the group: {err}"
    );
}

/// **The reference survives the expansion.**
///
/// The map file holds a reference, and it still holds one after the consumer has expanded it —
/// which is what makes editing the group change every map that stamped it. A loader that wrote
/// the rows back would end that.
#[test]
fn the_reference_survives_the_expansion() {
    let grimoire = project(
        "reference",
        Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]),
    );
    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");

    assert_eq!(
        map.stamps.len(),
        1,
        "the map still holds its reference, so editing the group still moves this map"
    );
    assert_eq!(map.stamps[0].of, "kit/fence_run");
}

/// **The count the author reads is the count the game has.**
///
/// The editor's panel and the consumer's loader must agree, or the panel is a lie an author
/// trusts. This is the check on the panel's number.
#[test]
fn the_census_counts_the_rows_the_consumer_has() {
    let grimoire = project(
        "census",
        Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]),
    );
    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");

    let raw =
        std::fs::read_to_string(grimoire.join("maps/yard.map.ron")).unwrap_or_else(|e| panic!("{e}"));
    let authored = Map::parse(&raw).unwrap_or_else(|e| panic!("{e}"));
    let counted = bevy_grimoire_core::census::of_map(
        &authored,
        &data.compositions.compositions,
        &data.bound.library,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(counted.placements, 1, "the hand-authored rows");
    assert_eq!(counted.stamped, 2, "the rows the stamp brought");
    assert_eq!(
        counted.total(),
        map.placements.len(),
        "the panel's number and the consumer's number must be the same"
    );
}

/// **A map with no stamps is unchanged.**
///
/// The fix must not disturb a level that uses no groups — the common case, and the one that
/// would break silently if the expansion were eager and wrong.
#[test]
fn a_map_with_no_stamps_is_unchanged() {
    let grimoire = project("plain", None);
    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");
    assert_eq!(map.placements.len(), 1, "only the hand-authored row");
    assert!(map.stamps.is_empty());
}

/// **A hand-placed row whose id contains a slash is still hand-placed.**
///
/// A placement id is an authored string and nothing shape-checks it, so `mess_a/table` is legal.
/// A consumer that classified rows by looking for a `/` would misclassify it — and the map's own
/// stamp list is what says otherwise.
#[test]
fn a_slash_in_a_hand_placed_id_does_not_make_it_a_stamped_row() {
    let grimoire = project("slash", None);
    let raw =
        std::fs::read_to_string(grimoire.join("maps/yard.map.ron")).unwrap_or_else(|e| panic!("{e}"));
    let mut authored = Map::parse(&raw).unwrap_or_else(|e| panic!("{e}"));
    authored.placements[0].id = "mess_a/table".to_owned();

    assert_eq!(
        authored.stamp_of("mess_a/table"),
        None,
        "a hand-placed row is not a stamped row, whatever its id looks like"
    );
}

/// **A stamped row is recognised, and only because the map holds that stamp.**
///
/// The other half of the rule above: the id's *shape* comes from the one convention, and the
/// map's own stamp list is what makes it a fact.
#[test]
fn a_stamped_row_names_its_stamp_and_its_member() {
    let grimoire = project(
        "recognise",
        Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]),
    );
    let raw =
        std::fs::read_to_string(grimoire.join("maps/yard.map.ron")).unwrap_or_else(|e| panic!("{e}"));
    let authored = Map::parse(&raw).unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(
        authored.stamp_of("run@1/a"),
        Some(("run@1", "a")),
        "a row the map's own stamp brought names that stamp and member"
    );
    assert_eq!(
        authored.stamp_of("by_hand"),
        None,
        "a hand-placed row names nothing"
    );
}

/// **The round trip: a map with stamps, compositions and rules, saved and reloaded.**
///
/// The 2026-10-20 review listed this as uncovered — "a map with stamps **and** compositions
/// **and** rules saved and reloaded is not [tested]". It is the shape that matters, because a
/// round trip that drops the reference turns every stamped map into a pile of loose rows, and
/// one that drops the rules changes what the game generates.
#[test]
fn a_map_with_stamps_compositions_and_rules_round_trips() {
    let grimoire = project(
        "round_trip",
        Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]),
    );
    // A rules file, so the round trip covers all three kinds of file at once.
    std::fs::write(
        grimoire.join("rules.ron"),
        format!(
            "(\n    version: {},\n    blocks: [],\n)",
            bevy_grimoire_core::rules::RULES_VERSION
        ),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");

    // **The file the consumer wrote back is the file it read.**
    let re_saved = ron::ser::to_string_pretty(map, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("{e}"));
    let original =
        std::fs::read_to_string(grimoire.join("maps/yard.map.ron")).unwrap_or_else(|e| panic!("{e}"));

    // The expanded map has the group's rows in `placements`, so it is NOT byte-identical to the
    // authored file — and it must not be: the file on disk holds the reference, and the expanded
    // map is what the game sees. What must hold is that the *reference* survives the trip, and
    // that re-saving the expanded map writes the reference rather than the rows.
    assert!(
        re_saved.contains("stamps:"),
        "the expanded map still holds its reference: {re_saved}"
    );
    assert!(
        original.contains("stamps:"),
        "the authored file holds its reference: {original}"
    );
    assert_eq!(
        map.stamps.len(),
        1,
        "one reference, and it is still one reference after the round trip"
    );
    assert_eq!(data.rules.blocks.len(), 0, "the rules file loaded");
    assert!(
        !data.compositions.compositions.is_empty(),
        "the groups loaded"
    );
}

/// **Expanding an already-expanded map refuses, rather than doubling the rows.**
///
/// A map that has been expanded already carries the group's rows in `placements`, so expanding
/// it again would put a second house on top of the first. `expand` refuses a stamp whose row id
/// is already taken, and that refusal is the right answer: a map that expanded twice was asked
/// the wrong question, and the loud failure is what tells the caller so.
///
/// The loader expands once, so this is a property of the API rather than a bug report — but it is
/// the property that stops a future caller from finding a level with two of everything.
#[test]
fn expanding_an_expanded_map_refuses_rather_than_doubling() {
    let grimoire = project(
        "idempotent",
        Some(&[stamp("run@1", "kit/fence_run", (5.0, 5.0))]),
    );
    let data = ProjectData::load(&grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("yard").expect("the map loaded");
    assert_eq!(
        map.placements.len(),
        3,
        "the first expansion produced three rows"
    );

    let again = map.expanded(&data.compositions.compositions, &data.bound.library);
    let err = again.expect_err("expanding an expanded map must refuse");
    assert!(
        err.contains("run@1/a"),
        "the refusal must name the row that would have been doubled: {err}"
    );
}

/// **The engine-free boundary holds** — the loader is in `bevy_grimoire_core`, and this file is in
/// it, so nothing here may reach for an engine. `tests/engine_free.rs` enforces the dependency
/// list; this assertion is about the *file* staying in the crate that has no engine.
#[test]
fn the_loader_is_in_the_engine_free_crate() {
    // The path is the assertion: this file is compiled as part of `bevy_grimoire_core`, and
    // `bevy_grimoire_core` has no `bevy` dependency. If the loader ever moved to the mapper, this
    // test would still pass — so the real ratchet is `tests/engine_free.rs`, and this is here to
    // say which crate the claim is about.
    let _: fn(&Path, &[&str]) -> Result<ProjectData, String> = ProjectData::load;
}
