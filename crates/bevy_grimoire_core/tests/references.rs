//! **Reference images as project data** (E-075, #124): a person's "like this" is stored beside the map,
//! given an id the agent can cite back, and checked against the disk.

use bevy_grimoire_core::references::{path_for_map, References};

#[test]
fn a_reference_gets_a_stable_id_the_agent_can_cite() {
    let mut r = References::default();
    let a = r.add("refs/courthouse.png", "columns, steps up to the door").unwrap();
    let b = r.add("refs/street.jpg", "narrow, shop fronts").unwrap();
    assert_eq!((a.as_str(), b.as_str()), ("r1", "r2"));
    assert_eq!(r.cite("r1").unwrap(), "reference r1 (refs/courthouse.png): columns, steps up to the door");
    // an id is never reused, even after a removal
    r.remove("r2").unwrap();
    assert_eq!(r.add("refs/market.png", "stalls").unwrap(), "r3");
    assert!(r.cite("r2").unwrap_err().contains("no reference r2"));
}

#[test]
fn the_same_picture_is_one_reference_and_paths_stay_inside_the_project() {
    let mut r = References::default();
    r.add("refs/a.png", "one").unwrap();
    let e = r.add("refs/./a.png", "again").unwrap_err();
    assert!(e.contains("r1"), "{e}");
    for bad in ["/etc/x.png", "../x.png", "refs/../../x.png", "refs/notes.txt", ""] {
        assert!(r.add(bad, "x").is_err(), "{bad} was accepted");
    }
}

#[test]
fn saved_beside_the_map_and_missing_files_are_reported() {
    let dir = std::env::temp_dir().join(format!("refs_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("refs")).unwrap();
    std::fs::write(dir.join("refs/here.png"), b"x").unwrap();
    let file = path_for_map(&dir.join("maps/town.map.yaml"));
    assert_eq!(file, dir.join("maps/town.references.yaml"));

    let mut r = References::default();
    r.add("refs/here.png", "present").unwrap();
    r.add("refs/gone.png", "deleted").unwrap();
    std::fs::create_dir_all(dir.join("maps")).unwrap();
    r.save(&file).unwrap();
    let back = References::load(&file).unwrap();
    assert_eq!(back, r);
    assert_eq!(back.missing(&dir), vec![("r2".to_owned(), "refs/gone.png".to_owned())]);
    // no references and no file writes nothing
    let none = dir.join("maps/none.references.yaml");
    References::default().save(&none).unwrap();
    assert!(!none.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
