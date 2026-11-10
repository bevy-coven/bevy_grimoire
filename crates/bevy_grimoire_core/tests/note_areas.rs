use bevy_grimoire_core::map::{Map, Placed};
use bevy_grimoire_core::notes::{path_for_map, Notes, State, Target};

fn map_with(placed: &[(&str, f32, f32)]) -> Map {
    let mut m = Map::default();
    for (id, x, z) in placed {
        let mut p = m.placements.first().cloned().unwrap_or_else(blank);
        p.id = (*id).into();
        p.at = (*x, *z);
        m.placements.push(p);
    }
    m
}

fn blank() -> Placed {
    // **A placement is a row of a map, not a file of its own** (GD-03, #365): the map's YAML reader is where
    // its shape is checked (`tests/yaml_round_trips_every_project_type.rs`), so a fixture here is the struct.
    Placed { id: "x".into(), descriptor: "d".into(), at: (0.0, 0.0), yaw: 0.0, ..Placed::default() }
}

const CAM: (f32, f32, f32, f32, f32) = (1.0, 9.0, 2.0, 0.5, -0.8);

#[test]
fn a_4x3_area_note_is_a_draft_that_records_its_place_and_survives_a_restart() {
    let map = map_with(&[("in_a", 1.0, 1.0), ("in_b", 7.0, 5.0), ("out", 9.0, 1.0)]);
    let mut notes = Notes::default();
    // cells x 0..=3, z 0..=2 at 2 m: world 0..8 by 0..6
    let id = notes.add_area(&map, (3, 2, 0, 0), 2.0, CAM, "tidy this square").unwrap();
    let n = notes.get(&id).unwrap();
    assert_eq!(n.state, State::Draft);
    let site = n.site.as_ref().unwrap();
    assert_eq!(site.cells, (0, 0, 3, 2));
    assert_eq!(site.world, (0.0, 0.0, 8.0, 6.0));
    assert_eq!(site.pieces, ["in_a", "in_b"]);
    assert_eq!((site.margin, site.camera), (1, CAM));
    assert_eq!(site.reach(), (-1, -1, 4, 3));
    assert_eq!(n.targets, [Target::Placement("in_a".into()), Target::Placement("in_b".into())]);

    let dir = std::env::temp_dir().join(format!("grimoire_note_areas_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = path_for_map(&dir.join("t.map.yaml"));
    notes.save(&path).unwrap();
    assert_eq!(Notes::load(&path).unwrap(), notes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_area_targets_its_cells_and_two_clicked_pieces_record_theirs() {
    let map = map_with(&[("a", 1.0, 1.0), ("b", 5.0, 3.0)]);
    let mut notes = Notes::default();
    let e = notes.add_area(&map, (10, 10, 11, 10), 2.0, CAM, "empty").unwrap();
    assert_eq!(notes.get(&e).unwrap().targets, [Target::Cell(10, 10), Target::Cell(11, 10)]);
    let p = notes.add_pieces(&map, &["a".into(), "b".into()], 2.0, CAM, "move").unwrap();
    let site = notes.get(&p).unwrap().site.clone().unwrap();
    assert_eq!((site.cells, site.pieces.len()), ((0, 0, 2, 1), 2));
    assert!(notes.add_pieces(&map, &["nope".into()], 2.0, CAM, "x").is_err());
    assert!(notes.add_pieces(&map, &[], 2.0, CAM, "x").is_err());
}

#[test]
fn a_draft_stands_is_not_resolved_can_be_edited_and_removed() {
    let map = map_with(&[]);
    let mut notes = Notes::default();
    let id = notes.add_area(&map, (0, 0, 0, 0), 2.0, CAM, "one").unwrap();
    assert!(notes.get(&id).unwrap().stands() && notes.resolved().count() == 0);
    notes.edit(&id, "two").unwrap();
    assert_eq!(notes.get(&id).unwrap().text, "two");
    assert!(notes.reopen(&id).is_err());
    let gone = notes.remove(&id).unwrap();
    assert_eq!(gone.text, "two");
    assert!(notes.remove(&id).is_err() && notes.notes.is_empty());
}
