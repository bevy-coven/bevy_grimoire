//! **Relations, not coordinates: between** (#159, research lens #2).
//!
//! `Kind::Between` keeps a piece between two others: a table between two chairs, a rug between the
//! hearth and the bed, a guard between the gate and the hall. It holds when the piece's centre
//! projects strictly inside the segment joining the two centres and stands no more than `within`
//! metres off it. `check` measures the distance to that segment; `solve` moves the piece onto it.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{check, solve, Kind, Outcome, Relation, Relations, RELATIONS_VERSION};

fn piece(id: &str, x: f32, z: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id.trim_end_matches(|c: char| c == '_' || c.is_ascii_digit()).to_owned(),
        at: (x, z),
        yaw: 0.0,
        ..Placed::default()
    }
}

fn table_between_chairs() -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "table".into(),
            kind: Kind::Between {
                a: "chair_1".into(),
                b: "chair_2".into(),
                within: 0.5,
            },
            hard: false,
        }],
    }
}

/// Chairs at x = 0 and x = 4 on z = 0; the table at (x, z).
fn outcome(x: f32, z: f32) -> Outcome {
    check(
        &[piece("chair_1", 0.0, 0.0), piece("chair_2", 4.0, 0.0), piece("table", x, z)],
        &table_between_chairs(),
    )
    .remove(0)
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("{id} missing"),
    }
}

#[test]
fn a_table_just_off_the_line_between_the_chairs_is_met_by_its_distance_to_it() {
    let o = outcome(1.0, 0.3);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 0.3).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_table_too_far_off_the_line_is_unmet() {
    let o = outcome(2.0, -1.5);
    assert!(matches!(o, Outcome::Unmet { by: None, .. }), "{o:?}");
    assert!((measured(&o) - 1.5).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_table_beyond_either_chair_is_not_between_even_when_close() {
    // 0.2 m past chair_2 on the line: near the segment's end, but not between.
    let o = outcome(4.2, 0.0);
    assert!(matches!(o, Outcome::Unmet { .. }), "past chair_2: {o:?}");
    assert!((measured(&o) - 0.2).abs() < 1e-4, "measured to the nearer end: {o:?}");
    assert!(matches!(outcome(-0.1, 0.0), Outcome::Unmet { .. }), "before chair_1");
}

#[test]
fn chairs_on_one_spot_have_nothing_between_them() {
    let o = check(
        &[piece("chair_1", 1.0, 1.0), piece("chair_2", 1.0, 1.0), piece("table", 1.0, 1.0)],
        &table_between_chairs(),
    )
    .remove(0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
}

#[test]
fn a_missing_chair_is_missing_not_met() {
    let o = check(&[piece("chair_1", 0.0, 0.0), piece("table", 2.0, 0.0)], &table_between_chairs()).remove(0);
    assert_eq!(o, Outcome::Missing { id: "chair_2".into() });
}

#[test]
fn solve_moves_the_table_between_the_chairs_and_its_rider_with_it() {
    let mut candle = piece("candle", 6.0, 3.0);
    candle.on = Some("table".into());
    let map = vec![
        piece("chair_1", 0.0, 0.0),
        piece("chair_2", 4.0, 0.0),
        piece("table", 6.0, 3.0),
        candle,
    ];
    let solved = solve(&map, &table_between_chairs());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0));
    assert_eq!(solved.moved, vec!["table".to_owned()]);
    let get = |id: &str| solved.placements.iter().find(|p| p.id == id).unwrap().clone();
    assert!(
        matches!(check(&solved.placements, &table_between_chairs())[0], Outcome::Met { .. }),
        "met after solve"
    );
    assert_eq!(get("candle").at, get("table").at, "the rider goes with it");
    assert_eq!(get("chair_1").at, (0.0, 0.0), "the ends stay");
    assert_eq!(get("chair_2").at, (4.0, 0.0), "the ends stay");
}

#[test]
fn the_file_reads_and_writes_between_and_refuses_a_negative_distance() {
    let text = r"version: 1
relations:
- of: table
  kind: !Between
    a: chair_1
    b: chair_2
    within: 0.5
";
    let parsed = Relations::parse(text).unwrap();
    assert_eq!(parsed, table_between_chairs());
    assert_eq!(Relations::parse(&parsed.to_yaml().unwrap()).unwrap(), parsed);
    let bad = text.replace("0.5", "-1.0");
    assert!(Relations::parse(&bad).is_err());
}
