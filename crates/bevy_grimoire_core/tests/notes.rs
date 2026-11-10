use bevy_grimoire_core::live::{FileWatch, SETTLE_SECS};
use bevy_grimoire_core::notes::{path_for_map, Notes, State, Target};
use std::path::Path;

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("grimoire_notes_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_file_sits_beside_the_map() {
    assert_eq!(path_for_map(Path::new("a/maps/town.map.yaml")), Path::new("a/maps/town.notes.yaml"));
}

#[test]
fn a_note_on_two_placements_and_a_tile_survives_a_restart() {
    let dir = scratch("roundtrip");
    let path = path_for_map(&dir.join("town.map.yaml"));
    let mut notes = Notes::default();
    let id = notes
        .add(
            vec![Target::Placement("lamp_1".into()), Target::Placement("bench_2".into()), Target::Cell(3, -4)],
            "Move these 2 m east; it's too close to the road.",
        )
        .unwrap();
    notes.save(&path).unwrap();
    let back = Notes::load(&path).unwrap();
    assert_eq!(back, notes);
    assert_eq!(back.get(&id).unwrap().targets.len(), 3);
}

#[test]
fn resolved_notes_keep_their_record_and_ids_are_not_reused() {
    let mut notes = Notes::default();
    let a = notes.add(vec![Target::Cell(0, 0)], "one").unwrap();
    let b = notes.add(vec![Target::Cell(1, 0)], "two").unwrap();
    notes.resolve(&b, vec!["placement_move".into()], "moved lamp").unwrap();
    assert_eq!(notes.open().map(|n| n.id.as_str()).collect::<Vec<_>>(), [a.as_str()]);
    assert!(matches!(&notes.get(&b).unwrap().state, State::Resolved { ops, .. } if ops == &["placement_move"]));
    assert!(notes.edit(&b, "x").is_err() && notes.resolve(&b, vec![], "").is_err());
    let c = notes.add(vec![Target::Cell(2, 0)], "three").unwrap();
    assert!(c != a && c != b);
    notes.reopen(&b).unwrap();
    assert_eq!(notes.open().count(), 3);
}

#[test]
fn a_note_needs_a_target_and_text_and_ids_are_unique_on_load() {
    let mut notes = Notes::default();
    assert!(notes.add(vec![], "x").is_err());
    assert!(notes.add(vec![Target::Cell(0, 0)], "  ").is_err());
    let mut twice = Notes::default();
    twice.add(vec![Target::Cell(0, 0)], "a").unwrap();
    let again = twice.notes[0].clone();
    twice.notes.push(again);
    let doubled = twice.to_yaml().unwrap();
    assert!(Notes::parse(&doubled).unwrap_err().contains("twice"));
}

#[test]
fn no_notes_writes_no_file_and_a_missing_file_is_empty() {
    let dir = scratch("empty");
    let path = dir.join("town.notes.yaml");
    assert_eq!(Notes::load(&path).unwrap(), Notes::default());
    Notes::default().save(&path).unwrap();
    assert!(!path.exists());
}

#[test]
fn a_change_to_the_notes_file_is_a_reload_for_a_file_watch() {
    let dir = scratch("watch");
    let path = dir.join("town.notes.yaml");
    let mut watch = FileWatch::new(&dir, &["ron"]);
    let mut notes = Notes::default();
    notes.add(vec![Target::Cell(0, 0)], "look").unwrap();
    notes.save(&path).unwrap();
    assert!(!watch.poll(10.0));
    assert!(watch.poll(10.0 + SETTLE_SECS + 1.0));
}
