//! **Relations, not coordinates: balances** (#185, research lens #2).
//!
//! `Kind::Balances { with, about, within }` keeps a group balanced about a point: the mean of the
//! centres of `of` and every `with` placement stands within `within` metres of `about`'s centre
//! (Merrell et al., *Interactive Furniture Layout Using Interior Design Guidelines*, visual balance:
//! "place the mean of the distribution of visual weight at the center of the composition"). Every
//! piece weighs the same: relations read only `Placed`, never the library. `check` measures the
//! distance from the mean to the point; `solve` moves `of` so the mean lands on it, keeping its heading.
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

/// `stall_a` balances `stall_b` and `stall_c` about the well, within `within` metres.
fn balanced(within: f32) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "stall_a".into(),
            kind: Kind::Balances {
                with: vec!["stall_b".into(), "stall_c".into()],
                about: "well".into(),
                within,
            },
            hard: false,
        }],
    }
}

/// A well at the origin, and three stalls: `a` at `first`, `b` at (-2, 0), `c` at (0, -2).
fn rows(first: (f32, f32)) -> Vec<Placed> {
    vec![
        piece("well", (0.0, 0.0), 0.0),
        piece("stall_a", first, 40.0),
        piece("stall_b", (-2.0, 0.0), 0.0),
        piece("stall_c", (0.0, -2.0), 0.0),
    ]
}

fn outcome(first: (f32, f32)) -> Outcome {
    check_map(&rows(first), &balanced(0.3)).remove(0)
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
fn a_group_whose_mean_stands_on_the_point_is_met() {
    // The mean of (a, (-2, 0), (0, -2)) is the origin when a is (2, 2).
    let o = outcome((2.0, 2.0));
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!(measured(&o) < 1e-4, "{o:?}");
}

#[test]
fn a_lopsided_group_is_unmet_by_how_far_its_mean_is_off_the_point() {
    // All three on one side: a at (-2, -2) puts the mean at (-4/3, -4/3), 1.8856 m off.
    let o = outcome((-2.0, -2.0));
    assert!(matches!(o, Outcome::Unmet { by: None, .. }), "{o:?}");
    assert!((measured(&o) - (2.0f32 * (4.0f32 / 3.0).powi(2)).sqrt()).abs() < 1e-4, "{o:?}");
}

#[test]
fn a_missing_member_or_point_is_missing() {
    let ps = rows((2.0, 2.0));
    let o = check_map(&ps, &{
        let mut r = balanced(0.3);
        r.relations[0].kind = Kind::Balances { with: vec!["stall_b".into(), "stall_z".into()], about: "well".into(), within: 0.3 };
        r
    })
    .remove(0);
    assert_eq!(o, Outcome::Missing { id: "stall_z".into() });
    let ps: Vec<Placed> = ps.into_iter().filter(|p| p.id != "well").collect();
    let o = check_map(&ps, &balanced(0.3)).remove(0);
    assert_eq!(o, Outcome::Missing { id: "well".into() });
}

#[test]
fn solve_puts_the_mean_on_the_point_and_keeps_the_heading_and_the_others() {
    let before = rows((-2.0, -2.0));
    let solved = solve_map(&before, &balanced(0.3));
    assert_eq!(solved.unmet_before, 1);
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    assert_eq!(solved.moved, vec!["stall_a".to_owned()]);
    let (x, z) = at(&solved.placements, "stall_a");
    assert!((x - 2.0).abs() < 1e-3 && (z - 2.0).abs() < 1e-3, "stall_a at ({x}, {z})");
    let row = solved.placements.iter().find(|p| p.id == "stall_a").unwrap();
    assert_eq!(row.yaw, 40.0, "the heading is kept");
    for id in ["well", "stall_b", "stall_c"] {
        let (was, now) = (before.iter().find(|p| p.id == id).unwrap(), solved.placements.iter().find(|p| p.id == id).unwrap());
        assert_eq!(was, now, "{id} moved");
    }
}

#[test]
fn a_group_balanced_about_one_of_its_own_members_is_solved_to_the_others_mean() {
    // A well at the centre of its stalls: `about` is the piece itself, so it stands on the mean of the others.
    let relations = Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "well".into(),
            kind: Kind::Balances { with: vec!["stall_b".into(), "stall_c".into()], about: "well".into(), within: 0.3 },
            hard: false,
        }],
    };
    let solved = solve_map(&rows((2.0, 2.0)), &relations);
    assert_eq!(solved.unmet_after, 0, "{:?}", solved.unsatisfied);
    let (x, z) = at(&solved.placements, "well");
    assert!((x + 1.0).abs() < 1e-3 && (z + 1.0).abs() < 1e-3, "well at ({x}, {z})");
}

#[test]
fn an_empty_group_a_negative_distance_and_a_repeated_member_are_refused() {
    let file = |kind: &str| format!("(version: {RELATIONS_VERSION}, relations: [(of: \"stall_a\", kind: {kind})])");
    let ok = file("Balances(with: [\"stall_b\", \"stall_c\"], about: \"well\", within: 0.25)");
    let r = Relations::parse(&ok).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(r, balanced(0.25));

    let empty = Relations::parse(&file("Balances(with: [], about: \"well\", within: 0.25)")).expect_err("an empty group is Near");
    assert!(empty.contains("Near"), "{empty}");
    let negative = Relations::parse(&file("Balances(with: [\"stall_b\"], about: \"well\", within: -1.0)")).expect_err("negative");
    assert!(negative.contains("-1"), "{negative}");
    let repeated = Relations::parse(&file("Balances(with: [\"stall_b\", \"stall_b\"], about: \"well\", within: 0.25)")).expect_err("a duplicate");
    assert!(repeated.contains("stall_b"), "{repeated}");
    let itself = Relations::parse(&file("Balances(with: [\"stall_a\"], about: \"well\", within: 0.25)")).expect_err("of in with");
    assert!(itself.contains("stall_a"), "{itself}");
}
