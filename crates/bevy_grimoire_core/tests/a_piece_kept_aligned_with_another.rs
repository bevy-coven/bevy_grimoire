//! **Relations, not coordinates: aligned with** (#155, research lens #2).
//!
//! `Kind::AlignedWith` keeps a piece on another's heading: a row of pews facing the altar, stalls
//! along a street. `check` measures the angle between the two headings, degrees, the short way
//! round; `solve` turns the piece to the other's heading and never moves it.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{check, solve, Kind, Outcome, Relation, Relations, FACING_TOLERANCE_DEG, RELATIONS_VERSION};

fn pew(id: &str, x: f32, yaw: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: "pew".to_owned(),
        at: (x, 0.0),
        yaw,
        ..Placed::default()
    }
}

fn second_with_first() -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "pew_2".into(),
            kind: Kind::AlignedWith("pew_1".into()),
            hard: false,
        }],
    }
}

fn outcome(first_yaw: f32, second_yaw: f32) -> Outcome {
    check(&[pew("pew_1", 0.0, first_yaw), pew("pew_2", 2.0, second_yaw)], &second_with_first())
        .remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

#[test]
fn a_pew_turned_a_quarter_off_its_neighbour_is_unmet_by_ninety_degrees() {
    let o = outcome(0.0, FRAC_PI_2);
    assert!(matches!(o, Outcome::Unmet { by: None, .. }), "{o:?}");
    assert!((measured(&o) - 90.0).abs() < 1e-3, "{o:?}");
}

#[test]
fn a_pew_on_its_neighbour_s_heading_is_met_the_short_way_round() {
    // 350° and 10° are 20° apart, inside the tolerance, not 340°.
    let o = outcome(10f32.to_radians(), 350f32.to_radians());
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 20.0).abs() < 1e-3, "{o:?}");
    assert!(measured(&o) <= FACING_TOLERANCE_DEG);
}

#[test]
fn a_pew_turned_round_is_not_aligned() {
    // Facing the other way is the opposite heading: a pew turned to face the congregation.
    let o = outcome(0.0, PI);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!((measured(&o) - 180.0).abs() < 1e-3, "{o:?}");
}

#[test]
fn a_missing_neighbour_is_missing_not_met() {
    let o = check(&[pew("pew_2", 2.0, 0.0)], &second_with_first()).remove(0);
    assert_eq!(o, Outcome::Missing { id: "pew_1".into() });
}

#[test]
fn solve_turns_the_pew_in_place_and_its_rider_with_it() {
    let mut cushion = pew("cushion", 2.0, FRAC_PI_2);
    cushion.descriptor = "cushion".into();
    cushion.on = Some("pew_2".into());
    let map = vec![pew("pew_1", 0.0, 0.3), pew("pew_2", 2.0, FRAC_PI_2), cushion];
    let solved = solve(&map, &second_with_first());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0));
    assert_eq!(solved.moved, vec!["pew_2".to_owned()]);
    let get = |id: &str| solved.placements.iter().find(|p| p.id == id).unwrap().clone();
    let turned = get("pew_2");
    assert!((turned.yaw - 0.3).abs() < 1e-4, "on pew_1's heading: {}", turned.yaw);
    assert_eq!(turned.at, (2.0, 0.0), "turned, never moved");
    assert!((get("cushion").yaw - 0.3).abs() < 1e-4, "the rider turns with it");
    assert_eq!(get("pew_1").yaw, 0.3, "the reference stays");
}

#[test]
fn the_file_reads_and_writes_aligned_with() {
    let text = r#"(version: 1, relations: [(of: "pew_2", kind: AlignedWith("pew_1"))])"#;
    let parsed = Relations::parse(text).unwrap();
    assert_eq!(parsed, second_with_first());
    assert_eq!(Relations::parse(&parsed.to_yaml().unwrap()).unwrap(), parsed);
}
