//! **The crowd preview's heat overlay data** (#107): normalised to each channel's peak, the peak
//! cell is exactly 1.0 and is the reported hotspot, no solid cell is ever hot, and the operation
//! carries the same cells to the editor.
#![cfg(all(feature = "bevy_orca", feature = "bevy_stigmergy"))]

use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, Placed};
use bevy_cauldron::crowd_preview::{Preview, Spec};
use bevy_cauldron::editor::CrowdHeat;

fn walled() -> (Map, Library) {
    let library = Library {
        version: 1,
        descriptors: vec![Descriptor {
            id: "block".to_owned(),
            extent: Extent { footprint: Some((1.0, 1.0)), height: Some(2.0) },
            ..Descriptor::default()
        }],
        ..Library::default()
    };
    let mut map = Map { bounds: (20.0, 4.0, 20.0), ..Map::default() };
    for i in -10..10 {
        let x = i as f32 + 0.5;
        if x.abs() < 1.0 {
            continue;
        }
        map.placements.push(Placed { id: format!("b{i}"), descriptor: "block".to_owned(), at: (x, 0.0), ..Placed::default() });
    }
    (map, library)
}

#[test]
fn the_peak_is_one_at_the_hotspot_and_nothing_solid_is_hot() {
    let (map, library) = walled();
    let mut p = Preview::new(&map, &library, Spec::new((-9.0, -9.0, 9.0, -6.0), (-9.0, 6.0, 9.0, 9.0), 60, 3)).unwrap();
    let report = p.run(2000);
    let heat = p.heat();
    assert!(!heat.is_empty());
    let (spot, _) = report.hotspot.unwrap();
    let hot = heat.iter().find(|(_, _, g)| *g == 1.0).expect("the congestion peak is 1.0");
    assert_eq!(hot.0, spot, "the hottest cell is the reported hotspot");
    assert!(heat.iter().all(|(_, t, g)| (0.0..=1.0).contains(t) && (0.0..=1.0).contains(g)));
    assert!(heat.iter().all(|(at, _, _)| !p.is_solid_at(*at)), "a wall cell is never drawn hot");
    assert_eq!(heat, p.heat(), "reading the overlay twice agrees");
}

#[test]
fn the_overlay_reads_back_from_an_operation_outcome() {
    let after = serde_json::json!({"cell": 0.5, "heat": [[1.0, 2.0, 0.25, 1.0], [3.0, 4.0, 1.0, 0.0], [1.0]]});
    let h = CrowdHeat::from_after(&after);
    assert_eq!(h.cell, 0.5);
    assert_eq!(h.cells, vec![[1.0, 2.0, 0.25, 1.0], [3.0, 4.0, 1.0, 0.0]], "a short row is skipped");
}
