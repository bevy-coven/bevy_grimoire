//! **Relations, not coordinates: backs onto** (#181, research lens #2).
//!
//! `Kind::BacksOnto` keeps a piece with its back to another: a bookshelf against a wall, a throne
//! backed onto the apse, a sofa with its back to the window. It is Bukowski and Séquin's object
//! association, a bookshelf that "slides along walls without penetration or separation", as Merrell
//! et al. cite it (*Interactive Furniture Layout Using Interior Design Guidelines*). The piece stands
//! off the other's face (the plane through its centre square to its forward) by no more than
//! `within`, on either side, facing straight out from it; sideways along the other is free, so a
//! long wall backs a piece anywhere along it. `check` measures the distance off the plane; `solve`
//! turns the piece out from the plane and sets it just inside `within`.
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

/// `shelf` backs onto `of`, within `within` metres.
fn backs(of: &str, within: f32) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "shelf".into(),
            kind: Kind::BacksOnto { of: of.into(), within },
            hard: false,
        }],
    }
}

/// A wall at the origin turned to `wall_deg`, and the shelf at `at` turned to `shelf_deg`.
fn rows(wall_deg: f32, at: (f32, f32), shelf_deg: f32) -> Vec<Placed> {
    vec![piece("wall", (0.0, 0.0), wall_deg), piece("shelf", at, shelf_deg)]
}

fn outcome(wall_deg: f32, at: (f32, f32), shelf_deg: f32) -> Outcome {
    check_map(&rows(wall_deg, at, shelf_deg), &backs("wall", 0.5)).remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

#[test]
fn a_shelf_facing_out_from_the_wall_close_to_it_is_met_anywhere_along_it() {
    // The wall faces +z, so its face is the plane z = 0; the shelf faces +z, away from it.
    let o = outcome(0.0, (0.0, 0.3), 0.0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 0.3).abs() < 1e-4, "{o:?}");
    // Far along a long wall is still against it: sideways is free.
    let o = outcome(0.0, (1.8, 0.3), 0.0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    // A little turned is still backed, inside the facing tolerance; past it is not.
    assert!(matches!(outcome(0.0, (0.0, 0.3), 20.0), Outcome::Met { .. }));
    assert!(matches!(outcome(0.0, (0.0, 0.3), 60.0), Outcome::Unmet { .. }));
}

#[test]
fn a_shelf_facing_the_wall_or_far_from_it_is_unmet() {
    // Close, but facing into the wall: its front is against it, not its back.
    let o = outcome(0.0, (0.0, 0.3), 180.0);
    assert!(matches!(o, Outcome::Unmet { by: None, .. }), "{o:?}");
    // Facing away, but 3 m out in the room.
    let o = outcome(0.0, (0.0, 3.0), 0.0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!((measured(&o) - 3.0).abs() < 1e-4, "{o:?}");
    // Its centre in the wall's own plane is through the wall, not against it.
    let o = outcome(0.0, (0.0, 0.0), 0.0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
}

#[test]
fn either_face_of_the_wall_backs_a_piece_and_turning_the_wall_turns_its_face() {
    // Behind the wall, facing -z, is backed onto its other face.
    assert!(matches!(outcome(0.0, (0.0, -0.3), 180.0), Outcome::Met { .. }));
    assert!(matches!(outcome(0.0, (0.0, -0.3), 0.0), Outcome::Unmet { .. }));
    // Turned 90 degrees the wall faces +x: its face is the plane x = 0.
    let o = outcome(90.0, (0.3, 1.0), 90.0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 0.3).abs() < 1e-4, "{o:?}");
    assert!(matches!(outcome(90.0, (0.0, 0.3), 0.0), Outcome::Unmet { .. }));
}

#[test]
fn a_missing_wall_is_missing() {
    let o = check_map(&rows(0.0, (0.0, 0.3), 0.0), &backs("apse", 0.5)).remove(0);
    assert_eq!(o, Outcome::Missing { id: "apse".into() });
}

#[test]
fn solve_turns_the_shelf_out_from_the_wall_and_sets_it_close_where_it_stood_along_it() {
    // 3 m out in the room, facing the wall, 1.5 m along it.
    let before = rows(0.0, (1.5, 3.0), 180.0);
    let solved = solve_map(&before, &backs("wall", 0.5));
    assert_eq!(solved.unmet_before, 1);
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    assert_eq!(solved.moved, vec!["shelf".to_owned()]);
    let shelf = solved.placements.iter().find(|p| p.id == "shelf").unwrap();
    assert!((shelf.at.0 - 1.5).abs() < 1e-4, "kept its place along the wall: {:?}", shelf.at);
    assert!(shelf.at.1 > 0.0 && shelf.at.1 <= 0.5, "on its own side, inside within: {:?}", shelf.at);
    assert!(shelf.yaw.rem_euclid(360.0).min(360.0 - shelf.yaw.rem_euclid(360.0)) < 1e-3, "faces +z: {}", shelf.yaw);
    assert_eq!(before[0], solved.placements[0], "the wall moved");
    assert!(matches!(check_map(&solved.placements, &backs("wall", 0.5)).remove(0), Outcome::Met { .. }));

    // Behind the wall it stays behind, turned out from the wall's other face.
    let solved = solve_map(&rows(90.0, (-2.0, 0.7), 90.0), &backs("wall", 0.5));
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    let shelf = solved.placements.iter().find(|p| p.id == "shelf").unwrap();
    assert!(shelf.at.0 < 0.0 && shelf.at.0 >= -0.5 && (shelf.at.1 - 0.7).abs() < 1e-4, "{:?}", shelf.at);
}

#[test]
fn a_negative_distance_is_refused() {
    let text = format!("version: {RELATIONS_VERSION}\nrelations:\n- of: shelf\n  kind: !BacksOnto\n    of: wall\n    within: -1.0\n");
    let err = Relations::parse(&text).expect_err("a negative within must not read");
    assert!(err.contains("-1"), "{err}");
    let ok = text.replace("-1.0", "0.25");
    let r = Relations::parse(&ok).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(r, backs("wall", 0.25));
}
