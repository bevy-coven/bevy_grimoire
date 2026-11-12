//! **Repeated clutter is the repeat a player notices; repeated structure is meant to repeat** (#136).
//!
//! A count of copies cannot tell the two apart: a town holds hundreds of identical fence pieces on
//! purpose and a handful of barrels that should not look stamped. `repetition::repeats` counts
//! *visible* repeats (same piece, same yaw, same tip, close together) and weighs each by the piece's
//! role, read off its `kind` tokens.

use bevy_grimoire_core::descriptor::Descriptor;
use bevy_grimoire_core::library::{Library, LIBRARY_VERSION};
use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::repetition::{repeats, Role, RepetitionRules};

fn piece(id: &str, kind: &str) -> Descriptor {
    Descriptor {
        id: id.to_owned(),
        kind: vec![kind.to_owned()],
        ..Descriptor::default()
    }
}

fn library() -> Library {
    Library {
        version: LIBRARY_VERSION,
        note: None,
        descriptors: vec![piece("barrel", "container"), piece("fence", "fence")],
    }
}

fn at(id: &str, descriptor: &str, x: f32, yaw: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: descriptor.to_owned(),
        at: (x, 0.0),
        yaw,
        ..Placed::default()
    }
}

/// Three barrels in a row and a run of ten fence pieces, all at one yaw.
fn yard() -> Vec<Placed> {
    let mut rows: Vec<Placed> = (0..3)
        .map(|i| at(&format!("barrel_{i}"), "barrel", i as f32, 0.0))
        .collect();
    rows.extend((0..10).map(|i| at(&format!("fence_{i}"), "fence", 40.0 + i as f32 * 0.5, 0.0)));
    rows
}

#[test]
fn three_barrels_outrank_ten_fence_pieces() {
    let report = repeats(&yard(), &library(), &RepetitionRules::default());
    let first = &report[0];
    assert_eq!(first.descriptor, "barrel", "clutter must rank first: {report:?}");
    assert_eq!(first.role, Role::Clutter);
    assert_eq!(first.pairs, 3, "three barrels within the radius are three pairs");
    let fence = report.iter().find(|r| r.descriptor == "fence").expect("the fence repeats");
    assert_eq!(fence.role, Role::Structure);
    assert_eq!(fence.pairs, 45, "ten fence pieces within the radius are 45 pairs");
    assert!(first.score > fence.score, "{report:?}");
}

#[test]
fn a_turned_barrel_is_not_a_repeat() {
    let mut rows = yard();
    rows[1].yaw = 90.0;
    let report = repeats(&rows, &library(), &RepetitionRules::default());
    let barrel = report.iter().find(|r| r.descriptor == "barrel").expect("two barrels still match");
    assert_eq!(barrel.pairs, 1, "only barrel_0 and barrel_2 still look alike: {report:?}");

    // A full turn is the same look.
    rows[1].yaw = 360.0;
    let report = repeats(&rows, &library(), &RepetitionRules::default());
    assert_eq!(report.iter().find(|r| r.descriptor == "barrel").unwrap().pairs, 3);
}

#[test]
fn a_tipped_barrel_is_not_a_repeat() {
    let mut rows = yard();
    rows[2].tip = (1, 0);
    let report = repeats(&rows, &library(), &RepetitionRules::default());
    assert_eq!(report.iter().find(|r| r.descriptor == "barrel").unwrap().pairs, 1);
}

#[test]
fn a_far_barrel_is_not_a_repeat() {
    let rows = vec![at("a", "barrel", 0.0, 0.0), at("b", "barrel", 20.0, 0.0)];
    let report = repeats(&rows, &library(), &RepetitionRules::default());
    assert!(report.is_empty(), "20 m apart is out of sight of each other: {report:?}");
}

#[test]
fn the_rules_are_data() {
    // An author who wants fences judged like clutter says so, and the order follows.
    let rules = RepetitionRules {
        structure_weight: 1.0,
        ..RepetitionRules::default()
    };
    let report = repeats(&yard(), &library(), &rules);
    assert_eq!(report[0].descriptor, "fence", "{report:?}");

    // **The file's own writer and its own reader** (GD-03, #365): `repetition.yaml` is read as YAML now, so
    // what an author's file holds is what these two produce.
    let text = bevy_grimoire_core::yaml::to_string(&RepetitionRules::default()).unwrap();
    let back = RepetitionRules::parse(&text).unwrap();
    assert_eq!(back, RepetitionRules::default());
    let partial = RepetitionRules::parse("radius_m: 2.0\n").unwrap();
    assert_eq!(partial.radius_m, 2.0);
    assert_eq!(partial.structure, RepetitionRules::default().structure);
}
