//! **Kit standards: a kit says which pieces break the project's dimensions** (#138, lens #4).
//!
//! Shipped modular kits conform to project-wide standards: a footprint module every piece is an
//! integer multiple of, a minimum door opening, a wall height. `standards::check` reads those as
//! data (`standards.ron`) and names every piece that breaks one, with what it measured; a piece a
//! rule covers but whose dimension was never measured is reported, never silently met.

use bevy_grimoire_core::descriptor::Descriptor;
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::standards::{Breach, Dim, Rule, STANDARDS_VERSION, Standards, Why, check};

fn piece(
    id: &str,
    kind: &[&str],
    footprint: Option<(f32, f32)>,
    height: Option<f32>,
) -> Descriptor {
    let mut d = Descriptor {
        id: id.to_owned(),
        kind: kind.iter().map(|k| (*k).to_owned()).collect(),
        ..Descriptor::default()
    };
    d.extent.footprint = footprint;
    d.extent.height = height;
    d
}

fn kit() -> Library {
    Library {
        version: 1,
        note: None,
        descriptors: vec![
            piece("wall", &["wall"], Some((2.0, 0.2)), Some(2.4)),
            // 3.1 m is not a multiple of the 2 m module.
            piece("wall_odd", &["wall"], Some((3.1, 0.2)), Some(2.4)),
            piece("door", &["door"], Some((2.0, 0.2)), Some(2.4)),
            // A door too narrow and too short for the character.
            piece("door_narrow", &["door"], Some((0.6, 0.2)), Some(1.7)),
            // A door nobody measured.
            piece("door_unmeasured", &["door"], None, None),
            // A crate is not a wall or a door, so only the every-piece rule applies to it.
            piece("crate", &["prop"], Some((0.5, 0.5)), Some(0.5)),
        ],
    }
}

fn rule(name: &str, kind: Option<&str>, dim: Dim) -> Rule {
    Rule {
        name: name.to_owned(),
        kind: kind.map(str::to_owned),
        dim,
        min: None,
        max: None,
        multiple_of: None,
    }
}

fn standards() -> Standards {
    Standards {
        version: STANDARDS_VERSION,
        rules: vec![
            Rule {
                multiple_of: Some(2.0),
                ..rule("wall module", Some("wall"), Dim::Width)
            },
            Rule {
                min: Some(0.9),
                ..rule("door passage", Some("door"), Dim::Width)
            },
            Rule {
                min: Some(2.0),
                ..rule("door headroom", Some("door"), Dim::Height)
            },
            Rule {
                max: Some(3.0),
                ..rule("storey", None, Dim::Height)
            },
        ],
    }
}

fn breaches_of<'a>(all: &'a [Breach], piece: &str) -> Vec<&'a Breach> {
    all.iter().filter(|b| b.piece == piece).collect()
}

#[test]
fn a_piece_that_meets_every_rule_is_not_reported() {
    let all = check(&kit(), &standards());
    for ok in ["wall", "door", "crate"] {
        assert!(
            breaches_of(&all, ok).is_empty(),
            "{ok} meets every standard, got {:?}",
            breaches_of(&all, ok)
        );
    }
}

#[test]
fn a_footprint_off_the_module_is_named_with_what_was_measured() {
    let all = check(&kit(), &standards());
    let odd = breaches_of(&all, "wall_odd");
    assert_eq!(odd.len(), 1, "{odd:?}");
    assert_eq!(odd[0].rule, "wall module");
    assert_eq!(odd[0].dim, Dim::Width);
    assert_eq!(odd[0].found, Some(3.1));
    assert_eq!(odd[0].why, Why::NotMultipleOf(2.0));
}

#[test]
fn a_door_below_the_passage_and_headroom_breaks_both_rules() {
    let all = check(&kit(), &standards());
    let narrow = breaches_of(&all, "door_narrow");
    let whys: Vec<(&str, &Why)> = narrow.iter().map(|b| (b.rule.as_str(), &b.why)).collect();
    assert_eq!(
        whys,
        vec![
            ("door passage", &Why::BelowMin(0.9)),
            ("door headroom", &Why::BelowMin(2.0)),
        ]
    );
}

#[test]
fn an_unmeasured_piece_a_rule_covers_is_reported_not_met() {
    let all = check(&kit(), &standards());
    let un = breaches_of(&all, "door_unmeasured");
    // passage, headroom and the every-piece storey rule all cover it, and none can be measured.
    assert_eq!(un.len(), 3, "{un:?}");
    assert!(
        un.iter()
            .all(|b| b.why == Why::Unmeasured && b.found.is_none())
    );
}

#[test]
fn a_rule_with_no_kind_covers_every_piece() {
    let mut lib = kit();
    lib.descriptors
        .push(piece("tower", &["prop"], Some((1.0, 1.0)), Some(6.0)));
    let all = check(&lib, &standards());
    let tower = breaches_of(&all, "tower");
    assert_eq!(tower.len(), 1, "{tower:?}");
    assert_eq!(tower[0].why, Why::AboveMax(3.0));
}

#[test]
fn a_dimension_measured_as_nan_is_unmeasured_not_met() {
    let mut lib = kit();
    lib.descriptors.push(piece(
        "wall_nan",
        &["wall"],
        Some((f32::NAN, 0.2)),
        Some(2.4),
    ));
    let all = check(&lib, &standards());
    let nan = breaches_of(&all, "wall_nan");
    assert_eq!(nan.len(), 1, "{nan:?}");
    assert_eq!(nan[0].why, Why::Unmeasured);
}

#[test]
fn a_multiple_within_a_centimetre_is_on_the_module() {
    let mut lib = kit();
    // Exported meshes are rarely exact: 4.004 m is a 2 m module piece.
    lib.descriptors
        .push(piece("wall_long", &["wall"], Some((4.004, 0.2)), Some(2.4)));
    lib.descriptors
        .push(piece("wall_off", &["wall"], Some((4.05, 0.2)), Some(2.4)));
    let all = check(&lib, &standards());
    assert!(breaches_of(&all, "wall_long").is_empty());
    assert_eq!(breaches_of(&all, "wall_off").len(), 1);
}

#[test]
fn the_file_round_trips_and_refuses_what_could_never_hold() {
    let s = standards();
    let text = s.to_yaml().unwrap();
    assert_eq!(Standards::parse(&text).unwrap(), s);

    let bad =
        |body: &str| Standards::parse(&format!("(version: {STANDARDS_VERSION}, rules: [{body}])"));
    // A rule with no bound says nothing.
    assert!(bad(r#"(name: "x", dim: Width)"#).is_err());
    // A module of zero or less has no multiples.
    assert!(bad(r#"(name: "x", dim: Width, multiple_of: Some(0.0))"#).is_err());
    // A module within twice the tolerance has every length on it, so it would check nothing.
    assert!(bad(r#"(name: "x", dim: Width, multiple_of: Some(0.01))"#).is_err());
    // A negative or non-finite bound is a typo, not a standard.
    assert!(bad(r#"(name: "x", dim: Height, min: Some(-1.0))"#).is_err());
    assert!(bad(r#"(name: "x", dim: Height, max: Some(inf))"#).is_err());
    // min above max can never be met.
    assert!(bad(r#"(name: "x", dim: Height, min: Some(3.0), max: Some(2.0))"#).is_err());
    // An unknown field is refused, so a misspelt bound is not silently dropped.
    assert!(bad(r#"(name: "x", dim: Height, minimum: Some(1.0))"#).is_err());
    // A version this build does not read.
    assert!(Standards::parse("(version: 99, rules: [])").is_err());
}
