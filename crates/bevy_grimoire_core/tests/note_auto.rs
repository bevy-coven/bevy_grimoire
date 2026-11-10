//! **"Decide these yourself"** (E-070): an auto note's holder may not ask about open choices, the
//! fence still refuses what needs the person, and the flag is saved with the note.

use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::notes::{Change, Notes};

fn claimed(auto: bool) -> (Notes, String) {
    let mut n = Notes::default();
    let id = n.add_area(&Map::default(), (4, 4, 5, 5), 1.0, (0.0, 0.0, 0.0, 0.0, 0.0), "tidy").unwrap();
    n.set_auto(&id, auto).unwrap();
    n.send(&id).unwrap();
    n.claim(&id, "omp", 1000).unwrap();
    (n, id)
}

#[test]
fn an_auto_note_cannot_ask_but_a_plain_one_can() {
    let (mut plain, id) = claimed(false);
    plain.ask(&id, "omp", "how tall is the wall?", 1001).unwrap();
    let (mut auto, id) = claimed(true);
    let e = auto.ask(&id, "omp", "how tall is the wall?", 1001).unwrap_err();
    assert!(e.contains("auto") && e.contains("assumed"), "{e}");
}

#[test]
fn auto_does_not_open_the_fence() {
    let (n, id) = claimed(true);
    let c = Change { person_placed: vec!["p7".into()], cells: Some((4, 4, 5, 5)), pieces: 1, ..Change::default() };
    let e = n.fence(&id, "omp", &c).unwrap_err();
    assert!(e.contains("p7") && e.contains("auto") && e.contains("nothing was changed"), "{e}");
}

#[test]
fn the_flag_survives_a_save_and_an_old_file_without_it_reads_false() {
    let (n, id) = claimed(true);
    let text = bevy_grimoire_core::yaml::to_string(&n).unwrap();
    let back = Notes::parse(&text).unwrap();
    assert!(back.get(&id).unwrap().auto);
    let (plain, id) = claimed(false);
    let text = bevy_grimoire_core::yaml::to_string(&plain).unwrap();
    assert!(!text.contains("auto"), "a false flag is not written: {text}");
    assert!(!Notes::parse(&text).unwrap().get(&id).unwrap().auto);
}
