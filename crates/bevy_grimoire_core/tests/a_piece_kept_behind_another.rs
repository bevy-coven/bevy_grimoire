//! **Relations, not coordinates: behind** (#161, research lens #2).
//!
//! `Kind::Behind` keeps a piece behind another: a chest behind the counter, a guard behind the
//! throne, a well behind the house. It is `InFrontOf` turned round: it holds when the piece's
//! centre stands against the other's forward, inside a 45° half-cone, no more than `within` metres
//! along it. `check` measures the distance along the other's backward; `solve` sets the piece on
//! that axis.

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

fn chest_behind_counter() -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "chest".into(),
            kind: Kind::Behind {
                of: "counter".into(),
                within: 3.0,
            },
            hard: false,
        }],
    }
}

/// The counter at the origin facing +z (yaw 0); the chest at (x, z).
fn outcome(x: f32, z: f32) -> Outcome {
    check(&[piece("counter", 0.0, 0.0), piece("chest", x, z)], &chest_behind_counter()).remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

#[test]
fn a_chest_against_the_counters_forward_is_met_by_its_distance_behind() {
    let o = outcome(0.5, -2.0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 2.0).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_chest_in_front_of_the_counter_is_not_behind_it() {
    let o = outcome(0.0, 2.0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!(measured(&o) < 0.0, "in front measures negative behind: {o:?}");
}

#[test]
fn a_chest_too_far_behind_or_off_to_the_side_is_unmet() {
    assert!(matches!(outcome(0.0, -3.5), Outcome::Unmet { .. }), "too far");
    assert!(matches!(outcome(2.0, -1.0), Outcome::Unmet { .. }), "outside the half-cone");
}

#[test]
fn behind_follows_the_counters_heading() {
    // Turned to face -z, the counter's back is +z.
    let mut counter = piece("counter", 0.0, 0.0);
    counter.yaw = std::f32::consts::PI;
    let o = check(&[counter, piece("chest", 0.0, 2.0)], &chest_behind_counter()).remove(0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
}

#[test]
fn a_missing_counter_is_missing_not_met() {
    let o = check(&[piece("chest", 0.0, -1.0)], &chest_behind_counter()).remove(0);
    assert_eq!(o, Outcome::Missing { id: "counter".into() });
}

#[test]
fn solve_sets_the_chest_behind_the_counter_and_its_rider_with_it() {
    let mut coin = piece("coin", 0.0, 2.0);
    coin.on = Some("chest".into());
    let map = vec![piece("counter", 0.0, 0.0), piece("chest", 0.0, 2.0), coin];
    let solved = solve(&map, &chest_behind_counter());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0));
    assert_eq!(solved.moved, vec!["chest".to_owned()]);
    let get = |id: &str| solved.placements.iter().find(|p| p.id == id).unwrap().clone();
    assert!(get("chest").at.1 < 0.0, "behind is -z: {:?}", get("chest").at);
    assert_eq!(get("coin").at, get("chest").at, "the rider goes with it");
    assert_eq!(get("counter").at, (0.0, 0.0), "the counter stays");
}

#[test]
fn the_file_reads_and_writes_behind_and_refuses_a_negative_distance() {
    let text = r"version: 1
relations:
- of: chest
  kind: !Behind
    of: counter
    within: 3.0
";
    let parsed = Relations::parse(text).unwrap();
    assert_eq!(parsed, chest_behind_counter());
    assert_eq!(Relations::parse(&parsed.to_yaml().unwrap()).unwrap(), parsed);
    let bad = text.replace("3.0", "-1.0");
    assert!(Relations::parse(&bad).is_err());
}
