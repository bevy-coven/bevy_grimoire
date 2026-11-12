//! **Relations, not coordinates: left of and right of** (#163, research lens #2).
//!
//! `Kind::LeftOf` and `Kind::RightOf` keep a piece beside another: a lamp left of the door, a
//! chair right of the desk, a guard at the throne's right hand. They are `InFrontOf` turned a
//! quarter: each holds when the piece's centre stands out along the other's own left (right), as
//! the other faces its forward, inside a 45° half-cone, no more than `within` metres out. `check`
//! measures the distance along that side; `solve` sets the piece on that axis.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{check, solve, Kind, Outcome, Relation, Relations, RELATIONS_VERSION};

fn piece(id: &str, x: f32, z: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id.to_owned(),
        at: (x, z),
        yaw: 0.0,
        ..Placed::default()
    }
}

fn lamp(kind: Kind) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "lamp".into(),
            kind,
            hard: false,
        }],
    }
}

fn left_of_door() -> Relations {
    lamp(Kind::LeftOf {
        of: "door".into(),
        within: 3.0,
    })
}

fn right_of_door() -> Relations {
    lamp(Kind::RightOf {
        of: "door".into(),
        within: 3.0,
    })
}

/// The door at the origin facing +z (yaw 0), so its own left is +x and its right is -x; the lamp
/// at (x, z).
fn outcome(relations: &Relations, x: f32, z: f32) -> Outcome {
    check(&[piece("door", 0.0, 0.0), piece("lamp", x, z)], relations).remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

#[test]
fn a_lamp_out_along_the_doors_left_is_met_by_its_distance_out() {
    let o = outcome(&left_of_door(), 2.0, 0.5);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 2.0).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_lamp_out_along_the_doors_right_is_met_by_its_distance_out() {
    let o = outcome(&right_of_door(), -2.0, -0.5);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 2.0).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_lamp_on_the_other_side_measures_negative_and_is_unmet() {
    let o = outcome(&left_of_door(), -2.0, 0.0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!(measured(&o) < 0.0, "right of the door measures negative left: {o:?}");
    let o = outcome(&right_of_door(), 2.0, 0.0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!(measured(&o) < 0.0, "left of the door measures negative right: {o:?}");
}

#[test]
fn a_lamp_too_far_out_or_in_front_or_behind_is_unmet() {
    for relations in [left_of_door(), right_of_door()] {
        let x = if matches!(relations.relations[0].kind, Kind::LeftOf { .. }) { 1.0 } else { -1.0 };
        assert!(matches!(outcome(&relations, 3.5 * x, 0.0), Outcome::Unmet { .. }), "too far");
        assert!(matches!(outcome(&relations, x, 2.0), Outcome::Unmet { .. }), "in front, outside the half-cone");
        assert!(matches!(outcome(&relations, x, -2.0), Outcome::Unmet { .. }), "behind, outside the half-cone");
    }
}

#[test]
fn the_sides_turn_with_the_doors_heading() {
    // Turned to face +x, the door's left is -z and its right is +z.
    let mut door = piece("door", 0.0, 0.0);
    door.yaw = std::f32::consts::FRAC_PI_2;
    let left = check(&[door.clone(), piece("lamp", 0.0, -2.0)], &left_of_door()).remove(0);
    assert!(matches!(left, Outcome::Met { .. }), "{left:?}");
    let right = check(&[door.clone(), piece("lamp", 0.0, 2.0)], &right_of_door()).remove(0);
    assert!(matches!(right, Outcome::Met { .. }), "{right:?}");
    let wrong = check(&[door, piece("lamp", 2.0, 0.0)], &left_of_door()).remove(0);
    assert!(matches!(wrong, Outcome::Unmet { .. }), "+x is now in front: {wrong:?}");
}

#[test]
fn a_missing_door_is_missing_not_met() {
    for relations in [left_of_door(), right_of_door()] {
        let o = check(&[piece("lamp", 1.0, 0.0)], &relations).remove(0);
        assert_eq!(o, Outcome::Missing { id: "door".into() });
    }
}

#[test]
fn solve_sets_the_lamp_on_the_named_side_and_its_rider_with_it() {
    for (relations, sign) in [(left_of_door(), 1.0), (right_of_door(), -1.0)] {
        let mut flame = piece("flame", 0.0, 2.0);
        flame.on = Some("lamp".into());
        let map = vec![piece("door", 0.0, 0.0), piece("lamp", 0.0, 2.0), flame];
        let solved = solve(&map, &relations);
        assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0));
        assert_eq!(solved.moved, vec!["lamp".to_owned()]);
        let get = |id: &str| solved.placements.iter().find(|p| p.id == id).unwrap().clone();
        assert!(get("lamp").at.0 * sign > 0.0, "on the named side: {:?}", get("lamp").at);
        assert_eq!(get("flame").at, get("lamp").at, "the rider goes with it");
        assert_eq!(get("door").at, (0.0, 0.0), "the door stays");
    }
}

#[test]
fn the_file_reads_and_writes_both_sides_and_refuses_a_negative_distance() {
    for (name, relations) in [("LeftOf", left_of_door()), ("RightOf", right_of_door())] {
        let text = format!("version: 1\nrelations:\n- of: lamp\n  kind: !{name}\n    of: door\n    within: 3.0\n");
        let parsed = Relations::parse(&text).unwrap();
        assert_eq!(parsed, relations);
        assert_eq!(Relations::parse(&parsed.to_yaml().unwrap()).unwrap(), parsed);
        let bad = text.replace("3.0", "-1.0");
        assert!(Relations::parse(&bad).is_err(), "{name}");
    }
}
