//! **Per-turn measurement** (#157, research lens #7–#9): a session's turns each say whether a good
//! agent acts or asks, and every later turn re-checks the relations earlier turns left true.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{Kind, Outcome, Relation};
use bevy_grimoire_core::session::{score, Ask, Expect, Played, Session, Turn, SESSION_VERSION};

fn at(id: &str, x: f32, z: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id.to_owned(),
        at: (x, z),
        ..Placed::default()
    }
}

fn near(of: &str, to: &str, within: f32) -> Relation {
    Relation {
        of: of.to_owned(),
        kind: Kind::Near { to: to.to_owned(), within },
        hard: false,
    }
}

fn turn(request: &str, expect: Expect, keeps: Vec<Relation>) -> Turn {
    Turn { request: request.to_owned(), expect, keeps }
}

fn played(asked: bool, placements: Vec<Placed>) -> Played {
    Played { asked, placements }
}

/// Turn 0 puts a lamp by the bench; turn 1 is vague and should be asked about; turn 2 adds a well
/// and turn 3 moves the lamp away, breaking turn 0's relation.
fn session() -> Session {
    Session {
        version: SESSION_VERSION,
        turns: vec![
            turn("a lamp beside the bench", Expect::Act, vec![near("lamp", "bench", 2.0)]),
            turn("make it nicer", Expect::Ask, vec![]),
            turn("a well near the bench", Expect::Act, vec![near("well", "bench", 4.0)]),
            turn("put the lamp by the well", Expect::Act, vec![near("lamp", "well", 2.0)]),
        ],
    }
}

#[test]
fn asking_too_often_and_too_rarely_each_count() {
    let base = vec![at("bench", 0.0, 0.0), at("lamp", 1.0, 0.0)];
    let with_well = vec![at("bench", 0.0, 0.0), at("lamp", 1.0, 0.0), at("well", 0.0, 3.0)];
    let mut s = session();
    s.turns.truncate(3);
    // Acts on the vague turn 1 (under-asks), and asks on the clear turn 2 (over-asks), yet the
    // well is there by the end of turn 2 so its relation is checked against something.
    let p = vec![played(false, base.clone()), played(false, base), played(true, with_well)];
    let sc = score(&s, &p).unwrap();
    assert_eq!(sc.turns.iter().map(|t| t.ask).collect::<Vec<_>>(), vec![Ask::Right, Ask::Under, Ask::Over]);
    assert_eq!((sc.over_asked(), sc.under_asked()), (1, 1));
    assert_eq!(sc.regressions(), 0);
    assert!(!sc.passed());
}

#[test]
fn a_relation_kept_then_broken_is_named_at_the_turn_that_broke_it() {
    let t0 = vec![at("bench", 0.0, 0.0), at("lamp", 1.0, 0.0)];
    let t2 = vec![at("bench", 0.0, 0.0), at("lamp", 1.0, 0.0), at("well", 0.0, 3.0)];
    // Turn 3 moves the lamp beside the well, 3.6 m from the bench: turn 0's relation breaks.
    let t3 = vec![at("bench", 0.0, 0.0), at("lamp", 1.0, 3.5), at("well", 0.0, 3.0)];
    let p = vec![played(false, t0.clone()), played(true, t0), played(false, t2), played(false, t3)];
    let sc = score(&session(), &p).unwrap();
    assert!(sc.turns[..3].iter().all(|t| t.broken.is_empty()), "nothing broke before turn 3");
    assert_eq!(sc.turns[3].broken.len(), 1, "{:?}", sc.turns[3].broken);
    let r = &sc.turns[3].broken[0];
    assert_eq!((r.turn, r.relation), (0, 0), "turn 0's lamp-by-bench relation");
    assert!(matches!(r.now, Outcome::Unmet { .. }), "{:?}", r.now);
    assert!(matches!(sc.turns[3].own[0], Outcome::Met { .. }), "the lamp is by the well");
    assert_eq!((sc.unmet(), sc.over_asked(), sc.under_asked()), (0, 0, 0));
    assert!(!sc.passed(), "a regression fails the session");
}

#[test]
fn a_relation_never_kept_is_counted_once_and_a_broken_one_is_not_recounted() {
    let mut s = session();
    s.turns[3].keeps.clear();
    // Turn 0 never meets its relation: the lamp stands 5 m off. It is unmet at turn 0 and never a
    // regression later.
    let far = vec![at("bench", 0.0, 0.0), at("lamp", 5.0, 0.0), at("well", 0.0, 3.0)];
    let p = vec![played(false, far.clone()), played(true, far.clone()), played(false, far.clone()), played(false, far)];
    let sc = score(&s, &p).unwrap();
    assert_eq!(sc.unmet(), 1);
    assert_eq!(sc.regressions(), 0);

    // Kept at turn 0, broken at turn 2, still broken at turn 3: one regression, at turn 2.
    let near_ = vec![at("bench", 0.0, 0.0), at("lamp", 1.0, 0.0)];
    let p = vec![
        played(false, near_.clone()),
        played(true, near_),
        played(false, vec![at("bench", 0.0, 0.0), at("lamp", 5.0, 0.0), at("well", 0.0, 3.0)]),
        played(false, vec![at("bench", 0.0, 0.0), at("lamp", 6.0, 0.0), at("well", 0.0, 3.0)]),
    ];
    let sc = score(&s, &p).unwrap();
    assert_eq!(sc.regressions(), 1);
    assert_eq!(sc.turns[2].broken.len(), 1);
    assert!(sc.turns[3].broken.is_empty());
}

#[test]
fn a_played_list_that_is_not_one_per_turn_is_refused() {
    let e = score(&session(), &[played(false, vec![])]).unwrap_err();
    assert!(e.contains("1 turn(s) played for a session of 4"), "{e}");
}

#[test]
fn a_session_file_round_trips_and_a_bad_one_is_refused() {
    let s = session();
    assert_eq!(Session::parse(&s.to_yaml().unwrap()).unwrap(), s);
    let mut bad = session();
    bad.version = SESSION_VERSION + 1;
    assert!(Session::parse(&bad.to_yaml().unwrap()).unwrap_err().contains("version"));
    let mut negative = session();
    negative.turns[2].keeps[0] = near("well", "bench", -1.0);
    assert!(Session::parse(&negative.to_yaml().unwrap()).unwrap_err().contains("turn 2"));
}
