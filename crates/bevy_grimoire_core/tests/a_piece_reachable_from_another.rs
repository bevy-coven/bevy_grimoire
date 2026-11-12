//! **Relations, not coordinates: reachable from** (#148, research lens #2's "reachable from").
//!
//! `Kind::ReachableFrom` asks that a walker keeping `clearance` metres from every other placement's
//! centre can walk from one piece to another, around whatever stands between. `check` measures the
//! walk; a sealed one names the piece whose removal opens it, and `solve` slides that piece aside.

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

fn chest_from_gate(clearance: f32) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![Relation {
            of: "chest".into(),
            kind: Kind::ReachableFrom {
                from: "gate".into(),
                clearance,
            },
            hard: false,
        }],
    }
}

/// The chest at the origin, the gate 8 m east, and twelve crates in a ring of radius 2 round the
/// chest: 1.04 m apart, too close for a walker keeping 0.6 m from each.
fn ringed() -> Vec<Placed> {
    let mut map = vec![at("chest", 0.0, 0.0), at("gate", 8.0, 0.0)];
    for k in 0..12 {
        let t = k as f32 * std::f32::consts::TAU / 12.0;
        map.push(at(&format!("crate{k}"), 2.0 * t.cos(), 2.0 * t.sin()));
    }
    map
}

fn measured(o: &Outcome) -> f32 {
    match o {
        Outcome::Met { measured } | Outcome::Unmet { measured, .. } => *measured,
        Outcome::Missing { id } => panic!("nothing is missing here: {id}"),
    }
}

#[test]
fn an_open_walk_is_met_and_about_the_straight_distance() {
    let map = vec![at("chest", 0.0, 0.0), at("gate", 8.0, 0.0), at("well", 4.0, 5.0)];
    let o = check(&map, &chest_from_gate(0.6)).remove(0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    assert!((measured(&o) - 8.0).abs() < 0.5, "a straight walk of 8 m: {o:?}");
}

#[test]
fn a_wall_between_is_walked_round_and_the_detour_measured() {
    // A wall of barrels along x = 4, 0.5 m apart, from z = -3 to 3: the walk goes round an end.
    let mut map = vec![at("chest", 0.0, 0.0), at("gate", 8.0, 0.0)];
    for k in 0..=12 {
        map.push(at(&format!("barrel{k}"), 4.0, -3.0 + k as f32 * 0.5));
    }
    let o = check(&map, &chest_from_gate(0.4)).remove(0);
    assert!(matches!(o, Outcome::Met { .. }), "the way round the end is open: {o:?}");
    // Round an end at |z| ≥ 3.4: at least 2 · √(4² + 3.4²) ≈ 10.5 m.
    assert!(measured(&o) > 10.0, "the detour, not the straight 8 m: {o:?}");
}

#[test]
fn a_sealed_ring_is_unmet_and_names_the_crate_that_opens_it() {
    let o = check(&ringed(), &chest_from_gate(0.6)).remove(0);
    match &o {
        Outcome::Unmet { by: Some(by), measured } => {
            // The crate on the straight line to the gate, at (2, 0).
            assert_eq!(by, "crate0", "{o:?}");
            assert!(*measured > 7.0, "the walk once it is gone: {o:?}");
        }
        _ => panic!("a ring 1.04 m apart seals a 0.6 m walker: {o:?}"),
    }
    // A thinner walker slips between the crates.
    assert!(matches!(check(&ringed(), &chest_from_gate(0.4)).remove(0), Outcome::Met { .. }));
}

#[test]
fn solve_slides_the_crate_aside_and_the_chest_is_reachable_after() {
    let relations = chest_from_gate(0.6);
    let solved = solve(&ringed(), &relations);
    assert_eq!((solved.unmet_before, solved.unmet_after), (1, 0), "{:?}", solved.moved);
    assert_eq!(solved.moved, vec!["crate0".to_owned()], "only the opener moves");
    let chest = solved.placements.iter().find(|p| p.id == "chest").unwrap();
    let gate = solved.placements.iter().find(|p| p.id == "gate").unwrap();
    assert_eq!((chest.at, gate.at), ((0.0, 0.0), (8.0, 0.0)), "both ends stay");
    assert!(matches!(check(&solved.placements, &relations).remove(0), Outcome::Met { .. }));
}

#[test]
fn what_rests_on_an_end_never_walls_it_off() {
    // A lid resting on the chest stands on its centre; it is the chest's, not in the way. Counted
    // as an obstacle it would block every cell beside the chest.
    let mut map = ringed();
    map.retain(|p| !p.id.starts_with("crate"));
    let mut lid = at("lid", 0.05, 0.0);
    lid.on = Some("chest".into());
    map.push(lid.clone());
    let o = check(&map, &chest_from_gate(0.6)).remove(0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
    // The same piece standing free beside the chest seals it.
    lid.on = None;
    let free = vec![at("chest", 0.0, 0.0), at("gate", 8.0, 0.0), lid];
    let o = check(&free, &chest_from_gate(0.6)).remove(0);
    assert!(matches!(&o, Outcome::Unmet { by: Some(by), .. } if by == "lid"), "{o:?}");
}

#[test]
fn an_end_resting_on_a_host_is_not_walled_in_by_it() {
    // The chest stands on a table; the table is the chest's end, not a piece in the way.
    let mut chest = at("chest", 0.0, 0.0);
    chest.on = Some("table".into());
    let map = vec![at("table", 0.0, 0.0), chest, at("gate", 8.0, 0.0)];
    let o = check(&map, &chest_from_gate(0.6)).remove(0);
    assert!(matches!(o, Outcome::Met { .. }), "{o:?}");
}

#[test]
fn a_sealed_ring_stays_sealed_on_a_map_too_large_for_fine_cells() {
    // The gate 2 km off makes the walk's window kilometres wide, so it runs on coarse cells; a
    // coarse grid may read a ring as closed when it is open, never as open when it is sealed.
    let mut map = ringed();
    map[1] = at("gate", 2000.0, 0.0);
    let o = check(&map, &chest_from_gate(0.6)).remove(0);
    assert!(matches!(o, Outcome::Unmet { .. }), "{o:?}");
    // And a far piece elsewhere does not coarsen a near walk: the thin walker still slips through.
    let mut map = ringed();
    map.push(at("far", 2000.0, 2000.0));
    assert!(matches!(check(&map, &chest_from_gate(0.4)).remove(0), Outcome::Met { .. }));
    assert!(matches!(check(&map, &chest_from_gate(0.6)).remove(0), Outcome::Unmet { .. }));
}

#[test]
fn a_missing_end_is_missing() {
    let map = vec![at("chest", 0.0, 0.0)];
    assert_eq!(check(&map, &chest_from_gate(0.6)), vec![Outcome::Missing { id: "gate".into() }]);
}

#[test]
fn the_file_round_trips_and_a_negative_clearance_is_refused() {
    let ok = r"version: 1
relations:
- of: chest
  kind: !ReachableFrom
    from: gate
    clearance: 0.6
";
    assert_eq!(Relations::parse(ok), Ok(chest_from_gate(0.6)));
    let text = chest_from_gate(0.6).to_yaml().unwrap();
    assert_eq!(Relations::parse(&text), Ok(chest_from_gate(0.6)));
    assert!(Relations::parse(&ok.replace("0.6", "-0.6")).is_err());
}
