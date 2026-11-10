//! **A round piece repeats at any turn** (#184, research lens #5).
//!
//! `repeats` and `vary` take a turn as the cheapest variation an author has, which is true of a
//! chair and false of a barrel: a round barrel turned a quarter is the same barrel. So the project
//! names, in `repetition.ron`'s `round`, the `kind` tokens of pieces that look the same at any yaw.
//! Their copies repeat whatever their yaw, and `vary` never turns one (the turn would change nothing
//! a player sees) but counts it as left, so the report does not say a repeat is gone when it is not.

use bevy_grimoire_core::descriptor::Descriptor;
use bevy_grimoire_core::library::{Library, LIBRARY_VERSION};
use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::repetition::{repeats, vary, RepetitionRules, Role};

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
        descriptors: vec![piece("barrel", "container"), piece("chair", "seat")],
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

fn round(tokens: &[&str]) -> RepetitionRules {
    RepetitionRules::parse(&format!(
        "round: [{}]",
        tokens.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", ")
    ))
    .unwrap_or_else(|e| panic!("{e}"))
}

/// Three barrels a metre apart, already turned 0, 90 and 180 degrees.
fn turned_barrels() -> Vec<Placed> {
    vec![at("b0", "barrel", 0.0, 0.0), at("b1", "barrel", 1.0, 90.0), at("b2", "barrel", 2.0, 180.0)]
}

#[test]
fn turned_round_copies_still_repeat() {
    let rows = turned_barrels();
    assert!(
        repeats(&rows, &library(), &RepetitionRules::default()).is_empty(),
        "with no round kinds, a turn is a different look"
    );
    let report = repeats(&rows, &library(), &round(&["container"]));
    assert_eq!(report.len(), 1, "{report:?}");
    let r = &report[0];
    assert_eq!((r.descriptor.as_str(), r.role), ("barrel", Role::Clutter), "{report:?}");
    assert_eq!((r.pairs, r.pieces), (3, 3), "three barrels in range of each other: {report:?}");
}

#[test]
fn a_round_piece_is_never_turned_and_counts_as_left() {
    // All at one yaw: under the default rules vary turns two of them out of the repeat.
    let same = vec![at("b0", "barrel", 0.0, 0.0), at("b1", "barrel", 1.0, 0.0), at("b2", "barrel", 2.0, 0.0)];
    assert_eq!(vary(&same, &library(), &RepetitionRules::default()).turned.len(), 2);

    // Round, a turn changes nothing a player sees: nothing is turned, and all three are left.
    let rules = round(&["container"]);
    for rows in [same, turned_barrels()] {
        let v = vary(&rows, &library(), &rules);
        assert!(v.turned.is_empty(), "a round piece is never turned: {v:?}");
        assert_eq!(v.left, 3, "every barrel still has a look-alike in range: {v:?}");
    }
}

#[test]
fn round_kinds_leave_other_pieces_turnable() {
    let rows = vec![
        at("b0", "barrel", 0.0, 0.0),
        at("b1", "barrel", 1.0, 0.0),
        at("c0", "chair", 10.0, 0.0),
        at("c1", "chair", 11.0, 0.0),
    ];
    let v = vary(&rows, &library(), &round(&["container"]));
    assert_eq!(v.turned.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![3], "only the second chair turns: {v:?}");
    assert_eq!(v.left, 2, "the two barrels are left: {v:?}");
    // A tip is still a different look, round or not.
    let mut tipped = rows[..2].to_vec();
    tipped[1].tip = (1, 0);
    assert!(repeats(&tipped, &library(), &round(&["container"])).is_empty());
}

#[test]
fn round_is_read_from_the_file_and_empty_when_left_out() {
    assert!(RepetitionRules::default().round.is_empty(), "no project's report changes by default");
    let partial = RepetitionRules::parse("radius_m: 3.0\n").unwrap_or_else(|e| panic!("{e}"));
    assert!(partial.round.is_empty());
    let rules = round(&["container", "pot"]);
    assert_eq!(rules.round, vec!["container".to_owned(), "pot".to_owned()]);
    // `RepetitionRules` has no writer of its own — a project authors this file by hand — so the test writes
    // it through the one YAML writer, which is what a person's editor would get.
    let text = bevy_grimoire_core::yaml::to_string(&rules).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(RepetitionRules::parse(&text).unwrap_or_else(|e| panic!("{e}")), rules);
}
