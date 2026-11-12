//! **Relations, not coordinates: inside** (#183, research lens #2).
//!
//! `Kind::Inside` keeps a piece inside an area of another: a table on the rug, a chest inside the
//! room's floor, a stall inside the market square. Merrell et al. (*Interactive Furniture Layout
//! Using Interior Design Guidelines*) penalise a piece's overlap with the complement of the room
//! polygon, and ProcTHOR places each floor object inside a rectangle of the room's open floor plan;
//! both keep a piece inside an area. `Near` is a circle, so around a long hall's centre it either
//! spills out of the hall's sides or misses its ends. The area is the `width` × `depth` rectangle
//! centred on the other, in its own frame (`width` across its left and right, `depth` along its
//! forward), so turning the other turns the area. `check` measures the distance from the piece's
//! centre to the rectangle (0 inside); `solve` sets a piece outside just inside each edge it stood
//! past and keeps its heading.
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

/// `table` inside the `width` × `depth` area of `of`.
fn inside(of: &str, width: f32, depth: f32) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "table".into(),
            kind: Kind::Inside { of: of.into(), width, depth },
            hard: false,
        }],
    }
}

/// A rug at the origin turned to `rug_deg`, and the table at `at` turned to `table_deg`.
fn rows(rug_deg: f32, at: (f32, f32), table_deg: f32) -> Vec<Placed> {
    vec![piece("rug", (0.0, 0.0), rug_deg), piece("table", at, table_deg)]
}

/// The table against a 2 m wide, 4 m deep rug: x within ±1, z within ±2 when the rug is unturned.
fn outcome(rug_deg: f32, at: (f32, f32)) -> Outcome {
    check_map(&rows(rug_deg, at, 0.0), &inside("rug", 2.0, 4.0)).remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

#[test]
fn a_table_anywhere_on_the_rug_is_met_and_measures_nothing() {
    for at in [(0.0, 0.0), (0.9, 1.9), (-0.5, -1.5), (1.0, 2.0)] {
        let o = outcome(0.0, at);
        assert!(matches!(o, Outcome::Met { .. }), "{at:?}: {o:?}");
        assert_eq!(measured(&o), 0.0, "{at:?}: {o:?}");
    }
}

#[test]
fn a_table_off_the_rug_is_unmet_by_its_distance_to_the_edge() {
    // Off the long side, 2 m from the centre: inside the 2.24 m circle around the corners, still off.
    let o = outcome(0.0, (2.0, 0.0));
    assert!(matches!(o, Outcome::Unmet { by: None, .. }), "{o:?}");
    assert!((measured(&o) - 1.0).abs() < 1e-4, "{o:?}");
    // Off the end.
    let o = outcome(0.0, (0.0, 3.0));
    assert!((measured(&o) - 1.0).abs() < 1e-4, "{o:?}");
    // Off a corner: the distance to the corner.
    let o = outcome(0.0, (2.0, -3.0));
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!((measured(&o) - 2f32.sqrt()).abs() < 1e-4, "{o:?}");
}

#[test]
fn turning_the_rug_turns_its_area() {
    // Turned 90 degrees the rug's forward is +x: its 4 m depth lies along x, its 2 m width along z.
    let o = outcome(90.0, (1.9, 0.0));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    let o = outcome(90.0, (0.0, 1.9));
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    assert!((measured(&o) - 0.9).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_missing_rug_is_missing() {
    let o = check_map(&rows(0.0, (0.0, 0.0), 0.0), &inside("carpet", 2.0, 4.0)).remove(0);
    assert_eq!(o, Outcome::Missing { id: "carpet".into() });
}

#[test]
fn solve_sets_the_table_just_inside_each_edge_it_stood_past_and_keeps_its_heading() {
    // Off the corner, turned 37 degrees.
    let before = rows(0.0, (3.0, 5.0), 37.0);
    let solved = solve_map(&before, &inside("rug", 2.0, 4.0));
    assert_eq!(solved.unmet_before, 1);
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    assert_eq!(solved.moved, vec!["table".to_owned()]);
    let table = solved.placements.iter().find(|p| p.id == "table").unwrap();
    assert!(table.at.0 > 0.0 && table.at.0 <= 1.0, "inside, on its own side: {:?}", table.at);
    assert!(table.at.1 > 0.0 && table.at.1 <= 2.0, "inside, on its own side: {:?}", table.at);
    assert!((table.yaw - 37.0).abs() < 1e-3, "kept its heading: {}", table.yaw);
    assert_eq!(before[0], solved.placements[0], "the rug moved");
    assert!(matches!(check_map(&solved.placements, &inside("rug", 2.0, 4.0)).remove(0), Outcome::Met { .. }));

    // Off the end only: it keeps where it stood across the rug.
    let solved = solve_map(&rows(0.0, (0.5, -5.0), 0.0), &inside("rug", 2.0, 4.0));
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    let table = solved.placements.iter().find(|p| p.id == "table").unwrap();
    assert!((table.at.0 - 0.5).abs() < 1e-4 && table.at.1 < 0.0 && table.at.1 >= -2.0, "{:?}", table.at);

    // Under a turned rug the area turns with it.
    let solved = solve_map(&rows(90.0, (0.0, 4.0), 0.0), &inside("rug", 2.0, 4.0));
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    let table = solved.placements.iter().find(|p| p.id == "table").unwrap();
    assert!(table.at.0.abs() < 1e-4 && table.at.1 > 0.0 && table.at.1 <= 1.0, "{:?}", table.at);

    // A rug away from the origin, turned: its 2 m width lies along z around z = -3.
    let away = vec![piece("rug", (5.0, -3.0), 90.0), piece("table", (5.0, 1.0), 0.0)];
    let solved = solve_map(&away, &inside("rug", 2.0, 4.0));
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    let table = solved.placements.iter().find(|p| p.id == "table").unwrap();
    assert!((table.at.0 - 5.0).abs() < 1e-4 && table.at.1 > -3.0 && table.at.1 <= -2.0, "{:?}", table.at);
}

#[test]
fn a_negative_or_unbounded_area_is_refused() {
    let text = format!("version: {RELATIONS_VERSION}\nrelations:\n- of: table\n  kind: !Inside\n    of: rug\n    width: -1.0\n    depth: 4.0\n");
    let err = Relations::parse(&text).expect_err("a negative width must not read");
    assert!(err.contains("-1"), "{err}");
    assert!(Relations::parse(&text.replace("width: -1.0, depth: 4.0", "width: 2.0, depth: inf")).is_err());
    let ok = text.replace("-1.0", "2.0");
    let r = Relations::parse(&ok).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(r, inside("rug", 2.0, 4.0));
}
