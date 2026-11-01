//! **A damaged bake or marks file is named down to the field** (#200, H3).
//!
//! `tests/reload_verdicts.rs` has the three bad files of every kind at the top of their root struct.
//! A bake is nested (chunks of points and faces), where a hand edit lands: the refusal says which
//! chunk and which list, and a file that reads is valid whatever it was baked from (a bake made from
//! another mesh is stale, which the editor handles by baking again, not damaged).

use bevy_grimoire_core::destructible::{Bake, DESTRUCTIBLE_VERSION, Marks};
use bevy_grimoire_core::reload::kind_by_id;
use bevy_grimoire_core::reload_check::{Check, check};

fn refusal(id: &str, file: &str, text: &str) -> bevy_grimoire_core::reload_check::Refusal {
    match check(kind_by_id(id).unwrap(), file, text) {
        Check::Refused(r) => r,
        other => panic!("`{id}`: expected a refusal of {text} but got {other:?}"),
    }
}

#[test]
fn a_damaged_chunk_is_named_by_its_index_and_list() {
    let file = "destructible/site__cart.bake.ron";
    let one = "(points: [(0.0, 0.0, 0.0)], faces: [[0, 0, 0]])";
    let r = refusal("destructible_bake", file, &format!("(version: 1, key: 5, chunks: [{one}, (points: 7, faces: [])])"));
    assert_eq!((r.kind, r.file.as_str(), r.field.as_deref()), ("destructible_bake", file, Some("chunks[1].points")), "{}", r.reason);
    // A point with two numbers is the third one missing, in the chunk that has it.
    let r = refusal("destructible_bake", file, &format!("(version: 1, key: 5, chunks: [{one}, (points: [(0.0, 0.0)], faces: [])])"));
    assert_eq!(r.field.as_deref(), Some("chunks[1].points[0]"), "{}", r.reason);
    // A field the type does not have is named, so a rename in a hand edit is not silently dropped.
    let r = refusal("destructible_marks", "destructible.ron", "(version: 1, pieces: [], piece: [\"site/cart\"])");
    assert_eq!(r.field.as_deref(), Some("piece"), "{}", r.reason);
}

#[test]
fn a_bake_made_from_another_mesh_or_version_reads_and_the_key_is_the_whole_u64() {
    let stale = format!("(version: {}, key: {}, chunks: [])", DESTRUCTIBLE_VERSION + 1, u64::MAX);
    let kind = kind_by_id("destructible_bake").unwrap();
    assert_eq!(check(kind, "destructible/a.bake.ron", &stale), Check::Valid, "staleness is the editor's to judge, not a refusal");
    assert_eq!(Bake::parse(&stale).unwrap().key, u64::MAX);
    assert_eq!(Marks::parse("(version: 1, pieces: [])").unwrap().pieces.len(), 0);
}
