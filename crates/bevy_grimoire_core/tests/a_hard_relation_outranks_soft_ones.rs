//! **Hard and soft relations** (#151, research lens #2: "reporting which soft constraints it could
//! not satisfy").
//!
//! A relation marked `hard` is one the map must keep: `relations::solve` never breaks it to mend
//! soft ones, and mends it even when that costs a soft one. A file with no `hard` reads as before,
//! every relation soft, and `Solved::unsatisfied` names the relations a solve left broken.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{solve, Kind, Relation, Relations, RELATIONS_VERSION};

fn at(id: &str, x: f32, z: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id.to_owned(),
        at: (x, z),
        ..Placed::default()
    }
}

fn near(of: &str, to: &str, within: f32, hard: bool) -> Relation {
    Relation {
        of: of.to_owned(),
        kind: Kind::Near { to: to.to_owned(), within },
        hard,
    }
}

fn set(relations: Vec<Relation>) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations,
    }
}

/// The crate must stand by the anchor (hard), and would like to stay by its home (soft). It stands
/// at home, far from the anchor: the one move that mends the hard relation breaks the soft one.
#[test]
fn a_hard_relation_is_mended_even_when_that_breaks_a_soft_one() {
    let map = vec![at("crate", 0.0, 0.0), at("home", 0.0, 0.5), at("anchor", 5.0, 0.0)];
    let relations = set(vec![near("crate", "anchor", 1.0, true), near("crate", "home", 1.0, false)]);
    let solved = solve(&map, &relations);
    assert_eq!(solved.moved, vec!["crate".to_owned()], "a one-for-one trade is taken when it mends a hard relation");
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 1));
    assert_eq!(solved.unsatisfied, vec![1], "the soft relation is the one left broken");
    let c = solved.placements.iter().find(|p| p.id == "crate").unwrap();
    assert!(((c.at.0 - 5.0).powi(2) + c.at.1.powi(2)).sqrt() <= 1.0, "{:?}", c.at);
}

/// The crate must stay by the anchor (hard) and would like to be by two lamps (soft). The move that
/// mends both soft relations breaks the hard one, so it is refused, though it lowers the count.
#[test]
fn soft_relations_are_never_mended_by_breaking_a_hard_one() {
    let map = vec![at("crate", 0.0, 0.0), at("anchor", 0.0, 0.0), at("lamp_a", 6.0, 0.0), at("lamp_b", 6.0, 0.3)];
    let relations = set(vec![
        near("crate", "anchor", 1.0, true),
        near("crate", "lamp_a", 1.0, false),
        near("crate", "lamp_b", 1.0, false),
    ]);
    let solved = solve(&map, &relations);
    assert!(solved.moved.is_empty(), "{:?}", solved.placements);
    assert_eq!(solved.placements, map);
    assert_eq!((solved.unmet_before, solved.unmet_after), (2, 2));
    assert_eq!(solved.unsatisfied, vec![1, 2]);

    // The same relations, all soft: the move lowers the count, so it is taken (the old behaviour).
    let soft = set(relations.relations.iter().map(|r| Relation { hard: false, ..r.clone() }).collect());
    let solved = solve(&map, &soft);
    assert_eq!(solved.moved, vec!["crate".to_owned()]);
    assert_eq!(solved.unsatisfied, vec![0]);
}

#[test]
fn a_file_without_hard_reads_as_soft_and_writes_without_it() {
    let old = r"version: 1
relations:
- of: crate
  kind: !Near
    to: home
    within: 1.0
";
    let r = Relations::parse(old).unwrap();
    assert!(!r.relations[0].hard);
    let text = r.to_yaml().unwrap();
    assert!(!text.contains("hard"), "a soft relation is written as before: {text}");
    assert_eq!(Relations::parse(&text).unwrap(), r);

    let hard = set(vec![near("crate", "anchor", 1.0, true)]);
    let text = hard.to_yaml().unwrap();
    assert!(text.contains("hard: true"), "{text}");
    assert_eq!(Relations::parse(&text).unwrap(), hard);
}
