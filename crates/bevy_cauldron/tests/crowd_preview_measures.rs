//! **The crowd preview is a measurement** (#107): walkers route round solid pieces, never enter a
//! solid cell, queue at a narrow gap more than at a wide one, and a seed repeats exactly.
#![cfg(all(feature = "bevy_orca", feature = "bevy_stigmergy"))]

use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, Placed};
use bevy_cauldron::crowd_preview::{Preview, Report, Spec};

/// A 20 x 20 m map with a wall of 1 m blocks across z = 0 from x = -10 to 10, leaving a gap
/// `gap` blocks wide in the middle.
fn walled(gap: i32) -> (Map, Library) {
    let library = Library {
        version: 1,
        descriptors: vec![
            Descriptor {
                id: "block".to_owned(),
                extent: Extent { footprint: Some((1.0, 1.0)), height: Some(2.0) },
                ..Descriptor::default()
            },
            Descriptor {
                id: "mat".to_owned(),
                extent: Extent { footprint: Some((1.0, 1.0)), height: Some(0.05) },
                ..Descriptor::default()
            },
        ],
        ..Library::default()
    };
    let mut map = Map { bounds: (20.0, 4.0, 20.0), ..Map::default() };
    let half = gap as f32 / 2.0;
    for i in -10..10 {
        let x = i as f32 + 0.5;
        if x.abs() < half {
            continue;
        }
        map.placements.push(Placed { id: format!("b{i}"), descriptor: "block".to_owned(), at: (x, 0.0), ..Placed::default() });
    }
    // A flat mat in the walkers' way: a low piece is not an obstacle.
    map.placements.push(Placed { id: "mat".to_owned(), descriptor: "mat".to_owned(), at: (0.0, -4.0), ..Placed::default() });
    (map, library)
}

fn spec(seed: u64) -> Spec {
    Spec::new((-9.0, -9.0, 9.0, -6.0), (-9.0, 6.0, 9.0, 9.0), 60, seed)
}

fn run(gap: i32, seed: u64) -> Report {
    let (map, library) = walled(gap);
    Preview::new(&map, &library, spec(seed)).unwrap().run(2000)
}

#[test]
fn walkers_never_stand_in_a_solid_cell_and_all_get_through_a_gap() {
    let (map, library) = walled(4);
    let mut p = Preview::new(&map, &library, spec(7)).unwrap();
    while p.step() {
        assert!(p.report().steps < 2000, "the crowd never arrived");
        for w in p.walkers() {
            assert!(!p.is_solid_at(w.pos), "walker {} stands in a solid cell at {:?}", w.key, w.pos);
        }
    }
    assert_eq!(p.report().arrived, 60);
}

#[test]
fn a_narrow_gap_congests_more_than_a_wide_one() {
    let (narrow, wide) = (run(2, 3), run(10, 3));
    assert_eq!((narrow.arrived, wide.arrived), (60, 60), "both runs finish: {narrow:?} {wide:?}");
    assert!(
        narrow.congestion_total > wide.congestion_total * 1.5,
        "narrow {} should clearly exceed wide {}",
        narrow.congestion_total,
        wide.congestion_total
    );
    let (spot, _) = narrow.hotspot.expect("a narrow gap has a hotspot");
    assert!(spot.y.abs() < 6.0, "the hotspot is near the wall, not at the start or goal: {spot:?}");
}

#[test]
fn a_seed_repeats_and_a_different_seed_differs() {
    assert_eq!(run(2, 5), run(2, 5));
    assert_ne!(run(2, 5), run(2, 6));
}

#[test]
fn a_crowd_that_cannot_fit_or_cannot_reach_says_so() {
    let (map, library) = walled(4);
    let mut s = spec(1);
    s.walkers = 100_000;
    assert!(Preview::new(&map, &library, s).err().unwrap().contains("walkers"));
    let mut s = spec(1);
    s.to = (-1.0, -0.4, 1.0, 0.4);
    // The goal sits inside the wall's gap row only partly; a goal wholly inside blocks is refused.
    let (map, library) = walled(0);
    assert!(Preview::new(&map, &library, s).err().unwrap().contains("destination"));
}
