//! **Relations, not coordinates: a map says which intended relations it breaks** (#137, lens #2).
//!
//! An agent places pieces by number; what it *meant* is relational ("the chair faces the table",
//! "nothing stands in front of the door"). `relations::check` reads those relations as data beside
//! the map and names every one the placements break, with what it measured.

use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{check, Kind, Outcome, Relation, Relations, RELATIONS_VERSION};

fn at(id: &str, x: f32, z: f32, yaw_deg: f32) -> Placed {
    Placed {
        id: id.to_owned(),
        descriptor: id
            .trim_end_matches(char::is_numeric)
            .trim_end_matches('_')
            .to_owned(),
        at: (x, z),
        yaw: yaw_deg.to_radians(),
        ..Placed::default()
    }
}

fn rel(of: &str, kind: Kind) -> Relation {
    Relation {
        of: of.to_owned(),
        kind,
        hard: false,
    }
}

/// A table at the origin; chair_1 south of it facing it, chair_2 north of it facing away; a door
/// on the east wall facing west into the room, with a crate 1 m in front of it.
fn room() -> Vec<Placed> {
    vec![
        at("table", 0.0, 0.0, 0.0),
        // Forward is (sin yaw, cos yaw): yaw 0 looks toward +z, at the table from z = -1.
        at("chair_1", 0.0, -1.0, 0.0),
        // Also yaw 0, but standing at z = +1: its back is to the table.
        at("chair_2", 0.0, 1.0, 0.0),
        // Yaw -90° looks toward -x, into the room from x = 4.
        at("door", 4.0, 0.0, -90.0),
        at("crate", 3.0, 0.2, 0.0),
    ]
}

fn relations() -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: vec![
            rel("chair_1", Kind::Faces("table".into())),
            rel("chair_2", Kind::Faces("table".into())),
            rel(
                "chair_1",
                Kind::Near {
                    to: "table".into(),
                    within: 1.5,
                },
            ),
            rel(
                "chair_2",
                Kind::Apart {
                    from: "chair_1".into(),
                    at_least: 1.0,
                },
            ),
            rel(
                "door",
                Kind::ClearInFront {
                    depth: 2.0,
                    width: 1.2,
                },
            ),
            rel(
                "table",
                Kind::InFrontOf {
                    of: "door".into(),
                    within: 5.0,
                },
            ),
            rel(
                "lamp",
                Kind::Near {
                    to: "table".into(),
                    within: 1.0,
                },
            ),
        ],
    }
}

#[test]
fn a_chair_turned_away_and_a_crate_at_the_door_are_the_two_broken_relations() {
    let verdicts = check(&room(), &relations());
    assert_eq!(
        verdicts.len(),
        7,
        "one verdict per relation, in order: {verdicts:?}"
    );

    let unmet: Vec<usize> = verdicts
        .iter()
        .enumerate()
        .filter(|(_, v)| matches!(v, Outcome::Unmet { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        unmet,
        vec![1, 4],
        "chair_2 faces away and the crate blocks the door: {verdicts:?}"
    );

    // chair_2's back is to the table: 180° off.
    match &verdicts[1] {
        Outcome::Unmet { measured, .. } => assert!((measured - 180.0).abs() < 0.5, "{measured}"),
        v => panic!("{v:?}"),
    }
    // The crate is the piece in the doorway, 1 m in.
    match &verdicts[4] {
        Outcome::Unmet { measured, by } => {
            assert_eq!(by.as_deref(), Some("crate"));
            assert!((measured - 1.0).abs() < 1e-4, "{measured}");
        }
        v => panic!("{v:?}"),
    }
    // chair_1 is 1 m from the table, inside 1.5 m.
    match &verdicts[2] {
        Outcome::Met { measured } => assert!((measured - 1.0).abs() < 1e-4),
        v => panic!("{v:?}"),
    }
    // A relation naming a piece the map does not hold is unresolved, never silently met.
    assert_eq!(verdicts[6], Outcome::Missing { id: "lamp".into() });
}

#[test]
fn moving_the_crate_out_of_the_doorway_meets_every_relation_it_can() {
    let mut map = room();
    map.iter_mut().find(|p| p.id == "crate").unwrap().at = (3.0, 3.0);
    map.iter_mut().find(|p| p.id == "chair_2").unwrap().yaw = 180f32.to_radians();
    let verdicts = check(&map, &relations());
    assert!(
        verdicts[..6]
            .iter()
            .all(|v| matches!(v, Outcome::Met { .. })),
        "{verdicts:?}"
    );
}

#[test]
fn relations_round_trip_as_ron() {
    let text = relations().to_yaml().expect("serialises");
    let back = Relations::parse(&text).expect("parses");
    assert_eq!(back, relations());
    assert!(
        Relations::parse(r"version: 99
relations: []
").is_err(),
        "a future version is refused"
    );
}

#[test]
fn the_nearest_intruder_in_a_corridor_is_the_one_named() {
    // Door at x = 4 looking toward -x; corridor 2 m deep, 1.2 m wide (0.6 m each side).
    let map = vec![
        at("door", 4.0, 0.0, -90.0),
        at("barrel", 2.5, -0.5, 0.0), // 1.5 m in, 0.5 m aside: inside
        at("crate", 3.4, 0.4, 0.0),   // 0.6 m in, 0.4 m aside: inside, nearer
        at("sack", 3.5, 0.9, 0.0),    // 0.5 m in but 0.9 m aside: outside a 1.2 m corridor
    ];
    let r = Relations {
        version: RELATIONS_VERSION,
        relations: vec![rel(
            "door",
            Kind::ClearInFront {
                depth: 2.0,
                width: 1.2,
            },
        )],
    };
    match &check(&map, &r)[0] {
        Outcome::Unmet { measured, by } => {
            assert_eq!(
                by.as_deref(),
                Some("crate"),
                "the nearer intruder, not the sack beside the corridor"
            );
            assert!((measured - 0.6).abs() < 1e-4, "{measured}");
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn in_front_of_is_a_cone_and_a_missing_target_is_missing() {
    // Door at x = 4 looking toward -x.
    let map = vec![
        at("door", 4.0, 0.0, -90.0),
        at("ahead", 1.0, 1.0, 0.0), // 3 m ahead, 1 m aside: inside the cone
        at("aside", 3.0, -2.0, 0.0), // 1 m ahead, 2 m aside: outside the 45° half-cone
        at("behind", 6.0, 0.0, 0.0), // behind the door
    ];
    let r = Relations {
        version: RELATIONS_VERSION,
        relations: vec![
            rel(
                "ahead",
                Kind::InFrontOf {
                    of: "door".into(),
                    within: 5.0,
                },
            ),
            rel(
                "aside",
                Kind::InFrontOf {
                    of: "door".into(),
                    within: 5.0,
                },
            ),
            rel(
                "behind",
                Kind::InFrontOf {
                    of: "door".into(),
                    within: 5.0,
                },
            ),
            rel(
                "ahead",
                Kind::InFrontOf {
                    of: "door".into(),
                    within: 2.0,
                },
            ),
            rel(
                "ahead",
                Kind::Near {
                    to: "well".into(),
                    within: 1.0,
                },
            ),
        ],
    };
    let v = check(&map, &r);
    assert!(
        matches!(v[0], Outcome::Met { measured } if (measured - 3.0).abs() < 1e-4),
        "{v:?}"
    );
    assert!(
        matches!(v[1], Outcome::Unmet { .. }),
        "outside the cone: {v:?}"
    );
    assert!(matches!(v[2], Outcome::Unmet { .. }), "behind: {v:?}");
    assert!(
        matches!(v[3], Outcome::Unmet { measured, .. } if (measured - 3.0).abs() < 1e-4),
        "too far: {v:?}"
    );
    assert_eq!(
        v[4],
        Outcome::Missing { id: "well".into() },
        "the target, not `of`, is missing"
    );
}

#[test]
fn a_negative_or_unknown_distance_is_refused_when_read() {
    let ok =
        r"version: 1
relations:
- of: door
  kind: !ClearInFront
    depth: 2.0
    width: 1.2
";
    assert!(Relations::parse(ok).is_ok());
    let negative = ok.replace("width: 1.2", "width: -1.2");
    assert!(
        Relations::parse(&negative).is_err(),
        "a negative corridor would always read as clear"
    );
    let typo = ok.replace("width: 1.2", "width: 1.2, wdith: 3.0");
    assert!(
        Relations::parse(&typo).is_err(),
        "an unknown field is refused"
    );
}
