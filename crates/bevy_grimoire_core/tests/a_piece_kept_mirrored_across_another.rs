//! **Relations, not coordinates: mirror of** (#170, research lens #2).
//!
//! `Kind::MirrorOf` keeps a piece opposite another across a third's forward axis: two lamps
//! flanking a door, a pair of thrones either side of the altar's axis. It is Merrell et al.'s
//! symmetry about a focal point (*Interactive Furniture Layout Using Interior Design Guidelines*,
//! "Emphasis"): the piece should stand where the other, reflected across the axis, stands. `check`
//! measures the distance from that mirror point; `solve` sets the piece on it.
//!
//! Rows are a map's, yaw in degrees, through `check_map` and `solve_map`.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{check_map, solve_map, Kind, Outcome, Relation, Relations, RELATIONS_VERSION};

fn piece(id: &str, at: (f32, f32), yaw_deg: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id.to_owned(),
        at,
        yaw: yaw_deg,
        ..Placed::default()
    }
}

/// `lamp_2` mirrors `lamp_1` across `about`'s axis, within 0.3 m.
fn pair(about: &str) -> Relations {
    pair_within(about, 0.3)
}

fn pair_within(about: &str, within: f32) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "lamp_2".into(),
            kind: Kind::MirrorOf {
                of: "lamp_1".into(),
                about: about.into(),
                within,
            },
            hard: false,
        }],
    }
}

/// A door at the origin turned to `door_deg`, `lamp_1` at (1, 0.5), `lamp_2` at `second`.
fn rows(door_deg: f32, second: (f32, f32)) -> Vec<Placed> {
    vec![
        piece("door", (0.0, 0.0), door_deg),
        piece("lamp_1", (1.0, 0.5), 0.0),
        piece("lamp_2", second, 0.0),
    ]
}

fn outcome(door_deg: f32, second: (f32, f32)) -> Outcome {
    check_map(&rows(door_deg, second), &pair("door")).remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

fn at(ps: &[Placed], id: &str) -> (f32, f32) {
    ps.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("{id} missing")).at
}

#[test]
fn a_lamp_opposite_its_pair_across_the_doors_axis_is_met() {
    // The door faces +z, so its axis is the z axis: (1, 0.5) mirrors to (-1, 0.5).
    let o = outcome(0.0, (-1.0, 0.5));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!(measured(&o) < 1e-4, "{o:?}");
}

#[test]
fn a_lamp_off_the_mirror_point_is_unmet_by_how_far_off() {
    let o = outcome(0.0, (-1.0, 1.5));
    assert!(matches!(o, Outcome::Unmet { by: None, .. }), "{o:?}");
    assert!((measured(&o) - 1.0).abs() < 1e-4, "{o:?}");
    // On the same side as its pair is not mirrored, however close to the door.
    let o = outcome(0.0, (1.0, 0.5));
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!((measured(&o) - 2.0).abs() < 1e-4, "{o:?}");
}

#[test]
fn turning_the_door_turns_the_axis() {
    // Turned 90 degrees the door faces +x: its axis is the x axis, so (1, 0.5) mirrors to (1, -0.5).
    let o = outcome(90.0, (-1.0, 0.5));
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    let o = outcome(90.0, (1.0, -0.5));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!(measured(&o) < 1e-4, "{o:?}");
    // At 0 and 90 the axis is the same line whichever way the door turns; at 45 it is not. Facing
    // (1, 1), the axis is x = z and (1, 0.5) mirrors to (0.5, 1); turned the other way, -45, the
    // axis is x = -z and it mirrors to (-0.5, -1).
    let o = outcome(45.0, (0.5, 1.0));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!(measured(&o) < 1e-4, "{o:?}");
    let o = outcome(-45.0, (-0.5, -1.0));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!(matches!(outcome(45.0, (-0.5, -1.0)), Outcome::Unmet { .. }));
}

#[test]
fn a_missing_pair_or_axis_is_missing() {
    let ps = rows(0.0, (-1.0, 0.5));
    let o = check_map(&ps, &pair("gate")).remove(0);
    assert_eq!(o, Outcome::Missing { id: "gate".into() });
    let ps: Vec<Placed> = ps.into_iter().filter(|p| p.id != "lamp_1").collect();
    let o = check_map(&ps, &pair("door")).remove(0);
    assert_eq!(o, Outcome::Missing { id: "lamp_1".into() });
}

#[test]
fn solve_sets_the_lamp_on_the_mirror_point_and_leaves_the_others() {
    let before = rows(90.0, (-1.0, 2.0));
    let solved = solve_map(&before, &pair("door"));
    assert_eq!(solved.unmet_before, 1);
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    assert_eq!(solved.moved, vec!["lamp_2".to_owned()]);
    let (x, z) = at(&solved.placements, "lamp_2");
    assert!((x - 1.0).abs() < 1e-4 && (z + 0.5).abs() < 1e-4, "lamp_2 at ({x}, {z})");
    for id in ["door", "lamp_1"] {
        let (was, now) = (before.iter().find(|p| p.id == id).unwrap(), solved.placements.iter().find(|p| p.id == id).unwrap());
        assert_eq!(was, now, "{id} moved");
    }
}

#[test]
fn a_negative_distance_is_refused() {
    let text = format!(
        "version: {RELATIONS_VERSION}\nrelations:\n- of: lamp_2\n  kind: !MirrorOf\n    of: lamp_1\n    about: door\n    within: -1.0\n"
    );
    let err = Relations::parse(&text).expect_err("a negative within must not read");
    assert!(err.contains("-1"), "{err}");
    let ok = text.replace("-1.0", "0.25");
    let r = Relations::parse(&ok).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(r, pair_within("door", 0.25));
}
