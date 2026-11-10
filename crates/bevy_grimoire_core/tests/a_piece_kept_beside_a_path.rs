//! **Relations, not coordinates: along a path** (#146, research lens #2's "along path").
//!
//! `Kind::Along` keeps a piece beside the polyline through other placements' centres, in order: a
//! lamp along the road from the gate past the well to the hall. `check` measures the distance to the
//! nearest segment; `solve` moves the piece toward the path, on its own side, just inside `within`.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{check, solve, Kind, Outcome, Relation, Relations, RELATIONS_VERSION};

fn at(id: &str, x: f32, z: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id.to_owned(),
        at: (x, z),
        ..Placed::default()
    }
}

/// The road: gate (0,0) → well (10,0) → hall (10,10). The lamp stands within 1 m of it.
fn road() -> [Placed; 3] {
    [at("gate", 0.0, 0.0), at("well", 10.0, 0.0), at("hall", 10.0, 10.0)]
}

fn lamp_along_road() -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "lamp".into(),
            kind: Kind::Along {
                path: vec!["gate".into(), "well".into(), "hall".into()],
                within: 1.0,
            },
            hard: false,
        }],
    }
}

fn outcome(lamp: Placed) -> Outcome {
    let mut map = road().to_vec();
    map.push(lamp);
    check(&map, &lamp_along_road()).remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("nothing is missing here: {id}"),
    }
}

#[test]
fn a_lamp_off_the_road_is_unmet_and_measured_to_the_nearest_segment() {
    let o = outcome(at("lamp", 4.0, 3.0));
    assert!(matches!(o, Outcome::Unmet { .. }), "3 m off the first segment: {o:?}");
    assert!((measured(&o) - 3.0).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_lamp_beside_the_second_segment_is_met() {
    let o = outcome(at("lamp", 10.5, 6.0));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 0.5).abs() < 1e-4, "{o:?}");
}

#[test]
fn past_the_road_s_end_the_lamp_is_measured_to_the_end() {
    // Behind the gate, on the first segment's line: near the line, but 2 m from the road itself.
    let o = outcome(at("lamp", -2.0, 0.0));
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!((measured(&o) - 2.0).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_missing_waypoint_is_missing_not_met() {
    let map = [at("gate", 0.0, 0.0), at("hall", 10.0, 10.0), at("lamp", 0.0, 0.0)];
    assert_eq!(
        check(&map, &lamp_along_road()),
        vec![Outcome::Missing { id: "well".into() }]
    );
}

#[test]
fn solve_brings_the_lamp_and_its_rider_beside_the_road_on_its_own_side() {
    let mut map = road().to_vec();
    map.push(at("lamp", 4.0, 3.0));
    let mut moth = at("moth", 4.0, 3.5);
    moth.on = Some("lamp".into());
    map.push(moth);
    let solved = solve(&map, &lamp_along_road());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0));
    assert_eq!(solved.moved, vec!["lamp".to_owned()]);
    let get = |id: &str| solved.placements.iter().find(|p| p.id == id).unwrap().at;
    let lamp = get("lamp");
    assert!((lamp.0 - 4.0).abs() < 1e-4, "slid straight toward the road: {lamp:?}");
    assert!(lamp.1 > 0.0 && lamp.1 <= 1.0, "on its own side, inside 1 m: {lamp:?}");
    let moth = get("moth");
    assert!((moth.0 - lamp.0).abs() < 1e-4 && (moth.1 - lamp.1 - 0.5).abs() < 1e-4, "the rider rides: {moth:?}");
    for id in ["gate", "well", "hall"] {
        assert_eq!(get(id), road().iter().find(|p| p.id == id).unwrap().at, "{id} stays");
    }
}

#[test]
fn a_path_of_one_waypoint_or_a_negative_within_is_refused() {
    let ok = r"version: 1
relations:
- of: lamp
  kind: !Along
    path:
    - gate
    - well
    - hall
    within: 1.0
";
    assert_eq!(Relations::parse(ok), Ok(lamp_along_road()));
    let one = r"version: 1
relations:
- of: lamp
  kind: !Along
    path:
    - gate
    within: 1.0
";
    let err = Relations::parse(one).unwrap_err();
    assert!(err.contains("two"), "the refusal says a path needs two waypoints: {err}");
    let err = Relations::parse(&ok.replace("1.0", "-1.0")).unwrap_err();
    assert!(err.contains("-1"), "{err}");
}
