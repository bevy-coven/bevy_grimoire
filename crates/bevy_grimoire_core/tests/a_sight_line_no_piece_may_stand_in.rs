//! **Relations, not coordinates: a sight line** (#144, research lens #2's "visible from").
//!
//! `Kind::VisibleFrom` keeps a piece in view of another: no third placement's centre may stand
//! within `clearance` metres of the line between the two. `check` names the nearest blocker and how
//! far off the line it stands; `solve` moves that blocker (or the piece it rests on) off the line.

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

fn on(mut p: Placed, host: &str) -> Placed {
    p.on = Some(host.to_owned());
    p
}

/// The sign must be seen from the gate, 10 m away along +x, with 1 m either side of the line clear.
fn sign_seen_from_gate() -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "sign".into(),
            kind: Kind::VisibleFrom {
                from: "gate".into(),
                clearance: 1.0,
            },
            hard: false,
        }],
    }
}

fn outcome(placements: &[Placed]) -> Outcome {
    check(placements, &sign_seen_from_gate()).remove(0)
}

#[test]
fn a_crate_on_the_line_breaks_it_and_is_named() {
    let map = [at("gate", 0.0, 0.0), at("sign", 10.0, 0.0), at("crate", 5.0, 0.4), at("barrel", 3.0, 0.8)];
    match outcome(&map) {
        Outcome::Unmet { measured, by } => {
            assert_eq!(by.as_deref(), Some("crate"), "the nearest to the line is named");
            assert!((measured - 0.4).abs() < 1e-4, "measures the distance off the line: {measured}");
        }
        other => panic!("a crate on the line must break the sight line: {other:?}"),
    }
}

#[test]
fn a_piece_off_the_line_or_past_its_ends_leaves_it_clear() {
    let map = [
        at("gate", 0.0, 0.0),
        at("sign", 10.0, 0.0),
        at("wide", 5.0, 1.5),
        at("behind_gate", -0.5, 0.0),
        at("behind_sign", 10.5, 0.0),
    ];
    assert_eq!(outcome(&map), Outcome::Met { measured: 1.0 });
}

#[test]
fn what_rests_on_either_end_does_not_block_it() {
    let map = [
        at("gate", 0.0, 0.0),
        at("sign", 10.0, 0.0),
        on(at("lantern", 9.6, 0.1), "sign"),
        on(at("banner", 0.4, 0.0), "gate"),
    ];
    assert_eq!(outcome(&map), Outcome::Met { measured: 1.0 });
}

#[test]
fn a_missing_viewer_is_missing_not_met() {
    let map = [at("sign", 10.0, 0.0)];
    assert_eq!(outcome(&map), Outcome::Missing { id: "gate".into() });
}

#[test]
fn a_negative_clearance_is_refused() {
    let ok = "version: 1\nrelations:\n- of: sign\n  kind: !VisibleFrom\n    from: gate\n    clearance: 1.0\n";
    assert_eq!(Relations::parse(ok), Ok(sign_seen_from_gate()));
    let err = Relations::parse(&ok.replace("1.0", "-1.0")).unwrap_err();
    assert!(err.contains("-1"), "the refusal says what is wrong: {err}");
}

#[test]
fn solve_moves_the_blocker_and_its_rider_off_the_line_and_leaves_the_ends() {
    let map = vec![
        at("gate", 0.0, 0.0),
        at("sign", 10.0, 0.0),
        at("cart", 5.0, -0.2),
        on(at("sack", 5.0, -0.2), "cart"),
    ];
    let solved = solve(&map, &sign_seen_from_gate());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0));
    assert_eq!(solved.moved, vec!["cart".to_owned()], "the host moves, carrying the sack");
    let get = |id: &str| solved.placements.iter().find(|p| p.id == id).unwrap().at;
    assert_eq!(get("gate"), (0.0, 0.0));
    assert_eq!(get("sign"), (10.0, 0.0));
    let (cart, sack) = (get("cart"), get("sack"));
    assert!(cart.1 < -1.0 && (cart.0 - 5.0).abs() < 1e-4, "off the line on its own side: {cart:?}");
    assert!((sack.0 - cart.0).abs() < 1e-4 && (sack.1 - cart.1).abs() < 1e-4, "the sack rides: {sack:?}");
    assert_eq!(check(&solved.placements, &sign_seen_from_gate()), vec![Outcome::Met { measured: 1.0 }]);
}

#[test]
fn a_rider_off_its_hosts_centre_is_carried_clear_too() {
    // The sack is the nearer blocker; sliding the cart by the cart's own offset would leave it on.
    let map = vec![
        at("gate", 0.0, 0.0),
        at("sign", 10.0, 0.0),
        at("cart", 5.0, -0.6),
        on(at("sack", 5.0, 0.3), "cart"),
    ];
    let solved = solve(&map, &sign_seen_from_gate());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0), "{:?}", solved.placements);
    assert_eq!(solved.moved, vec!["cart".to_owned()]);
}

#[test]
fn every_blocker_on_the_line_is_cleared_in_one_move() {
    // Moving one of two blockers leaves the line blocked, so a one-piece move could never be kept.
    let map = vec![
        at("gate", 0.0, 0.0),
        at("sign", 10.0, 0.0),
        at("crate", 3.0, 0.2),
        at("barrel", 6.0, -0.3),
        at("well", 5.0, 4.0),
    ];
    let solved = solve(&map, &sign_seen_from_gate());
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0), "{:?}", solved.placements);
    assert_eq!(solved.moved, vec!["crate".to_owned(), "barrel".to_owned()]);
    let well = solved.placements.iter().find(|p| p.id == "well").unwrap();
    assert_eq!(well.at, (5.0, 4.0), "a piece off the line is never moved");
}

#[test]
fn a_rider_two_deep_on_an_end_does_not_block_it() {
    let map = [
        at("gate", 0.0, 0.0),
        at("sign", 10.0, 0.0),
        on(at("shelf", 9.5, 0.0), "sign"),
        on(at("cup", 9.0, 0.1), "shelf"),
    ];
    assert_eq!(outcome(&map), Outcome::Met { measured: 1.0 });
}
